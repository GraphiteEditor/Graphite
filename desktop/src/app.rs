use rand::Rng;
use rfd::AsyncFileDialog;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::data_transfer::{DataTransferSendBuilder, TypeHint};
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::run_on_demand::EventLoopExtRunOnDemand;
use winit::event_loop::{ActiveEventLoop, AsyncRequestSerial, ControlFlow, DndAction, EventLoop};
use winit::window::WindowId;

use crate::dirs;
use crate::event::{AppEvent, AppEventScheduler};
use crate::input::InputState;
use crate::persist;
use crate::preferences;
use crate::render::{RenderError, RenderState};
use crate::spawner::TokioSpawner;
use crate::ui::{Acceleration, InputEvent, UiCommand, UiConfig, UiInstance};
use crate::window::Window;
use crate::wrapper::messages::{DesktopFrontendMessage, DesktopWrapperMessage, IngestAction};
use crate::wrapper::{DesktopWrapper, FolderStore, MmapResourceStorage, WgpuContext, WgpuExecutor, serialize_frontend_messages};

pub(crate) struct App {
	render_state: Option<RenderState>,
	wgpu_executor: WgpuExecutor,
	window: Option<Window>,
	window_scale: f64,
	window_size: PhysicalSize<u32>,
	window_maximized: bool,
	window_fullscreen: bool,
	window_pending_drag: bool,
	pending_dnd_fetch: Option<AsyncRequestSerial>,
	pending_clipboard_fetch: Option<AsyncRequestSerial>,
	input_state: InputState,
	app_event_receiver: Receiver<AppEvent>,
	app_event_scheduler: AppEventScheduler,
	desktop_wrapper: DesktopWrapper,
	ui: UiInstance,
	ui_frame_received: bool,
	start_render_sender: SyncSender<()>,
	ui_initialized: Option<Instant>,
	web_communication_startup_buffer: Vec<Vec<u8>>,
	preferences: preferences::DesktopPreferences,
	launch_documents: Option<Vec<PathBuf>>,
	exiting: Arc<AtomicBool>,
	exit_reason: ExitReason,
}

impl App {
	pub(crate) fn init() {
		Window::init();
	}

	pub(crate) fn new(
		ui: UiInstance,
		wgpu_context: WgpuContext,
		app_event_receiver: Receiver<AppEvent>,
		app_event_scheduler: AppEventScheduler,
		preferences: preferences::DesktopPreferences,
		launch_documents: Vec<PathBuf>,
	) -> Self {
		let ctrlc_app_event_scheduler = app_event_scheduler.clone();
		ctrlc::set_handler(move || {
			tracing::info!("Termination signal received, exiting...");
			ctrlc_app_event_scheduler.schedule(AppEvent::Exit);
		})
		.expect("Error setting Ctrl-C handler");

		let exiting = Arc::new(AtomicBool::new(false));

		let (start_render_sender, start_render_receiver) = std::sync::mpsc::sync_channel(1);

		let async_runtime = tokio::runtime::Builder::new_multi_thread()
			.thread_name("graphite-async")
			.enable_all()
			.build()
			.expect("failed to construct tokio runtime");

		let resource_storage = MmapResourceStorage::new(dirs::app_resources_dir()).expect("Failed to initialize on-disk resource storage");

		let wake_scheduler = app_event_scheduler.clone();
		let editor_wake = move || {
			wake_scheduler.schedule(AppEvent::DesktopWrapperMessage(DesktopWrapperMessage::Wake));
		};
		let runtime_wake_sender = start_render_sender.clone();
		let runtime_wake = move || {
			let _ = runtime_wake_sender.try_send(());
		};
		let wgpu_executor = WgpuExecutor::with_context(wgpu_context).expect("Failed to create WgpuExecutor");
		let (desktop_wrapper, mut node_runtime) = DesktopWrapper::new(
			rand::rng().random(),
			Arc::new(resource_storage),
			Arc::new(FolderStore::new(dirs::app_autosave_documents_dir())),
			wgpu_executor.clone(),
			TokioSpawner::new(async_runtime.handle().clone()),
			editor_wake,
			runtime_wake,
		);

		let rendering_app_event_scheduler = app_event_scheduler.clone();
		let exiting_clone = exiting.clone();
		std::thread::spawn(move || {
			loop {
				let texture = async_runtime.block_on(node_runtime.run());
				rendering_app_event_scheduler.schedule(AppEvent::ViewportTexture(texture));
				let frame_ended = match node_runtime.until_next_frame() {
					Some(wait) => {
						thread::sleep(wait);
						true
					}
					None => start_render_receiver.recv_timeout(Duration::from_millis(10)).is_err(),
				};
				if frame_ended {
					node_runtime.begin_frame();
				}
				if exiting_clone.load(Ordering::Relaxed) {
					break;
				}
			}
		});

		Self {
			render_state: None,
			wgpu_executor,
			window: None,
			window_scale: 1.,
			window_size: PhysicalSize { width: 0, height: 0 },
			window_maximized: false,
			window_fullscreen: false,
			window_pending_drag: false,
			pending_dnd_fetch: None,
			pending_clipboard_fetch: None,
			input_state: InputState::new(),
			app_event_receiver,
			app_event_scheduler,
			desktop_wrapper,
			ui,
			ui_frame_received: false,
			start_render_sender,
			ui_initialized: None,
			web_communication_startup_buffer: Vec::new(),
			preferences,
			launch_documents: Some(launch_documents),
			exiting,
			exit_reason: ExitReason::Shutdown,
		}
	}

