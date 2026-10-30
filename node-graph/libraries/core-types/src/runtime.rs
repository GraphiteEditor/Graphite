use crate::poll::Poll;
use crate::uuid::NodeId;
use dyn_any::WasmNotSend;
pub use futures::future::AbortHandle;
use futures::future::Abortable;
use std::collections::HashMap;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Waker};
use std::time::Duration;

#[cfg(not(target_family = "wasm"))]
pub type Task = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
#[cfg(target_family = "wasm")]
pub type Task = Pin<Box<dyn Future<Output = ()> + 'static>>;

pub trait Spawn {
	fn spawn(&self, task: Task);
}

/// Monotonic time since an arbitrary origin.
pub trait Clock: Send + Sync {
	fn now(&self) -> Duration;
}

#[cfg(not(target_family = "wasm"))]
impl Clock for std::time::Instant {
	fn now(&self) -> Duration {
		self.elapsed()
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scheduling {
	/// How far into a frame a re-evaluation caused by finished tasks may start. `None` never defers it.
	pub frame_budget: Option<Duration>,
}

impl Scheduling {
	pub const UNLIMITED: Self = Self { frame_budget: None };
}

impl Default for Scheduling {
	fn default() -> Self {
		Self {
			frame_budget: Some(Duration::from_millis(16)),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, graphene_hash::CacheHash, dyn_any::DynAny)]
pub struct SourceId(pub u64);

impl SourceId {
	#[must_use]
	pub fn from_path(path: &[NodeId]) -> Self {
		let mut hasher = rustc_hash::FxHasher::default();
		path.hash(&mut hasher);
		Self(hasher.finish())
	}
}

pub type Generations = Arc<[(SourceId, u64)]>;

pub trait Runtime {
	/// Bumps the generation of `source` once `task` finishes, unless it was aborted or the source was removed first.
	fn spawn(&self, source: SourceId, task: Task) -> AbortHandle;
}

#[derive(Clone, dyn_any::DynAny)]
pub struct RuntimeHandle(Arc<RuntimeInner>);

impl RuntimeHandle {
	pub fn clock(&self) -> &dyn Clock {
		&*self.0.clock
	}
}

impl Runtime for RuntimeHandle {
	fn spawn(&self, source: SourceId, task: Task) -> AbortHandle {
		self.0.spawn(source, task)
	}
}

impl std::fmt::Debug for RuntimeHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RuntimeHandle").finish_non_exhaustive()
	}
}

impl PartialEq for RuntimeHandle {
	fn eq(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.0, &other.0)
	}
}

impl graphene_hash::CacheHash for RuntimeHandle {
	fn cache_hash<H: Hasher>(&self, _state: &mut H) {}
}

pub struct AsyncRuntime(Arc<RuntimeInner>);

impl AsyncRuntime {
	pub fn new(spawner: Arc<dyn Spawn + Send + Sync>, clock: Arc<dyn Clock>) -> Self {
		Self(Arc::new(RuntimeInner {
			sources: Mutex::default(),
			next_task: AtomicU64::new(0),
			bumped: AtomicBool::new(false),
			spawner,
			clock,
		}))
	}

	/// Aborts the unfinished tasks of removed sources.
	pub fn retain_sources(&self, live: &[SourceId]) {
		let mut sources = self.0.lock();
		for (_, state) in sources.extract_if(|source, _| !live.contains(source)) {
			state.tasks.values().for_each(AbortHandle::abort);
		}
		for source in live {
			sources.entry(*source).or_default();
		}
	}

	pub fn snapshot(&self) -> Generations {
		let sources = self.0.lock();
		let mut snapshot: Vec<_> = sources.iter().map(|(source, state)| (*source, state.generation)).collect();
		snapshot.sort_unstable();
		snapshot.into()
	}

	/// Whether any generation was bumped since the last call.
	pub fn take_bumped(&self) -> bool {
		self.0.bumped.swap(false, Ordering::AcqRel)
	}

	pub fn clock(&self) -> &dyn Clock {
		&*self.0.clock
	}

	#[must_use]
	pub fn handle(&self) -> RuntimeHandle {
		RuntimeHandle(Arc::clone(&self.0))
	}
}

impl Runtime for AsyncRuntime {
	fn spawn(&self, source: SourceId, task: Task) -> AbortHandle {
		self.0.spawn(source, task)
	}
}

/// A live source's generation and its unfinished tasks.
#[derive(Default)]
struct SourceState {
	generation: u64,
	tasks: HashMap<u64, AbortHandle>,
}

struct RuntimeInner {
	sources: Mutex<HashMap<SourceId, SourceState>>,
	next_task: AtomicU64,
	bumped: AtomicBool,
	spawner: Arc<dyn Spawn + Send + Sync>,
	clock: Arc<dyn Clock>,
}

