use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_app_t, cef_base_ref_counted_t};
use cef::{App, BrowserProcessHandler, ImplApp, SchemeRegistrar, WrapApp};

use crate::cef::CefEventHandler;
use crate::cef::scheme_handler::GraphiteSchemeHandlerFactory;

use super::browser_process_handler::BrowserProcessHandlerImpl;

pub(crate) struct AppImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_app_t, Self>,
	event_handler: H,
}
impl<H: CefEventHandler> AppImpl<H> {
	pub(crate) fn new(event_handler: H) -> App {
		App::new(Self {
			object: std::ptr::null_mut(),
			event_handler,
		})
	}
}

impl<H: CefEventHandler> ImplApp for AppImpl<H> {
	fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
		Some(BrowserProcessHandlerImpl::new(self.event_handler.clone()))
	}

	fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
		GraphiteSchemeHandlerFactory::register_schemes(registrar);
	}

	fn get_raw(&self) -> *mut _cef_app_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler> Clone for AppImpl<H> {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			event_handler: self.event_handler.clone(),
		}
	}
}
impl<H: CefEventHandler> Rc for AppImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapApp for AppImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_app_t, Self>) {
		self.object = object;
	}
}
