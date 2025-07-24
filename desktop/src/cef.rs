mod context;
mod input;
mod internal;
mod scheme_handler;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

pub(crate) use context::{Context, InitError, Initialized, Setup, SetupError};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WindowSize {
	pub(crate) width: u32,
	pub(crate) height: u32,
}

impl WindowSize {
	pub(crate) fn new(width: u32, height: u32) -> Self {
		Self { width, height }
	}
}

// Shared between the CEF render handler and the Winit app
#[derive(Clone, Default)]
pub(crate) struct WindowSizeHandle {
	inner: Arc<Mutex<Option<WindowSize>>>,
}

impl WindowSizeHandle {
	pub fn with<P>(&self, p: P) -> Result<(), PoisonError<MutexGuard<Option<WindowSize>>>>
	where
		P: FnOnce(&mut Option<WindowSize>),
	{
		let mut guard = self.inner.lock()?;
		p(&mut guard);
		Ok(())
	}
}
