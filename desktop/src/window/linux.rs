use winit::event_loop::ActiveEventLoop;
use winit::platform::wayland::ActiveEventLoopExtWayland;
use winit::platform::wayland::WindowAttributesWayland;
use winit::platform::x11::WindowAttributesX11;
use winit::window::{Window, WindowAttributes};

use crate::consts::{APP_ID, APP_NAME};

use super::NativeWindow;

pub(super) struct NativeWindowImpl {}

impl NativeWindow for NativeWindowImpl {
	fn configure(attributes: WindowAttributes, event_loop: &dyn ActiveEventLoop) -> WindowAttributes {
		if event_loop.is_wayland() {
			let wayland_attributes = WindowAttributesWayland::default().with_name(APP_ID, "").with_prefer_csd(true);
			attributes.with_platform_attributes(Box::new(wayland_attributes))
		} else {
			let x11_attributes = WindowAttributesX11::default().with_name(APP_ID, APP_NAME);
			attributes.with_platform_attributes(Box::new(x11_attributes))
		}
	}

	fn new(_window: &dyn Window) -> Self {
		NativeWindowImpl {}
	}
}
