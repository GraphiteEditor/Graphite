use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_client_t, cef_base_ref_counted_t};
use cef::{ContextMenuHandler, DisplayHandler, ImplClient, LifeSpanHandler, LoadHandler, RenderHandler, RequestHandler, WrapClient};

use crate::delegate::BrowserDelegate;
use crate::frames::FrameStreamer;
use crate::ipc::{MessageType, UnpackMessage, UnpackedMessage};

use super::context_menu_handler::ContextMenuHandlerImpl;
use super::display_handler::DisplayHandlerImpl;
use super::life_span_handler::LifeSpanHandlerImpl;
use super::load_handler::LoadHandlerImpl;
use super::render_handler::RenderHandlerImpl;
use super::request_handler::RequestHandlerImpl;

pub(crate) struct BrowserProcessClientImpl {
	object: *mut RcImpl<_cef_client_t, Self>,
	delegate: BrowserDelegate,
	load_handler: LoadHandler,
	render_handler: RenderHandler,
	display_handler: DisplayHandler,
	request_handler: RequestHandler,
}
impl BrowserProcessClientImpl {
	pub(crate) fn new(delegate: &BrowserDelegate, frames: FrameStreamer) -> Self {
		Self {
			object: std::ptr::null_mut(),
			delegate: delegate.clone(),
			load_handler: LoadHandler::new(LoadHandlerImpl::new(delegate.clone())),
			render_handler: RenderHandler::new(RenderHandlerImpl::new(delegate.clone(), frames)),
			display_handler: DisplayHandler::new(DisplayHandlerImpl::new(delegate.clone())),
			request_handler: RequestHandler::new(RequestHandlerImpl::new()),
		}
	}
}

impl ImplClient for BrowserProcessClientImpl {
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
			}) => self.delegate.initialized_web_communication(),
			Some(UnpackedMessage {
				message_type: MessageType::SendToNative,
				data,
			}) => self.delegate.receive_web_message(data),

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

	fn request_handler(&self) -> Option<cef::RequestHandler> {
		Some(self.request_handler.clone())
	}

	fn context_menu_handler(&self) -> Option<cef::ContextMenuHandler> {
		Some(ContextMenuHandler::new(ContextMenuHandlerImpl::new()))
	}

	fn get_raw(&self) -> *mut _cef_client_t {
		self.object.cast()
	}
}

impl Clone for BrowserProcessClientImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			delegate: self.delegate.clone(),
			load_handler: self.load_handler.clone(),
			render_handler: self.render_handler.clone(),
			display_handler: self.display_handler.clone(),
			request_handler: self.request_handler.clone(),
		}
	}
}
impl Rc for BrowserProcessClientImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapClient for BrowserProcessClientImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_client_t, Self>) {
		self.object = object;
	}
}
