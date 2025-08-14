mod editor_wrapper;
pub use editor_wrapper::EditorWrapper;

pub mod messages;
use messages::{EditorMessage, NativeMessage};

pub trait EditorApi {
	fn dispatch(&mut self, message: EditorMessage) -> Vec<NativeMessage>;
	fn poll() -> Vec<NativeMessage>;
}
