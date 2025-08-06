use crate::CustomEvent;
use crate::WindowSize;
use crate::dialogs::dialog_open_graphite_file;
use crate::dialogs::dialog_save_graphite_file;
use crate::render::GraphicsState;
use crate::render::WgpuContext;
use graph_craft::wasm_application_io::WasmApplicationIo;
use graphite_editor::application::Editor;
use graphite_editor::messages::prelude::*;
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
	editor: Editor,
}

impl WinitApp {
	pub(crate) fn new(cef_context: cef::Context<cef::Initialized>, window_size_sender: Sender<WindowSize>, wgpu_context: WgpuContext, event_loop_proxy: EventLoopProxy<CustomEvent>) -> Self {
		Self {
			cef_context,
			window: None,
			cef_schedule: Some(Instant::now()),
			graphics_state: None,
			window_size_sender,
			wgpu_context,
			event_loop_proxy,
			editor: Editor::new(),
		}
	}

	fn dispatch_message(&mut self, message: Message) {
		let responses = self.editor.handle_message(message);
		self.send_messages_to_editor(responses);
	}

	fn send_messages_to_editor(&mut self, mut responses: Vec<FrontendMessage>) {
		for message in responses.extract_if(.., |m| matches!(m, FrontendMessage::RenderOverlays(_))) {
			let FrontendMessage::RenderOverlays(overlay_context) = message else { unreachable!() };
			if let Some(graphics_state) = &mut self.graphics_state {
				let scene = overlay_context.take_scene();
				graphics_state.set_overlays_scene(scene);
			}
		}

		for _ in responses.extract_if(.., |m| matches!(m, FrontendMessage::TriggerOpenDocument)) {
			let event_loop_proxy = self.event_loop_proxy.clone();
			let _ = thread::spawn(move || {
				let path = futures::executor::block_on(dialog_open_graphite_file());
				if let Some(path) = path {
					let content = std::fs::read_to_string(&path).unwrap_or_else(|_| {
						tracing::error!("Failed to read file: {}", path.display());
						String::new()
					});
					let message = PortfolioMessage::OpenDocumentFile {
						document_name: path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string(),
						document_serialized_content: content,
					};
					let _ = event_loop_proxy.send_event(CustomEvent::DispatchMessage(message.into()));
				}
			});
		}

		for message in responses.extract_if(.., |m| matches!(m, FrontendMessage::TriggerSaveDocument { .. })) {
			let FrontendMessage::TriggerSaveDocument { document_id, name, path, document } = message else {
				unreachable!()
			};
			if let Some(path) = path {
				let _ = std::fs::write(&path, document);
			} else {
				let event_loop_proxy = self.event_loop_proxy.clone();
				let _ = thread::spawn(move || {
					let path = futures::executor::block_on(dialog_save_graphite_file(name));
					if let Some(path) = path {
						if let Err(e) = std::fs::write(&path, document) {
							tracing::error!("Failed to save file: {}: {}", path.display(), e);
						} else {
							let message = Message::Portfolio(PortfolioMessage::DocumentPassMessage {
								document_id,
								message: DocumentMessage::SavedDocument { path: Some(path) },
							});
							let _ = event_loop_proxy.send_event(CustomEvent::DispatchMessage(message));
						}
					}
				});
			}
		}

		if responses.is_empty() {
			return;
		}
		let Ok(message) = ron::to_string(&responses) else {
			tracing::error!("Failed to serialize Messages");
			return;
		};
		self.cef_context.send_web_message(message.as_bytes());
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
		let window = Arc::new(
			event_loop
				.create_window(
					Window::default_attributes()
						.with_title("CEF Offscreen Rendering")
						.with_inner_size(winit::dpi::LogicalSize::new(1200, 800)),
				)
				.unwrap(),
		);
		let graphics_state = GraphicsState::new(window.clone(), self.wgpu_context.clone());

		self.window = Some(window);
		self.graphics_state = Some(graphics_state);

		tracing::info!("Winit window created and ready");

		let application_io = WasmApplicationIo::new_with_context(self.wgpu_context.clone());

		futures::executor::block_on(graphite_editor::node_graph_executor::replace_application_io(application_io));
	}

	fn user_event(&mut self, _: &ActiveEventLoop, event: CustomEvent) {
		match event {
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
			CustomEvent::DispatchMessage(message) => {
				self.dispatch_message(message);
			}
			CustomEvent::MessageReceived(message) => {
				if let Message::InputPreprocessor(_) = &message {
					if let Some(window) = &self.window {
						window.request_redraw();
					}
				}
				if let Message::InputPreprocessor(InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports }) = &message {
					if let Some(graphic_state) = &mut self.graphics_state {
						let window_size = self.window.as_ref().unwrap().inner_size();
						let window_size = glam::Vec2::new(window_size.width as f32, window_size.height as f32);
						let top_left = bounds_of_viewports[0].top_left.as_vec2() / window_size;
						let bottom_right = bounds_of_viewports[0].bottom_right.as_vec2() / window_size;
						let offset = top_left.to_array();
						let scale = (bottom_right - top_left).recip();
						graphic_state.set_viewport_offset(offset);
						graphic_state.set_viewport_scale(scale.to_array());
					} else {
						panic!("graphics state not intialized, viewport offset might be lost");
					}
				}

				self.dispatch_message(message);
			}
			CustomEvent::NodeGraphRan(texture) => {
				if let Some(texture) = texture
					&& let Some(graphics_state) = &mut self.graphics_state
				{
					graphics_state.bind_viewport_texture(texture);
				}
				let mut responses = VecDeque::new();
				let err = self.editor.poll_node_graph_evaluation(&mut responses);
				if let Err(e) = err {
					if e != "No active document" {
						tracing::error!("Error poling node graph: {}", e);
					}
				}

				for message in responses {
					self.dispatch_message(message);
				}
			}
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		let Some(event) = self.cef_context.handle_window_event(event) else { return };

		match event {
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
