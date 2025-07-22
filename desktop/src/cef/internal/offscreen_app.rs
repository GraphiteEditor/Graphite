use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_app_t, cef_base_ref_counted_t};
use cef::{App, BrowserProcessHandler, ImplApp, SchemeRegistrar, WrapApp};

use crate::cef::internal::browser_process_handler::OffscreenBrowserProcessHandler;
use crate::cef::scheme_handler::GraphiteSchemeHandlerFactory;
use crate::render::FrameBufferHandle;

pub(crate) struct OffscreenApp {
	object: *mut RcImpl<_cef_app_t, Self>,
}
impl OffscreenApp {
	pub(crate) fn new() -> App {
		App::new(Self { object: std::ptr::null_mut() })
	}
}

impl ImplApp for OffscreenApp {
	fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
		Some(OffscreenBrowserProcessHandler::new())
	}

	fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
		GraphiteSchemeHandlerFactory::register_schemes(registrar);
	}

	fn get_raw(&self) -> *mut _cef_app_t {
		self.object.cast()
	}
}

impl Clone for OffscreenApp {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for OffscreenApp {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapApp for OffscreenApp {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_app_t, Self>) {
		self.object = object;
	}
}
