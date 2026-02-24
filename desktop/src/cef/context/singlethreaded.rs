use cef::{Browser, ImplBrowser, ImplBrowserHost};
use winit::event::WindowEvent;

use crate::cef::input::InputState;
use crate::cef::ipc::{MessageType, SendMessage};
use crate::cef::{CefEventHandler, input};

use super::CefContext;

pub(super) struct SingleThreadedCefContext {
	pub(super) event_handler: Box<dyn CefEventHandler>,
	pub(super) browser: Browser,
	pub(super) input_state: InputState,
	pub(super) instance_dir: std::path::PathBuf,
}

impl CefContext for SingleThreadedCefContext {
	fn work(&mut self) {
		cef::do_message_loop_work();
	}

	fn handle_window_event(&mut self, event: &WindowEvent) {
		input::handle_window_event(&self.browser, &mut self.input_state, event);
	}

	fn notify_view_info_changed(&self) {
		let view_info = self.event_handler.view_info();
		let host = self.browser.host().unwrap();
		host.set_zoom_level(view_info.zoom());
		host.was_resized();

		// Fix for CEF not updating the view after resize on windows and mac
		// TODO: remove once https://github.com/chromiumembedded/cef/issues/3822 is fixed
		#[cfg(any(target_os = "windows", target_os = "macos"))]
		host.invalidate(cef::PaintElementType::default());
	}

	fn send_web_message(&self, message: Vec<u8>) {
		self.send_message(MessageType::SendToJS, &message);
	}
}

impl Drop for SingleThreadedCefContext {
	fn drop(&mut self) {
		tracing::debug!("Shutting down CEF");

		// CEF wants us to close the browser before shutting down, otherwise it may run longer that necessary.
		self.browser.host().unwrap().close_browser(1);
		cef::shutdown();

		// Sometimes some CEF processes still linger at this point and hold file handles to the cache directory.
		// To mitigate this, we try to remove the directory multiple times with some delay.
		// TODO: find a better solution if possible.
		for _ in 0..30 {
			match std::fs::remove_dir_all(&self.instance_dir) {
				Ok(_) => break,
				Err(e) => {
					tracing::warn!("Failed to remove CEF cache directory, retrying...: {e}");
					std::thread::sleep(std::time::Duration::from_millis(100));
				}
			}
		}
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
