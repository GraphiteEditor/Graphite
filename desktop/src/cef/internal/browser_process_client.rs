use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_client_t, cef_base_ref_counted_t};
use cef::{ImplClient, RenderHandler, WrapClient};

use crate::cef::CefEventHandler;
use crate::cef::ipc::{MessageType, UnpackMessage, UnpackedMessage};

pub(crate) struct BrowserProcessClientImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_client_t, Self>,
	render_handler: RenderHandler,
	event_handler: H,
}
impl<H: CefEventHandler> BrowserProcessClientImpl<H> {
	pub(crate) fn new(render_handler: RenderHandler, event_handler: H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			render_handler,
			event_handler,
		}
	}
}

impl<H: CefEventHandler> ImplClient for BrowserProcessClientImpl<H> {
	fn on_process_message_received(
		&self,
		_browser: Option<&mut cef::Browser>,
		_frame: Option<&mut cef::Frame>,
		_source_process: cef::ProcessId,
		message: Option<&mut cef::ProcessMessage>,
	) -> ::std::os::raw::c_int {
		let unpacked_message = unsafe { message.and_then(|m| m.unpack()) };
		match unpacked_message {
			Some(UnpackedMessage {
				message_type: MessageType::SendToNative,
				data,
			}) => self.event_handler.receive_web_message(data),

			_ => {
				tracing::error!("Unexpected message type received in browser process");
				return 0;
			}
		}
		1
	}

	fn render_handler(&self) -> Option<RenderHandler> {
		Some(self.render_handler.clone())
	}

	fn get_raw(&self) -> *mut _cef_client_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler> Clone for BrowserProcessClientImpl<H> {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			render_handler: self.render_handler.clone(),
			event_handler: self.event_handler.clone(),
		}
	}
}
impl<H: CefEventHandler> Rc for BrowserProcessClientImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapClient for BrowserProcessClientImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_client_t, Self>) {
		self.object = object;
	}
}
