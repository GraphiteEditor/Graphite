use crate::CustomEvent;
use crate::WindowSize;
use crate::consts::APP_NAME;
use crate::desktop_wrapper::EditorWrapper;
use crate::desktop_wrapper::WgpuContext;
use crate::desktop_wrapper::messages::DesktopFrontendMessage;
use crate::desktop_wrapper::messages::DesktopWrapperMessage;
use crate::render::GraphicsState;
use rfd::AsyncFileDialog;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::StartCause;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;
use winit::window::WindowId;

use crate::cef;

pub(crate) struct WinitApp {
	cef_context: cef::Context<cef::Initialized>,
	window: Option<Arc<Window>>,
	cef_schedule: Option<Instant>,
	window_size_sender: Sender<WindowSize>,
	graphics_state: Option<GraphicsState>,
	wgpu_context: WgpuContext,
	event_loop_proxy: EventLoopProxy<CustomEvent>,
	editor_wrapper: EditorWrapper,
}

impl WinitApp {
	pub(crate) fn new(cef_context: cef::Context<cef::Initialized>, window_size_sender: Sender<WindowSize>, wgpu_context: WgpuContext, event_loop_proxy: EventLoopProxy<CustomEvent>) -> Self {
		let editor_wrapper = EditorWrapper::new();
		Self {
			cef_context,
			window: None,
			cef_schedule: Some(Instant::now()),
			graphics_state: None,
			window_size_sender,
			wgpu_context,
			event_loop_proxy,
			editor_wrapper,
		}
	}

	fn handle_desktop_frontend_message(&mut self, message: DesktopFrontendMessage) {
		match message {
			DesktopFrontendMessage::ToWeb(bytes) => {
				self.cef_context.send_web_message(bytes.as_slice());
			}
			DesktopFrontendMessage::OpenFileDialog { title, filters, context } => {
				let event_loop_proxy = self.event_loop_proxy.clone();
				let _ = thread::spawn(move || {
					let mut dialog = AsyncFileDialog::new().set_title(title);
					for filter in filters {
						dialog = dialog.add_filter(filter.name, &filter.extensions);
					}

					let show_dialog = async move { dialog.pick_file().await.map(|f| f.path().to_path_buf()) };

					if let Some(path) = futures::executor::block_on(show_dialog)
						&& let Ok(content) = std::fs::read(&path)
					{
						let message = DesktopWrapperMessage::OpenFileDialogResult { path, content, context };
						let _ = event_loop_proxy.send_event(CustomEvent::DesktopWrapperMessage(message));
					}
				});
			}
			DesktopFrontendMessage::SaveFileDialog {
				title,
				default_filename,
				default_folder,
				filters,
				context,
			} => {
				let event_loop_proxy = self.event_loop_proxy.clone();
				let _ = thread::spawn(move || {
					let mut dialog = AsyncFileDialog::new().set_title(title).set_file_name(default_filename);
					if let Some(folder) = default_folder {
						dialog = dialog.set_directory(folder);
					}
					for filter in filters {
						dialog = dialog.add_filter(filter.name, &filter.extensions);
					}

					let show_dialog = async move { dialog.save_file().await.map(|f| f.path().to_path_buf()) };

					if let Some(path) = futures::executor::block_on(show_dialog) {
						let message = DesktopWrapperMessage::SaveFileDialogResult { path, context };
						let _ = event_loop_proxy.send_event(CustomEvent::DesktopWrapperMessage(message));
					}
				});
			}
			DesktopFrontendMessage::WriteFile { path, content } => {
				if let Err(e) = std::fs::write(&path, content) {
					tracing::error!("Failed to write file {}: {}", path.display(), e);
				}
			}
			DesktopFrontendMessage::OpenUrl(url) => {
				let _ = thread::spawn(move || {
					if let Err(e) = open::that(&url) {
						tracing::error!("Failed to open URL: {}: {}", url, e);
					}
				});
			}
			DesktopFrontendMessage::RequestRedraw => {
				if let Some(window) = &self.window {
					window.request_redraw();
				}
			}
			DesktopFrontendMessage::UpdateViewport(texture) => {
				if let Some(graphics_state) = &mut self.graphics_state {
					graphics_state.bind_viewport_texture(texture);
				}
			}
			DesktopFrontendMessage::UpdateViewportBounds { x, y, width, height } => {
				if let Some(graphics_state) = &mut self.graphics_state
					&& let Some(window) = &self.window
				{
					let window_size = window.inner_size();

					let viewport_offset_x = x / window_size.width as f32;
					let viewport_offset_y = y / window_size.height as f32;
					graphics_state.set_viewport_offset([viewport_offset_x, viewport_offset_y]);

					let viewport_scale_x = if width != 0.0 { window_size.width as f32 / width } else { 1.0 };
					let viewport_scale_y = if height != 0.0 { window_size.height as f32 / height } else { 1.0 };
					graphics_state.set_viewport_scale([viewport_scale_x, viewport_scale_y]);
				}
			}
			DesktopFrontendMessage::UpdateOverlays(scene) => {
				if let Some(graphics_state) = &mut self.graphics_state {
					graphics_state.set_overlays_scene(scene);
				}
			}
			DesktopFrontendMessage::Loopback(editor_message) => self.dispatch_desktop_wrapper_message(editor_message),
		}
	}

