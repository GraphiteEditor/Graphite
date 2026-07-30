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
	fn spawn(&self, source: SourceId, future: SourceFuture);
}

#[derive(Clone)]
pub struct RuntimeHandle(pub Arc<DynRuntime>);

impl std::fmt::Debug for RuntimeHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RuntimeHandle").finish_non_exhaustive()
	}
}

impl graphene_hash::CacheHash for RuntimeHandle {
	fn cache_hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

pub trait Spawner {
	fn spawn(&self, task: SourceFuture);
}

#[cfg(not(target_family = "wasm"))]
pub type DynSpawner = dyn Spawner + Send + Sync;
#[cfg(target_family = "wasm")]
pub type DynSpawner = dyn Spawner;

impl<S: Spawner + ?Sized> Spawner for Box<S> {
	fn spawn(&self, task: SourceFuture) {
		(**self).spawn(task)
	}
}

/// Dropped tasks never complete.
pub struct NoopSpawner;

impl Spawner for NoopSpawner {
	fn spawn(&self, _task: SourceFuture) {
		log::warn!("async source spawned before a host spawner is wired; the task is dropped");
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
	spawner: S,
}

impl<S> GraphRuntime<S> {
	pub fn new(spawner: S) -> Self {
		Self {
			generations: Arc::default(),
			dirty: Arc::default(),
			spawner,
		}
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
	fn spawn(&self, source: SourceId, future: SourceFuture) {
		let generations = Arc::clone(&self.generations);
		let dirty = Arc::clone(&self.dirty);
		self.spawner.spawn(Box::pin(async move {
			future.await;
			let mut generations = generations.lock().unwrap_or_else(PoisonError::into_inner);
			if let Some(generation) = generations.get_mut(&source) {
				*generation += 1;
				dirty.store(true, Ordering::Release);
			}
		}));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::arena::Arena;
	use crate::context::{ContextImpl, Ctx, CtxSnapshot, EvalScope, ExtractFootprint, ExtractVarArgs, VarArgLink, VarArgSlots};
	use crate::gnode::GNode;
	use crate::gpoll::GPoll;
	use crate::transform::Footprint;
	use std::sync::Mutex;
	use std::sync::atomic::{AtomicU32, Ordering};

	#[derive(Default)]
	struct MockRuntime {
		futures: Mutex<Vec<(SourceId, SourceFuture)>>,
	}

	impl Runtime for MockRuntime {
		fn spawn(&self, source: SourceId, future: SourceFuture) {
			self.futures.lock().unwrap().push((source, future));
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
		fn spawn(&self, task: SourceFuture) {
			self.tasks.lock().unwrap().push(task);
		}
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

	struct SourceNode<T>(T);

	impl<T: Clone, Input> GNode<Input> for SourceNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
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

	struct GatedSource(Arc<std::sync::atomic::AtomicBool>, f64);

	impl<Input> GNode<Input> for GatedSource {
		type Output = f64;

		fn eval(&self, _input: &Input) -> GPoll<f64> {
			match self.0.load(Ordering::Relaxed) {
				true => GPoll::Final(self.1),
				false => GPoll::Pending,
			}
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(None, None, None, generations, arena)
	}

	#[test]
	fn async_source_spawns_once_and_lands_via_the_slot() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = SlowDoubleNode::new(SourceNode(21.0f64), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(7u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		assert_eq!(SLOW_DOUBLE_RUNS.load(Ordering::Relaxed), 0);
		assert_eq!(runtime.drain(), vec![7]);
		assert_eq!(SLOW_DOUBLE_RUNS.load(Ordering::Relaxed), 1);
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
		assert_eq!(SLOW_DOUBLE_RUNS.load(Ordering::Relaxed), 1);
	}

	#[test]
	fn async_source_reports_the_placeholder_while_in_flight() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = PreviewDoubleNode::new(SourceNode(21.0f64), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(1u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Partial(-1.0));
		runtime.drain();
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
	}

	#[test]
	fn no_partial_maps_the_placeholder_frame_to_pending() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = StrictDoubleNode::new(SourceNode(21.0f64), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(2u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		runtime.drain();
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
	}

	#[test]
	fn prologue_runs_sync_and_spawns_once() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = StagedDoubleNode::new(SourceNode(21.0f64), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(8u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		assert_eq!(STAGED_RUNS.load(Ordering::Relaxed), 1, "the prologue runs synchronously on the miss");
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		assert_eq!(STAGED_RUNS.load(Ordering::Relaxed), 1, "in flight must not rerun the prologue");
		assert_eq!(runtime.drain(), vec![8]);
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
		assert_eq!(STAGED_RUNS.load(Ordering::Relaxed), 1);
	}

	#[test]
	fn prologue_interrupt_defers_the_spawn() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let runtime = Arc::new(MockRuntime::default());
		let graph = StagedSumNode::new(SourceNode(40.0f64), GatedSource(gate.clone(), 2.0), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(9u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		assert_eq!(runtime.drain(), Vec::<SourceId>::new(), "an interrupted prologue must not spawn or claim the slot");
		gate.store(true, Ordering::Relaxed);
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		assert_eq!(runtime.drain(), vec![9]);
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
	}

	#[test]
	fn async_kernels_read_captured_varargs() {
		let arena = Arena::new(64);
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
		let graph = SnapshotVarargNode::new(SourceNode(()), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(5u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		runtime.drain();
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(21.5));
	}

	#[test]
	fn async_kernels_read_the_captured_context_snapshot() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);
		let footprint = Footprint::DEFAULT;
		let ctx = root.with_footprint(&footprint);

		let runtime = Arc::new(MockRuntime::default());
		let graph = SnapshotResolutionNode::new(SourceNode(()), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(3u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		runtime.drain();
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(Footprint::DEFAULT.resolution.x));
	}

	#[test]
	fn the_epilogue_bumps_the_generation_and_sets_dirty() {
		let runtime = GraphRuntime::new(CollectSpawner::default());
		runtime.retain_sources(&[7]);

		Runtime::spawn(&runtime, 7, Box::pin(async {}));
		assert_eq!(runtime.snapshot(), vec![(7, 0)], "no bump before the future completes");
		assert!(!runtime.take_dirty());

		assert_eq!(runtime.spawner().drain(), 1);
		assert_eq!(runtime.snapshot(), vec![(7, 1)]);
		assert!(runtime.take_dirty());
		assert!(!runtime.take_dirty(), "take_dirty drains the flag");
	}

	#[test]
	fn the_epilogue_of_a_removed_source_is_inert() {
		let runtime = GraphRuntime::new(CollectSpawner::default());
		runtime.retain_sources(&[7]);

		Runtime::spawn(&runtime, 7, Box::pin(async {}));
		runtime.retain_sources(&[]);
		assert_eq!(runtime.spawner().drain(), 1);

		assert_eq!(runtime.snapshot(), Vec::<(SourceId, u64)>::new());
		assert!(!runtime.take_dirty(), "a removed source must not invalidate");
	}

	#[test]
	fn retain_sources_preserves_live_generations() {
		let runtime = GraphRuntime::new(CollectSpawner::default());
		runtime.retain_sources(&[7]);
		Runtime::spawn(&runtime, 7, Box::pin(async {}));
		runtime.spawner().drain();

		runtime.retain_sources(&[7, 9]);
		assert_eq!(runtime.snapshot(), vec![(7, 1), (9, 0)]);

		runtime.retain_sources(&[9]);
		assert_eq!(runtime.snapshot(), vec![(9, 0)]);
	}

	#[node_macro::node(category(""))]
	async fn epilogue_double(_: impl Ctx, value: f64) -> f64 {
		value * 2.
	}

	#[test]
	fn a_source_slot_lands_through_the_runtime_while_downstream_keys_invalidate() {
		let arena = Arena::new(64);
		let runtime = Arc::new(GraphRuntime::new(CollectSpawner::default()));
		runtime.retain_sources(&[11]);
		let graph = EpilogueDoubleNode::new(SourceNode(21.0f64), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(11u64));

		let snapshot = runtime.snapshot();
		let scope = EvalScope::new(None, None, None, &snapshot, &arena);
		let ctx = ContextImpl::root(&scope);
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		assert!(!runtime.take_dirty());

		assert_eq!(runtime.spawner().drain(), 1);
		assert!(runtime.take_dirty());
		let bumped = runtime.snapshot();
		assert_eq!(bumped, vec![(11, 1)]);

		let bumped_scope = EvalScope::new(None, None, None, &bumped, &arena);
		let bumped_ctx = ContextImpl::root(&bumped_scope);
		assert_eq!(GNode::eval(&graph, &bumped_ctx), GPoll::Final(42.0), "the own-generation-excluded key replays the landed slot");
		assert_eq!(runtime.spawner().drain(), 0, "a slot hit must not respawn");

		let downstream_key = crate::registry::cache_key(&ContextImpl::root(&scope));
		let bumped_downstream_key = crate::registry::cache_key(&ContextImpl::root(&bumped_scope));
		assert_ne!(downstream_key, bumped_downstream_key, "unretained keys see the bump");
	}
}
