use crate::CustomEvent;
use crate::WindowSize;
use crate::render::GraphicsState;
use crate::render::WgpuContext;
use ::cef::CefString;
use ::cef::ImplBrowser;
use ::cef::ImplFrame;
use ::cef::ImplListValue;
use ::cef::ImplProcessMessage;
use ::cef::process_message_create;
use graph_craft::wasm_application_io::WasmApplicationIo;
use graphite_editor::application::Editor;
use graphite_editor::messages::prelude::*;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::StartCause;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::window::Window;
use winit::window::WindowId;

use crate::cef;

pub(crate) struct WinitApp {
	pub(crate) cef_context: cef::Context<cef::Initialized>,
	pub(crate) window: Option<Arc<Window>>,
	cef_schedule: Option<Instant>,
	// Cached frame buffer from CEF, used to check if mouse is on a transparent pixel
	_ui_frame_buffer: Option<wgpu::Texture>,
	window_size_sender: Sender<WindowSize>,
	_viewport_frame_buffer: Option<wgpu::Texture>,
	graphics_state: Option<GraphicsState>,
	wgpu_context: WgpuContext,
	pub(crate) editor: Editor,
}

impl WinitApp {
	pub(crate) fn new(cef_context: cef::Context<cef::Initialized>, window_size_sender: Sender<WindowSize>, wgpu_context: WgpuContext) -> Self {
		Self {
			cef_context,
			window: None,
			cef_schedule: Some(Instant::now()),
			_viewport_frame_buffer: None,
			_ui_frame_buffer: None,
			graphics_state: None,
			window_size_sender,
			wgpu_context,
			editor: Editor::new(),
		}
	}

	fn dispatch_message(&mut self, message: Message) {
		let responses = self.editor.handle_message(message);
		self.send_messages_to_editor(responses);
	}

	fn send_messages_to_editor(&mut self, responses: Vec<FrontendMessage>) {
		if responses.is_empty() {
			return;
		}
		let Some(frame) = self.cef_context.browser.as_ref().unwrap().main_frame() else {
			tracing::error!("Could not get frame after editor processed messages");
			return;
		};
		let Some(mut process_message) = process_message_create(Some(&CefString::from("editorResponseToJs"))) else {
			tracing::event!(tracing::Level::ERROR, "Failed to create process message");
			return;
		};
		let Some(arg_list) = process_message.argument_list() else { return };
		let string = ron::to_string(&responses).unwrap();
		let buffer = string.as_bytes().to_vec();
		let mut value = ::cef::binary_value_create(Some(&buffer));
		arg_list.set_binary(0, value.as_mut());
		frame.send_process_message(::cef::sys::cef_process_id_t::PID_RENDERER.into(), Some(&mut process_message));
		let message = format!("window.sendMessageToFrontend({})", buffer.len());
		let code = CefString::from(message.as_str());
		frame.execute_java_script(Some(&code), None, 0);
	}
}

impl ApplicationHandler<CustomEvent> for WinitApp {
	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		// Set a timeout in case we miss any cef schedule requests
		let timeout = Instant::now() + Duration::from_millis(10);
		let wait_until = timeout.min(self.cef_schedule.unwrap_or(timeout));
		self.cef_context.work();

		let (_has_run, texture) = futures::executor::block_on(graphite_editor::node_graph_executor::run_node_graph());
		if _has_run {
			let mut responses = VecDeque::new();
			let err = self.editor.poll_node_graph_evaluation(&mut responses);
			if let Err(e) = err {
				tracing::error!("Error poling node graph: {}", e);
			}
			let frontend_messages = responses
				.into_iter()
				.flat_map(|response| if let Message::Frontend(frontend) = response { Some(frontend) } else { None })
				.collect();
			self.send_messages_to_editor(frontend_messages);
		}
		if let Some(texture) = texture
			&& let Some(graphics_state) = &mut self.graphics_state
		{
			graphics_state.bind_viewport_texture(texture.texture.as_ref());
		}

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
		let mut graphics_state = GraphicsState::new(window.clone(), self.wgpu_context.clone());

		let mut test_data = vec![0u8; 800 * 600 * 4];

		for y in 0..600 {
			for x in 0..800 {
				let idx = (y * 800 + x) * 4;
				test_data[idx + 1] = (x * 255 / 800) as u8; // Blue
				test_data[idx + 2] = (y * 255 / 600) as u8; // Green
				test_data[idx] = 255; // Red
				test_data[idx + 3] = 255; // Alpha
			}
		}

		let texture = self.wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("Viewport Texture"),
			size: wgpu::Extent3d {
				width: 800,
				height: 600,
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
			wgpu::ImageCopyTexture {
				texture: &texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			test_data.as_slice(),
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: Some(4 * 800),
				rows_per_image: Some(600),
			},
			wgpu::Extent3d {
				width: 800,
				height: 600,
				depth_or_array_layers: 1,
			},
		);

		graphics_state.bind_viewport_texture(&texture);

		self.window = Some(window);
		self.graphics_state = Some(graphics_state);

		tracing::info!("Winit window created and ready");

		// let platform = Platform::Linux;
		// dbg!(self.editor.handle_message(GlobalsMessage::SetPlatform { platform }));
		// self.dispatch_message(PortfolioMessage::Init.into());
		graphite_editor::application::set_uuid_seed(42);

		let application_io = WasmApplicationIo::new_with_context(self.wgpu_context.clone());

		futures::executor::block_on(graphite_editor::node_graph_executor::replace_application_io(application_io));
	}

	fn user_event(&mut self, _: &ActiveEventLoop, event: CustomEvent) {
		match event {
			CustomEvent::UiUpdate(texture) => {
				if let Some(graphics_state) = self.graphics_state.as_mut() {
					graphics_state.bind_ui_texture(&texture);
					graphics_state.resize(texture.width(), texture.height());
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
			CustomEvent::MessageReceived { message } => {
				let Ok(message) = serde_json::from_str::<Message>(&message) else {
					tracing::error!("Message could not be deserialized: {:?}", message);
					return;
				};

				if let Message::InputPreprocessor(ipp_message) = &message {
					if let Some(window) = &self.window {
						window.request_redraw();
					}
					if let InputPreprocessorMessage::CurrentTime { .. } | InputPreprocessorMessage::PointerMove { .. } = &ipp_message {
					} else {
						println!("got ipp message: {:?}", &ipp_message.to_discriminant());
					}
				}
				if let Message::InputPreprocessor(InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports }) = &message {
					if let Some(graphic_state) = &mut self.graphics_state {
						let window_size = self.window.as_ref().unwrap().inner_size();
						let bounds = bounds_of_viewports[0].top_left.as_vec2() / glam::Vec2::new(window_size.width as f32, window_size.height as f32);
						let bounds = bounds.to_array();
						graphic_state.set_viewport_offset(bounds);
					} else {
						panic!("graphics state not intialized, viewport offset might be lost");
					}
				}
				self.dispatch_message(message);
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
