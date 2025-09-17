use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use crate::consts::APP_NAME;

#[cfg(target_os = "windows")]
mod windows;

pub(crate) enum NativeWindowHandle {
	#[cfg(target_os = "windows")]
	#[expect(private_interfaces, dead_code)]
	Windows(windows::WindowsNativeWindowHandle),
	None,
}
impl Default for NativeWindowHandle {
	fn default() -> Self {
		Self::None
	}
}
impl NativeWindowHandle {
	#[allow(unused_variables)]
	pub(super) fn build(&mut self, event_loop: &dyn ActiveEventLoop) -> WindowAttributes {
		let mut window = WindowAttributes::default()
			.with_title(APP_NAME)
			.with_min_surface_size(winit::dpi::LogicalSize::new(400, 300))
			.with_surface_size(winit::dpi::LogicalSize::new(1200, 800))
			.with_resizable(true);

		#[cfg(target_os = "linux")]
		{
			use crate::consts::{APP_ID, APP_NAME};
			use winit::platform::wayland::ActiveEventLoopExtWayland;
			use winit::platform::wayland::WindowAttributesWayland;
			use winit::platform::x11::WindowAttributesX11;
			window = if event_loop.is_wayland() {
				let wayland_window = WindowAttributesWayland::default().with_name(APP_ID, "");
				window.with_platform_attributes(Box::new(wayland_window))
			} else {
				let x11_window = WindowAttributesX11::default().with_name(APP_ID, APP_NAME);
				window.with_platform_attributes(Box::new(x11_window))
			}
		}
		window
	}
	#[allow(unused_variables)]
	pub(crate) fn setup(&mut self, window: &dyn Window) {
		#[cfg(target_os = "windows")]
		{
			*self = NativeWindowHandle::Windows(windows::WindowsNativeWindowHandle::new(window));
		}
	}
}
