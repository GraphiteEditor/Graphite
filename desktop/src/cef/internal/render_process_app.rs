use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_app_t, cef_base_ref_counted_t};
use cef::{App, ImplApp, RenderProcessHandler, SchemeRegistrar, WrapApp};

use super::render_process_handler::RenderProcessHandlerImpl;
use crate::cef::scheme_handler::GraphiteSchemeHandlerFactory;

pub(crate) struct RenderProcessAppImpl {
	object: *mut RcImpl<_cef_app_t, Self>,
	render_process_handler: RenderProcessHandler,
}
impl RenderProcessAppImpl {
	pub(crate) fn app() -> App {
		App::new(Self {
			object: std::ptr::null_mut(),
			render_process_handler: RenderProcessHandler::new(RenderProcessHandlerImpl::new()),
		})
	}
}

impl ImplApp for RenderProcessAppImpl {
	fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
		GraphiteSchemeHandlerFactory::register_schemes(registrar);
	}

	fn render_process_handler(&self) -> Option<RenderProcessHandler> {
		Some(self.render_process_handler.clone())
	}

	fn get_raw(&self) -> *mut _cef_app_t {
		self.object.cast()
	}
}

impl Clone for RenderProcessAppImpl {
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
impl Rc for RenderProcessAppImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapApp for RenderProcessAppImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_app_t, Self>) {
		self.object = object;
	}
}
