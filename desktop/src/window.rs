use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window as WinitWindow, WindowAttributes};

use crate::consts::APP_NAME;

pub(crate) trait NativeWindow {
	fn configure(attributes: WindowAttributes, event_loop: &dyn ActiveEventLoop) -> WindowAttributes;
	fn new(window: &dyn WinitWindow) -> Self;
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as native;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
use mac as native;

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
use win as native;

pub(crate) struct Window {
	winit_window: Arc<dyn winit::window::Window>,
	#[allow(dead_code)]
	native_handle: native::NativeWindowImpl,
}

impl Window {
	pub(crate) fn new(event_loop: &dyn ActiveEventLoop) -> Self {
		let mut attributes = WindowAttributes::default()
			.with_title(APP_NAME)
			.with_min_surface_size(winit::dpi::LogicalSize::new(400, 300))
			.with_surface_size(winit::dpi::LogicalSize::new(1200, 800))
			.with_resizable(true)
			.with_theme(Some(winit::window::Theme::Dark));

		attributes = native::NativeWindowImpl::configure(attributes, event_loop);

		let winit_window = event_loop.create_window(attributes).unwrap();
		let native_handle = native::NativeWindowImpl::new(winit_window.as_ref());
		Self {
			winit_window: winit_window.into(),
			native_handle,
		}
	}

	pub(crate) fn request_redraw(&self) {
		self.winit_window.request_redraw();
	}

	pub(crate) fn create_surface(&self, instance: Arc<wgpu::Instance>) -> wgpu::Surface<'static> {
		instance.create_surface(self.winit_window.clone()).unwrap()
	}

	pub(crate) fn pre_present_notify(&self) {
		self.winit_window.pre_present_notify();
	}

	pub(crate) fn surface_size(&self) -> winit::dpi::PhysicalSize<u32> {
		self.winit_window.surface_size()
	}

	pub(crate) fn minimize(&self) {
		self.winit_window.set_minimized(true);
	}

	pub(crate) fn toggle_maximize(&self) {
		self.winit_window.set_maximized(!self.winit_window.is_maximized());
	}

	pub(crate) fn is_maximized(&self) -> bool {
		self.winit_window.is_maximized()
	}

	pub(crate) fn start_drag(&self) {
		let _ = self.winit_window.drag_window();
	}

	pub(crate) fn set_cursor(&self, cursor: winit::cursor::Cursor) {
		self.winit_window.set_cursor(cursor);
	}
}
