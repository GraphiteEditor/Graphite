use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_client_t, cef_base_ref_counted_t};
use cef::{ContextMenuHandler, DisplayHandler, ImplClient, LifeSpanHandler, LoadHandler, RenderHandler, WrapClient};

use crate::cef::CefEventHandler;
use crate::cef::ipc::{MessageType, UnpackMessage, UnpackedMessage};

use super::context_menu_handler::ContextMenuHandlerImpl;
use super::display_handler::DisplayHandlerImpl;
use super::life_span_handler::LifeSpanHandlerImpl;
use super::load_handler::LoadHandlerImpl;
use super::render_handler::RenderHandlerImpl;

pub(crate) struct BrowserProcessClientImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_client_t, Self>,
	event_handler: H,
	load_handler: LoadHandler,
	render_handler: RenderHandler,
	display_handler: DisplayHandler,
}
impl<H: CefEventHandler> BrowserProcessClientImpl<H> {
	pub(crate) fn new(event_handler: &H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler: event_handler.duplicate(),
			load_handler: LoadHandler::new(LoadHandlerImpl::new(event_handler.duplicate())),
			render_handler: RenderHandler::new(RenderHandlerImpl::new(event_handler.duplicate())),
			display_handler: DisplayHandler::new(DisplayHandlerImpl::new(event_handler.duplicate())),
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
	) -> std::ffi::c_int {
		let unpacked_message = unsafe { message.and_then(|m| m.unpack()) };
		match unpacked_message {
			Some(UnpackedMessage {
				message_type: MessageType::Initialized,
				data: _,
			}) => self.event_handler.initialized_web_communication(),
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

	fn load_handler(&self) -> Option<cef::LoadHandler> {
		Some(self.load_handler.clone())
	}

	fn render_handler(&self) -> Option<RenderHandler> {
		Some(self.render_handler.clone())
	}

	fn life_span_handler(&self) -> Option<cef::LifeSpanHandler> {
		Some(LifeSpanHandler::new(LifeSpanHandlerImpl::new()))
	}

	fn display_handler(&self) -> Option<cef::DisplayHandler> {
		Some(self.display_handler.clone())
	}

	fn context_menu_handler(&self) -> Option<cef::ContextMenuHandler> {
		Some(ContextMenuHandler::new(ContextMenuHandlerImpl::new()))
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
			event_handler: self.event_handler.duplicate(),
			load_handler: self.load_handler.clone(),
			render_handler: self.render_handler.clone(),
			display_handler: self.display_handler.clone(),
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
