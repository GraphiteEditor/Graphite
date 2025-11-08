#[cfg(not(target_os = "macos"))]
mod multithreaded;
mod singlethreaded;

mod builder;
pub(crate) use builder::{CefContextBuilder, InitError};

pub(crate) trait CefContext {
	fn work(&mut self);

	fn handle_window_event(&mut self, event: &winit::event::WindowEvent, scale: f64);

	fn notify_view_info_changed(&self);

	fn send_web_message(&self, message: Vec<u8>);
}