	fn handle_desktop_frontend_messages(&mut self, messages: Vec<DesktopFrontendMessage>) {
		for message in messages {
			self.handle_desktop_frontend_message(message);
		}
	}

	fn dispatch_desktop_wrapper_message(&mut self, message: DesktopWrapperMessage) {
		let responses = self.editor_wrapper.dispatch(message);
		self.handle_desktop_frontend_messages(responses);
	}
}

impl ApplicationHandler<CustomEvent> for WinitApp {
	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		// Set a timeout in case we miss any cef schedule requests
		let timeout = Instant::now() + Duration::from_millis(10);
		let wait_until = timeout.min(self.cef_schedule.unwrap_or(timeout));
		self.cef_context.work();

		event_loop.set_control_flow(ControlFlow::WaitUntil(wait_until));
	}

	fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
		if let Some(schedule) = self.cef_schedule
			&& schedule < Instant::now()
		{
			self.cef_schedule = None;
			self.cef_context.work();
		}
		if let StartCause::ResumeTimeReached { .. } = cause {
			if let Some(window) = &self.window {
				window.request_redraw();
			}
		}
	}

	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let mut window = Window::default_attributes()
			.with_title(APP_NAME)
			.with_min_inner_size(winit::dpi::LogicalSize::new(400, 300))
			.with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

		#[cfg(target_family = "unix")]
		{
			use crate::consts::APP_ID;
			use winit::platform::wayland::ActiveEventLoopExtWayland;

			window = if event_loop.is_wayland() {
				winit::platform::wayland::WindowAttributesExtWayland::with_name(window, APP_ID, "")
			} else {
				winit::platform::x11::WindowAttributesExtX11::with_name(window, APP_ID, APP_NAME)
			}
		}

		let window = Arc::new(event_loop.create_window(window).unwrap());
		let graphics_state = GraphicsState::new(window.clone(), self.wgpu_context.clone());

		self.window = Some(window);
		self.graphics_state = Some(graphics_state);

		tracing::info!("Winit window created and ready");

		self.editor_wrapper.init(self.wgpu_context.clone());
	}

	fn user_event(&mut self, _: &ActiveEventLoop, event: CustomEvent) {
		match event {
			CustomEvent::DesktopWrapperMessage(message) => self.dispatch_desktop_wrapper_message(message),
			CustomEvent::DesktopFrontendMessages(messages) => self.handle_desktop_frontend_messages(messages),
			CustomEvent::UiUpdate(texture) => {
				if let Some(graphics_state) = self.graphics_state.as_mut() {
					graphics_state.resize(texture.width(), texture.height());
					graphics_state.bind_ui_texture(texture);
				}
				if let Some(window) = &self.window {
					window.request_redraw();
				}
			}
			CustomEvent::ScheduleBrowserWork(instant) => {
				if instant <= Instant::now() {
					self.cef_context.work();
				} else {
					self.cef_schedule = Some(instant);
				}
			}
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		let Some(event) = self.cef_context.handle_window_event(event) else { return };

		match event {
			// Currently not supported on wayland see https://github.com/rust-windowing/winit/issues/1881
			// WindowEvent::DroppedFile(path) => {
			// 	let name = path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string());
			// 	let Some(extension) = path.extension().and_then(|s| s.to_str()) else {
			// 		tracing::warn!("Unsupported file dropped: {}", path.display());
			// 		// Fine to early return since we don't need to do cef work in this case
			// 		return;
			// 	};
			// 	let load_string = |path: &std::path::PathBuf| {
			// 		let Ok(content) = fs::read_to_string(path) else {
			// 			tracing::error!("Failed to read file: {}", path.display());
			// 			return None;
			// 		};

			// 		if content.is_empty() {
			// 			tracing::warn!("Dropped file is empty: {}", path.display());
			// 			return None;
			// 		}
			// 		Some(content)
			// 	};
			// 	// TODO: Consider moving this logic to the editor so we have one message to load data which is then demultiplexed in the portfolio message handler
			// 	match extension {
			// 		"graphite" => {
			// 			let Some(content) = load_string(&path) else { return };

			// 			let message = PortfolioMessage::OpenDocumentFile {
			// 				document_name: None,
			// 				document_path: Some(path),
			// 				document_serialized_content: content,
			// 			};
			// 			self.dispatch_message(message.into());
			// 		}
			// 		"svg" => {
			// 			let Some(content) = load_string(&path) else { return };

			// 			let message = PortfolioMessage::PasteSvg {
			// 				name: path.file_stem().map(|s| s.to_string_lossy().to_string()),
			// 				svg: content,
			// 				mouse: None,
			// 				parent_and_insert_index: None,
			// 			};
			// 			self.dispatch_message(message.into());
			// 		}
			// 		_ => match image::ImageReader::open(&path) {
			// 			Ok(reader) => match reader.decode() {
			// 				Ok(image) => {
			// 					let width = image.width();
			// 					let height = image.height();
			// 					// TODO: support loading images with more than 8 bits per channel
			// 					let image_data = image.to_rgba8();
			// 					let image = Image::<Color>::from_image_data(image_data.as_raw(), width, height);

			// 					let message = PortfolioMessage::PasteImage {
			// 						name,
			// 						image,
			// 						mouse: None,
			// 						parent_and_insert_index: None,
			// 					};
			// 					self.dispatch_message(message.into());
			// 				}
			// 				Err(e) => {
			// 					tracing::error!("Failed to decode image: {}: {}", path.display(), e);
			// 				}
			// 			},
			// 			Err(e) => {
			// 				tracing::error!("Failed to open image file: {}: {}", path.display(), e);
			// 			}
			// 		},
			// 	}
			// }
			WindowEvent::CloseRequested => {
				tracing::info!("The close button was pressed; stopping");
				event_loop.exit();
			}
			WindowEvent::Resized(PhysicalSize { width, height }) => {
				let _ = self.window_size_sender.send(WindowSize::new(width as usize, height as usize));
				self.cef_context.notify_of_resize();
			}

			WindowEvent::RedrawRequested => {
				let Some(ref mut graphics_state) = self.graphics_state else { return };
				// Only rerender once we have a new ui texture to display

				match graphics_state.render() {
					Ok(_) => {}
					Err(wgpu::SurfaceError::Lost) => {
						tracing::warn!("lost surface");
					}
					Err(wgpu::SurfaceError::OutOfMemory) => {
						event_loop.exit();
					}
					Err(e) => tracing::error!("{:?}", e),
				}
			}
			_ => {}
		}

		// Notify cef of possible input events
		self.cef_context.work();
	}
}
