use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use super::NativeWindow;

pub(super) struct NativeWindowImpl {
	native_handle: windows::WindowsNativeWindowHandle,
}

impl NativeWindow for NativeWindowImpl {
	fn configure(attributes: WindowAttributes, _event_loop: &dyn ActiveEventLoop) -> WindowAttributes {
		if let Ok(win_icon) = winit::platform::windows::WinIcon::from_resource(1, None) {
			let icon = winit::icon::Icon(std::sync::Arc::new(win_icon));
			attributes.with_window_icon(Some(icon))
		} else {
			attributes
		}
	}

	fn new(_window: &dyn Window) -> Self {
		let native_handle = windows::WindowsNativeWindowHandle::new(window);
		NativeWindowImpl { native_handle }
	}
}