impl RuntimeInner {
	fn lock(&self) -> MutexGuard<'_, HashMap<SourceId, SourceState>> {
		self.sources.lock().unwrap_or_else(PoisonError::into_inner)
	}

	fn spawn(self: &Arc<Self>, source: SourceId, task: Task) -> AbortHandle {
		let (handle, registration) = AbortHandle::new_pair();
		let id = self.next_task.fetch_add(1, Ordering::Relaxed);
		if let Some(state) = self.lock().get_mut(&source) {
			state.tasks.insert(id, handle.clone());
		}

		let inner = Arc::clone(self);
		self.spawner.spawn(Box::pin(async move {
			let finished = Abortable::new(task, registration).await.is_ok();
			if let Some(state) = inner.lock().get_mut(&source)
				&& state.tasks.remove(&id).is_some()
				&& finished
			{
				state.generation += 1;
				inner.bumped.store(true, Ordering::Release);
			}
		}));
		handle
	}
}

pub struct Slots<T>(Arc<Mutex<HashMap<u64, Slot<T>>>>);

enum Slot<T> {
	Ready(Poll<T>),
	Pending(AbortHandle),
}

impl<T> Drop for Slot<T> {
	fn drop(&mut self) {
		if let Slot::Pending(task) = self {
			task.abort();
		}
	}
}

impl<T> Default for Slots<T> {
	fn default() -> Self {
		Self(Arc::default())
	}
}

impl<T> Clone for Slots<T> {
	fn clone(&self) -> Self {
		Self(Arc::clone(&self.0))
	}
}

impl<T> std::fmt::Debug for Slots<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Slots").finish_non_exhaustive()
	}
}

