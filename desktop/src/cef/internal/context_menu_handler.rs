use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_context_menu_handler_t, cef_base_ref_counted_t};
use cef::{ImplContextMenuHandler, WrapContextMenuHandler};

pub(crate) struct ContextMenuHandlerImpl {
	object: *mut RcImpl<_cef_context_menu_handler_t, Self>,
}
impl ContextMenuHandlerImpl {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplContextMenuHandler for ContextMenuHandlerImpl {
	fn run_context_menu(
		&self,
		_browser: Option<&mut cef::Browser>,
		_frame: Option<&mut cef::Frame>,
		_params: Option<&mut cef::ContextMenuParams>,
		_model: Option<&mut cef::MenuModel>,
		_callback: Option<&mut cef::RunContextMenuCallback>,
	) -> std::ffi::c_int {
		// Prevent context menu
		1
	}

	fn run_quick_menu(
		&self,
		_browser: Option<&mut cef::Browser>,
		_frame: Option<&mut cef::Frame>,
		_location: Option<&cef::Point>,
		_size: Option<&cef::Size>,
		_edit_state_flags: cef::QuickMenuEditStateFlags,
		_callback: Option<&mut cef::RunQuickMenuCallback>,
	) -> std::ffi::c_int {
		// Prevent quick menu
		1
	}

	fn get_raw(&self) -> *mut _cef_context_menu_handler_t {
		self.object.cast()
	}
}

impl Clone for ContextMenuHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for ContextMenuHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapContextMenuHandler for ContextMenuHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_context_menu_handler_t, Self>) {
		self.object = object;
	}
}
