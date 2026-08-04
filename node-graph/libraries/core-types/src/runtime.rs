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
	fn spawn(&self, task: SourceFuture);
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
	fn spawn(&self, source: SourceId, future: SourceFuture) {
		let generations = Arc::clone(&self.generations);
		let dirty = Arc::clone(&self.dirty);
		let notifier = Arc::clone(&self.notifier);
		self.spawner.spawn(Box::pin(async move {
			future.await;
			let mut generations = generations.lock().unwrap_or_else(PoisonError::into_inner);
			if let Some(generation) = generations.get_mut(&source) {
				*generation += 1;
				dirty.store(true, Ordering::Release);
				drop(generations);
				let notifier = Arc::clone(&notifier.lock().unwrap_or_else(PoisonError::into_inner));
				notifier();
			}
		}));
	}
}