impl<T: Clone + WasmNotSend + 'static> Slots<T> {
	pub fn resolve<F>(&self, key: u64, runtime: &dyn Runtime, source: SourceId, placeholder: Option<T>, future: impl FnOnce() -> F) -> Poll<T>
	where
		F: Future<Output = Poll<T>> + WasmNotSend + 'static,
	{
		match self.0.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
			Some(Slot::Ready(landed)) => return landed.clone(),
			Some(Slot::Pending(_)) => return placeholder.map_or(Poll::Pending, Poll::Partial),
			None => {}
		}

		// Fast path for futures that are ready immediately
		// TODO(Timon): check if this would ever cause a problem; running a future on the graph executor thread might also be a bit wierd
		let mut future = Box::pin(future());
		if let std::task::Poll::Ready(state) = future.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
			self.0.lock().unwrap_or_else(PoisonError::into_inner).insert(key, Slot::Ready(state.clone()));
			return state;
		}

		let slots = Arc::downgrade(&self.0);
		let handle = runtime.spawn(
			source,
			Box::pin(async move {
				let state = future.await;
				if let Some(slots) = slots.upgrade() {
					slots.lock().unwrap_or_else(PoisonError::into_inner).insert(key, Slot::Ready(state));
				}
			}),
		);
		let mut slots = self.0.lock().unwrap_or_else(PoisonError::into_inner);
		match slots.get(&key) {
			Some(Slot::Ready(landed)) => landed.clone(),
			_ => {
				slots.insert(key, Slot::Pending(handle));
				placeholder.map_or(Poll::Pending, Poll::Partial)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	struct NoClock;

	impl Clock for NoClock {
		fn now(&self) -> Duration {
			Duration::ZERO
		}
	}

	fn poll_once(task: &mut Task) -> bool {
		task.as_mut().poll(&mut Context::from_waker(Waker::noop())).is_ready()
	}

	#[derive(Default)]
	struct CollectSpawner(Mutex<Vec<Task>>);

	impl Spawn for CollectSpawner {
		fn spawn(&self, task: Task) {
			self.0.lock().unwrap().push(task);
		}
	}

	impl CollectSpawner {
		fn drain(&self) -> usize {
			let tasks = std::mem::take(&mut *self.0.lock().unwrap());
			let count = tasks.len();
			for mut task in tasks {
				while !poll_once(&mut task) {}
			}
			count
		}
	}

	struct BlockingSpawner;

	impl Spawn for BlockingSpawner {
		fn spawn(&self, mut task: Task) {
			while !poll_once(&mut task) {}
		}
	}

	struct YieldOnce(bool);

	impl Future for YieldOnce {
		type Output = ();
		fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> std::task::Poll<()> {
			if self.0 {
				return std::task::Poll::Ready(());
			}
			self.0 = true;
			context.waker().wake_by_ref();
			std::task::Poll::Pending
		}
	}

	const SOURCE: SourceId = SourceId(7);

	#[test]
	fn a_completion_bumps_its_generation() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(NoClock));
		runtime.retain_sources(&[SOURCE]);

		runtime.handle().spawn(SOURCE, Box::pin(YieldOnce(false)));
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 0)]);

		assert_eq!(spawner.drain(), 1);
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 1)]);
	}

	#[test]
	fn retaining_sources_keeps_live_generations() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(NoClock));
		runtime.retain_sources(&[SOURCE]);
		runtime.spawn(SOURCE, Box::pin(YieldOnce(false)));
		spawner.drain();

		runtime.retain_sources(&[SOURCE, SourceId(9)]);
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 1), (SourceId(9), 0)]);

		runtime.retain_sources(&[SourceId(9)]);
		assert_eq!(&*runtime.snapshot(), &[(SourceId(9), 0)]);
	}

	#[test]
	fn an_inline_completion_and_a_dropped_source_bump_nothing() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(NoClock));
		runtime.retain_sources(&[SOURCE]);

		let slots = Slots::default();
		assert_eq!(slots.resolve(1, &runtime, SOURCE, None, || async { Poll::Final(1u32) }), Poll::Final(1));
		assert_eq!(spawner.drain(), 0);
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 0)]);

		runtime.spawn(SOURCE, Box::pin(YieldOnce(false)));
		runtime.retain_sources(&[]);
		assert_eq!(spawner.drain(), 1);
		assert!(runtime.snapshot().is_empty());
	}

	#[test]
	fn a_slot_serves_the_placeholder_until_the_response_lands() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(NoClock));
		runtime.retain_sources(&[SOURCE]);
		let slots = Slots::default();
		let fetch = || async {
			YieldOnce(false).await;
			Poll::Final(42u32)
		};

		assert_eq!(slots.resolve(1, &runtime, SOURCE, Some(0), fetch), Poll::Partial(0));
		assert_eq!(slots.resolve(1, &runtime, SOURCE, None, fetch), Poll::Pending);
		assert_eq!(spawner.drain(), 1, "an in-flight slot is not spawned twice");
		assert_eq!(slots.resolve(1, &runtime, SOURCE, Some(0), fetch), Poll::Final(42));
		assert_eq!(slots.resolve(2, &runtime, SOURCE, None, || async { Poll::Final(1u32) }), Poll::Final(1));
	}

	#[test]
	fn a_blocking_spawner_lands_the_slot_in_the_same_resolve() {
		let runtime = AsyncRuntime::new(Arc::new(BlockingSpawner), Arc::new(NoClock));
		runtime.retain_sources(&[SOURCE]);
		let slots = Slots::default();
		let fetch = || async {
			YieldOnce(false).await;
			Poll::Final(42u32)
		};

		assert_eq!(slots.resolve(1, &runtime, SOURCE, Some(0), fetch), Poll::Final(42));
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 1)]);
	}

	#[test]
	fn a_clone_keeps_the_task_running() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(NoClock));
		let slots = Slots::default();
		let fetch = || async {
			YieldOnce(false).await;
			Poll::Final(42u32)
		};

		assert_eq!(slots.resolve(1, &runtime, SOURCE, None, fetch), Poll::Pending);
		let clone = slots.clone();
		drop(slots);
		assert_eq!(spawner.drain(), 1);
		assert_eq!(clone.resolve(1, &runtime, SOURCE, None, fetch), Poll::Final(42));
	}

	#[test]
	fn dropping_the_last_clone_aborts_the_task() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(NoClock));
		let slots = Slots::default();
		let finished = Arc::new(AtomicBool::new(false));
		let fetch = || {
			let finished = Arc::clone(&finished);
			async move {
				YieldOnce(false).await;
				finished.store(true, Ordering::Relaxed);
				Poll::Final(42u32)
			}
		};

		assert_eq!(slots.resolve(1, &runtime, SOURCE, None, fetch), Poll::Pending);
		drop(slots);
		assert_eq!(spawner.drain(), 1);
		assert!(!finished.load(Ordering::Relaxed));
	}

	#[test]
	fn finished_tasks_bump_once() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(NoClock));
		runtime.retain_sources(&[SOURCE]);
		runtime.spawn(SOURCE, Box::pin(YieldOnce(false)));
		runtime.spawn(SOURCE, Box::pin(YieldOnce(false)));

		assert_eq!(spawner.drain(), 2);
		assert!(runtime.take_bumped());
		assert!(!runtime.take_bumped());
	}

	#[test]
	fn aborted_tasks_bump_nothing() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(NoClock));
		runtime.retain_sources(&[SOURCE, SourceId(9)]);
		runtime.spawn(SOURCE, Box::pin(std::future::pending())).abort();
		let removed = runtime.spawn(SourceId(9), Box::pin(std::future::pending()));
		runtime.retain_sources(&[SOURCE]);

		assert!(removed.is_aborted());
		assert_eq!(spawner.drain(), 2);
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 0)]);
		assert!(!runtime.take_bumped());
	}
}
