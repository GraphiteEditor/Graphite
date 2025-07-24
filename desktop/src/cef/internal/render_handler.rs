use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_render_handler_t, cef_base_ref_counted_t};
use cef::{Browser, ImplRenderHandler, PaintElementType, Rect, WrapRenderHandler};
use winit::event_loop::EventLoopProxy;

use crate::WinitEvent;
use crate::cef::WindowSizeHandle;
use crate::render::FrameBuffer;

pub(crate) struct RenderHandlerImpl {
	object: *mut RcImpl<_cef_render_handler_t, Self>,
	event_loop_proxy: EventLoopProxy<WinitEvent>,
	window_size: WindowSizeHandle,
}

impl RenderHandlerImpl {
	pub(crate) fn new(event_loop_proxy: EventLoopProxy<WinitEvent>, window_size: WindowSizeHandle) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_loop_proxy,
			window_size,
		}
	}
}
impl ImplRenderHandler for RenderHandlerImpl {
	fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
		if let Some(rect) = rect {
			let _ = self.window_size.with(|window_size| {
				*rect = Rect {
					x: 0,
					y: 0,
					width: window_size.as_ref().map(|w| w.width).unwrap_or(1) as i32,
					height: window_size.as_ref().map(|w| w.height).unwrap_or(1) as i32,
				};
			});
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
		let frame_buffer = FrameBuffer::new(buffer_slice.to_vec(), width as u32, height as u32).expect("Failed to create frame buffer");

		let _ = self.event_loop_proxy.send_event(WinitEvent::UIUpdate { frame_buffer });
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
			event_loop_proxy: self.event_loop_proxy.clone(),
			window_size: self.window_size.clone(),
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
