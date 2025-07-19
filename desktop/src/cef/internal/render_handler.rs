use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_render_handler_t, cef_base_ref_counted_t};
use cef::{Browser, ImplBrowser, ImplBrowserHost, ImplRenderHandler, PaintElementType, Rect, RenderHandler, WrapRenderHandler};

use crate::cef::EventHandler;

pub(crate) struct RenderHandlerImpl<H: EventHandler> {
	object: *mut RcImpl<_cef_render_handler_t, Self>,
	event_handler: H,
}
impl<H: EventHandler> RenderHandlerImpl<H> {
	pub(crate) fn new(event_handler: H) -> RenderHandler {
		RenderHandler::new(Self {
			object: std::ptr::null_mut(),
			event_handler,
		})
	}
}
impl<H: EventHandler> ImplRenderHandler for RenderHandlerImpl<H> {
	fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
		if let Some(rect) = rect {
			let view = self.event_handler.view();
			*rect = Rect {
				x: 0,
				y: 0,
				width: view.width as i32,
				height: view.height as i32,
			};
		}
	}

	fn on_paint(
		&self,
		browser: Option<&mut Browser>,
		_type_: PaintElementType,
		_dirty_rect_count: usize,
		_dirty_rects: Option<&Rect>,
		buffer: *const u8,
		width: ::std::os::raw::c_int,
		height: ::std::os::raw::c_int,
	) {
		let buffer_size = (width * height * 4) as usize;
		let buffer_slice = unsafe { std::slice::from_raw_parts(buffer, buffer_size) };
		let draw_successful = self.event_handler.draw(buffer_slice.to_vec(), width as usize, height as usize);
		if !draw_successful {
			if let Some(browser) = browser {
				browser.host().unwrap().was_resized();
			}
		}
	}

	fn get_raw(&self) -> *mut _cef_render_handler_t {
		self.object.cast()
	}
}

impl<H: EventHandler> Clone for RenderHandlerImpl<H> {
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
impl<H: EventHandler> Rc for RenderHandlerImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: EventHandler> WrapRenderHandler for RenderHandlerImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_render_handler_t, Self>) {
		self.object = object;
	}
}
