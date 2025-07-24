use crate::CustomEvent;
use crate::FrameBuffer;
use crate::WindowSize;
use crate::render::GraphicsState;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;
use winit::window::WindowId;

use crate::cef;

pub(crate) struct WinitApp {
	pub(crate) cef_context: cef::Context<cef::Initialized>,
	pub(crate) window: Option<Arc<Window>>,
	cef_schedule: Option<Instant>,
	ui_frame_buffer: Option<FrameBuffer>,
	window_size_sender: Sender<WindowSize>,
	_viewport_frame_buffer: Option<FrameBuffer>,
	graphics_state: Option<GraphicsState>,
	event_loop_proxy: EventLoopProxy<CustomEvent>,
	handle: Option<JoinHandle<()>>,
}

impl WinitApp {
	pub(crate) fn new(cef_context: cef::Context<cef::Initialized>, window_size_sender: Sender<WindowSize>, event_loop_proxy: EventLoopProxy<CustomEvent>, handle: JoinHandle<()>) -> Self {
		Self {
			cef_context,
			window: None,
			cef_schedule: Some(Instant::now()),
			_viewport_frame_buffer: None,
			ui_frame_buffer: None,
			graphics_state: None,
			window_size_sender,
			event_loop_proxy,
			handle: Some(handle),
		}
	}
}

impl ApplicationHandler<CustomEvent> for WinitApp {
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
		match event {
			CustomEvent::UiUpdate(frame_buffer) => {
				if let Some(graphics_state) = self.graphics_state.as_mut() {
					graphics_state.update_texture(&frame_buffer);
				}
				self.ui_frame_buffer = Some(frame_buffer);
				if let Some(window) = &self.window {
					window.request_redraw();
				}
			}
			CustomEvent::WorkCef => {
				self.cef_context.work();
			}
			CustomEvent::KeepProcessAliveWhenResizing(window_size) => {
				let Some(frame_buffer) = &self.ui_frame_buffer else {
					return;
				};
				if window_size.width != frame_buffer.width() || window_size.height != frame_buffer.height() {
					let _ = self.event_loop_proxy.send_event(CustomEvent::KeepProcessAliveWhenResizing(window_size));
				};
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
				let window_size = WindowSize::new(width as usize, height as usize);
				let _ = self.window_size_sender.send(window_size);
				self.cef_context.notify_of_resize();
				self.event_loop_proxy.send_event(CustomEvent::KeepProcessAliveWhenResizing(window_size));
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
	}

	fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
		let _ = self.handle.take().unwrap().join();
	}
}
