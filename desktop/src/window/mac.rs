use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use super::NativeWindow;

pub(super) struct NativeWindowImpl {}

impl NativeWindow for NativeWindowImpl {
	fn configure(attributes: WindowAttributes, _event_loop: &dyn ActiveEventLoop) -> WindowAttributes {
		let mac_window = winit::platform::macos::WindowAttributesMacOS::default()
			.with_titlebar_transparent(true)
			.with_fullsize_content_view(true)
			.with_title_hidden(true);
		attributes.with_platform_attributes(Box::new(mac_window))
	}

	fn new(_window: &dyn Window) -> Self {
		NativeWindowImpl {}
	}
}
