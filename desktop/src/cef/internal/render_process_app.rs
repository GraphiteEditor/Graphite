use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_app_t, cef_base_ref_counted_t};
use cef::{App, ImplApp, RenderProcessHandler, SchemeRegistrar, WrapApp};

use super::render_process_handler::RenderProcessHandlerImpl;
use super::scheme_handler_factory::SchemeHandlerFactoryImpl;
use crate::cef::CefEventHandler;

pub(crate) struct RenderProcessAppImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_app_t, Self>,
	render_process_handler: RenderProcessHandler,
}
impl<H: CefEventHandler> RenderProcessAppImpl<H> {
	pub(crate) fn app() -> App {
		App::new(Self {
			object: std::ptr::null_mut(),
			render_process_handler: RenderProcessHandler::new(RenderProcessHandlerImpl::new()),
		})
	}
}

impl<H: CefEventHandler> ImplApp for RenderProcessAppImpl<H> {
	fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
		SchemeHandlerFactoryImpl::<H>::register_schemes(registrar);
	}

	fn render_process_handler(&self) -> Option<RenderProcessHandler> {
		Some(self.render_process_handler.clone())
	}

	fn get_raw(&self) -> *mut _cef_app_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler> Clone for RenderProcessAppImpl<H> {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			render_process_handler: self.render_process_handler.clone(),
		}
	}
}
impl<H: CefEventHandler> Rc for RenderProcessAppImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapApp for RenderProcessAppImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_app_t, Self>) {
		self.object = object;
	}
}
