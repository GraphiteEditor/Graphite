use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_task_t, cef_base_ref_counted_t};
use cef::{ImplTask, WrapTask};
use std::cell::RefCell;

// Closure-based task wrapper following CEF patterns
pub struct ClosureTask<F> {
	pub(crate) object: *mut RcImpl<_cef_task_t, Self>,
	pub(crate) closure: RefCell<Option<F>>,
}

impl<F: FnOnce() + Send + 'static> ClosureTask<F> {
	pub fn new(closure: F) -> Self {
		Self {
			object: std::ptr::null_mut(),
			closure: RefCell::new(Some(closure)),
		}
	}
}

impl<F: FnOnce() + Send + 'static> ImplTask for ClosureTask<F> {
	fn execute(&self) {
		if let Some(closure) = self.closure.borrow_mut().take() {
			closure();
		}
	}

	fn get_raw(&self) -> *mut _cef_task_t {
		self.object.cast()
	}
}

impl<F: FnOnce() + Send + 'static> Clone for ClosureTask<F> {
	fn clone(&self) -> Self {
		unsafe {
			if !self.object.is_null() {
				let rc_impl = &mut *self.object;
				rc_impl.interface.add_ref();
			}
		}
		Self {
			object: self.object,
			closure: RefCell::new(None), // Closure can only be executed once
		}
	}
}

impl<F: FnOnce() + Send + 'static> Rc for ClosureTask<F> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}

impl<F: FnOnce() + Send + 'static> WrapTask for ClosureTask<F> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_task_t, Self>) {
		self.object = object;
	}
}
