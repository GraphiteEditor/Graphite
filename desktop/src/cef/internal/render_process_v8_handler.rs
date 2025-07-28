use cef::{ImplV8Handler, ImplV8Value, V8Value, WrapV8Handler, rc::Rc, v8_context_get_current_context};

use crate::cef::ipc::{MessageType, SendMessage};

pub struct BrowserProcessV8HandlerImpl {
	object: *mut cef::rc::RcImpl<cef::sys::_cef_v8_handler_t, Self>,
}

impl BrowserProcessV8HandlerImpl {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplV8Handler for BrowserProcessV8HandlerImpl {
	fn execute(
		&self,
		name: Option<&cef::CefString>,
		_object: Option<&mut V8Value>,
		arguments: Option<&[Option<V8Value>]>,
		_retval: Option<&mut Option<V8Value>>,
		_exception: Option<&mut cef::CefString>,
	) -> ::std::os::raw::c_int {
		if let Some(name) = name {
			if name.to_string() == "sendNativeMessage" {
				let Some(args) = arguments else {
					tracing::error!("No arguments provided to sendNativeMessage");
					return 0;
				};
				let Some(arg1) = args.first() else {
					tracing::error!("No arguments provided to sendNativeMessage");
					return 0;
				};
				let Some(arg1) = arg1.as_ref() else {
					tracing::error!("First argument to sendNativeMessage is not an ArrayBuffer");
					return 0;
				};
				if arg1.is_array_buffer() == 0 {
					tracing::error!("First argument to sendNativeMessage is not an ArrayBuffer");
					return 0;
				}

				let size = arg1.array_buffer_byte_length();
				let ptr = arg1.array_buffer_data();
				let data = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, size) };

				v8_context_get_current_context().send_message(MessageType::SendToNative, data);

				return 1;
			}
		}
		1
	}

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
		Self { object: self.object }
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
