use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_render_handler_t, cef_base_ref_counted_t};
use cef::{Browser, ImplRenderHandler, PaintElementType, Rect, WrapRenderHandler};

use crate::delegate::BrowserDelegate;
use crate::frames::FrameStreamer;

pub(crate) struct RenderHandlerImpl {
	object: *mut RcImpl<_cef_render_handler_t, Self>,
	delegate: BrowserDelegate,
	frames: FrameStreamer,
}
impl RenderHandlerImpl {
	pub(crate) fn new(delegate: BrowserDelegate, frames: FrameStreamer) -> Self {
		Self {
			object: std::ptr::null_mut(),
			delegate,
			frames,
		}
	}
}

impl ImplRenderHandler for RenderHandlerImpl {
	fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
		if let Some(rect) = rect {
			let view_info = self.delegate.view_info();
			*rect = Rect {
				x: 0,
				y: 0,
				width: view_info.width() as i32,
				height: view_info.height() as i32,
			};
		}
	}

	fn on_paint(&self, _browser: Option<&mut Browser>, type_: PaintElementType, _dirty_rects: Option<&[Rect]>, buffer: *const u8, width: std::ffi::c_int, height: std::ffi::c_int) {
		if type_ != PaintElementType::default() {
			return;
		}

		let buffer_size = (width * height * 4) as usize;
		let buffer_slice = unsafe { std::slice::from_raw_parts(buffer, buffer_size) };

		self.frames.stage_buffer(buffer_slice, width as u32, height as u32);
		self.frames.publish();
	}

	#[cfg(feature = "accelerated_paint")]
	fn on_accelerated_paint(&self, _browser: Option<&mut Browser>, type_: PaintElementType, _dirty_rects: Option<&[Rect]>, info: Option<&cef::AcceleratedPaintInfo>) {
		if type_ != PaintElementType::default() {
			return;
		}

		let Some(info) = info else {
			tracing::error!("Accelerated paint callback received no info about the painted frame");
			return;
		};
		self.frames.stage_texture(info);
		self.frames.publish();
	}

	fn get_raw(&self) -> *mut _cef_render_handler_t {
		self.object.cast()
	}
}

impl Clone for RenderHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			delegate: self.delegate.clone(),
			frames: self.frames.clone(),
		}
	}
}
impl Rc for RenderHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapRenderHandler for RenderHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_render_handler_t, Self>) {
		self.object = object;
	}
}
