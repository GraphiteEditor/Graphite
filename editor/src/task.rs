use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
pub type Task = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
#[cfg(target_family = "wasm")]
pub type Task = Pin<Box<dyn Future<Output = ()> + 'static>>;

pub trait Spawn {
	fn spawn(&self, task: Task);
}

#[derive(Clone)]
pub struct TaskExecutorHandle {
	spawner: Arc<Spawner>,
}

impl Spawn for TaskExecutorHandle {
	fn spawn(&self, task: Task) {
		self.spawner.spawn(task);
	}
}

pub struct TaskExecutor {
	spawner: Arc<Spawner>,
}

impl TaskExecutor {
	pub fn new() -> Self {
		let spawner = {
			#[cfg(target_family = "wasm")]
			{
				Arc::new(Spawner::Wasm(WasmSpawner))
			}
			#[cfg(not(target_family = "wasm"))]
			{
				Arc::new(Spawner::Tokio(TokioSpawner::new()))
			}
		};
		Self { spawner }
	}
	pub fn blocking() -> Self {
		let spawner = Arc::new(Spawner::Blocking(BlockingSpawner));
		Self { spawner }
	}
	pub fn handle(&self) -> TaskExecutorHandle {
		TaskExecutorHandle { spawner: self.spawner.clone() }
	}
}

impl Default for TaskExecutor {
	fn default() -> Self {
		Self::new()
	}
}

enum Spawner {
	#[cfg(target_family = "wasm")]
	Wasm(WasmSpawner),
	#[cfg(not(target_family = "wasm"))]
	Tokio(TokioSpawner),
	Blocking(BlockingSpawner),
}

impl Spawn for Spawner {
	fn spawn(&self, task: Task) {
		match self {
			#[cfg(target_family = "wasm")]
			Spawner::Wasm(spawner) => spawner.spawn(task),
			#[cfg(not(target_family = "wasm"))]
			Spawner::Tokio(spawner) => spawner.spawn(task),
			Spawner::Blocking(spawner) => spawner.spawn(task),
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
