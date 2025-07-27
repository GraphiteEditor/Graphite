use cef::{CefString, ImplFrame, ImplV8Context, ImplV8Handler, ImplV8Value, V8Value, WrapV8Handler, process_message_create, rc::Rc, sys::cef_process_id_t, v8_context_get_current_context};
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
	fn execute(
		&self,
		name: Option<&cef::CefString>,
		_object: Option<&mut V8Value>,
		arguments: Option<&[Option<V8Value>]>,
		_retval: Option<&mut Option<V8Value>>,
		_exception: Option<&mut cef::CefString>,
	) -> ::std::os::raw::c_int {
		if let Some(name) = name {
			if name.to_string() == "sendMessageToCef" {
				let string = arguments.unwrap().first().unwrap().as_ref().unwrap().string_value();

				let pointer: *mut cef::sys::_cef_string_utf16_t = string.into();
				let message = unsafe { super::utility::pointer_to_string(pointer) };

				let Some(mut process_message) = process_message_create(Some(&CefString::from(message.as_str()))) else {
					tracing::event!(tracing::Level::ERROR, "Failed to create process message");
					return 0;
				};

				let Some(frame) = v8_context_get_current_context().and_then(|context| context.frame()) else {
					tracing::event!(tracing::Level::ERROR, "No current V8 context in V8HandlerImpl::execute");
					return 0;
				};
				frame.send_process_message(cef_process_id_t::PID_BROWSER.into(), Some(&mut process_message));
			}
			if name.to_string() == "readMessageData" {
				let Ok(data) = self.receiver.lock().as_mut().unwrap().recv() else { return 0 };

				let arg1 = arguments.unwrap().first().unwrap().as_ref().unwrap();
				let size = arg1.array_buffer_byte_length();
				let ptr = arg1.array_buffer_data();

				let js_buffer = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, size) };
				js_buffer.copy_from_slice(&data);
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
