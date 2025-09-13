use cef::{Browser, ImplBrowser, ImplBrowserHost};
use winit::event::WindowEvent;

use crate::cef::input;
use crate::cef::input::InputState;
use crate::cef::ipc::{MessageType, SendMessage};

use super::CefContext;

pub(super) struct SingleThreadedCefContext {
	pub(super) browser: Browser,
	pub(super) input_state: InputState,
	pub(super) instance_dir: std::path::PathBuf,
}

impl CefContext for SingleThreadedCefContext {
	fn work(&mut self) {
		cef::do_message_loop_work();
	}

	fn handle_window_event(&mut self, event: &WindowEvent) {
		input::handle_window_event(&self.browser, &mut self.input_state, event)
	}

	fn notify_of_resize(&self) {
		self.browser.host().unwrap().was_resized();
	}

	fn send_web_message(&self, message: Vec<u8>) {
		self.send_message(MessageType::SendToJS, &message);
	}
}

impl Drop for SingleThreadedCefContext {
	fn drop(&mut self) {
		cef::shutdown();
		std::fs::remove_dir_all(&self.instance_dir).expect("Failed to remove CEF cache directory");
	}
}

impl SendMessage for SingleThreadedCefContext {
	fn send_message(&self, message_type: MessageType, message: &[u8]) {
		let Some(frame) = self.browser.main_frame() else {
			tracing::error!("Main frame is not available, cannot send message");
			return;
		};

		frame.send_message(message_type, message);
	}
}
