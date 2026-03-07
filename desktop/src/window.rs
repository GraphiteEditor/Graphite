use crate::consts::APP_NAME;
use crate::event::AppEventScheduler;
use crate::wrapper::messages::MenuItem;
use std::collections::HashMap;
use std::sync::Arc;
use winit::cursor::{CursorIcon, CustomCursor, CustomCursorSource};
use winit::event_loop::ActiveEventLoop;
use winit::monitor::Fullscreen;
use winit::window::{Window as WinitWindow, WindowAttributes};

pub(crate) trait NativeWindow {
	fn init() {}
	fn configure(attributes: WindowAttributes, event_loop: &dyn ActiveEventLoop) -> WindowAttributes;
	fn new(window: &dyn WinitWindow, app_event_scheduler: AppEventScheduler) -> Self;
	fn can_render(&self) -> bool {
		true
	}
	fn update_menu(&self, _entries: Vec<MenuItem>) {}
	fn hide(&self) {}
	fn hide_others(&self) {}
	fn show_all(&self) {}
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
	custom_cursors: HashMap<CustomCursorSource, CustomCursor>,
	clipboard: Option<window_clipboard::Clipboard>,
}
impl Drop for Window {
	fn drop(&mut self) {
		// Clipboard must be dropped before `winit_window`
		drop(self.clipboard.take());
	}
}

impl Window {
	pub(crate) fn init() {
		native::NativeWindowImpl::init();
	}

	pub(crate) fn new(event_loop: &dyn ActiveEventLoop, app_event_scheduler: AppEventScheduler) -> Self {
		let mut attributes = WindowAttributes::default()
			.with_title(APP_NAME)
			.with_min_surface_size(winit::dpi::LogicalSize::new(400, 300))
			.with_surface_size(winit::dpi::LogicalSize::new(1200, 800))
			.with_resizable(true)
			.with_visible(false)
			.with_theme(Some(winit::window::Theme::Dark));

		attributes = native::NativeWindowImpl::configure(attributes, event_loop);

		let winit_window = event_loop.create_window(attributes).unwrap();
		let native_handle = native::NativeWindowImpl::new(winit_window.as_ref(), app_event_scheduler);
		let clipboard = unsafe { window_clipboard::Clipboard::connect(&winit_window) }.ok();
		Self {
			winit_window: winit_window.into(),
			native_handle,
			custom_cursors: HashMap::new(),
			clipboard,
		}
	}

	pub(crate) fn show(&self) {
		self.winit_window.set_visible(true);
		self.winit_window.focus_window();
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

	pub(crate) fn can_render(&self) -> bool {
		self.native_handle.can_render()
	}

	pub(crate) fn surface_size(&self) -> winit::dpi::PhysicalSize<u32> {
		self.winit_window.surface_size()
	}

	pub(crate) fn scale_factor(&self) -> f64 {
		self.winit_window.scale_factor()
	}

	pub(crate) fn minimize(&self) {
		self.winit_window.set_minimized(true);
	}

	pub(crate) fn toggle_maximize(&self) {
		if self.is_fullscreen() {
			return;
		}
		self.winit_window.set_maximized(!self.winit_window.is_maximized());
	}

	pub(crate) fn is_maximized(&self) -> bool {
		self.winit_window.is_maximized()
	}

	pub(crate) fn toggle_fullscreen(&mut self) {
		if self.is_fullscreen() {
			self.winit_window.set_fullscreen(None);
		} else {
			self.winit_window.set_fullscreen(Some(Fullscreen::Borderless(None)));
		}
	}

	pub(crate) fn is_fullscreen(&self) -> bool {
		self.winit_window.fullscreen().is_some()
	}

	pub(crate) fn start_drag(&self) {
		if self.is_fullscreen() {
			return;
		}
		let _ = self.winit_window.drag_window();
	}

	pub(crate) fn hide(&self) {
		self.native_handle.hide();
	}

	pub(crate) fn hide_others(&self) {
		self.native_handle.hide_others();
	}

	pub(crate) fn show_all(&self) {
		self.native_handle.show_all();
	}

	pub(crate) fn set_cursor(&mut self, event_loop: &dyn ActiveEventLoop, cursor: Cursor) {
		let cursor = match cursor {
			Cursor::Icon(cursor_icon) => cursor_icon.into(),
			Cursor::Custom(custom_cursor_source) => {
				let custom_cursor = match self.custom_cursors.get(&custom_cursor_source).cloned() {
					Some(cursor) => cursor,
					None => {
						let Ok(custom_cursor) = event_loop.create_custom_cursor(custom_cursor_source.clone()) else {
							tracing::error!("Failed to create custom cursor");
							return;
						};
						self.custom_cursors.insert(custom_cursor_source, custom_cursor.clone());
						custom_cursor
					}
				};
				custom_cursor.into()
			}
			Cursor::None => {
				self.winit_window.set_cursor_visible(false);
				return;
			}
		};
		self.winit_window.set_cursor_visible(true);
		self.winit_window.set_cursor(cursor);
	}

	pub(crate) fn start_pointer_lock(&self) {
		let _ = self.winit_window.set_cursor_grab(winit::window::CursorGrabMode::Locked);
		self.winit_window.set_cursor_visible(false);
	}

	pub(crate) fn end_pointer_lock(&self) {
		let _ = self.winit_window.set_cursor_grab(winit::window::CursorGrabMode::None);
		self.winit_window.set_cursor_visible(true);
	}

	pub(crate) fn update_menu(&self, entries: Vec<MenuItem>) {
		self.native_handle.update_menu(entries);
	}

	pub(crate) fn clipboard_read(&self) -> Option<String> {
		let Some(clipboard) = &self.clipboard else {
			tracing::error!("Clipboard not available");
			return None;
		};
		match clipboard.read() {
			Ok(data) => Some(data),
			Err(e) => {
				tracing::error!("Failed to read from clipboard: {e}");
				None
			}
		}
	}

	pub(crate) fn clipboard_write(&mut self, data: String) {
		let Some(clipboard) = &mut self.clipboard else {
			tracing::error!("Clipboard not available");
			return;
		};
		if let Err(e) = clipboard.write(data) {
			tracing::error!("Failed to write to clipboard: {e}")
		}
	}
}

pub(crate) enum Cursor {
	Icon(CursorIcon),
	Custom(CustomCursorSource),
	None,
}
impl From<CursorIcon> for Cursor {
	fn from(icon: CursorIcon) -> Self {
		Cursor::Icon(icon)
	}
}
impl From<CustomCursorSource> for Cursor {
	fn from(custom: CustomCursorSource) -> Self {
		Cursor::Custom(custom)
	}
}
