use crate::SourceId;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

#[cfg(not(target_family = "wasm"))]
pub type SourceFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
#[cfg(target_family = "wasm")]
pub type SourceFuture<T = ()> = Pin<Box<dyn Future<Output = T> + 'static>>;

#[cfg(not(target_family = "wasm"))]
pub type DynRuntime = dyn Runtime + Send + Sync;
#[cfg(target_family = "wasm")]
pub type DynRuntime = dyn Runtime;

pub trait Runtime {
	/// Returns true when the future completed during the call, so its result is already observable.
	fn spawn(&self, source: SourceId, future: SourceFuture) -> bool;
}

#[derive(Clone, dyn_any::DynAny)]
pub struct RuntimeHandle(pub Arc<DynRuntime>);

// SAFETY: wasm is single threaded, so the handle never actually crosses a thread.
#[cfg(target_family = "wasm")]
unsafe impl Send for RuntimeHandle {}
// SAFETY: as in Send.
#[cfg(target_family = "wasm")]
unsafe impl Sync for RuntimeHandle {}

impl std::fmt::Debug for RuntimeHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RuntimeHandle").finish_non_exhaustive()
	}
}

impl graphene_hash::CacheHash for RuntimeHandle {
	fn cache_hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

pub trait Spawner {
	/// Returns true when the task completed during the call, so its result is already observable.
	fn spawn(&self, task: SourceFuture) -> bool;
}

/// Polls `task` once with a no-op waker, returning true if it completed.
pub fn poll_once(task: &mut SourceFuture) -> bool {
	let mut context = std::task::Context::from_waker(std::task::Waker::noop());
	task.as_mut().poll(&mut context).is_ready()
}

#[cfg(not(target_family = "wasm"))]
pub type DynSpawner = dyn Spawner + Send + Sync;
#[cfg(target_family = "wasm")]
pub type DynSpawner = dyn Spawner;

#[cfg(not(target_family = "wasm"))]
pub type DynNotifier = dyn Fn() + Send + Sync;
#[cfg(target_family = "wasm")]
pub type DynNotifier = dyn Fn();

impl<S: Spawner + ?Sized> Spawner for Box<S> {
	fn spawn(&self, task: SourceFuture) -> bool {
		(**self).spawn(task)
	}
}

/// Polls each task once inline. Tasks that are not immediately ready never complete.
pub struct NoopSpawner;

impl Spawner for NoopSpawner {
	fn spawn(&self, mut task: SourceFuture) -> bool {
		if poll_once(&mut task) {
			return true;
		}
		log::warn!("async source is not immediately ready and no host spawner is wired; the task is dropped");
		false
	}
}

pub type DynGraphRuntime = GraphRuntime<Box<DynSpawner>>;

impl Default for RuntimeHandle {
	fn default() -> Self {
		Self(Arc::new(GraphRuntime::new(Box::new(NoopSpawner) as Box<DynSpawner>)))
	}
}

pub struct GraphRuntime<S> {
	generations: Arc<Mutex<HashMap<SourceId, u64>>>,
	dirty: Arc<AtomicBool>,
	notifier: Arc<Mutex<Arc<DynNotifier>>>,
	spawner: S,
}

// SAFETY: wasm is single threaded, so the runtime never actually crosses a thread.
#[cfg(target_family = "wasm")]
unsafe impl<S> Send for GraphRuntime<S> {}
// SAFETY: as in Send.
#[cfg(target_family = "wasm")]
unsafe impl<S> Sync for GraphRuntime<S> {}

impl<S> GraphRuntime<S> {
	pub fn new(spawner: S) -> Self {
		Self {
			generations: Arc::default(),
			dirty: Arc::default(),
			notifier: Arc::new(Mutex::new(Arc::new(|| {}))),
			spawner,
		}
	}

	pub fn set_notifier(&self, notifier: Arc<DynNotifier>) {
		*self.notifier.lock().unwrap_or_else(PoisonError::into_inner) = notifier;
	}

	pub fn retain_sources(&self, live: &[SourceId]) {
		let mut generations = self.generations.lock().unwrap_or_else(PoisonError::into_inner);
		generations.retain(|source, _| live.contains(source));
		for source in live {
			generations.entry(*source).or_insert(0);
		}
	}

	pub fn snapshot(&self) -> Vec<(SourceId, u64)> {
		let generations = self.generations.lock().unwrap_or_else(PoisonError::into_inner);
		let mut snapshot: Vec<_> = generations.iter().map(|(source, generation)| (*source, *generation)).collect();
		snapshot.sort_unstable();
		snapshot
	}

