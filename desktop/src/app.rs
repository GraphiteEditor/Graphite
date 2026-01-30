use rand::Rng;
use rfd::AsyncFileDialog;
use std::fs;
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::thread;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ButtonSource, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::cef;
use crate::cli::Cli;
use crate::consts::CEF_MESSAGE_LOOP_MAX_ITERATIONS;
use crate::event::{AppEvent, AppEventScheduler};
use crate::persist::PersistentData;
use crate::render::{RenderError, RenderState};
use crate::window::Window;
use crate::wrapper::messages::{DesktopFrontendMessage, DesktopWrapperMessage, InputMessage, MouseKeys, MouseState};
use crate::wrapper::{DesktopWrapper, NodeGraphExecutionResult, WgpuContext, serialize_frontend_messages};

pub(crate) struct App {
	render_state: Option<RenderState>,
	wgpu_context: WgpuContext,
	window: Option<Window>,
	window_scale: f64,
	window_size: PhysicalSize<u32>,
	window_maximized: bool,
	window_fullscreen: bool,
	pointer_position: PhysicalPosition<f64>,
	pointer_lock_position: Option<PhysicalPosition<f64>>,
	ui_scale: f64,
	app_event_receiver: Receiver<AppEvent>,
	app_event_scheduler: AppEventScheduler,
	desktop_wrapper: DesktopWrapper,
	cef_context: Box<dyn cef::CefContext>,
	cef_schedule: Option<Instant>,
	cef_view_info_sender: Sender<cef::ViewInfoUpdate>,
	cef_init_successful: bool,
	start_render_sender: SyncSender<()>,
	web_communication_initialized: bool,
	web_communication_startup_buffer: Vec<Vec<u8>>,
	persistent_data: PersistentData,
	cli: Cli,
	startup_time: Option<Instant>,
	exit_reason: ExitReason,
}

impl App {
	pub(crate) fn init() {
		Window::init();
	}

	pub(crate) fn new(
		cef_context: Box<dyn cef::CefContext>,
		cef_view_info_sender: Sender<cef::ViewInfoUpdate>,
		wgpu_context: WgpuContext,
		app_event_receiver: Receiver<AppEvent>,
		app_event_scheduler: AppEventScheduler,
		cli: Cli,
	) -> Self {
		let ctrlc_app_event_scheduler = app_event_scheduler.clone();
		ctrlc::set_handler(move || {
			tracing::info!("Termination signal received, exiting...");
			ctrlc_app_event_scheduler.schedule(AppEvent::Exit);
		})
		.expect("Error setting Ctrl-C handler");

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

		let desktop_wrapper = DesktopWrapper::new(rand::rng().random());

		Self {
			render_state: None,
			wgpu_context,
			window: None,
			window_scale: 1.,
			window_size: PhysicalSize { width: 0, height: 0 },
			window_maximized: false,
			window_fullscreen: false,
			pointer_position: Default::default(),
			pointer_lock_position: Default::default(),
			ui_scale: 1.,
			app_event_receiver,
			app_event_scheduler,
			desktop_wrapper,
			cef_context,
			cef_schedule: Some(Instant::now()),
			cef_view_info_sender,
			cef_init_successful: false,
			start_render_sender,
			web_communication_initialized: false,
			web_communication_startup_buffer: Vec::new(),
			persistent_data,
			cli,
			exit_reason: ExitReason::Shutdown,
			startup_time: None,
		}
	}

	pub(crate) fn run(mut self, event_loop: EventLoop) -> ExitReason {
		event_loop.run_app(&mut self).unwrap();
		self.exit_reason
	}

	fn exit(&mut self, reason: Option<ExitReason>) {
		if let Some(reason) = reason {
			self.exit_reason = reason;
		}
		self.app_event_scheduler.schedule(AppEvent::Exit);
	}

	fn resize(&mut self) {
		let Some(window) = &self.window else {
			tracing::error!("Resize failed due to missing window");
			return;
		};

		let maximized = window.is_maximized();
		if maximized != self.window_maximized {
			self.window_maximized = maximized;
			self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(DesktopWrapperMessage::UpdateMaximized { maximized }));
		}

		let fullscreen = window.is_fullscreen();
		if fullscreen != self.window_fullscreen {
			self.window_fullscreen = fullscreen;
			self.app_event_scheduler
				.schedule(AppEvent::DesktopWrapperMessage(DesktopWrapperMessage::UpdateFullscreen { fullscreen }));
		}

		let size = window.surface_size();
		let scale = window.scale_factor() * self.ui_scale;
		let is_new_size = size != self.window_size;
		let is_new_scale = scale != self.window_scale;

		if !is_new_size && !is_new_scale {
			return;
		}

