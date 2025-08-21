use cef::sys::cef_thread_id_t;
use cef::{Task, ThreadId, post_task};
use std::cell::RefCell;
use winit::event::WindowEvent;

use crate::cef::internal::task::ClosureTask;

use super::CefContext;
use super::singlethreaded::SingleThreadedCefContext;

thread_local! {
	pub(super) static CONTEXT: RefCell<Option<SingleThreadedCefContext>> = const { RefCell::new(None) };
}

pub(super) struct MultiThreadedCefContextProxy;

impl CefContext for MultiThreadedCefContextProxy {
	fn work(&mut self) {
		// CEF handles its own message loop in multi-threaded mode
	}

	fn handle_window_event(&mut self, event: &WindowEvent) {
		let event_clone = event.clone();
		run_on_ui_thread(move || {
			CONTEXT.with(|b| {
				if let Some(context) = b.borrow_mut().as_mut() {
					context.handle_window_event(&event_clone);
				}
			});
		});
	}

	fn notify_of_resize(&self) {
		run_on_ui_thread(move || {
			CONTEXT.with(|b| {
				if let Some(context) = b.borrow_mut().as_mut() {
					context.notify_of_resize();
				}
			});
		});
	}

	fn send_web_message(&self, message: Vec<u8>) {
		run_on_ui_thread(move || {
			CONTEXT.with(|b| {
				if let Some(context) = b.borrow_mut().as_mut() {
					context.send_web_message(message);
				}
			});
		});
	}
}

impl Drop for MultiThreadedCefContextProxy {
	fn drop(&mut self) {
		cef::shutdown();
	}
}

pub(super) fn run_on_ui_thread<F>(closure: F)
where
	F: FnOnce() + Send + 'static,
{
	let closure_task = ClosureTask::new(closure);
	let mut task = Task::new(closure_task);
	post_task(ThreadId::from(cef_thread_id_t::TID_UI), Some(&mut task));
}
