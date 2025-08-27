use cef::rc::{ConvertReturnValue, Rc, RcImpl};
use cef::sys::{_cef_render_process_handler_t, cef_base_ref_counted_t, cef_render_process_handler_t, cef_v8_propertyattribute_t, cef_v8_value_create_array_buffer_with_copy};
use cef::{CefString, ImplFrame, ImplRenderProcessHandler, ImplV8Context, ImplV8Value, V8Handler, V8Propertyattribute, V8Value, WrapRenderProcessHandler, v8_value_create_function};

use crate::cef::ipc::{MessageType, UnpackMessage, UnpackedMessage};

use super::render_process_v8_handler::BrowserProcessV8HandlerImpl;

pub(crate) struct RenderProcessHandlerImpl {
	object: *mut RcImpl<cef_render_process_handler_t, Self>,
}
impl RenderProcessHandlerImpl {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}
}

impl ImplRenderProcessHandler for RenderProcessHandlerImpl {
	fn on_process_message_received(
		&self,
		_browser: Option<&mut cef::Browser>,
		frame: Option<&mut cef::Frame>,
		_source_process: cef::ProcessId,
		message: Option<&mut cef::ProcessMessage>,
	) -> ::std::os::raw::c_int {
		let unpacked_message = unsafe { message.and_then(|m| m.unpack()) };
		match unpacked_message {
			Some(UnpackedMessage {
				message_type: MessageType::SendToJS,
				data,
			}) => {
				let Some(frame) = frame else {
					tracing::error!("Frame is not available");
					return 0;
				};
				let Some(context) = frame.v8_context() else {
					tracing::error!("V8 context is not available");
					return 0;
				};
				if context.enter() == 0 {
					tracing::error!("Failed to enter V8 context");
					return 0;
				}
				let mut value: V8Value = unsafe { cef_v8_value_create_array_buffer_with_copy(data.as_ptr() as *mut std::ffi::c_void, data.len()) }.wrap_result();
				let Some(global) = context.global() else {
					tracing::error!("Global object is not available in V8 context");
					return 0;
				};

				let function_name = "receiveNativeMessage";
				let property_name = "receiveNativeMessageData";

				let function_call = format!("window.{function_name}(window.{property_name})");

				global.set_value_bykey(
					Some(&CefString::from(property_name)),
					Some(&mut value),
					cef_v8_propertyattribute_t::V8_PROPERTY_ATTRIBUTE_READONLY.wrap_result(),
				);

				if global.value_bykey(Some(&CefString::from(function_name))).is_some() {
					frame.execute_java_script(Some(&CefString::from(function_call.as_str())), None, 0);
				}

				if context.exit() == 0 {
					tracing::error!("Failed to exit V8 context");
					return 0;
				}
			}
			_ => {
				tracing::error!("Unexpected message type received in render process");
				return 0;
			}
		}
		1
	}

	fn on_context_created(&self, _browser: Option<&mut cef::Browser>, _frame: Option<&mut cef::Frame>, context: Option<&mut cef::V8Context>) {
		let register_js_function = |context: &mut cef::V8Context, name: &'static str| {
			let mut v8_handler = V8Handler::new(BrowserProcessV8HandlerImpl::new());
			let Some(mut function) = v8_value_create_function(Some(&CefString::from(name)), Some(&mut v8_handler)) else {
				tracing::error!("Failed to create V8 function {name}");
				return;
			};

			let Some(global) = context.global() else {
				tracing::error!("Global object is not available in V8 context");
				return;
			};
			global.set_value_bykey(Some(&CefString::from(name)), Some(&mut function), V8Propertyattribute::default());
		};

		let Some(context) = context else {
			tracing::error!("V8 context is not available");
			return;
		};

		let initialized_function_name = "initializeNativeCommunication";
		let send_function_name = "sendNativeMessage";

		register_js_function(context, initialized_function_name);
		register_js_function(context, send_function_name);
	}

	fn get_raw(&self) -> *mut _cef_render_process_handler_t {
		self.object.cast()
	}
}

impl Clone for RenderProcessHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl Rc for RenderProcessHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapRenderProcessHandler for RenderProcessHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_render_process_handler_t, Self>) {
		self.object = object;
	}
}
