use rfd::AsyncFileDialog;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::window::WindowId;

use crate::cef;
use crate::consts::CEF_MESSAGE_LOOP_MAX_ITERATIONS;
use crate::event::{AppEvent, AppEventScheduler};
use crate::persist::PersistentData;
use crate::render::GraphicsState;
use crate::window::Window;
use graphite_desktop_wrapper::messages::{DesktopFrontendMessage, DesktopWrapperMessage, Platform};
use graphite_desktop_wrapper::{DesktopWrapper, NodeGraphExecutionResult, WgpuContext, serialize_frontend_messages};

pub(crate) struct App {
	cef_context: Box<dyn cef::CefContext>,
	window: Option<Window>,
	cef_schedule: Option<Instant>,
	cef_window_size_sender: Sender<cef::WindowSize>,
	graphics_state: Option<GraphicsState>,
	wgpu_context: WgpuContext,
	app_event_receiver: Receiver<AppEvent>,
	app_event_scheduler: AppEventScheduler,
	desktop_wrapper: DesktopWrapper,
	last_ui_update: Instant,
	avg_frame_time: f32,
	start_render_sender: SyncSender<()>,
	web_communication_initialized: bool,
	web_communication_startup_buffer: Vec<Vec<u8>>,
	persistent_data: PersistentData,
	launch_documents: Vec<PathBuf>,
}

impl App {
	pub(crate) fn new(
		cef_context: Box<dyn cef::CefContext>,
		window_size_sender: Sender<cef::WindowSize>,
		wgpu_context: WgpuContext,
		app_event_receiver: Receiver<AppEvent>,
		app_event_scheduler: AppEventScheduler,
		launch_documents: Vec<PathBuf>,
	) -> Self {
		let rendering_app_event_scheduler = app_event_scheduler.clone();
		let (start_render_sender, start_render_receiver) = std::sync::mpsc::sync_channel(1);
		std::thread::spawn(move || {
			loop {
				let result = futures::executor::block_on(DesktopWrapper::execute_node_graph());
				rendering_app_event_scheduler.schedule(AppEvent::NodeGraphExecutionResult(result));
				let _ = start_render_receiver.recv();
			}
		});

		let mut persistent_data = PersistentData::default();
		persistent_data.load_from_disk();

		Self {
			cef_context,
			window: None,
			cef_schedule: Some(Instant::now()),
			graphics_state: None,
			cef_window_size_sender: window_size_sender,
			wgpu_context,
			app_event_receiver,
			app_event_scheduler,
			desktop_wrapper: DesktopWrapper::new(),
			last_ui_update: Instant::now(),
			avg_frame_time: 0.,
			start_render_sender,
			web_communication_initialized: false,
			web_communication_startup_buffer: Vec::new(),
			persistent_data,
			launch_documents,
		}
	}

