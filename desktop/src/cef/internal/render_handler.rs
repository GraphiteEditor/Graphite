use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_render_handler_t, cef_base_ref_counted_t};
use cef::{Browser, ImplRenderHandler, PaintElementType, Rect, WrapRenderHandler};

use crate::cef::CefEventHandler;
use crate::render::FrameBufferRef;

pub(crate) struct RenderHandlerImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_render_handler_t, Self>,
	event_handler: H,
}
impl<H: CefEventHandler> RenderHandlerImpl<H> {
	pub(crate) fn new(event_handler: H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler,
		}
	}
}
impl<H: CefEventHandler> ImplRenderHandler for RenderHandlerImpl<H> {
	fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
		if let Some(rect) = rect {
			let view = self.event_handler.window_size();
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
		_browser: Option<&mut Browser>,
		_type_: PaintElementType,
		_dirty_rect_count: usize,
		_dirty_rects: Option<&Rect>,
		buffer: *const u8,
		width: ::std::os::raw::c_int,
		height: ::std::os::raw::c_int,
	) {
		let buffer_size = (width * height * 4) as usize;
		let buffer_slice = unsafe { std::slice::from_raw_parts(buffer, buffer_size) };
		let frame_buffer = FrameBufferRef::new(buffer_slice, width as usize, height as usize).expect("Failed to create frame buffer");

		self.event_handler.draw(frame_buffer)
	}

	fn get_raw(&self) -> *mut _cef_render_handler_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler> Clone for RenderHandlerImpl<H> {
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
impl<H: CefEventHandler> Rc for RenderHandlerImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapRenderHandler for RenderHandlerImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_render_handler_t, Self>) {
		self.object = object;
	}
}
