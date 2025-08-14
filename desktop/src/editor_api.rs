mod editor_wrapper;
pub use editor_wrapper::EditorWrapper;

pub mod messages;
use messages::{EditorMessage, NativeMessage};

pub use wgpu_executor::Context as WgpuContext;

pub trait EditorApi {
	fn dispatch(&mut self, message: EditorMessage) -> Vec<NativeMessage>;
	fn poll() -> Vec<NativeMessage>;
}
