use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::icon::Icon;
use winit::platform::windows::{WinIcon, WindowAttributesWindows};
use winit::window::{Window, WindowAttributes};

use crate::event::AppEventScheduler;

pub(super) struct NativeWindowImpl {
	native_handle: native_handle::NativeWindowHandle,
}

impl super::NativeWindow for NativeWindowImpl {
	fn configure(attributes: WindowAttributes, _event_loop: &dyn ActiveEventLoop) -> WindowAttributes {
		let icon = WinIcon::from_resource(1, Some(PhysicalSize::new(256, 256))).ok().map(|icon| Icon(std::sync::Arc::new(icon)));
		let win_window = WindowAttributesWindows::default().with_taskbar_icon(icon);
		let icon = WinIcon::from_resource(1, None).ok().map(|icon| Icon(std::sync::Arc::new(icon)));
		attributes.with_window_icon(icon).with_platform_attributes(Box::new(win_window))
	}

	fn new(window: &dyn Window, _app_event_scheduler: AppEventScheduler) -> Self {
		let native_handle = native_handle::NativeWindowHandle::new(window);
		NativeWindowImpl { native_handle }
	}
}

impl Drop for NativeWindowImpl {
	fn drop(&mut self) {
		self.native_handle.destroy();
	}
}

mod native_handle;
