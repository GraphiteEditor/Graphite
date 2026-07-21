use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_load_handler_t, cef_base_ref_counted_t, cef_load_handler_t};
use cef::{ImplBrowser, ImplBrowserHost, ImplLoadHandler, WrapLoadHandler};

use crate::delegate::BrowserDelegate;

pub(crate) struct LoadHandlerImpl {
	object: *mut RcImpl<cef_load_handler_t, Self>,
	delegate: BrowserDelegate,
}
impl LoadHandlerImpl {
	pub(crate) fn new(delegate: BrowserDelegate) -> Self {
		Self {
			object: std::ptr::null_mut(),
			delegate,
		}
	}
}

impl ImplLoadHandler for LoadHandlerImpl {
	fn on_loading_state_change(&self, browser: Option<&mut cef::Browser>, is_loading: std::ffi::c_int, _can_go_back: std::ffi::c_int, _can_go_forward: std::ffi::c_int) {
		let view_info = self.delegate.view_info();

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

impl Clone for LoadHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			delegate: self.delegate.clone(),
		}
	}
}
impl Rc for LoadHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapLoadHandler for LoadHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_load_handler_t, Self>) {
		self.object = object;
	}
}
