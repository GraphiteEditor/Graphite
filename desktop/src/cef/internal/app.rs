use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_app_t, cef_base_ref_counted_t};
use cef::{App, BrowserProcessHandler, Frame, ImplApp, SchemeRegistrar, WrapApp};

use crate::cef::scheme_handler::GraphiteSchemeHandlerFactory;
use crate::cef::EventHandler;
use crate::render::{FrameBuffer, FrameBufferHandle};

use super::browser_process_handler::OffscreenBrowserProcessHandler;

struct OffscreenApp {
	object: *mut RcImpl<cef_dll_sys::_cef_app_t, Self>,
	frame_buffer: Arc<Mutex<FrameBuffer>>,
}

impl OffscreenApp {
	fn new(frame_buffer: Arc<Mutex<FrameBuffer>>) -> App {
		App::new(Self {
			object: std::ptr::null_mut(),
			frame_buffer,
		})
	}
}

impl ImplApp for OffscreenApp {
	fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
		println!("browser_process_handler");
		Some(OffscreenBrowserProcessHandler::new(self.frame_buffer.clone()))
	}

	fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
		println!("on_register_custom_schemes");
		GraphiteSchemeHandlerFactory::register_schemes(registrar);
	}

	fn get_raw(&self) -> *mut _cef_app_t {
		self.object.cast()
	}
}

impl<H: EventHandler> Clone for AppImpl<H> {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			event_handler: self.event_handler.clone(),
		}
	}
}
impl<H: EventHandler> Rc for AppImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: EventHandler> WrapApp for AppImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_app_t, Self>) {
		self.object = object;
	}
}