	fn handle_desktop_frontend_message(&mut self, message: DesktopFrontendMessage, responses: &mut Vec<DesktopWrapperMessage>) {
		match message {
			DesktopFrontendMessage::ToWeb(messages) => {
				let Some(bytes) = serialize_frontend_messages(messages) else {
					tracing::error!("Failed to serialize frontend messages");
					return;
				};
				self.send_or_queue_web_message(bytes);
			}
			DesktopFrontendMessage::OpenFileDialog { title, filters, context } => {
				let app_event_scheduler = self.app_event_scheduler.clone();
				let _ = thread::spawn(move || {
					let mut dialog = AsyncFileDialog::new().set_title(title);
					for filter in filters {
						dialog = dialog.add_filter(filter.name, &filter.extensions);
					}

					let show_dialog = async move { dialog.pick_file().await.map(|f| f.path().to_path_buf()) };

					if let Some(path) = futures::executor::block_on(show_dialog)
						&& let Ok(content) = fs::read(&path)
					{
						let message = DesktopWrapperMessage::OpenFileDialogResult { path, content, context };
						app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
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
				let app_event_scheduler = self.app_event_scheduler.clone();
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
						app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
					}
				});
			}
			DesktopFrontendMessage::WriteFile { path, content } => {
				if let Err(e) = fs::write(&path, content) {
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
					let window_size = window.surface_size();

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
			DesktopFrontendMessage::MinimizeWindow => {
				if let Some(window) = &self.window {
					window.minimize();
				}
			}
			DesktopFrontendMessage::MaximizeWindow => {
				if let Some(window) = &self.window {
					window.toggle_maximize();
				}
			}
			DesktopFrontendMessage::DragWindow => {
				if let Some(window) = &self.window {
					let _ = window.start_drag();
				}
			}
			DesktopFrontendMessage::CloseWindow => {
				self.app_event_scheduler.schedule(AppEvent::CloseWindow);
			}
			DesktopFrontendMessage::PersistenceWriteDocument { id, document } => {
				self.persistent_data.write_document(id, document);
			}
			DesktopFrontendMessage::PersistenceDeleteDocument { id } => {
				self.persistent_data.delete_document(&id);
			}
			DesktopFrontendMessage::PersistenceUpdateCurrentDocument { id } => {
				self.persistent_data.set_current_document(id);
			}
			DesktopFrontendMessage::PersistenceUpdateDocumentsList { ids } => {
				self.persistent_data.set_document_order(ids);
			}
			DesktopFrontendMessage::PersistenceWritePreferences { preferences } => {
				self.persistent_data.write_preferences(preferences);
			}
			DesktopFrontendMessage::PersistenceLoadPreferences => {
				let preferences = self.persistent_data.load_preferences();
				let message = DesktopWrapperMessage::LoadPreferences { preferences };
				responses.push(message);
			}
			DesktopFrontendMessage::PersistenceLoadCurrentDocument => {
				if let Some((id, document)) = self.persistent_data.current_document() {
					let message = DesktopWrapperMessage::LoadDocument {
						id,
						document,
						to_front: false,
						select_after_open: true,
					};
					responses.push(message);
				}
			}
			DesktopFrontendMessage::PersistenceLoadRemainingDocuments => {
				for (id, document) in self.persistent_data.documents_before_current().into_iter().rev() {
					let message = DesktopWrapperMessage::LoadDocument {
						id,
						document,
						to_front: true,
						select_after_open: false,
					};
					responses.push(message);
				}
				for (id, document) in self.persistent_data.documents_after_current() {
					let message = DesktopWrapperMessage::LoadDocument {
						id,
						document,
						to_front: false,
						select_after_open: false,
					};
					responses.push(message);
				}
				if let Some(id) = self.persistent_data.current_document_id() {
					let message = DesktopWrapperMessage::SelectDocument { id };
					responses.push(message);
				}
			}
			DesktopFrontendMessage::OpenLaunchDocuments => {
				if self.launch_documents.is_empty() {
					return;
				}
				let app_event_scheduler = self.app_event_scheduler.clone();
				let launch_documents = std::mem::take(&mut self.launch_documents);
				let _ = thread::spawn(move || {
					for path in launch_documents {
						tracing::info!("Opening file from command line: {}", path.display());
						if let Ok(content) = fs::read(&path) {
							let message = DesktopWrapperMessage::OpenFile { path, content };
							app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
						} else {
							tracing::error!("Failed to read file: {}", path.display());
						}
					}
				});
			}
		}
	}

	fn handle_desktop_frontend_messages(&mut self, messages: Vec<DesktopFrontendMessage>) {
		let mut responses = Vec::new();
		for message in messages {
			self.handle_desktop_frontend_message(message, &mut responses);
		}
		for message in responses {
			self.dispatch_desktop_wrapper_message(message);
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

	fn user_event(&mut self, event_loop: &dyn ActiveEventLoop, event: AppEvent) {
		match event {
			AppEvent::WebCommunicationInitialized => {
				self.web_communication_initialized = true;
				for message in self.web_communication_startup_buffer.drain(..) {
					self.cef_context.send_web_message(message);
				}
			}
			AppEvent::DesktopWrapperMessage(message) => self.dispatch_desktop_wrapper_message(message),
			AppEvent::NodeGraphExecutionResult(result) => match result {
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
			AppEvent::UiUpdate(texture) => {
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
			AppEvent::ScheduleBrowserWork(instant) => {
				if instant <= Instant::now() {
					self.cef_context.work();
				} else {
					self.cef_schedule = Some(instant);
				}
			}
			AppEvent::CursorChange(cursor) => {
				if let Some(window) = &self.window {
					window.set_cursor(cursor);
				}
			}
			AppEvent::CloseWindow => {
				// TODO: Implement graceful shutdown

				tracing::info!("Exiting main event loop");
				event_loop.exit();
			}
		}
	}
}
impl ApplicationHandler for App {
	fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
		let window = Window::new(event_loop);
		self.window = Some(window);

		let graphics_state = GraphicsState::new(self.window.as_ref().unwrap(), self.wgpu_context.clone());

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

	fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
		while let Ok(event) = self.app_event_receiver.try_recv() {
			self.user_event(event_loop, event);
		}
	}

	fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		self.cef_context.handle_window_event(&event);

		match event {
			WindowEvent::CloseRequested => {
				self.app_event_scheduler.schedule(AppEvent::CloseWindow);
			}
			WindowEvent::SurfaceResized(size) => {
				let _ = self.cef_window_size_sender.send(size.into());
				self.cef_context.notify_of_resize();
				if let Some(window) = &self.window {
					let maximized = window.is_maximized();
					self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(DesktopWrapperMessage::UpdateMaximized { maximized }));
				}
			}
			WindowEvent::RedrawRequested => {
				let Some(ref mut graphics_state) = self.graphics_state else { return };
				// Only rerender once we have a new UI texture to display
				if let Some(window) = &self.window {
					match graphics_state.render(window) {
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
			WindowEvent::DragDropped { paths, .. } => {
				for path in paths {
					match fs::read(&path) {
						Ok(content) => {
							let message = DesktopWrapperMessage::OpenFile { path, content };
							self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
						}
						Err(e) => {
							tracing::error!("Failed to read dropped file {}: {}", path.display(), e);
							return;
						}
					};
				}
			}
			_ => {}
		}

		// Notify cef of possible input events
		self.cef_context.work();
	}

	fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
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
}
