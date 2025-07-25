use crate::{CustomEvent, WgpuContext, render::FrameBufferRef};
use std::{
	sync::{Arc, Mutex, mpsc::Receiver},
	thread,
	time::Instant,
};

mod context;
mod dirs;
mod input;
mod internal;
mod scheme_handler;

pub(crate) use context::{Context, InitError, Initialized, Setup, SetupError};
use winit::event_loop::EventLoopProxy;

pub(crate) trait CefEventHandler: Clone {
	fn window_size(&self) -> WindowSize;
	fn draw<'a>(&self, frame_buffer: FrameBufferRef<'a>);
	fn file_dialog(&self, mode: FileDialogMode, title: &str, default_file: &str) -> Receiver<Option<Vec<String>>>;
	/// Scheudule the main event loop to run the cef event loop after the timeout
	///  [`_cef_browser_process_handler_t::on_schedule_message_pump_work`] for more documentation.
	fn schedule_cef_message_loop_work(&self, scheduled_time: Instant);
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

pub(crate) enum FileDialogMode {
	Open,
	OpenMultiple,
	OpenFolder,
	Save,
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

	fn file_dialog(&self, mode: FileDialogMode, title: &str, default_file: &str) -> Receiver<Option<Vec<String>>> {
		let title = title.to_owned();
		let default_file = default_file.to_owned();
		let (sender, receiver) = std::sync::mpsc::channel();
		dbg!("Opening file dialog with title: {}, default file: {}", &title, &default_file);
		let _ = thread::spawn(move || {
			let builder = native_dialog::FileDialogBuilder::default().set_title(&title).set_location(&default_file);
			match mode {
				FileDialogMode::OpenMultiple => match builder.open_multiple_file().show() {
					Ok(paths) => {
						let selected_files = paths.into_iter().map(|p| p.to_string_lossy().to_string()).collect();
						sender.send(Some(selected_files)).unwrap_or_else(|_| tracing::error!("Failed to send selected files"));
					}
					Err(e) => {
						tracing::error!("File dialog error: {}", e);
						sender.send(None).unwrap_or_else(|_| tracing::error!("Failed to send None on error"));
					}
				},
				_ => {
					let res = match mode {
						FileDialogMode::Open => builder.open_single_file().show(),
						FileDialogMode::OpenFolder => builder.open_single_dir().show(),
						FileDialogMode::Save => builder.save_single_file().show(),
						FileDialogMode::OpenMultiple => unreachable!("OpenMultiple is handled above"),
					};
					match res {
						Ok(Some(path)) => {
							let selected_files = vec![path.to_string_lossy().to_string()];
							sender.send(Some(selected_files)).unwrap_or_else(|_| tracing::error!("Failed to send selected files"));
						}
						Ok(None) => sender.send(None).unwrap_or_else(|_| tracing::error!("Failed to send None")),
						Err(e) => {
							tracing::error!("File dialog error: {}", e);
							sender.send(None).unwrap_or_else(|_| tracing::error!("Failed to send None on error"));
						}
					}
				}
			}
		});
		receiver
	}
}
