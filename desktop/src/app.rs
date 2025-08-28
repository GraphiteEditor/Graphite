use crate::CustomEvent;
use crate::cef::WindowSize;
use crate::consts::{APP_NAME, CEF_MESSAGE_LOOP_MAX_ITERATIONS};
use crate::render::GraphicsState;
use graphite_desktop_wrapper::messages::{DesktopFrontendMessage, DesktopWrapperMessage, Platform};
use graphite_desktop_wrapper::{DesktopWrapper, NodeGraphExecutionResult, WgpuContext, serialize_frontend_messages};

use rfd::AsyncFileDialog;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;
use winit::window::WindowId;

use crate::cef;

pub(crate) struct WinitApp {
	cef_context: Box<dyn cef::CefContext>,
	window: Option<Arc<Window>>,
	cef_schedule: Option<Instant>,
	window_size_sender: Sender<WindowSize>,
	graphics_state: Option<GraphicsState>,
	wgpu_context: WgpuContext,
	event_loop_proxy: EventLoopProxy<CustomEvent>,
	desktop_wrapper: DesktopWrapper,
	last_ui_update: Instant,
	avg_frame_time: f32,
	start_render_sender: SyncSender<()>,
	web_communication_initialized: bool,
	web_communication_startup_buffer: Vec<Vec<u8>>,
}

impl WinitApp {
	pub(crate) fn new(cef_context: Box<dyn cef::CefContext>, window_size_sender: Sender<WindowSize>, wgpu_context: WgpuContext, event_loop_proxy: EventLoopProxy<CustomEvent>) -> Self {
		let rendering_loop_proxy = event_loop_proxy.clone();
		let (start_render_sender, start_render_receiver) = std::sync::mpsc::sync_channel(1);
		std::thread::spawn(move || {
			loop {
				let result = futures::executor::block_on(DesktopWrapper::execute_node_graph());
				let _ = rendering_loop_proxy.send_event(CustomEvent::NodeGraphExecutionResult(result));
				let _ = start_render_receiver.recv();
			}
		});

		Self {
			cef_context,
			window: None,
			cef_schedule: Some(Instant::now()),
			graphics_state: None,
			window_size_sender,
			wgpu_context,
			event_loop_proxy,
			desktop_wrapper: DesktopWrapper::new(),
			last_ui_update: Instant::now(),
			avg_frame_time: 0.,
			start_render_sender,
			web_communication_initialized: false,
			web_communication_startup_buffer: Vec::new(),
		}
	}

	fn handle_desktop_frontend_message(&mut self, message: DesktopFrontendMessage) {
		match message {
			DesktopFrontendMessage::ToWeb(messages) => {
				let Some(bytes) = serialize_frontend_messages(messages) else {
					tracing::error!("Failed to serialize frontend messages");
					return;
				};
				self.send_or_queue_web_message(bytes);
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
			DesktopFrontendMessage::UpdateWindowState { maximized, minimized } => {
				if let Some(window) = &self.window {
					window.set_maximized(maximized);
					window.set_minimized(minimized);
				}
			}
			DesktopFrontendMessage::CloseWindow => {
				let _ = self.event_loop_proxy.send_event(CustomEvent::CloseWindow);
			}
		}
	}

	fn handle_desktop_frontend_messages(&mut self, messages: Vec<DesktopFrontendMessage>) {
		for message in messages {
			self.handle_desktop_frontend_message(message);
		}
	}

	fn dispatch_desktop_wrapper_message(&mut self, message: DesktopWrapperMessage) {
		let responses = self.desktop_wrapper.dispatch(message);
		self.handle_desktop_frontend_messages(responses);
	}

	fn send_or_queue_web_message(&mut self, message: Vec<u8>) {
		if self.web_communication_initialized {
			self.cef_context.send_web_message(message);
		} else {
			self.web_communication_startup_buffer.push(message);
		}
	}
}

impl ApplicationHandler<CustomEvent> for WinitApp {
	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		// Set a timeout in case we miss any cef schedule requests
		let timeout = Instant::now() + Duration::from_millis(10);
		let wait_until = timeout.min(self.cef_schedule.unwrap_or(timeout));
		if let Some(schedule) = self.cef_schedule
			&& schedule < Instant::now()
		{
			self.cef_schedule = None;
			// Poll cef message loop multiple times to avoid message loop starvation
			for _ in 0..CEF_MESSAGE_LOOP_MAX_ITERATIONS {
				self.cef_context.work();
			}
		}
		if let Some(window) = &self.window.as_ref() {
			window.request_redraw();
		}

