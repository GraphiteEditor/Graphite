use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_render_handler_t, cef_base_ref_counted_t};
use cef::{Browser, ImplRenderHandler, PaintElementType, Rect, WrapRenderHandler};

use crate::cef::CefEventHandler;
use crate::render::FrameBufferRef;

#[cfg(target_os = "linux")]
use std::os::fd::RawFd;
#[cfg(all(feature = "accelerated_paint", any(target_os = "windows", target_os = "macos")))]
use std::os::raw::c_void;

pub(crate) struct RenderHandlerImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_render_handler_t, Self>,
	event_handler: H,
}

#[cfg(feature = "accelerated_paint")]
pub enum SharedTextureHandle {
	#[cfg(target_os = "windows")]
	D3D11 {
		handle: *mut c_void,
		format: cef::sys::cef_color_type_t,
		width: u32,
		height: u32,
	},
	#[cfg(target_os = "macos")]
	IOSurface(*mut c_void),
	#[cfg(target_os = "linux")]
	DmaBuf {
		fds: Vec<RawFd>,
		format: cef::sys::cef_color_type_t,
		modifier: u64,
		width: u32,
		height: u32,
		strides: Vec<u32>,
		offsets: Vec<u32>,
	},
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

	#[cfg(feature = "accelerated_paint")]
	fn on_accelerated_paint(&self, _browser: Option<&mut Browser>, type_: PaintElementType, _dirty_rect_count: usize, _dirty_rects: Option<&Rect>, info: Option<&cef::AcceleratedPaintInfo>) {
		if type_ != PaintElementType::default() {
			return;
		}
		let info = info.unwrap();

		#[cfg(target_os = "linux")]
		{
			// Extract DMA-BUF information
			let shared_handle = SharedTextureHandle::DmaBuf {
				fds: extract_fds_from_info(info),
				format: *info.format.as_ref(),
				modifier: info.modifier,
				width: info.extra.coded_size.width as u32,
				height: info.extra.coded_size.height as u32,
				strides: extract_strides_from_info(info),
				offsets: extract_offsets_from_info(info),
			};

			self.event_handler.on_accelerated_paint(shared_handle);
		}

		#[cfg(target_os = "windows")]
		{
			// Extract D3D11 shared handle with texture metadata
			let shared_handle = SharedTextureHandle::D3D11 {
				handle: info.shared_texture_handle,
				format: *info.format.as_ref(),
				width: info.extra.coded_size.width as u32,
				height: info.extra.coded_size.height as u32,
			};
			self.event_handler.on_accelerated_paint(shared_handle);
		}

		#[cfg(target_os = "macos")]
		{
			// Extract IOSurface handle
			let shared_handle = SharedTextureHandle::IOSurface(info.shared_texture_handle);
			self.event_handler.on_accelerated_paint(shared_handle);
		}
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

#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
fn extract_fds_from_info(info: &cef::AcceleratedPaintInfo) -> Vec<RawFd> {
	let plane_count = info.plane_count as usize;
	let mut fds = Vec::with_capacity(plane_count);

	for i in 0..plane_count {
		if let Some(plane) = info.planes.get(i) {
			fds.push(plane.fd);
		}
	}

	fds
}

#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
fn extract_strides_from_info(info: &cef::AcceleratedPaintInfo) -> Vec<u32> {
	let plane_count = info.plane_count as usize;
	let mut strides = Vec::with_capacity(plane_count);

	for i in 0..plane_count {
		if let Some(plane) = info.planes.get(i) {
			strides.push(plane.stride);
		}
	}

	strides
}

#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
fn extract_offsets_from_info(info: &cef::AcceleratedPaintInfo) -> Vec<u32> {
	let plane_count = info.plane_count as usize;
	let mut offsets = Vec::with_capacity(plane_count);

	for i in 0..plane_count {
		if let Some(plane) = info.planes.get(i) {
			offsets.push(plane.offset as u32);
		}
	}

	offsets
}
