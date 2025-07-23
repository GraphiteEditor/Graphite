mod input;

mod scheme_handler;

mod context;

mod internal;

pub(crate) trait CefEventHandler: Clone {
	fn window_size(&self) -> WindowSize;
	fn draw(&self, frame_buffer: FrameBuffer) -> bool;
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

pub(crate) use context::{Context, InitError, Initialized, Setup, SetupError};

use crate::FrameBuffer;