	pub fn take_dirty(&self) -> bool {
		self.dirty.swap(false, Ordering::Acquire)
	}

	pub fn spawner(&self) -> &S {
		&self.spawner
	}
}

impl<S: Spawner> Runtime for GraphRuntime<S> {
	fn spawn(&self, source: SourceId, mut future: SourceFuture) -> bool {
		let generations = Arc::clone(&self.generations);
		let dirty = Arc::clone(&self.dirty);
		let notifier = Arc::clone(&self.notifier);
		let mut first = true;
		self.spawner.spawn(Box::pin(std::future::poll_fn(move |task_context| {
			let poll = future.as_mut().poll(task_context);
			if poll.is_ready() && !first {
				let mut generations = generations.lock().unwrap_or_else(PoisonError::into_inner);
				if let Some(generation) = generations.get_mut(&source) {
					*generation += 1;
					dirty.store(true, Ordering::Release);
					drop(generations);
					let notifier = Arc::clone(&notifier.lock().unwrap_or_else(PoisonError::into_inner));
					notifier();
				}
			}
			first = false;
			poll
		})))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::arena::Arena;
	use crate::context::{ContextImpl, Ctx, EvalScope, ExtractFootprint, ExtractVarArgs, VarArgLink, VarArgSlots};
	use crate::gpoll::GPoll;
	use crate::node::Node;
	use crate::record::{Layout, LiftedSource, RecordExtract, element_write};
	use crate::transform::Footprint;
	use std::sync::Mutex;
	use std::sync::atomic::{AtomicU32, Ordering};

	#[derive(Default)]
	struct MockRuntime {
		futures: Mutex<Vec<(SourceId, SourceFuture)>>,
	}

	impl Runtime for MockRuntime {
		fn spawn(&self, source: SourceId, future: SourceFuture) -> bool {
			self.futures.lock().unwrap().push((source, future));
			false
		}
	}

	impl MockRuntime {
		fn drain(&self) -> Vec<SourceId> {
			let futures = std::mem::take(&mut *self.futures.lock().unwrap());
			let mut task_ctx = std::task::Context::from_waker(std::task::Waker::noop());
			futures
				.into_iter()
				.map(|(source, mut future)| {
					assert!(future.as_mut().poll(&mut task_ctx).is_ready());
					source
				})
				.collect()
		}
	}

	#[derive(Default)]
	struct CollectSpawner {
		tasks: Mutex<Vec<SourceFuture>>,
	}

	impl Spawner for CollectSpawner {
		fn spawn(&self, mut task: SourceFuture) -> bool {
			if poll_once(&mut task) {
				return true;
			}
			self.tasks.lock().unwrap().push(task);
			false
		}
	}

	struct YieldOnce(bool);

	impl Future for YieldOnce {
		type Output = ();

		fn poll(mut self: Pin<&mut Self>, task_context: &mut std::task::Context<'_>) -> std::task::Poll<()> {
			if self.0 {
				return std::task::Poll::Ready(());
			}
			self.0 = true;
			task_context.waker().wake_by_ref();
			std::task::Poll::Pending
		}
	}

	fn yield_once() -> YieldOnce {
		YieldOnce(false)
	}

	impl CollectSpawner {
		fn drain(&self) -> usize {
			let tasks = std::mem::take(&mut *self.tasks.lock().unwrap());
			let mut task_ctx = std::task::Context::from_waker(std::task::Waker::noop());
			let count = tasks.len();
			for mut task in tasks {
				assert!(task.as_mut().poll(&mut task_ctx).is_ready());
			}
			count
		}
	}

	fn element_layout<T: Clone + Send + Sync + dyn_any::StaticTypeSized>() -> Layout
	where
		T::Static: Clone + Send + Sync,
	{
		Layout::default().with_writes(0, element_write::<T>(), &[])
	}

	fn lifted<T: Clone + Send + Sync + dyn_any::StaticTypeSized>(value: T) -> LiftedSource<T, impl for<'c> Fn(&ContextImpl<'c>) -> GPoll<T>>
	where
		T::Static: Clone + Send + Sync,
	{
		LiftedSource::new(move |_: &ContextImpl<'_>| GPoll::Final(value.clone()))
	}

	fn extract<El: Clone + Send + Sync + dyn_any::StaticTypeSized, N: Node<ContextImpl<'static>>>(mut graph: N) -> RecordExtract<El, N>
	where
		El::Static: Clone + Send + Sync,
	{
		let layout = element_layout::<El>();
		graph.set_layout(crate::record::RecordLayout {
			frame_bytes: layout.frame_bytes(),
			plan: Vec::new(),
			layout: layout.clone(),
			lane_invariant: u32::MAX,
		});
		RecordExtract::new(graph, &layout)
	}

	static SLOW_DOUBLE_RUNS: AtomicU32 = AtomicU32::new(0);

	#[node_macro::node(category(""))]
	async fn slow_double(_: impl Ctx, value: f64) -> f64 {
		SLOW_DOUBLE_RUNS.fetch_add(1, Ordering::Relaxed);
		value * 2.
	}

	fn stand_in(_value: &f64) -> f64 {
		-1.
	}

	#[node_macro::node(category(""), placeholder(stand_in))]
	async fn preview_double(_: impl Ctx, value: f64) -> f64 {
		value * 2.
	}

	#[node_macro::node(category(""), placeholder(stand_in), no_partial)]
	async fn strict_double(_: impl Ctx, value: f64) -> f64 {
		value * 2.
	}

	#[node_macro::node(category(""))]
	async fn snapshot_resolution(ctx: CtxSnapshot, _primary: ()) -> u32 {
		ctx.try_footprint().map(|footprint| footprint.resolution.x).unwrap_or(0)
	}

	#[node_macro::node(category(""))]
	async fn snapshot_vararg(ctx: CtxSnapshot, _primary: ()) -> f64 {
		ctx.vararg(0).ok().and_then(|slot| slot.downcast_ref::<f64>()).copied().unwrap_or(0.)
	}

	static STAGED_RUNS: AtomicU32 = AtomicU32::new(0);

	#[node_macro::node(category(""))]
	fn staged_double(_: impl Ctx, value: f64) -> SourceFuture<f64> {
		STAGED_RUNS.fetch_add(1, Ordering::Relaxed);
		Box::pin(async move { value * 2. })
	}

	#[node_macro::node(category(""))]
	fn staged_sum(ctx: impl Ctx, value: f64, addend: impl Node<Context<'_>, Output = f64>) -> Result<SourceFuture<f64>, crate::gpoll::Interrupt> {
		let addend = addend.eval(ctx)?;
		Ok(Box::pin(async move { value + addend }))
	}

	fn gated(gate: Arc<std::sync::atomic::AtomicBool>, value: f64) -> LiftedSource<f64, impl for<'c> Fn(&ContextImpl<'c>) -> GPoll<f64>> {
		LiftedSource::new(move |_: &ContextImpl<'_>| match gate.load(Ordering::Relaxed) {
			true => GPoll::Final(value),
			false => GPoll::Pending,
		})
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(None, None, None, generations, arena)
	}