	pub(crate) fn run(mut self, mut event_loop: EventLoop) -> ExitReason {
		event_loop.run_app_on_demand(&mut self).unwrap();
		self.exit_reason
	}

	fn exit(&mut self, reason: Option<ExitReason>) {
		if self.exiting.swap(true, Ordering::Relaxed) {
			return;
		}
		let _ = self.start_render_sender.send(());
		if let Some(reason) = reason {
			self.exit_reason = reason;
		}
		self.app_event_scheduler.schedule(AppEvent::Exit);
	}

	fn restart_ui(&mut self) {
		let acceleration = self.preferences.acceleration;
		if let Err(error) = self.ui.restart(UiConfig { acceleration }) {
			tracing::error!("Failed to restart the UI: {error}");
			self.exit(Some(ExitReason::Shutdown));
			return;
		}
		self.ui_initialized = None;

		self.window_size = PhysicalSize { width: 0, height: 0 };
		self.window_scale = 1.;
		self.window_maximized = false;
		self.window_fullscreen = false;
		self.resize();
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
		let scale = window.scale_factor() * self.preferences.ui_scale;
		let is_new_size = size != self.window_size;
		let is_new_scale = scale != self.window_scale;

		if !is_new_size && !is_new_scale {
			return;
		}

		if is_new_size {
			self.ui.send(UiCommand::Resized {
				width: size.width,
				height: size.height,
			});
		}
		if is_new_scale {
			self.ui.send(UiCommand::ScaleChanged(scale));
		}

		self.ui.send(UiCommand::Refresh);

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
			DesktopFrontendMessage::OpenFileDialog { title, options, action } => {
				let app_event_scheduler = self.app_event_scheduler.clone();
				let _ = thread::spawn(move || {
					let mut dialog = AsyncFileDialog::new().set_title(title);
					for filter in options.filters {
						dialog = dialog.add_filter(filter.name, &filter.extensions);
					}

					let handles = if options.multiple {
						futures::executor::block_on(dialog.pick_files()).unwrap_or_default()
					} else {
						futures::executor::block_on(dialog.pick_file()).into_iter().collect()
					};

					for handle in handles {
						let path = handle.path().to_path_buf();
						match fs::read(&path) {
							Ok(content) => {
								let action = action.clone();
								let message = DesktopWrapperMessage::IngestFile { path, content, action };
								app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
							}
							Err(e) => tracing::error!("Failed to read file {}: {}", path.display(), e),
						}
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
				self.input_state.set_viewport_info(x, y, width, height, self.window_scale);

				if let Some(render_state) = &mut self.render_state
					&& let Some(window) = &self.window
				{
					let window_size = window.surface_size();

					let viewport_offset_x = x / window_size.width as f64;
					let viewport_offset_y = y / window_size.height as f64;
					render_state.set_viewport_offset([viewport_offset_x as f32, viewport_offset_y as f32]);

					let viewport_scale_x = if width != 0. { window_size.width as f64 / width } else { 1. };
					let viewport_scale_y = if height != 0. { window_size.height as f64 / height } else { 1. };
					render_state.set_viewport_scale([viewport_scale_x as f32, viewport_scale_y as f32]);
				}
			}
			DesktopFrontendMessage::WindowUpdateDirectInput { enabled } => {
				self.input_state.set_direct_input(enabled);
			}
			DesktopFrontendMessage::UpdateOverlays(scene) => {
				if let Some(render_state) = &mut self.render_state {
					render_state.set_overlays_scene(*scene);
				}
				if let Some(window) = &self.window {
					window.request_redraw();
				}
			}
			DesktopFrontendMessage::PersistenceWriteState { state } => {
				persist::write_state(state);
			}
			DesktopFrontendMessage::PersistenceReadState => {
				responses.push(DesktopWrapperMessage::LoadPersistedState { state: persist::read_state() });
			}
			DesktopFrontendMessage::PersistenceUpdatePreferences { preferences } => {
				let old_preferences = self.preferences.clone();
				self.preferences = preferences::DesktopPreferences::from(&preferences);

				preferences::write(preferences);

				if old_preferences.present_mode != self.preferences.present_mode
					&& let Some(render_state) = &mut self.render_state
				{
					render_state.set_present_mode(self.preferences.present_mode);
				}

				if old_preferences.ui_scale != self.preferences.ui_scale {
					self.resize();
				}

				if old_preferences.acceleration != self.preferences.acceleration {
					tracing::info!("UI acceleration preference changed, restarting UI");
					self.restart_ui();
				}
			}
			DesktopFrontendMessage::PersistenceLoadPreferences => {
				let preferences = preferences::read();
				let message = DesktopWrapperMessage::LoadPreferences { preferences };
				responses.push(message);
			}
			DesktopFrontendMessage::OpenLaunchDocuments => {
				let Some(launch_documents) = std::mem::take(&mut self.launch_documents) else {
					return;
				};
				self.app_event_scheduler.schedule(AppEvent::OpenFiles(launch_documents));
			}
			DesktopFrontendMessage::UpdateMenu { entries } => {
				if let Some(window) = &self.window {
					window.update_menu(entries);
				}
			}
			DesktopFrontendMessage::ClipboardRead => {
				self.app_event_scheduler.schedule(AppEvent::ClipboardRead);
			}
			DesktopFrontendMessage::ClipboardWrite { content } => {
				self.app_event_scheduler.schedule(AppEvent::ClipboardWrite { content });
			}
			DesktopFrontendMessage::PointerLock => {
				self.input_state.lock_pointer();
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
				self.window_pending_drag = true;
			}
			DesktopFrontendMessage::WindowFocus => {
				if let Some(window) = &self.window {
					window.focus();
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
			DesktopFrontendMessage::Restart => {
				self.exit(Some(ExitReason::Restart));
			}
			DesktopFrontendMessage::LoadThirdPartyLicenses => {
				let compressed = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/third-party-licenses.txt.xz"));
				let mut reader = lzma_rust2::XzReader::new(compressed.as_slice(), false);
				let mut text = String::new();
				if let Err(e) = reader.read_to_string(&mut text) {
					tracing::error!("Failed to decompress third-party licenses: {e}");
					return;
				}

				let message = DesktopWrapperMessage::LoadThirdPartyLicenses { text };
				responses.push(message);
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
		if self.ui_initialized.is_some() {
			self.ui.send(UiCommand::Message(message));
		} else {
			self.web_communication_startup_buffer.push(message);
		}
	}

	fn user_event(&mut self, event_loop: &dyn ActiveEventLoop, event: AppEvent) {
		match event {
			AppEvent::WebCommunicationInitialized => {
				self.ui_initialized = Some(Instant::now());
				self.ui_frame_received = false;
				for message in self.web_communication_startup_buffer.drain(..) {
					self.ui.send(UiCommand::Message(message));
				}
			}
			AppEvent::DesktopWrapperMessage(message) => self.dispatch_desktop_wrapper_message(message),
			AppEvent::ViewportTexture(texture) => {
				if let Some(texture) = texture
					&& let Some(render_state) = self.render_state.as_mut()
					&& let Some(window) = self.window.as_ref()
				{
					render_state.bind_viewport_texture(texture);
					window.request_redraw();
				}
			}
			AppEvent::UiUpdate(texture) => {
				if let Some(render_state) = self.render_state.as_mut() {
					render_state.bind_ui_texture(texture);
				}
				if let Some(window) = &self.window {
					window.request_redraw();
				}
				if !self.ui_frame_received {
					self.ui_frame_received = true;
				}
			}
			AppEvent::ClipboardRead => {
				let result = event_loop.clipboard().and_then(|id| {
					let Some(id) = id else { return Ok(None) };
					let data_transfer = event_loop.data_transfer(id)?;
					if !data_transfer.has_type(&TypeHint::Plaintext) {
						return Ok(None);
					}
					event_loop.fetch_data_transfer(id, &TypeHint::Plaintext).map(Some)
				});
				match result {
					Ok(Some(serial)) => self.pending_clipboard_fetch = Some(serial),
					Ok(None) => self.dispatch_desktop_wrapper_message(DesktopWrapperMessage::ClipboardReadResult { content: None }),
					Err(e) => {
						tracing::error!("Failed to read from clipboard: {e}");
						self.dispatch_desktop_wrapper_message(DesktopWrapperMessage::ClipboardReadResult { content: None });
					}
				}
			}
			AppEvent::ClipboardWrite { content } => {
				let send_data = DataTransferSendBuilder::new(content)
					.with_type(TypeHint::Plaintext, |content: &String, _| Some(content.clone()))
					.build();
				if let Err(e) = event_loop.set_clipboard(send_data) {
					tracing::error!("Failed to write to clipboard: {e}");
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
			AppEvent::UiCrashed if self.ui_initialized.is_some() => {
				tracing::error!("UI process crashed, restarting it.");
				self.restart_ui();
			}
			AppEvent::UiCrashed => {
				tracing::error!("UI process crashed, exiting.");
				self.exit(Some(ExitReason::Shutdown));
			}
			AppEvent::OpenFiles(paths) => {
				// Accumulate launch documents until OpenLaunchDocuments message is received
				if let Some(launch_documents) = &mut self.launch_documents {
					launch_documents.extend(paths);
					return;
				}

				if paths.is_empty() {
					return;
				}
				let app_event_scheduler = self.app_event_scheduler.clone();
				let _ = thread::spawn(move || {
					for path in paths {
						tracing::info!("Opening file: {}", path.display());
						if let Ok(content) = fs::read(&path) {
							let message = DesktopWrapperMessage::IngestFile {
								path,
								content,
								action: IngestAction::Open,
							};
							app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
						} else {
							tracing::error!("Failed to read file: {}", path.display());
						}
					}
				});
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

		let mut render_state = RenderState::new(self.window.as_ref().unwrap(), self.wgpu_executor.clone());
		if self.preferences.present_mode.is_some() {
			render_state.set_present_mode(self.preferences.present_mode);
		}
		self.render_state = Some(render_state);

		if let Some(window) = &self.window.as_ref() {
			window.show();
		}

		self.resize();
	}

	fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
		while let Ok(event) = self.app_event_receiver.try_recv() {
			self.user_event(event_loop, event);
		}
	}

	fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		// Handle pointer lock release
		if let WindowEvent::PointerButton {
			state: ElementState::Released,
			button,
			..
		} = &event && button.clone().mouse_button() == Some(MouseButton::Left)
			&& let Some(pointer_lock_position) = self.input_state.unlock_pointer()
		{
			if let Some(window) = &self.window {
				window.end_pointer_lock();
			}
			self.ui.send(UiCommand::Input(
				InputEvent::pointer().position(pointer_lock_position).moved().modifiers(self.input_state.modifiers()).build(),
			));
		}

		self.input_state.process(
			&event,
			|message| self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(DesktopWrapperMessage::Input(message))),
			|input| self.ui.send(UiCommand::Input(input)),
		);

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
							self.ui.send(UiCommand::Refresh);
						}
						Err(RenderError::SurfaceLost) => {
							tracing::warn!("lost surface");
						}
						Err(other) => tracing::error!("Render error: {:?}", other),
					}
					let _ = self.start_render_sender.try_send(());
				}

				if !self.ui_frame_received
					&& self.preferences.acceleration != Acceleration::Disabled
					&& let Some(initialized) = self.ui_initialized
					&& initialized.elapsed() > Duration::from_secs(3)
				{
					tracing::error!("UI acceleration not working, disabling it.");
					let mut preferences = preferences::read();
					preferences.disable_ui_acceleration = true;
					self.dispatch_desktop_wrapper_message(DesktopWrapperMessage::LoadPreferences { preferences });
				}
			}
			WindowEvent::DragEntered { id, .. } => {
				let accepts = event_loop.data_transfer(id).is_ok_and(|data_transfer| data_transfer.has_type(&TypeHint::UriList));
				let actions: &[DndAction] = if accepts { &[DndAction::Copy] } else { &[] };
				if let Err(e) = event_loop.set_valid_dnd_actions(id, actions) {
					tracing::error!("Failed to set valid drag and drop actions: {e}");
				}
			}
			WindowEvent::DragDropped { id, .. } => match event_loop.fetch_data_transfer(id, &TypeHint::UriList) {
				Ok(serial) => self.pending_dnd_fetch = Some(serial),
				Err(e) => tracing::error!("Failed to fetch dropped data: {e}"),
			},
			WindowEvent::DataTransferReceived { serial, ref value, .. } if self.pending_clipboard_fetch == Some(serial) => match value.try_as_string() {
				Ok(content) => {
					self.pending_clipboard_fetch = None;
					let message = DesktopWrapperMessage::ClipboardReadResult { content: Some(content) };
					self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
				}
				Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
				Err(e) => {
					self.pending_clipboard_fetch = None;
					tracing::error!("Failed to read from clipboard: {e}");
					let message = DesktopWrapperMessage::ClipboardReadResult { content: None };
					self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
				}
			},
			WindowEvent::DataTransferReceived { serial, ref value, .. } if self.pending_dnd_fetch == Some(serial) => match value.try_as_uris() {
				Ok(uris) => {
					self.pending_dnd_fetch = None;
					let paths = uris.iter().filter_map(|uri| url::Url::parse(uri).ok()?.to_file_path().ok());
					for path in paths {
						match fs::read(&path) {
							Ok(content) => {
								let message = DesktopWrapperMessage::IngestFile {
									path,
									content,
									action: IngestAction::Import,
								};
								self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
							}
							Err(e) => {
								tracing::error!("Failed to read dropped file {}: {}", path.display(), e);
								return;
							}
						};
					}
				}
				Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
				Err(e) => {
					self.pending_dnd_fetch = None;
					tracing::error!("Failed to read dropped data: {e}");
				}
			},

			WindowEvent::PointerMoved { .. } | WindowEvent::PointerLeft { position: Some(_), .. } | WindowEvent::PointerEntered { .. }
				if !self.input_state.pointer_locked() && self.window_pending_drag =>
			{
				self.window_pending_drag = false;
				if let Some(window) = &self.window {
					window.start_drag();
				}
			}

			WindowEvent::PointerButton {
				button,
				state: ElementState::Released,
				..
			} if button.clone().mouse_button() == Some(MouseButton::Left) => {
				self.window_pending_drag = false;
			}

			_ => {}
		}
	}

	fn device_event(&mut self, _event_loop: &dyn ActiveEventLoop, _device_id: Option<winit::event::DeviceId>, event: winit::event::DeviceEvent) {
		if self.input_state.pointer_locked()
			&& let winit::event::DeviceEvent::PointerMotion { delta: (x, y) } = event
		{
			let message = DesktopWrapperMessage::PointerLockMove { x, y };
			self.app_event_scheduler.schedule(AppEvent::DesktopWrapperMessage(message));
		}
	}

	fn new_events(&mut self, _event_loop: &dyn ActiveEventLoop, cause: winit::event::StartCause) {
		if let StartCause::ResumeTimeReached { .. } = cause
			&& let Some(window) = &self.window
		{
			window.request_redraw();
		}
	}

	fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
		event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(10)));
	}
}

pub(crate) enum ExitReason {
	Shutdown,
	Restart,
}
