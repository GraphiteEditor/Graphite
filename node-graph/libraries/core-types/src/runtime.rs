use std::pin::Pin;

#[cfg(not(target_family = "wasm"))]
pub type Task = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
#[cfg(target_family = "wasm")]
pub type Task = Pin<Box<dyn Future<Output = ()> + 'static>>;

pub trait Spawn {
	fn spawn(&self, task: Task);
}
