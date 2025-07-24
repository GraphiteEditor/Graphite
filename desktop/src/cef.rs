use crate::{CustomEvent, FrameBuffer};
use std::{
	sync::{Arc, Mutex, mpsc::Receiver},
	time::Instant,
};

mod context;
mod dirs;
mod input;
mod internal;
mod scheme_handler;

pub(crate) use context::{Context, InitError, Initialized, Setup, SetupError};
use winit::event_loop::EventLoopProxy;

pub(crate) trait CefEventHandler: Clone {
	fn window_size(&self) -> WindowSize;
	fn draw(&self, frame_buffer: FrameBuffer);
	/// Scheudule the main event loop to run the cef event loop after the timeout
	///  [`_cef_browser_process_handler_t::on_schedule_message_pump_work`] for more documentation.
	fn schedule_cef_message_loop_work(&self, scheduled_time: Instant);
}

#[derive(Clone, Copy)]
pub(crate) struct WindowSize {
	pub(crate) width: usize,
	pub(crate) height: usize,
}

impl WindowSize {
	pub(crate) fn new(width: usize, height: usize) -> Self {
		Self { width, height }
	}
}

#[derive(Clone)]
pub(crate) struct CefHandler {
	window_size_receiver: Arc<Mutex<WindowSizeReceiver>>,
	event_loop_proxy: EventLoopProxy<CustomEvent>,
}
struct WindowSizeReceiver {
	receiver: Receiver<WindowSize>,
	window_size: WindowSize,
}
impl WindowSizeReceiver {
	fn new(window_size_receiver: Receiver<WindowSize>) -> Self {
		Self {
			window_size: WindowSize { width: 1, height: 1 },
			receiver: window_size_receiver,
		}
	}
}
impl CefHandler {
	pub(crate) fn new(window_size_receiver: Receiver<WindowSize>, event_loop_proxy: EventLoopProxy<CustomEvent>) -> Self {
		Self {
			window_size_receiver: Arc::new(Mutex::new(WindowSizeReceiver::new(window_size_receiver))),
			event_loop_proxy,
		}
	}
}

impl CefEventHandler for CefHandler {
	fn window_size(&self) -> WindowSize {
		let Ok(mut guard) = self.window_size_receiver.lock() else {
			tracing::error!("Failed to lock window_size_receiver");
			return WindowSize::new(1, 1);
		};
		let WindowSizeReceiver { receiver, window_size } = &mut *guard;
		for new_window_size in receiver.try_iter() {
			*window_size = new_window_size;
		}
		*window_size
	}
	fn draw(&self, frame_buffer: FrameBuffer) {
		let _ = self.event_loop_proxy.send_event(CustomEvent::UiUpdate(frame_buffer));
	}

	fn schedule_cef_message_loop_work(&self, scheduled_time: std::time::Instant) {
		let _ = self.event_loop_proxy.send_event(CustomEvent::ScheduleBrowserWork(scheduled_time));
	}
}
