use std::time::{Duration, Instant};

use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_browser_process_handler_t, cef_base_ref_counted_t, cef_browser_process_handler_t};
use cef::{CefString, ImplBrowserProcessHandler, SchemeHandlerFactory, WrapBrowserProcessHandler};

use crate::cef::CefEventHandler;
use crate::cef::scheme_handler::{GRAPHITE_SCHEME, GraphiteSchemeHandlerFactory};

pub(crate) struct BrowserProcessHandlerImpl<H: CefEventHandler> {
	object: *mut RcImpl<cef_browser_process_handler_t, Self>,
	event_handler: H,
}
impl<H: CefEventHandler> BrowserProcessHandlerImpl<H> {
	pub(crate) fn new(event_handler: H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler,
		}
	}
}

impl<H: CefEventHandler> ImplBrowserProcessHandler for BrowserProcessHandlerImpl<H> {
	fn on_context_initialized(&self) {
		cef::register_scheme_handler_factory(Some(&CefString::from(GRAPHITE_SCHEME)), None, Some(&mut SchemeHandlerFactory::new(GraphiteSchemeHandlerFactory::new())));
	}

	fn get_raw(&self) -> *mut _cef_browser_process_handler_t {
		self.object.cast()
	}

	fn on_schedule_message_pump_work(&self, delay_ms: i64) {
		self.event_handler.schedule_cef_message_loop_work(Instant::now() + Duration::from_millis(delay_ms as u64));
	}
}

impl<H: CefEventHandler> Clone for BrowserProcessHandlerImpl<H> {
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
impl<H: CefEventHandler> Rc for BrowserProcessHandlerImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapBrowserProcessHandler for BrowserProcessHandlerImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_browser_process_handler_t, Self>) {
		self.object = object;
	}
}