		event_loop.set_control_flow(ControlFlow::WaitUntil(wait_until));
	}

	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let mut window = Window::default_attributes()
			.with_title(APP_NAME)
			.with_min_inner_size(winit::dpi::LogicalSize::new(400, 300))
			.with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

		#[cfg(target_os = "linux")]
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

		self.desktop_wrapper.init(self.wgpu_context.clone());

		#[cfg(target_os = "windows")]
		let platform = Platform::Windows;
		#[cfg(target_os = "macos")]
		let platform = Platform::Mac;
		#[cfg(target_os = "linux")]
		let platform = Platform::Linux;
		self.dispatch_desktop_wrapper_message(DesktopWrapperMessage::UpdatePlatform(platform));
	}

	fn user_event(&mut self, event_loop: &ActiveEventLoop, event: CustomEvent) {
		match event {
			CustomEvent::WebCommunicationInitialized => {
				self.web_communication_initialized = true;
				for message in self.web_communication_startup_buffer.drain(..) {
					self.cef_context.send_web_message(message);
				}
			}
			CustomEvent::DesktopWrapperMessage(message) => self.dispatch_desktop_wrapper_message(message),
			CustomEvent::NodeGraphExecutionResult(result) => match result {
				NodeGraphExecutionResult::HasRun(texture) => {
					self.dispatch_desktop_wrapper_message(DesktopWrapperMessage::PollNodeGraphEvaluation);
					if let Some(texture) = texture
						&& let Some(graphics_state) = self.graphics_state.as_mut()
						&& let Some(window) = self.window.as_ref()
					{
						graphics_state.bind_viewport_texture(texture);
						window.request_redraw();
					}
				}
				NodeGraphExecutionResult::NotRun => {}
			},
			CustomEvent::UiUpdate(texture) => {
				if let Some(graphics_state) = self.graphics_state.as_mut() {
					graphics_state.resize(texture.width(), texture.height());
					graphics_state.bind_ui_texture(texture);
					let elapsed = self.last_ui_update.elapsed().as_secs_f32();
					self.last_ui_update = Instant::now();
					if elapsed < 0.5 {
						self.avg_frame_time = (self.avg_frame_time * 3. + elapsed) / 4.;
					}
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
			CustomEvent::CloseWindow => {
				// TODO: Implement graceful shutdown

				tracing::info!("Exiting main event loop");
				event_loop.exit();
			}
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		self.cef_context.handle_window_event(&event);

		match event {
			WindowEvent::CloseRequested => {
				let _ = self.event_loop_proxy.send_event(CustomEvent::CloseWindow);
			}
			WindowEvent::Resized(PhysicalSize { width, height }) => {
				let _ = self.window_size_sender.send(WindowSize::new(width as usize, height as usize));
				self.cef_context.notify_of_resize();
			}
			WindowEvent::RedrawRequested => {
				let Some(ref mut graphics_state) = self.graphics_state else { return };
				// Only rerender once we have a new ui texture to display
				if let Some(window) = &self.window {
					match graphics_state.render(window.as_ref()) {
						Ok(_) => {}
						Err(wgpu::SurfaceError::Lost) => {
							tracing::warn!("lost surface");
						}
						Err(wgpu::SurfaceError::OutOfMemory) => {
							event_loop.exit();
						}
						Err(e) => tracing::error!("{:?}", e),
					}
					let _ = self.start_render_sender.try_send(());
				}
			}
			// Currently not supported on wayland see https://github.com/rust-windowing/winit/issues/1881
			WindowEvent::DroppedFile(path) => {
				match std::fs::read(&path) {
					Ok(content) => {
						let message = DesktopWrapperMessage::OpenFile { path, content };
						let _ = self.event_loop_proxy.send_event(CustomEvent::DesktopWrapperMessage(message));
					}
					Err(e) => {
						tracing::error!("Failed to read dropped file {}: {}", path.display(), e);
						return;
					}
				};
			}
			_ => {}
		}

		// Notify cef of possible input events
		self.cef_context.work();
	}
}
