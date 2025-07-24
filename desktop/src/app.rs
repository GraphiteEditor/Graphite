use crate::CustomEvent;
use crate::FrameBuffer;
use crate::WindowSize;
use crate::render::GraphicsState;
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
	ui_dirty: bool,
	ui_frame_buffer: Option<FrameBuffer>,
	window_size_sender: Sender<WindowSize>,
	_viewport_frame_buffer: Option<FrameBuffer>,
	graphics_state: Option<GraphicsState>,
}

impl WinitApp {
	pub(crate) fn new(cef_context: cef::Context<cef::Initialized>, window_size_sender: Sender<WindowSize>) -> Self {
		Self {
			cef_context,
			window: None,
			cef_schedule: Some(Instant::now()),
			_viewport_frame_buffer: None,
			ui_frame_buffer: None,
			ui_dirty: false,
			graphics_state: None,
			window_size_sender,
		}
	}
	fn run_cef(&mut self) {
		if self.ui_frame_buffer.is_none() {
			self.cef_context.work();
		}
		if let Some(schedule) = self.cef_schedule
			&& schedule < Instant::now()
		{
			self.cef_schedule = None;
			self.cef_context.work();
		}
	}
}

impl ApplicationHandler<CustomEvent> for WinitApp {
	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		let timeout = Instant::now() + Duration::from_millis(1000);
		let wait_until = timeout.min(self.cef_schedule.unwrap_or(timeout));
		event_loop.set_control_flow(ControlFlow::WaitUntil(wait_until));
	}

	fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {
		self.run_cef();
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
		let graphics_state = pollster::block_on(GraphicsState::new(window.clone()));

		self.window = Some(window);
		self.graphics_state = Some(graphics_state);

		tracing::info!("Winit window created and ready");
	}

	fn user_event(&mut self, _: &ActiveEventLoop, event: CustomEvent) {
		self.run_cef();
		match event {
			CustomEvent::UiUpdate(frame_buffer) => {
				if let Some(graphics_state) = self.graphics_state.as_mut() {
					graphics_state.update_texture(&frame_buffer);
					self.ui_dirty = true;
				}
				self.ui_frame_buffer = Some(frame_buffer);
				if let Some(window) = &self.window {
					window.request_redraw();
				}
			}
			CustomEvent::ScheduleBrowserWork(instant) => {
				self.cef_schedule = Some(instant);
			}
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		self.run_cef();
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
				if self.ui_dirty {
					self.ui_dirty = false;

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
			}
			_ => {}
		}

		// Notify cef of possible input events
		self.cef_context.work();
	}
}