		if is_new_size {
			let _ = self.cef_view_info_sender.send(cef::ViewInfoUpdate::Size {
				width: size.width,
				height: size.height,
			});
		}
		if is_new_scale {
			let _ = self.cef_view_info_sender.send(cef::ViewInfoUpdate::Scale(scale));
		}

		self.cef_context.notify_view_info_changed();

		if let Some(render_state) = &mut self.render_state {
			render_state.resize(size.width, size.height);
		}

		window.request_redraw();

		self.window_size = size;
		self.window_scale = scale;
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
						let message = DesktopWrapperMessage::FileDialogResult { path, content, context };
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
			DesktopFrontendMessage::UpdateViewportPhysicalBounds { x, y, width, height } => {
				if let Some(render_state) = &mut self.render_state
					&& let Some(window) = &self.window
				{
					let window_size = window.surface_size();

					let viewport_offset_x = x / window_size.width as f64;
					let viewport_offset_y = y / window_size.height as f64;
					render_state.set_viewport_offset([viewport_offset_x as f32, viewport_offset_y as f32]);

					let viewport_scale_x = if width != 0.0 { window_size.width as f64 / width } else { 1.0 };
					let viewport_scale_y = if height != 0.0 { window_size.height as f64 / height } else { 1.0 };
					render_state.set_viewport_scale([viewport_scale_x as f32, viewport_scale_y as f32]);
				}
			}
			DesktopFrontendMessage::UpdateUIScale { scale } => {
				self.ui_scale = scale;
				self.resize();
			}
			DesktopFrontendMessage::UpdateOverlays(scene) => {
				if let Some(render_state) = &mut self.render_state {
					render_state.set_overlays_scene(scene);
				}
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
				if self.cli.files.is_empty() {
					return;
				}
				let app_event_scheduler = self.app_event_scheduler.clone();
				let launch_documents = std::mem::take(&mut self.cli.files);
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
			DesktopFrontendMessage::UpdateMenu { entries } => {
				if let Some(window) = &self.window {
					window.update_menu(entries);
				}
			}
			DesktopFrontendMessage::ClipboardRead => {
				if let Some(window) = &self.window {
					let content = window.clipboard_read();
					let message = DesktopWrapperMessage::ClipboardReadResult { content };
					self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
				}
			}
			DesktopFrontendMessage::ClipboardWrite { content } => {
				if let Some(window) = &mut self.window {
					window.clipboard_write(content);
				}
			}
			DesktopFrontendMessage::PointerLock => {
				self.pointer_lock_position = Some(self.pointer_position);
				if let Some(window) = &self.window {
					window.start_pointer_lock();
				}
			}
			DesktopFrontendMessage::WindowClose => {
				self.app_event_scheduler.schedule(AppEvent::Exit);
			}
			DesktopFrontendMessage::WindowMinimize => {
				if let Some(window) = &self.window {
					window.minimize();
				}
			}
			DesktopFrontendMessage::WindowMaximize => {
				if let Some(window) = &self.window {
					window.toggle_maximize();
				}
			}
			DesktopFrontendMessage::WindowFullscreen => {
				if let Some(window) = &mut self.window {
					window.toggle_fullscreen();
				}
			}
			DesktopFrontendMessage::WindowDrag => {
				if let Some(window) = &self.window {
					window.start_drag();
				}
			}
			DesktopFrontendMessage::WindowHide => {
				if let Some(window) = &self.window {
					window.hide();
				}
			}
			DesktopFrontendMessage::WindowHideOthers => {
				if let Some(window) = &self.window {
					window.hide_others();
				}
			}
			DesktopFrontendMessage::WindowShowAll => {
				if let Some(window) = &self.window {
					window.show_all();
				}
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
						&& let Some(render_state) = self.render_state.as_mut()
						&& let Some(window) = self.window.as_ref()
					{
						render_state.bind_viewport_texture(texture);
						window.request_redraw();
					}
				}
				NodeGraphExecutionResult::NotRun => {}
			},
			AppEvent::UiUpdate(texture) => {
				if let Some(render_state) = self.render_state.as_mut() {
					render_state.bind_ui_texture(texture);
				}
				if let Some(window) = &self.window {
					window.request_redraw();
				}
				if !self.cef_init_successful {
					self.cef_init_successful = true;
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
				if let Some(window) = &mut self.window {
					window.set_cursor(event_loop, cursor);
				}
			}
			AppEvent::Exit => {
				tracing::info!("Exiting main event loop");
				event_loop.exit();
			}
			#[cfg(target_os = "macos")]
			AppEvent::MenuEvent { id } => {
				self.dispatch_desktop_wrapper_message(DesktopWrapperMessage::MenuEvent { id });
			}
		}
	}
}
impl ApplicationHandler for App {
	fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
		let window = Window::new(event_loop, self.app_event_scheduler.clone());
		self.window = Some(window);

