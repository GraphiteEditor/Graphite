//! CEF (Chromium Embedded Framework) integration for Graphite Desktop
//!
//! This module provides CEF browser integration with hardware-accelerated texture sharing.
//!
//! # Hardware Acceleration
//!
//! The texture import system supports platform-specific hardware acceleration:
//!
//! - **Linux**: DMA-BUF via Vulkan external memory (`accelerated_paint_dmabuf` feature)
//! - **Windows**: D3D11 shared textures via either Vulkan or D3D12 interop (`accelerated_paint_d3d11` feature)
//! - **macOS**: IOSurface via Metal/Vulkan interop (`accelerated_paint_iosurface` feature)
//!
//! The system gracefully falls back to CPU textures when hardware acceleration is unavailable.

use crate::event::{AppEvent, AppEventScheduler};
use crate::render::FrameBufferRef;
use crate::wrapper::{WgpuContext, deserialize_editor_message};
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Instant;

mod consts;
mod context;
mod dirs;
mod input;
mod internal;
mod ipc;
mod platform;
mod utility;

#[cfg(feature = "accelerated_paint")]
mod texture_import;
#[cfg(feature = "accelerated_paint")]
use texture_import::SharedTextureHandle;

pub(crate) use context::{CefContext, CefContextBuilder, InitError};

pub(crate) trait CefEventHandler: Clone + Send + Sync + 'static {
	fn view_info(&self) -> ViewInfo;
	fn draw<'a>(&self, frame_buffer: FrameBufferRef<'a>);
	#[cfg(feature = "accelerated_paint")]
	fn draw_gpu(&self, shared_texture: SharedTextureHandle);
	fn load_resource(&self, path: PathBuf) -> Option<Resource>;
	fn cursor_change(&self, cursor: winit::cursor::Cursor);
	/// Schedule the main event loop to run the CEF event loop after the timeout.
	/// See [`_cef_browser_process_handler_t::on_schedule_message_pump_work`] for more documentation.
	fn schedule_cef_message_loop_work(&self, scheduled_time: Instant);
	fn initialized_web_communication(&self);
	fn receive_web_message(&self, message: &[u8]);
}

#[derive(Clone, Copy)]
pub(crate) struct ViewInfo {
	width: usize,
	height: usize,
	scale: f64,
}
impl ViewInfo {
	pub(crate) fn new() -> Self {
		Self { width: 1, height: 1, scale: 1.0 }
	}
	pub(crate) fn apply_update(&mut self, update: ViewInfoUpdate) {
		match update {
			ViewInfoUpdate::Size { width, height } if width > 0 && height > 0 => {
				self.width = width;
				self.height = height;
			}
			ViewInfoUpdate::Scale(scale) if scale > 0.0 => {
				self.scale = scale;
			}
			_ => {}
		}
	}
	pub(crate) fn scale(&self) -> f64 {
		self.scale
	}
	pub(crate) fn scaled_width(&self) -> usize {
		(self.width as f64 / self.scale).round() as usize
	}
	pub(crate) fn scaled_height(&self) -> usize {
		(self.height as f64 / self.scale).round() as usize
	}
}
impl Default for ViewInfo {
	fn default() -> Self {
		Self::new()
	}
}

pub(crate) enum ViewInfoUpdate {
	Size { width: usize, height: usize },
	Scale(f64),
}

#[derive(Clone)]
pub(crate) struct Resource {
	pub(crate) reader: ResourceReader,
	pub(crate) mimetype: Option<String>,
}

#[expect(dead_code)]
#[derive(Clone)]
pub(crate) enum ResourceReader {
	Embedded(Cursor<&'static [u8]>),
	File(Arc<File>),
}
impl Read for ResourceReader {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		match self {
			ResourceReader::Embedded(cursor) => cursor.read(buf),
			ResourceReader::File(file) => file.as_ref().read(buf),
		}
	}
}

#[derive(Clone)]
pub(crate) struct CefHandler {
	wgpu_context: WgpuContext,
	app_event_scheduler: AppEventScheduler,
	view_info_receiver: Arc<Mutex<ViewInfoReceiver>>,
}

