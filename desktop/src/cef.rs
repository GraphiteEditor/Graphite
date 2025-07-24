use crate::FrameBuffer;
use std::time::Instant;

mod context;
mod dirs;
mod input;
mod internal;
mod scheme_handler;

pub(crate) use context::{Context, InitError, Initialized, Setup, SetupError};

pub(crate) trait CefEventHandler: Clone {
	fn window_size(&self) -> WindowSize;
	fn draw(&self, frame_buffer: FrameBuffer) -> bool;
	/// Scheudule the main event loop to run the cef event loop after the timeout
	///  [`_cef_browser_process_handler_t::on_schedule_message_pump_work`] for more documentation.
	fn schedule_cef_message_loop_work(&self, scheduled_time: Instant);
}

#[derive(Clone)]
pub(crate) struct WindowSize {
	pub(crate) width: usize,
	pub(crate) height: usize,
}

impl WindowSize {
	pub(crate) fn new(width: usize, height: usize) -> Self {
		Self { width, height }
	}
}
