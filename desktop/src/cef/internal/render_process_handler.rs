use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_render_process_handler_t, cef_base_ref_counted_t};
use cef::{ImplBinaryValue, ImplListValue, ImplProcessMessage, ImplRenderProcessHandler, V8Handler, WrapRenderProcessHandler};

use super::render_process_v8_handler::BrowserProcessV8HandlerImpl;
use super::utility::V8ContextExt;

pub(crate) struct RenderProcessHandlerImpl {
	object: *mut RcImpl<_cef_render_process_handler_t, Self>,
	sender: Sender<Vec<u8>>,
	receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
}
impl RenderProcessHandlerImpl {
	pub(crate) fn new() -> Self {
		let (sender, receiver) = std::sync::mpsc::channel();
		Self {
			object: std::ptr::null_mut(),
			sender,
			receiver: Arc::new(receiver.into()),
		}
	}
}

impl ImplRenderProcessHandler for RenderProcessHandlerImpl {
	fn on_process_message_received(
		&self,
		_browser: Option<&mut cef::Browser>,
		_frame: Option<&mut cef::Frame>,
		_source_process: cef::ProcessId,
		message: Option<&mut cef::ProcessMessage>,
	) -> ::std::os::raw::c_int {
		let Some(message) = message else {
			tracing::error!("No message in RenderProcessHandlerImpl::on_process_message_received");
			return 1;
		};

		let message_name = cef::CefString::from(&message.name()).to_string();

		if message_name == "editorResponseToJs" {
			let Some(arglist) = message.argument_list() else { return 0 };
			let Some(binary) = arglist.binary(0) else { return 0 };
			let size = binary.size();
			let ptr = binary.raw_data();
			let buffer = unsafe { std::slice::from_raw_parts(ptr as *const u8, size) };
			let _ = self.sender.send(buffer.to_vec());
		}

		1
	}

	fn on_context_created(&self, _browser: Option<&mut cef::Browser>, _frame: Option<&mut cef::Frame>, context: Option<&mut cef::V8Context>) {
		let Some(context) = context else {
			tracing::error!("No browser in RenderProcessHandlerImpl::on_context_created");
			return;
		};
		let mut v8_handler = V8Handler::new(BrowserProcessV8HandlerImpl::new(self.receiver.clone()));

		context.register_global_function("editorResponseToJs", &mut v8_handler);
		context.register_global_function("sendMessageToCef", &mut v8_handler);
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
		Self {
			object: self.object,
			sender: self.sender.clone(),
			receiver: self.receiver.clone(),
		}
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
