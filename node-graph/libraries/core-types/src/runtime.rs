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

/// The host's monotonic time, measured from an arbitrary origin, which frames and the nodes' own budgets are timed with.
pub trait Clock: Send + Sync {
	fn now(&self) -> Duration;
}

/// How the host's node runtime schedules the evaluations that finished tasks ask for, set by the host as it builds the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scheduling {
	/// How long into a frame the evaluations finished tasks ask for may start, the rest waiting for the next frame so the
	/// host can show the latest output. None evaluates at once, frame or not.
	pub frame_budget: Option<Duration>,
}

impl Scheduling {
	/// Evaluates as soon as a finished task asks, for tests and headless hosts.
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
	/// Spawns `task` for `source`, whose generation its end bumps. Aborting the returned handle, or removing the source,
	/// drops the task unfinished and bumps nothing.
	fn spawn(&self, source: SourceId, task: Task) -> AbortHandle;
}

#[derive(Clone, dyn_any::DynAny)]
pub struct RuntimeHandle(Arc<RuntimeInner>);

impl RuntimeHandle {
	/// The clock the host measures frames with, for nodes that budget their work.
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
	fn cache_hash<H: Hasher>(&self, state: &mut H) {
		state.write_usize(Arc::as_ptr(&self.0) as usize);
	}
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

	/// Keeps the sources in `live`, adding new ones at generation zero. A removed source's unfinished tasks are aborted.
	pub fn retain_sources(&self, live: &[SourceId]) {
		let mut sources = self.0.lock();
		sources.retain(|source, state| {
			let keep = live.contains(source);
			if !keep {
				state.tasks.values().for_each(AbortHandle::abort);
			}
			keep
		});
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

	/// Whether a generation was bumped since the last call that returned true, so that any number of finished tasks
	/// ask for one evaluation.
	pub fn take_bumped(&self) -> bool {
		self.0.bumped.swap(false, Ordering::AcqRel)
	}

	/// Whether a generation was bumped since the last [`Self::take_bumped`], leaving that unchanged.
	pub fn bumped(&self) -> bool {
		self.0.bumped.load(Ordering::Acquire)
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

/// A live source's generation and the abort handles of its unfinished tasks, by task number.
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
		let number = self.next_task.fetch_add(1, Ordering::Relaxed);
		if let Some(state) = self.lock().get_mut(&source) {
			state.tasks.insert(number, handle.clone());
		}

		let inner = Arc::clone(self);
		self.spawner.spawn(Box::pin(async move {
			let finished = Abortable::new(task, registration).await.is_ok();
			// Only a task registered with a live source bumps its generation
			if let Some(state) = inner.lock().get_mut(&source)
				&& state.tasks.remove(&number).is_some()
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

	/// A clock that stands still until it is advanced.
	#[derive(Default)]
	struct FakeClock(Mutex<Duration>);

	impl Clock for FakeClock {
		fn now(&self) -> Duration {
			*self.0.lock().unwrap()
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
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
		runtime.retain_sources(&[SOURCE]);

		runtime.handle().spawn(SOURCE, Box::pin(YieldOnce(false)));
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 0)]);

		assert_eq!(spawner.drain(), 1);
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 1)]);
	}

	#[test]
	fn retaining_sources_keeps_live_generations() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
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
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
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
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
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
		let runtime = AsyncRuntime::new(Arc::new(BlockingSpawner), Arc::new(FakeClock::default()));
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
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
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
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
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

	/// A future that never finishes, as a request without a response.
	fn forever() -> Task {
		Box::pin(std::future::pending())
	}

	#[test]
	fn finished_tasks_ask_for_one_evaluation() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
		runtime.retain_sources(&[SOURCE, SourceId(9)]);
		for source in [SOURCE, SOURCE, SourceId(9)] {
			runtime.spawn(source, Box::pin(YieldOnce(false)));
		}
		assert!(!runtime.bumped());

		assert_eq!(spawner.drain(), 3);
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 2), (SourceId(9), 1)]);
		assert!(runtime.bumped());
		assert!(runtime.take_bumped());
		assert!(!runtime.take_bumped(), "three finished tasks asked for more than one evaluation");
	}

	#[test]
	fn an_aborted_task_is_dropped_and_bumps_nothing() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
		runtime.retain_sources(&[SOURCE]);
		let finished = Arc::new(AtomicBool::new(false));
		let task = {
			let finished = Arc::clone(&finished);
			async move { finished.store(true, Ordering::Relaxed) }
		};

		runtime.spawn(SOURCE, Box::pin(task)).abort();
		assert_eq!(spawner.drain(), 1);
		assert!(!finished.load(Ordering::Relaxed), "the aborted task ran");
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 0)]);
		assert!(!runtime.take_bumped());
	}

	#[test]
	fn removing_a_source_aborts_its_unfinished_tasks() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
		runtime.retain_sources(&[SOURCE, SourceId(9)]);
		let removed = runtime.spawn(SOURCE, forever());
		let kept = runtime.spawn(SourceId(9), forever());

		runtime.retain_sources(&[SourceId(9)]);
		assert!(removed.is_aborted());
		assert!(!kept.is_aborted());
		kept.abort();
		assert_eq!(spawner.drain(), 2, "an aborted task still waits for its response");
		assert_eq!(&*runtime.snapshot(), &[(SourceId(9), 0)]);
		assert!(!runtime.take_bumped());
	}

	#[test]
	fn a_slot_dropped_before_its_response_bumps_nothing() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(FakeClock::default()));
		runtime.retain_sources(&[SOURCE]);
		let slots = Slots::default();
		let fetch = || async {
			YieldOnce(false).await;
			Poll::Final(42u32)
		};

		assert_eq!(slots.resolve(1, &runtime, SOURCE, None, fetch), Poll::Pending);
		drop(slots);
		assert_eq!(spawner.drain(), 1);
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 0)]);
	}

	#[test]
	fn the_handle_reads_the_runtime_clock() {
		let clock = Arc::new(FakeClock::default());
		let runtime = AsyncRuntime::new(Arc::new(BlockingSpawner), clock.clone());
		*clock.0.lock().unwrap() = Duration::from_millis(16);
		assert_eq!(runtime.handle().clock().now(), Duration::from_millis(16));
	}
}
