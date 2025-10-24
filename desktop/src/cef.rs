use crate::{CustomEvent, WgpuContext, render::FrameBufferRef};
use std::{
	sync::{Arc, Mutex, mpsc::Receiver},
	time::Instant,
};

mod context;
mod dirs;
mod input;
mod internal;
mod ipc;
mod scheme_handler;
mod utility;

pub(crate) use context::{Context, InitError, Initialized, Setup, SetupError};
use winit::event_loop::EventLoopProxy;

pub(crate) trait CefEventHandler: Clone {
	fn window_size(&self) -> WindowSize;
	fn draw<'a>(&self, frame_buffer: FrameBufferRef<'a>);
	/// Scheudule the main event loop to run the cef event loop after the timeout
	///  [`_cef_browser_process_handler_t::on_schedule_message_pump_work`] for more documentation.
	fn schedule_cef_message_loop_work(&self, scheduled_time: Instant);
	fn receive_web_message(&self, message: &[u8]);
}

#[derive(Clone, Copy)]
pub(crate) struct WindowSize {
	pub(crate) width: usize,
	pub(crate) height: usize,
}

impl WindowSize {
	pub(crate) fn new(width: usize, height: usize) -> Self {
		Self { width, height }
	}
}

#[derive(Clone)]
pub(crate) struct CefHandler {
	window_size_receiver: Arc<Mutex<WindowSizeReceiver>>,
	event_loop_proxy: EventLoopProxy<CustomEvent>,
	wgpu_context: WgpuContext,
}
struct WindowSizeReceiver {
	receiver: Receiver<WindowSize>,
	window_size: WindowSize,
}
impl WindowSizeReceiver {
	fn new(window_size_receiver: Receiver<WindowSize>) -> Self {
		Self {
			window_size: WindowSize { width: 1, height: 1 },
			receiver: window_size_receiver,
		}
	}
}
impl CefHandler {
	pub(crate) fn new(window_size_receiver: Receiver<WindowSize>, event_loop_proxy: EventLoopProxy<CustomEvent>, wgpu_context: WgpuContext) -> Self {
		Self {
			window_size_receiver: Arc::new(Mutex::new(WindowSizeReceiver::new(window_size_receiver))),
			event_loop_proxy,
			wgpu_context,
		}
	}
}

impl CefEventHandler for CefHandler {
	fn window_size(&self) -> WindowSize {
		let Ok(mut guard) = self.window_size_receiver.lock() else {
			tracing::error!("Failed to lock window_size_receiver");
			return WindowSize::new(1, 1);
		};
		let WindowSizeReceiver { receiver, window_size } = &mut *guard;
		for new_window_size in receiver.try_iter() {
			*window_size = new_window_size;
		}
		*window_size
	}
	fn draw<'a>(&self, frame_buffer: FrameBufferRef<'a>) {
		let width = frame_buffer.width() as u32;
		let height = frame_buffer.height() as u32;
		let texture = self.wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("CEF Texture"),
			size: wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Bgra8UnormSrgb,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});
		self.wgpu_context.queue.write_texture(
			wgpu::TexelCopyTextureInfo {
				texture: &texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			frame_buffer.buffer(),
			wgpu::TexelCopyBufferLayout {
				offset: 0,
				bytes_per_row: Some(4 * width),
				rows_per_image: Some(height),
			},
			wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
		);

		let _ = self.event_loop_proxy.send_event(CustomEvent::UiUpdate(texture));
	}

	fn schedule_cef_message_loop_work(&self, scheduled_time: std::time::Instant) {
		let _ = self.event_loop_proxy.send_event(CustomEvent::ScheduleBrowserWork(scheduled_time));
	}

	fn receive_web_message(&self, message: &[u8]) {
		let str = std::str::from_utf8(message).unwrap();
		match ron::from_str(str) {
			Ok(message) => {
				let _ = self.event_loop_proxy.send_event(CustomEvent::MessageReceived(message));
			}
			Err(e) => {
				tracing::error!("Failed to deserialize message {:?}", e)
			}
		}
	}
}
