use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_render_handler_t, cef_base_ref_counted_t};
use cef::{Browser, ImplRenderHandler, PaintElementType, Rect, WrapRenderHandler};

use crate::cef::{CefEventHandler, View};
use crate::wrapper::WgpuContext;

pub(crate) struct RenderHandlerImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_render_handler_t, Self>,
	event_handler: H,
	view: View,
}
impl<H: CefEventHandler> RenderHandlerImpl<H> {
	pub(crate) fn new(event_handler: H, wgpu_context: WgpuContext) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler,
			view: View::new(wgpu_context),
		}
	}
}

impl<H: CefEventHandler> ImplRenderHandler for RenderHandlerImpl<H> {
	fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
		if let Some(rect) = rect {
			let view_info = self.event_handler.view_info();
			*rect = Rect {
				x: 0,
				y: 0,
				width: view_info.width() as i32,
				height: view_info.height() as i32,
			};
		}
	}

	fn on_paint(&self, _browser: Option<&mut Browser>, type_: PaintElementType, dirty_rects: Option<&[Rect]>, buffer: *const u8, width: std::ffi::c_int, height: std::ffi::c_int) {
		if type_ != PaintElementType::default() {
			return;
		}

		let buffer_size = (width * height * 4) as usize;
		let buffer_slice = unsafe { std::slice::from_raw_parts(buffer, buffer_size) };

		self.view.upload_frame_buffer(buffer_slice, width as u32, height as u32, dirty_rects.unwrap_or(&[]));
		self.event_handler.draw(&self.view)
	}

	#[cfg(feature = "accelerated_paint")]
	fn on_accelerated_paint(&self, _browser: Option<&mut Browser>, type_: PaintElementType, _dirty_rects: Option<&[Rect]>, info: Option<&cef::AcceleratedPaintInfo>) {
		if type_ != PaintElementType::default() {
			return;
		}

		self.view.import_shared_texture(info.unwrap());
		self.event_handler.draw(&self.view)
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
			event_handler: self.event_handler.duplicate(),
			view: self.view.clone(),
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
