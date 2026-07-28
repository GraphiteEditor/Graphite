use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_browser_process_handler_t, cef_base_ref_counted_t, cef_browser_process_handler_t};
use cef::{CefString, ImplBrowserProcessHandler, WrapBrowserProcessHandler};

pub(crate) struct BrowserProcessHandlerImpl {
	object: *mut RcImpl<cef_browser_process_handler_t, Self>,
}
impl BrowserProcessHandlerImpl {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplBrowserProcessHandler for BrowserProcessHandlerImpl {
	fn on_already_running_app_relaunch(&self, _command_line: Option<&mut cef::CommandLine>, _current_directory: Option<&CefString>) -> std::ffi::c_int {
		1 // Return 1 to prevent default behavior of opening a empty browser window
	}

	fn get_raw(&self) -> *mut _cef_browser_process_handler_t {
		self.object.cast()
	}
}

impl Clone for BrowserProcessHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for BrowserProcessHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapBrowserProcessHandler for BrowserProcessHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_browser_process_handler_t, Self>) {
		self.object = object;
	}
}
