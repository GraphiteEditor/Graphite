use crate::CustomEvent;
use crate::WindowState;
use crate::WindowStateHandle;
use crate::render::GraphicsState;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::StartCause;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::window::Window;
use winit::window::WindowId;

use crate::cef;

pub(crate) struct WinitApp {
	pub(crate) window_state: WindowStateHandle,
	pub(crate) cef_context: cef::Context<cef::Initialized>,
	pub(crate) window: Option<Arc<Window>>,
	cef_schedule: Option<Instant>,
}

impl WinitApp {
	pub(crate) fn new(window_state: WindowStateHandle, cef_context: cef::Context<cef::Initialized>) -> Self {
		Self {
			window_state,
			cef_context,
			window: None,
			cef_schedule: Some(Instant::now()),
		}
	}
}

impl ApplicationHandler<CustomEvent> for WinitApp {
	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		let timeout = Instant::now() + Duration::from_millis(10);
		let wait_until = timeout.min(self.cef_schedule.unwrap_or(timeout));
		event_loop.set_control_flow(ControlFlow::WaitUntil(wait_until));
	}

	fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {
		if let Some(schedule) = self.cef_schedule
			&& schedule < Instant::now()
		{
			self.cef_schedule = None;
			self.cef_context.work();
		}
	}

	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		self.window_state
			.with(|s| {
				if let WindowState { width: Some(w), height: Some(h), .. } = s {
					let window = Arc::new(
						event_loop
							.create_window(
								Window::default_attributes()
									.with_title("CEF Offscreen Rendering")
									.with_inner_size(winit::dpi::LogicalSize::new(*w as u32, *h as u32)),
							)
							.unwrap(),
					);
					let graphics_state = pollster::block_on(GraphicsState::new(window.clone()));

					self.window = Some(window.clone());
					s.graphics_state = Some(graphics_state);

					println!("Winit window created and ready");
				}
			})
			.unwrap();
	}

	fn user_event(&mut self, _: &ActiveEventLoop, event: CustomEvent) {
		match event {
			CustomEvent::UiUpdate => {
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
		let Some(event) = self.cef_context.handle_window_event(event) else { return };

		match event {
			WindowEvent::CloseRequested => {
				println!("The close button was pressed; stopping");
				event_loop.exit();
			}
			WindowEvent::Resized(physical_size) => {
				self.window_state
					.with(|s| {
						let width = physical_size.width as usize;
						let height = physical_size.height as usize;
						s.width = Some(width);
						s.height = Some(height);
						if let Some(graphics_state) = &mut s.graphics_state {
							graphics_state.resize(width, height);
						}
					})
					.unwrap();
				self.cef_context.notify_of_resize();
			}

			WindowEvent::RedrawRequested => {
				self.cef_context.work();

				self.window_state
					.with(|s| {
						if let WindowState {
							width: Some(width),
							height: Some(height),
							graphics_state: Some(graphics_state),
							ui_fb,
							..
						} = s
						{
							if let Some(fb) = &*ui_fb {
								graphics_state.update_texture(fb);
								if fb.width() != *width && fb.height() != *height {
									graphics_state.resize(*width, *height);
								}
							} else if let Some(window) = &self.window {
								window.request_redraw();
							}

							match graphics_state.render() {
								Ok(_) => {}
								Err(wgpu::SurfaceError::Lost) => {
									graphics_state.resize(*width, *height);
								}
								Err(wgpu::SurfaceError::OutOfMemory) => {
									event_loop.exit();
								}
								Err(e) => eprintln!("{:?}", e),
							}
						}
					})
					.unwrap();
			}
			_ => {}
		}
	}
}
