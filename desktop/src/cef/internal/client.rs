use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_client_t, cef_base_ref_counted_t};
use cef::{ImplClient, ImplProcessMessage, RenderHandler, WrapClient};

use crate::cef::CefEventHandler;

pub(crate) struct ClientImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_client_t, Self>,
	render_handler: RenderHandler,
	event_handler: H,
}
impl<H: CefEventHandler> ClientImpl<H> {
	pub(crate) fn new(render_handler: RenderHandler, event_handler: H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			render_handler,
			event_handler,
		}
	}
}

impl<H: CefEventHandler> ImplClient for ClientImpl<H> {
	fn render_handler(&self) -> Option<RenderHandler> {
		Some(self.render_handler.clone())
	}

	fn get_raw(&self) -> *mut _cef_client_t {
		self.object.cast()
	}

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

		let pointer: *mut cef::sys::_cef_string_utf16_t = message.name().into();
		let string_message = super::utility::pointer_to_string(pointer);
		let _ = self.event_handler.send_message_to_editor(string_message);
		0
	}
}

impl<H: CefEventHandler> Clone for ClientImpl<H> {
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
impl<H: CefEventHandler> Rc for ClientImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapClient for ClientImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_client_t, Self>) {
		self.object = object;
	}
}
