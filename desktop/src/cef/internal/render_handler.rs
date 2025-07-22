use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_render_handler_t, cef_base_ref_counted_t};
use cef::{Browser, ImplRenderHandler, PaintElementType, Rect, RenderHandler, WrapRenderHandler};

use crate::render::FrameBufferHandle;

// CEF render handler for offscreen rendering
pub struct OffscreenRenderHandler {
	object: *mut RcImpl<_cef_render_handler_t, Self>,
	frame_buffer: FrameBufferHandle,
}

impl OffscreenRenderHandler {
	pub(crate) fn new(frame_buffer: FrameBufferHandle) -> RenderHandler {
		RenderHandler::new(Self {
			object: std::ptr::null_mut(),
			frame_buffer,
		})
	}
}
impl ImplRenderHandler for OffscreenRenderHandler {
	fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
		let frame_buffer = self.frame_buffer.inner.lock().unwrap();
		let width = frame_buffer.width() as i32;
		let height = frame_buffer.height() as i32;
		if let Some(rect) = rect {
			*rect = Rect { x: 0, y: 0, width, height };
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
		self.frame_buffer.inner.lock().unwrap().add_buffer(buffer_slice, width, height);
	}

	fn get_raw(&self) -> *mut _cef_render_handler_t {
		self.object.cast()
	}
}

impl WrapRenderHandler for OffscreenRenderHandler {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_render_handler_t, Self>) {
		self.object = object;
	}
}

impl Clone for OffscreenRenderHandler {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			frame_buffer: self.frame_buffer.clone(),
		}
	}
}

impl Rc for OffscreenRenderHandler {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
