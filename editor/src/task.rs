pub use graphene_std::runtime::{Spawn, Task};
use std::sync::Arc;

#[derive(Clone)]
pub struct TaskExecutorHandle {
	backend: Arc<Backend>,
}

impl Spawn for TaskExecutorHandle {
	fn spawn(&self, task: Task) {
		self.backend.spawn(task);
	}
}

pub struct TaskExecutor {
	backend: Arc<Backend>,
}

impl TaskExecutor {
	pub fn new() -> Self {
		let backend = {
			#[cfg(target_family = "wasm")]
			{
				Arc::new(Backend::Wasm(WasmSpawner))
			}
			#[cfg(not(target_family = "wasm"))]
			{
				Arc::new(Backend::Tokio(TokioSpawner::new()))
			}
		};
		Self { backend }
	}
	pub fn blocking() -> Self {
		let backend = Arc::new(Backend::Blocking(BlockingSpawner));
		Self { backend }
	}
	pub fn handle(&self) -> TaskExecutorHandle {
		TaskExecutorHandle { backend: self.backend.clone() }
	}
}

impl Default for TaskExecutor {
	fn default() -> Self {
		Self::new()
	}
}

enum Backend {
	#[cfg(target_family = "wasm")]
	Wasm(WasmSpawner),
	#[cfg(not(target_family = "wasm"))]
	Tokio(TokioSpawner),
	Blocking(BlockingSpawner),
}

impl Spawn for Backend {
	fn spawn(&self, task: Task) {
		match self {
			#[cfg(target_family = "wasm")]
			Backend::Wasm(spawner) => spawner.spawn(task),
			#[cfg(not(target_family = "wasm"))]
			Backend::Tokio(spawner) => spawner.spawn(task),
			Backend::Blocking(spawner) => spawner.spawn(task),
		}
	}
}

#[cfg(target_family = "wasm")]
struct WasmSpawner;

#[cfg(target_family = "wasm")]
impl Spawn for WasmSpawner {
	fn spawn(&self, task: Task) {
		wasm_bindgen_futures::spawn_local(task);
	}
}

#[cfg(not(target_family = "wasm"))]
struct TokioSpawner(Option<tokio::runtime::Runtime>);

#[cfg(not(target_family = "wasm"))]
impl TokioSpawner {
	fn new() -> Self {
		let runtime = tokio::runtime::Builder::new_multi_thread()
			.worker_threads(1)
			.thread_name("graphite-async")
			.enable_all()
			.build()
			.expect("failed to construct the task tokio runtime");
		Self(Some(runtime))
	}
}

#[cfg(not(target_family = "wasm"))]
impl Spawn for TokioSpawner {
	fn spawn(&self, task: Task) {
		self.0.as_ref().expect("the runtime lives until drop").spawn(task);
	}
}

#[cfg(not(target_family = "wasm"))]
impl Drop for TokioSpawner {
	fn drop(&mut self) {
		if let Some(runtime) = self.0.take() {
			runtime.shutdown_background();
		}
	}
}

struct BlockingSpawner;

impl Spawn for BlockingSpawner {
	fn spawn(&self, task: Task) {
		futures::executor::block_on(task);
	}
}
