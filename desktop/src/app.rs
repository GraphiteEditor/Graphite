use crate::WindowSizeHandle;
use crate::WinitEvent;
use crate::cef::WindowSize;
use crate::render::FrameBuffer;
use crate::render::GraphicsState;
use std::time::Duration;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::StartCause;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

use crate::cef;

pub(crate) struct WinitApp {
	pub(crate) event_loop_proxy: EventLoopProxy<WinitEvent>,
	//
	pub(crate) shared_render_data: WindowSizeHandle,
	pub(crate) graphics_state: Option<GraphicsState>,
	pub(crate) cef_context: cef::Context<cef::Initialized>,
	// Cached frame buffer from CEF, used to check if mouse is on a transparent pixel
	pub(crate) frame_buffer: Option<FrameBuffer>,
}

impl WinitApp {
	pub(crate) fn new(elp: EventLoopProxy<WinitEvent>, shared_render_data: WindowSizeHandle, cef_context: cef::Context<cef::Initialized>) -> Self {
		Self {
			event_loop_proxy: elp,
			shared_render_data,
			cef_context,
			graphics_state: None,
			frame_buffer: None,
		}
	}
}

impl ApplicationHandler<WinitEvent> for WinitApp {
	// Runs on every event, but when resume time is reached (100x per second) it does the CEF work and queues a new timer.
	fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
		match cause {
			// When the event loop starts running, queue the timer.
			StartCause::Init => {
				event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(10)));
			}
			// When the timer expires, run the CEF work event and queue a new timer.
			StartCause::ResumeTimeReached { .. } => {
				self.cef_context.work();
				event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(10)));
			}
			_ => {}
		}
	}

	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		self.graphics_state = Some(futures::executor::block_on(GraphicsState::new(event_loop)));
	}

	fn user_event(&mut self, _: &ActiveEventLoop, event: WinitEvent) {
		match event {
			WinitEvent::TryLoopCefWorkWhenResizing { window_size } => {
				let Some(frame_buffer) = &self.frame_buffer else {
					return;
				};
				if window_size.width != frame_buffer.width() || window_size.height != frame_buffer.height() {
					let _ = self.event_loop_proxy.send_event(WinitEvent::TryLoopCefWorkWhenResizing { window_size });
					self.cef_context.work();
				};
			}
			WinitEvent::UIUpdate { frame_buffer } => {
				let Some(graphics_state) = &mut self.graphics_state else {
					println!("Graphics state must be initialized in UIUpdate");
					return;
				};
				graphics_state.update_ui_texture(&frame_buffer);
				graphics_state.window.request_redraw();
				self.frame_buffer = Some(frame_buffer);
			} // WinitEvent::ViewportResized {
			  // 	top_left
			  // } => {
			  // 	let Some(graphics_state) = &mut self.graphics_state else {
			  // 		println!("Graphics state must be initialized in load_frame_buffer");
			  // 		return Err("Graphics state must be initialized".to_string());
			  // 	};
			  // 	graphics_state._viewport_top_left = top_left;
			  // }
			  // 	,
			  // WinitEvent::ViewportUpdate { texture } => {
			  // 	let Some(graphics_state) = &mut self.graphics_state else {
			  // 		println!("Graphics state must be initialized in load_frame_buffer");
			  // 		return Err("Graphics state must be initialized".to_string());
			  // 	};
			  // 	graphics_state.viewport_texture = Some(texture.texture);
			  // }
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		let Some(event) = self.cef_context.handle_window_event(event) else { return };

		match event {
			WindowEvent::CloseRequested => {
				tracing::info!("The close button was pressed; stopping");
				event_loop.exit();
			}
			WindowEvent::Resized(physical_size) => {
				// The WaitUntil control flow for the timed event loop will not run when the window is being resized, so CEF needs to be manually worked
				let window_size = WindowSize::new(physical_size.width, physical_size.height);
				let _ = self.shared_render_data.with(|shared_render_data| {
					*shared_render_data = Some(window_size.clone());
				});
				self.cef_context.notify_of_resize();
				let _ = self.event_loop_proxy.send_event(WinitEvent::TryLoopCefWorkWhenResizing { window_size });
			}
			WindowEvent::RedrawRequested => {
				let Some(graphics_state) = &mut self.graphics_state else {
					println!("Graphics state must be initialized before RedrawRequested");
					return;
				};
				let _ = graphics_state.render();
			}
			_ => {}
		}
	}
}