impl CefHandler {
	pub(crate) fn new(wgpu_context: WgpuContext, app_event_scheduler: AppEventScheduler, view_info_receiver: Receiver<ViewInfoUpdate>) -> Self {
		Self {
			wgpu_context,
			app_event_scheduler,
			view_info_receiver: Arc::new(Mutex::new(ViewInfoReceiver::new(view_info_receiver))),
		}
	}
}

impl CefEventHandler for CefHandler {
	fn view_info(&self) -> ViewInfo {
		let Ok(mut guard) = self.view_info_receiver.lock() else {
			tracing::error!("Failed to lock view_info_receiver");
			return ViewInfo::new();
		};
		let ViewInfoReceiver { receiver, view_info } = &mut *guard;
		for update in receiver.try_iter() {
			view_info.apply_update(update);
		}
		*view_info
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

		self.app_event_scheduler.schedule(AppEvent::UiUpdate(texture));
	}

	#[cfg(feature = "accelerated_paint")]
	fn draw_gpu(&self, shared_texture: SharedTextureHandle) {
		match shared_texture.import_texture(&self.wgpu_context.device) {
			Ok(texture) => {
				self.app_event_scheduler.schedule(AppEvent::UiUpdate(texture));
			}
			Err(e) => {
				tracing::error!("Failed to import shared texture: {}", e);
			}
		}
	}

	fn load_resource(&self, path: PathBuf) -> Option<Resource> {
		let path = if path.as_os_str().is_empty() { PathBuf::from("index.html") } else { path };

		let mimetype = match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
			"html" => Some("text/html".to_string()),
			"css" => Some("text/css".to_string()),
			"txt" => Some("text/plain".to_string()),
			"wasm" => Some("application/wasm".to_string()),
			"js" => Some("application/javascript".to_string()),
			"png" => Some("image/png".to_string()),
			"jpg" | "jpeg" => Some("image/jpeg".to_string()),
			"svg" => Some("image/svg+xml".to_string()),
			"xml" => Some("application/xml".to_string()),
			"json" => Some("application/json".to_string()),
			"ico" => Some("image/x-icon".to_string()),
			"woff" => Some("font/woff".to_string()),
			"woff2" => Some("font/woff2".to_string()),
			"ttf" => Some("font/ttf".to_string()),
			"otf" => Some("font/otf".to_string()),
			"webmanifest" => Some("application/manifest+json".to_string()),
			"graphite" => Some("application/graphite+json".to_string()),
			_ => None,
		};

		#[cfg(feature = "embedded_resources")]
		{
			if let Some(resources) = &graphite_desktop_embedded_resources::EMBEDDED_RESOURCES
				&& let Some(file) = resources.get_file(&path)
			{
				return Some(Resource {
					reader: ResourceReader::Embedded(Cursor::new(file.contents())),
					mimetype,
				});
			}
		}

		#[cfg(not(feature = "embedded_resources"))]
		{
			use std::path::Path;
			let asset_path_env = std::env::var("GRAPHITE_RESOURCES").ok()?;
			let asset_path = Path::new(&asset_path_env);
			let file_path = asset_path.join(path.strip_prefix("/").unwrap_or(&path));
			if file_path.exists() && file_path.is_file() {
				if let Ok(file) = std::fs::File::open(file_path) {
					return Some(Resource {
						reader: ResourceReader::File(file.into()),
						mimetype,
					});
				}
			}
		}

		None
	}

	fn cursor_change(&self, cursor: winit::cursor::Cursor) {
		self.app_event_scheduler.schedule(AppEvent::CursorChange(cursor));
	}

	fn schedule_cef_message_loop_work(&self, scheduled_time: std::time::Instant) {
		self.app_event_scheduler.schedule(AppEvent::ScheduleBrowserWork(scheduled_time));
	}

	fn initialized_web_communication(&self) {
		self.app_event_scheduler.schedule(AppEvent::WebCommunicationInitialized);
	}

	fn receive_web_message(&self, message: &[u8]) {
		let Some(desktop_wrapper_message) = deserialize_editor_message(message) else {
			tracing::error!("Failed to deserialize web message");
			return;
		};
		self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(desktop_wrapper_message));
	}
}

struct ViewInfoReceiver {
	view_info: ViewInfo,
	receiver: Receiver<ViewInfoUpdate>,
}
impl ViewInfoReceiver {
	fn new(receiver: Receiver<ViewInfoUpdate>) -> Self {
		Self { view_info: ViewInfo::new(), receiver }
	}
}
