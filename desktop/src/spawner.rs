use crate::wrapper::{Spawn, Task};

pub(crate) struct TokioSpawner(tokio::runtime::Handle);
impl TokioSpawner {
	pub(crate) fn new(handle: tokio::runtime::Handle) -> Self {
		Self(handle)
	}
}
impl Spawn for TokioSpawner {
	fn spawn(&self, task: Task) {
		self.0.spawn(task);
	}
}
