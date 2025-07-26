use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_client_t, cef_base_ref_counted_t};
use cef::{ImplClient, RenderHandler, WrapClient};

pub(crate) struct ClientImpl {
	object: *mut RcImpl<_cef_client_t, Self>,
	render_handler: RenderHandler,
}
impl ClientImpl {
	pub(crate) fn new(render_handler: RenderHandler) -> Self {
		Self {
			object: std::ptr::null_mut(),
			render_handler,
		}
	}
}

impl ImplClient for ClientImpl {
	fn render_handler(&self) -> Option<RenderHandler> {
		Some(self.render_handler.clone())
	}

	fn get_raw(&self) -> *mut _cef_client_t {
		self.object.cast()
	}
}

impl Clone for ClientImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			render_handler: self.render_handler.clone(),
		}
	}
}
impl Rc for ClientImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapClient for ClientImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_client_t, Self>) {
		self.object = object;
	}
}
