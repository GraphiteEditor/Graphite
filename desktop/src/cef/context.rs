#[cfg(not(target_os = "macos"))]
mod multithreaded;
mod singlethreaded;

mod builder;
pub(crate) use builder::{CefContextBuilder, InitError};

pub(crate) trait CefContext {
	fn work(&mut self);

	fn handle_window_event(&mut self, event: &winit::event::WindowEvent);

	fn notify_of_resize(&self);

	fn send_web_message(&self, message: Vec<u8>);
}
