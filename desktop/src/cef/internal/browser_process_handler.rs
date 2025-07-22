use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_browser_process_handler_t, cef_base_ref_counted_t, cef_browser_process_handler_t};
use cef::{BrowserProcessHandler, CefString, ImplBrowserProcessHandler, SchemeHandlerFactory, WrapBrowserProcessHandler};

use crate::cef::scheme_handler::GraphiteSchemeHandlerFactory;

pub(crate) struct OffscreenBrowserProcessHandler {
	object: *mut RcImpl<cef_browser_process_handler_t, Self>,
}
impl OffscreenBrowserProcessHandler {
	pub(crate) fn new() -> BrowserProcessHandler {
		BrowserProcessHandler::new(Self { object: std::ptr::null_mut() })
	}
}

impl ImplBrowserProcessHandler for OffscreenBrowserProcessHandler {
	fn on_context_initialized(&self) {
		cef::register_scheme_handler_factory(Some(&CefString::from("graphite")), None, Some(&mut SchemeHandlerFactory::new(GraphiteSchemeHandlerFactory::new())));
	}

	fn get_raw(&self) -> *mut _cef_browser_process_handler_t {
		self.object.cast()
	}
}

impl Clone for OffscreenBrowserProcessHandler {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for OffscreenBrowserProcessHandler {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapBrowserProcessHandler for OffscreenBrowserProcessHandler {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_browser_process_handler_t, Self>) {
		self.object = object;
	}
}
