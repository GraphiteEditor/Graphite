use std::process::exit;

use winit::application::ApplicationHandler;
use winit::event::*;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::WindowId;

mod cef;
use cef::Setup;

mod winit_app;

mod render;
use render::{FrameBuffer, GraphicsState};

use crate::render::FrameBufferHandle;
use crate::winit_app::WinitApp;

impl ApplicationHandler for WinitApp {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		println!("resumed");
		let graphics_state = futures::executor::block_on(GraphicsState::init(event_loop));
		let width = graphics_state.window.inner_size().width;
		let height = graphics_state.window.inner_size().height;
		self.resize(width, height);
		
		// Initialize with a test pattern so we always have something to render
		let initial_data = vec![34u8; (width * height * 4) as usize]; // Gray texture #22222222
		self.frame_buffer.inner.lock().unwrap().add_buffer(&initial_data, width, height);

		self.graphics_state = Some(graphics_state);
	}

	fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
		// Try load the frame buffer into the ui texture if it changes
		self.try_load_frame_buffer();

		// Update the viewport texture if the canvas element changes
		self.try_load_viewport();
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		self.cef_context.handle_window_event(&event);

		match event {
			WindowEvent::CloseRequested => {
				println!("The close button was pressed; stopping");
				event_loop.exit();
			}
			WindowEvent::Resized(physical_size) => {
				self.resize(physical_size.width, physical_size.height);
			}
			WindowEvent::RedrawRequested => {
				

				match self.render() {
					Ok(_) => {}
					Err(wgpu::SurfaceError::OutOfMemory) => {
						event_loop.exit();
					}
					Err(e) => eprintln!("{:?}", e),
				}
			}
			_ => {}
		}
	}
}

fn main() {
	let args: Vec<String> = std::env::args().collect();

	let cef_context = match cef::Context::<Setup>::new() {
		Ok(c) => c,
		Err(cef::SetupError::Subprocess) => exit(0),
		Err(cef::SetupError::SubprocessFailed(t)) => {
			println!("Subprocess of type {t} failed. args: {:?}", args);
			exit(1);
		}
	};

	let frame_buffer = FrameBufferHandle::new();

	let cef_context = match cef_context.init(frame_buffer.clone()) {
		Ok(c) => c,
		Err(cef::InitError::InitializationFailed) => {
			println!("Cef initialization failed");
			exit(1);
		}
	};

	println!("Cef initialized successfully");

	let mut winit_app = WinitApp::new(cef_context, frame_buffer);

	// Start winit event loop
	let event_loop = EventLoop::new().unwrap();
	event_loop.set_control_flow(ControlFlow::Poll);
	event_loop.run_app(&mut winit_app).unwrap();

	winit_app.cef_context.shutdown();
}
