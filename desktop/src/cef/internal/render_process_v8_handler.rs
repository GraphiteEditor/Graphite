use cef::{ImplV8Handler, WrapV8Handler, rc::Rc};
use std::sync::{Arc, Mutex, mpsc::Receiver};

pub struct BrowserProcessV8HandlerImpl {
	object: *mut cef::rc::RcImpl<cef::sys::_cef_v8_handler_t, Self>,
	receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
}

impl BrowserProcessV8HandlerImpl {
	pub(crate) fn new(receiver: Arc<Mutex<Receiver<Vec<u8>>>>) -> Self {
		Self {
			object: std::ptr::null_mut(),
			receiver,
		}
	}
}

impl ImplV8Handler for BrowserProcessV8HandlerImpl {
	fn get_raw(&self) -> *mut cef::sys::_cef_v8_handler_t {
		self.object.cast()
	}
}

impl Clone for BrowserProcessV8HandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			receiver: self.receiver.clone(),
		}
	}
}

impl Rc for BrowserProcessV8HandlerImpl {
	fn as_base(&self) -> &cef::sys::cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}

impl WrapV8Handler for BrowserProcessV8HandlerImpl {
	fn wrap_rc(&mut self, object: *mut cef::rc::RcImpl<cef::sys::_cef_v8_handler_t, Self>) {
		self.object = object;
	}
}