	#[test]
	fn async_source_spawns_once_and_lands_via_the_slot() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = extract::<f64, _>(SlowDoubleNode::new(
			lifted(21.0f64),
			lifted(RuntimeHandle(runtime.clone())),
			lifted(7u64),
			&element_layout::<f64>(),
			&element_layout::<RuntimeHandle>(),
			&element_layout::<u64>(),
		));

		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		assert_eq!(SLOW_DOUBLE_RUNS.load(Ordering::Relaxed), 0);
		assert_eq!(runtime.drain(), vec![7]);
		assert_eq!(SLOW_DOUBLE_RUNS.load(Ordering::Relaxed), 1);
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Final(42.0));
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Final(42.0));
		assert_eq!(SLOW_DOUBLE_RUNS.load(Ordering::Relaxed), 1);
	}

	#[test]
	fn async_source_reports_the_placeholder_while_in_flight() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = extract::<f64, _>(PreviewDoubleNode::new(
			lifted(21.0f64),
			lifted(RuntimeHandle(runtime.clone())),
			lifted(1u64),
			&element_layout::<f64>(),
			&element_layout::<RuntimeHandle>(),
			&element_layout::<u64>(),
		));

		assert_eq!(graph.eval(&ctx, &frames), GPoll::Partial(-1.0));
		runtime.drain();
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Final(42.0));
	}

	#[test]
	fn no_partial_maps_the_placeholder_frame_to_pending() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = extract::<f64, _>(StrictDoubleNode::new(
			lifted(21.0f64),
			lifted(RuntimeHandle(runtime.clone())),
			lifted(2u64),
			&element_layout::<f64>(),
			&element_layout::<RuntimeHandle>(),
			&element_layout::<u64>(),
		));

		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		runtime.drain();
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Final(42.0));
	}

	#[test]
	fn prologue_runs_sync_and_spawns_once() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = extract::<f64, _>(StagedDoubleNode::new(
			lifted(21.0f64),
			lifted(RuntimeHandle(runtime.clone())),
			lifted(8u64),
			&element_layout::<f64>(),
			&element_layout::<RuntimeHandle>(),
			&element_layout::<u64>(),
		));

		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		assert_eq!(STAGED_RUNS.load(Ordering::Relaxed), 1, "the prologue runs synchronously on the miss");
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		assert_eq!(STAGED_RUNS.load(Ordering::Relaxed), 1, "in flight must not rerun the prologue");
		assert_eq!(runtime.drain(), vec![8]);
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Final(42.0));
		assert_eq!(STAGED_RUNS.load(Ordering::Relaxed), 1);
	}

	#[test]
	fn prologue_interrupt_defers_the_spawn() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let runtime = Arc::new(MockRuntime::default());
		let graph = extract::<f64, _>(StagedSumNode::new(
			lifted(40.0f64),
			gated(gate.clone(), 2.0),
			lifted(RuntimeHandle(runtime.clone())),
			lifted(9u64),
			&element_layout::<f64>(),
			&element_layout::<f64>(),
			&element_layout::<RuntimeHandle>(),
			&element_layout::<u64>(),
		));

		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		assert_eq!(runtime.drain(), Vec::<SourceId>::new(), "an interrupted prologue must not spawn or claim the slot");
		gate.store(true, Ordering::Relaxed);
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		assert_eq!(runtime.drain(), vec![9]);
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Final(42.0));
	}

	#[test]
	fn async_kernels_read_captured_varargs() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);
		let payload = 21.5f64;
		let link = VarArgLink {
			args: VarArgSlots::Single(&payload),
			outer: None,
		};
		let ctx = root.with_varargs(&link);

		let runtime = Arc::new(MockRuntime::default());
		let graph = extract::<f64, _>(SnapshotVarargNode::new(
			lifted(()),
			lifted(RuntimeHandle(runtime.clone())),
			lifted(5u64),
			&element_layout::<()>(),
			&element_layout::<RuntimeHandle>(),
			&element_layout::<u64>(),
		));

		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		runtime.drain();
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Final(21.5));
	}

	#[test]
	fn async_kernels_read_the_captured_context_snapshot() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);
		let footprint = Footprint::DEFAULT;
		let ctx = root.with_footprint(&footprint);

		let runtime = Arc::new(MockRuntime::default());
		let graph = extract::<u32, _>(SnapshotResolutionNode::new(
			lifted(()),
			lifted(RuntimeHandle(runtime.clone())),
			lifted(3u64),
			&element_layout::<()>(),
			&element_layout::<RuntimeHandle>(),
			&element_layout::<u64>(),
		));

		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		runtime.drain();
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Final(Footprint::DEFAULT.resolution.x));
	}

	#[test]
	fn the_epilogue_bumps_the_generation_and_sets_dirty() {
		let runtime = GraphRuntime::new(CollectSpawner::default());
		runtime.retain_sources(&[7]);

		Runtime::spawn(&runtime, 7, Box::pin(yield_once()));
		assert_eq!(runtime.snapshot(), vec![(7, 0)], "no bump before the future completes");
		assert!(!runtime.take_dirty());

		assert_eq!(runtime.spawner().drain(), 1);
		assert_eq!(runtime.snapshot(), vec![(7, 1)]);
		assert!(runtime.take_dirty());
		assert!(!runtime.take_dirty(), "take_dirty drains the flag");
	}

	#[test]
	fn the_epilogue_notifies_after_setting_dirty() {
		let runtime = GraphRuntime::new(CollectSpawner::default());
		runtime.retain_sources(&[7]);
		let observed_dirty = Arc::new(AtomicBool::new(false));
		let dirty_at_notify = Arc::clone(&runtime.dirty);
		let observed = Arc::clone(&observed_dirty);
		runtime.set_notifier(Arc::new(move || {
			observed.store(dirty_at_notify.load(Ordering::Acquire), Ordering::Relaxed);
		}));

		Runtime::spawn(&runtime, 7, Box::pin(yield_once()));
		assert_eq!(runtime.spawner().drain(), 1);
		assert!(observed_dirty.load(Ordering::Relaxed), "the notifier must observe the dirty flag already set");
	}

	#[test]
	fn the_epilogue_of_a_removed_source_does_not_notify() {
		let runtime = GraphRuntime::new(CollectSpawner::default());
		runtime.retain_sources(&[7]);
		let notified = Arc::new(AtomicBool::new(false));
		let flag = Arc::clone(&notified);
		runtime.set_notifier(Arc::new(move || flag.store(true, Ordering::Relaxed)));

		Runtime::spawn(&runtime, 7, Box::pin(yield_once()));
		runtime.retain_sources(&[]);
		assert_eq!(runtime.spawner().drain(), 1);
		assert!(!notified.load(Ordering::Relaxed));
	}

	#[test]
	fn the_epilogue_of_a_removed_source_is_inert() {
		let runtime = GraphRuntime::new(CollectSpawner::default());
		runtime.retain_sources(&[7]);

		Runtime::spawn(&runtime, 7, Box::pin(yield_once()));
		runtime.retain_sources(&[]);
		assert_eq!(runtime.spawner().drain(), 1);

		assert_eq!(runtime.snapshot(), Vec::<(SourceId, u64)>::new());
		assert!(!runtime.take_dirty(), "a removed source must not invalidate");
	}

	#[test]
	fn retain_sources_preserves_live_generations() {
		let runtime = GraphRuntime::new(CollectSpawner::default());
		runtime.retain_sources(&[7]);
		Runtime::spawn(&runtime, 7, Box::pin(yield_once()));
		runtime.spawner().drain();

		runtime.retain_sources(&[7, 9]);
		assert_eq!(runtime.snapshot(), vec![(7, 1), (9, 0)]);

		runtime.retain_sources(&[9]);
		assert_eq!(runtime.snapshot(), vec![(9, 0)]);
	}

	#[node_macro::node(category(""))]
	async fn epilogue_double(_: impl Ctx, value: f64) -> f64 {
		yield_once().await;
		value * 2.
	}

	#[node_macro::node(category(""))]
	async fn inline_double(_: impl Ctx, value: f64) -> f64 {
		value * 2.
	}

	#[test]
	fn an_immediately_ready_task_completes_inline_without_invalidating() {
		let runtime = GraphRuntime::new(CollectSpawner::default());
		runtime.retain_sources(&[7]);

		assert!(Runtime::spawn(&runtime, 7, Box::pin(async {})));
		assert_eq!(runtime.spawner().drain(), 0);
		assert_eq!(runtime.snapshot(), vec![(7, 0)], "inline completion must not bump the generation");
		assert!(!runtime.take_dirty());
	}

	#[test]
	fn an_immediately_ready_kernel_returns_final_on_the_first_eval() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(64).unwrap();
		let runtime = Arc::new(GraphRuntime::new(CollectSpawner::default()));
		runtime.retain_sources(&[13]);
		let graph = extract::<f64, _>(InlineDoubleNode::new(
			lifted(21.0f64),
			lifted(RuntimeHandle(runtime.clone())),
			lifted(13u64),
			&element_layout::<f64>(),
			&element_layout::<RuntimeHandle>(),
			&element_layout::<u64>(),
		));

		let snapshot = runtime.snapshot();
		let scope = EvalScope::new(None, None, None, &snapshot, &arena);
		let ctx = ContextImpl::root(&scope);
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Final(42.0));
		assert!(!runtime.take_dirty());
		assert_eq!(runtime.snapshot(), vec![(13, 0)]);
		assert_eq!(runtime.spawner().drain(), 0);
	}

	#[test]
	fn a_source_slot_lands_through_the_runtime_while_downstream_keys_invalidate() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(64).unwrap();
		let runtime = Arc::new(GraphRuntime::new(CollectSpawner::default()));
		runtime.retain_sources(&[11]);
		let graph = extract::<f64, _>(EpilogueDoubleNode::new(
			lifted(21.0f64),
			lifted(RuntimeHandle(runtime.clone())),
			lifted(11u64),
			&element_layout::<f64>(),
			&element_layout::<RuntimeHandle>(),
			&element_layout::<u64>(),
		));

		let snapshot = runtime.snapshot();
		let scope = EvalScope::new(None, None, None, &snapshot, &arena);
		let ctx = ContextImpl::root(&scope);
		assert_eq!(graph.eval(&ctx, &frames), GPoll::Pending);
		assert!(!runtime.take_dirty());

		assert_eq!(runtime.spawner().drain(), 1);
		assert!(runtime.take_dirty());
		let bumped = runtime.snapshot();
		assert_eq!(bumped, vec![(11, 1)]);

		let bumped_scope = EvalScope::new(None, None, None, &bumped, &arena);
		let bumped_ctx = ContextImpl::root(&bumped_scope);
		assert_eq!(graph.eval(&bumped_ctx, &frames), GPoll::Final(42.0), "the own-generation-excluded key replays the landed slot");
		assert_eq!(runtime.spawner().drain(), 0, "a slot hit must not respawn");

		let downstream_key = crate::registry::cache_key(&ContextImpl::root(&scope));
		let bumped_downstream_key = crate::registry::cache_key(&ContextImpl::root(&bumped_scope));
		assert_ne!(downstream_key, bumped_downstream_key, "unretained keys see the bump");
	}
}
