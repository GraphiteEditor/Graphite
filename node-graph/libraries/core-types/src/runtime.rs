use crate::poll::Poll;
use crate::uuid::NodeId;
use dyn_any::WasmNotSend;
use futures::future::{AbortHandle, Abortable};
use std::collections::HashMap;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Waker};
pub use web_time::Instant;

#[cfg(not(target_family = "wasm"))]
pub type Task = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
#[cfg(target_family = "wasm")]
pub type Task = Pin<Box<dyn Future<Output = ()> + 'static>>;

pub trait Spawn {
	fn spawn(&self, task: Task);
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
	fn spawn(&self, source: SourceId, task: Task);
}

#[derive(Clone, dyn_any::DynAny)]
pub struct RuntimeHandle(Arc<RuntimeInner>);

impl Runtime for RuntimeHandle {
	fn spawn(&self, source: SourceId, task: Task) {
		self.0.spawn(source, task);
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
	pub fn new(spawner: Arc<dyn Spawn + Send + Sync>) -> Self {
		Self(Arc::new(RuntimeInner {
			generations: Mutex::default(),
			spawner,
		}))
	}

	pub fn retain_sources(&self, live: &[SourceId]) {
		let mut generations = self.0.generations.lock().unwrap_or_else(PoisonError::into_inner);
		generations.retain(|source, _| live.contains(source));
		for source in live {
			generations.entry(*source).or_insert(0);
		}
	}

	pub fn snapshot(&self) -> Generations {
		let generations = self.0.generations.lock().unwrap_or_else(PoisonError::into_inner);
		let mut snapshot: Vec<_> = generations.iter().map(|(source, generation)| (*source, *generation)).collect();
		snapshot.sort_unstable();
		snapshot.into()
	}

	#[must_use]
	pub fn handle(&self) -> RuntimeHandle {
		RuntimeHandle(Arc::clone(&self.0))
	}
}

impl Runtime for AsyncRuntime {
	fn spawn(&self, source: SourceId, task: Task) {
		self.0.spawn(source, task);
	}
}

struct RuntimeInner {
	generations: Mutex<HashMap<SourceId, u64>>,
	spawner: Arc<dyn Spawn + Send + Sync>,
}

impl RuntimeInner {
	fn spawn(self: &Arc<Self>, source: SourceId, task: Task) {
		let inner = Arc::clone(self);
		self.spawner.spawn(Box::pin(async move {
			task.await;
			if let Some(generation) = inner.generations.lock().unwrap_or_else(PoisonError::into_inner).get_mut(&source) {
				*generation += 1;
			}
		}));
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

		let (handle, registration) = AbortHandle::new_pair();
		let slots = Arc::downgrade(&self.0);
		runtime.spawn(
			source,
			Box::pin(async move {
				if let Ok(state) = Abortable::new(future, registration).await
					&& let Some(slots) = slots.upgrade()
				{
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
	use std::sync::atomic::{AtomicBool, Ordering};

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
		let runtime = AsyncRuntime::new(spawner.clone());
		runtime.retain_sources(&[SOURCE]);

		runtime.handle().spawn(SOURCE, Box::pin(YieldOnce(false)));
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 0)]);

		assert_eq!(spawner.drain(), 1);
		assert_eq!(&*runtime.snapshot(), &[(SOURCE, 1)]);
	}

	#[test]
	fn retaining_sources_keeps_live_generations() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone());
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
		let runtime = AsyncRuntime::new(spawner.clone());
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
		let runtime = AsyncRuntime::new(spawner.clone());
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
		let runtime = AsyncRuntime::new(Arc::new(BlockingSpawner));
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
		let runtime = AsyncRuntime::new(spawner.clone());
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
		let runtime = AsyncRuntime::new(spawner.clone());
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
}
