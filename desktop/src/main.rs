use std::fmt::Debug;
use std::process::exit;

use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

mod cef;
use cef::Setup;

mod render;
use render::FrameBuffer;

mod app;
use app::WinitApp;

use crate::cef::{WindowSize, WindowSizeHandle};

#[derive(Debug)]
pub(crate) enum WinitEvent {
	// Constantly run CEF when resizing until the cef ui overlay matches the current window size
	// This is because the ResumeTimeReached event loop does not run when the window is being resized
	TryLoopCefWorkWhenResizing { window_size: WindowSize },
	// Called from the on_paint callback in OffscreenRenderHandler, and if the buffer is different than the previous buffer size
	UIUpdate { frame_buffer: FrameBuffer },
	// Called from the javascript binding to onResize for the canvas
	// ViewportResized { top_left: (u32, u32) },
	// // Called from the editor if the render node is evaluated and returns an UpdateViewport message
	// ViewportUpdate { texture: wgpu::TextureView },
}

fn main() {
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	let cef_context = match cef::Context::<Setup>::new() {
		Ok(c) => c,
		Err(cef::SetupError::Subprocess) => exit(0),
		Err(cef::SetupError::SubprocessFailed(t)) => {
			tracing::error!("Subprocess of type {t} failed");
			exit(1);
		}
	};

	let shared_window_data = WindowSizeHandle::default();

	let event_loop = EventLoop::<WinitEvent>::with_user_event().build().unwrap();

	let cef_context = match cef_context.init(event_loop.create_proxy(), shared_window_data.clone()) {
		Ok(c) => c,
		Err(cef::InitError::InitializationFailed) => {
			tracing::error!("Cef initialization failed");
			exit(1);
		}
	};

	tracing::info!("Cef initialized successfully");

	let mut winit_app = WinitApp::new(event_loop.create_proxy(), shared_window_data, cef_context);

	event_loop.run_app(&mut winit_app).unwrap();
}
