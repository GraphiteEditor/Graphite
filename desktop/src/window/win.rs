use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::HSTRING;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use crate::consts::APP_ID;
use crate::event::AppEventScheduler;

pub(super) struct NativeWindowImpl {
	native_handle: native_handle::NativeWindowHandle,
}

impl super::NativeWindow for NativeWindowImpl {
	fn init() {
		let app_id = HSTRING::from(APP_ID);
		unsafe {
			let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok();
			SetCurrentProcessExplicitAppUserModelID(&app_id).ok();
		}
	}

	fn configure(attributes: WindowAttributes, _event_loop: &dyn ActiveEventLoop) -> WindowAttributes {
		attributes
	}

	fn new(window: &dyn Window, _app_event_scheduler: AppEventScheduler) -> Self {
		let native_handle = native_handle::NativeWindowHandle::new(window);
		NativeWindowImpl { native_handle }
	}

	fn can_render(&self) -> bool {
		self.native_handle.can_render()
	}
}

impl Drop for NativeWindowImpl {
	fn drop(&mut self) {
		self.native_handle.destroy();
	}
}

mod native_handle;