		let render_state = RenderState::new(self.window.as_ref().unwrap(), self.wgpu_context.clone());
		self.render_state = Some(render_state);

		if let Some(window) = &self.window.as_ref() {
			window.show();
		}

		self.resize();

		self.desktop_wrapper.init(self.wgpu_context.clone());

		self.startup_time = Some(Instant::now());
	}

	fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
		while let Ok(event) = self.app_event_receiver.try_recv() {
			self.user_event(event_loop, event);
		}
	}

	fn window_event(&mut self, _event_loop: &dyn ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		// Handle pointer lock release
		if let Some(pointer_lock_position) = self.pointer_lock_position
			&& let WindowEvent::PointerButton {
				state: ElementState::Released,
				button: ButtonSource::Mouse(MouseButton::Left),
				..
			} = event
		{
			self.pointer_lock_position = None;
			if let Some(window) = &self.window {
				window.end_pointer_lock();
			}
			self.cef_context.handle_window_event(&WindowEvent::PointerMoved {
				device_id: None,
				position: pointer_lock_position,
				primary: true,
				source: winit::event::PointerSource::Mouse,
			});
		}

		self.cef_context.handle_window_event(&event);

		match event {
			WindowEvent::CloseRequested => {
				self.app_event_scheduler.schedule(AppEvent::Exit);
			}
			WindowEvent::SurfaceResized(_) | WindowEvent::ScaleFactorChanged { .. } => {
				self.resize();
			}
			WindowEvent::RedrawRequested => {
				#[cfg(target_os = "macos")]
				self.resize();

				let Some(render_state) = &mut self.render_state else { return };
				if let Some(window) = &self.window {
					if !window.can_render() {
						return;
					}

					match render_state.render(window) {
						Ok(_) => {}
						Err(RenderError::OutdatedUITextureError) => {
							self.cef_context.notify_view_info_changed();
						}
						Err(RenderError::SurfaceError(wgpu::SurfaceError::Lost)) => {
							tracing::warn!("lost surface");
						}
						Err(RenderError::SurfaceError(wgpu::SurfaceError::OutOfMemory)) => {
							tracing::error!("GPU out of memory");
							self.exit(None);
						}
						Err(RenderError::SurfaceError(e)) => tracing::error!("Render error: {:?}", e),
					}
					let _ = self.start_render_sender.try_send(());
				}

				if !self.cef_init_successful
					&& !self.cli.disable_ui_acceleration
					&& self.web_communication_initialized
					&& let Some(startup_time) = self.startup_time
					&& startup_time.elapsed() > Duration::from_secs(3)
				{
					tracing::error!("UI acceleration not working, exiting.");
					self.exit(Some(ExitReason::UiAccelerationFailure));
				}
			}
			WindowEvent::DragDropped { paths, .. } => {
				for path in paths {
					match fs::read(&path) {
						Ok(content) => {
							let message = DesktopWrapperMessage::ImportFile { path, content };
							self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
						}
						Err(e) => {
							tracing::error!("Failed to read dropped file {}: {}", path.display(), e);
							return;
						}
					};
				}
			}

			// Forward and Back buttons are not supported by CEF and thus need to be directly forwarded the editor
			WindowEvent::PointerButton {
				button: ButtonSource::Mouse(button),
				state: ElementState::Pressed,
				..
			} => {
				let mouse_keys = match button {
					MouseButton::Back => Some(MouseKeys::BACK),
					MouseButton::Forward => Some(MouseKeys::FORWARD),
					_ => None,
				};
				if let Some(mouse_keys) = mouse_keys {
					let message = DesktopWrapperMessage::Input(InputMessage::PointerDown {
						editor_mouse_state: MouseState { mouse_keys, ..Default::default() },
						modifier_keys: Default::default(),
					});
					self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));

					let message = DesktopWrapperMessage::Input(InputMessage::PointerUp {
						editor_mouse_state: Default::default(),
						modifier_keys: Default::default(),
					});
					self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
				}
			}

			WindowEvent::PointerMoved { position, .. } | WindowEvent::PointerLeft { position: Some(position), .. } | WindowEvent::PointerEntered { position, .. }
				if self.pointer_lock_position.is_none() =>
			{
				self.pointer_position = position;
			}

			_ => {}
		}

		// Notify cef of possible input events
		self.cef_context.work();
	}

	fn device_event(&mut self, _event_loop: &dyn ActiveEventLoop, _device_id: Option<winit::event::DeviceId>, event: winit::event::DeviceEvent) {
		if self.pointer_lock_position.is_some()
			&& let winit::event::DeviceEvent::PointerMotion { delta: (x, y) } = event
		{
			let message = DesktopWrapperMessage::PointerLockMove { x, y };
			self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
		}
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

pub(crate) enum ExitReason {
	Shutdown,
	UiAccelerationFailure,
}
