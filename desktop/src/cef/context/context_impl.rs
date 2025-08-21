use std::cell::RefCell;

use cef::sys::cef_thread_id_t;

use cef::{Browser, ImplBrowser, ImplBrowserHost, ThreadId};

use crate::cef::input;
use crate::cef::input::InputState;
use crate::cef::ipc;
use crate::cef::ipc::{MessageType, SendMessage};

use super::cef_task;

use winit::event::WindowEvent;

use super::{CefContext, Context, Initialized};

impl CefContext for Context<Initialized> {
	fn work(&mut self) {
		cef::do_message_loop_work();
	}

	fn handle_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent> {
		if let Some(browser) = &self.browser {
			input::handle_window_event(browser, &mut self.input_state, event)
		} else {
			Some(event)
		}
	}

	fn notify_of_resize(&self) {
		if let Some(browser) = &self.browser {
			browser.host().unwrap().was_resized();
		}
	}

	fn send_web_message(&self, message: Vec<u8>) {
		self.send_message(MessageType::SendToJS, &message);
	}
}
// Thread-local browser storage for UI thread
thread_local! {
	pub (super) static BROWSER: RefCell<Option<(Browser, InputState)>> = const { RefCell::new(None) };
}

// New proxy that uses closure tasks instead of channels
pub struct CefContextSendProxy;

impl CefContext for CefContextSendProxy {
	fn work(&mut self) {
		// CEF handles its own message loop in multi-threaded mode
	}

	fn handle_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent> {
		let event_clone = event.clone();
		cef_task::post_closure_task(ThreadId::from(cef_thread_id_t::TID_UI), move || {
			BROWSER.with(|b| {
				if let Some((browser, input_state)) = b.borrow_mut().as_mut() {
					// Forward window event to CEF input handling on UI thread
					input::handle_window_event(browser, input_state, event_clone);
				}
			});
		});
		Some(event)
	}

	fn notify_of_resize(&self) {
		cef_task::post_closure_task(ThreadId::from(cef_thread_id_t::TID_UI), || {
			BROWSER.with(|b| {
				if let Some((browser, _)) = b.borrow().as_ref() {
					if let Some(host) = browser.host() {
						host.was_resized();
					}
				}
			});
		});
	}

	fn send_web_message(&self, message: Vec<u8>) {
		cef_task::post_closure_task(ThreadId::from(cef_thread_id_t::TID_UI), move || {
			BROWSER.with(|b| {
				if let Some((browser, _)) = b.borrow().as_ref() {
					// Inline the send_message functionality
					use ipc::{MessageType, SendMessage};
					if let Some(frame) = browser.main_frame() {
						let message_bytes = &message;
						frame.send_message(MessageType::SendToJS, message_bytes);
					}
				}
			});
		});
	}
}
