mod input;

mod scheme_handler;

mod context;

mod internal;

pub(crate) trait EventHandler: Clone {
	fn view(&self) -> View;
	fn draw(&self, buffer: Vec<u8>, width: usize, height: usize) -> bool;
}

#[derive(Clone)]
pub(crate) struct View {
	pub(crate) width: usize,
	pub(crate) height: usize,
}

impl View {
	pub(crate) fn new(width: usize, height: usize) -> Self {
		Self { width, height }
	}
}

pub(crate) use context::{Context, InitError, Initialized, Setup, SetupError};
