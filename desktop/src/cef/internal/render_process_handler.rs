use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_render_process_handler_t, cef_base_ref_counted_t, cef_render_process_handler_t};
use cef::{ImplRenderProcessHandler, WrapRenderProcessHandler};

use crate::cef::ipc::{MessageType, UnpackMessage, UnpackedMessage};

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
		_frame: Option<&mut cef::Frame>,
		_source_process: cef::ProcessId,
		message: Option<&mut cef::ProcessMessage>,
	) -> ::std::os::raw::c_int {
		match message.unpack() {
			Some(UnpackedMessage {
				message_type: MessageType::SendToJS,
				data,
			}) => {}
			_ => {
				tracing::error!("Unexpected message type received in render process");
				return 0;
			}
		}
		1
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
