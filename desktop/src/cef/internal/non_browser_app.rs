use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_app_t, cef_base_ref_counted_t};
use cef::{App, ImplApp, SchemeRegistrar, WrapApp};

use crate::cef::scheme_handler::GraphiteSchemeHandlerFactory;

pub(crate) struct NonBrowserAppImpl {
	object: *mut RcImpl<_cef_app_t, Self>,
}
impl NonBrowserAppImpl {
	pub(crate) fn app() -> App {
		App::new(Self { object: std::ptr::null_mut() })
	}
}

impl ImplApp for NonBrowserAppImpl {
	fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
		GraphiteSchemeHandlerFactory::register_schemes(registrar);
	}

	fn get_raw(&self) -> *mut _cef_app_t {
		self.object.cast()
	}
}

impl Clone for NonBrowserAppImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for NonBrowserAppImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapApp for NonBrowserAppImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_app_t, Self>) {
		self.object = object;
	}
}
