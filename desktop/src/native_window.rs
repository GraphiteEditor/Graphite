use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

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
	pub(super) fn build(&mut self, window: WindowAttributes, event_loop: &ActiveEventLoop) -> WindowAttributes {
		#[cfg(target_os = "linux")]
		{
			use crate::consts::{APP_ID, APP_NAME};
			use winit::platform::wayland::ActiveEventLoopExtWayland;
			if event_loop.is_wayland() {
				winit::platform::wayland::WindowAttributesExtWayland::with_name(window, APP_ID, "")
			} else {
				winit::platform::x11::WindowAttributesExtX11::with_name(window, APP_ID, APP_NAME)
			}
		}
		#[cfg(not(target_os = "linux"))]
		{
			window
		}
	}

	#[allow(unused_variables)]
	pub(crate) fn setup(&mut self, window: &Window) {
		#[cfg(target_os = "windows")]
		{
			*self = NativeWindowHandle::Windows(windows::WindowsNativeWindowHandle::new(window));
		}
	}
}
