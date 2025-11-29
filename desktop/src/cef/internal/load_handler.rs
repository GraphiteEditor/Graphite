use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_load_handler_t, cef_base_ref_counted_t, cef_load_handler_t};
use cef::{ImplBrowser, ImplBrowserHost, ImplLoadHandler, WrapLoadHandler};

use crate::cef::CefEventHandler;

pub(crate) struct LoadHandlerImpl<H: CefEventHandler> {
	object: *mut RcImpl<cef_load_handler_t, Self>,
	event_handler: H,
}
impl<H: CefEventHandler> LoadHandlerImpl<H> {
	pub(crate) fn new(event_handler: H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler,
		}
	}
}

impl<H: CefEventHandler> ImplLoadHandler for LoadHandlerImpl<H> {
	fn on_loading_state_change(&self, browser: Option<&mut cef::Browser>, is_loading: std::ffi::c_int, _can_go_back: std::ffi::c_int, _can_go_forward: std::ffi::c_int) {
		let view_info = self.event_handler.view_info();

		if let Some(browser) = browser
			&& is_loading == 0
		{
			browser.host().unwrap().set_zoom_level(view_info.zoom());
		}
	}

	fn get_raw(&self) -> *mut _cef_load_handler_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler> Clone for LoadHandlerImpl<H> {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			event_handler: self.event_handler.duplicate(),
		}
	}
}
impl<H: CefEventHandler> Rc for LoadHandlerImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapLoadHandler for LoadHandlerImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_load_handler_t, Self>) {
		self.object = object;
	}
}
