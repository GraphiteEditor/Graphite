use std::fmt::Debug;
use std::process::exit;
use std::time::Instant;

use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

mod cef;
use cef::{Setup, WindowSize};

mod render;
use render::FrameBuffer;

mod app;
use app::WinitApp;

mod dirs;

#[derive(Debug)]
pub(crate) enum CustomEvent {
	UiUpdate(FrameBuffer),
	ScheduleBrowserWork(Instant),
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

	let event_loop = EventLoop::<CustomEvent>::with_user_event().build().unwrap();

	let (send, recv) = std::sync::mpsc::channel();

	let cef_context = match cef_context.init(cef::CefHandler::new(recv, event_loop.create_proxy())) {
		Ok(c) => c,
		Err(cef::InitError::InitializationFailed) => {
			tracing::error!("Cef initialization failed");
			exit(1);
		}
	};

	tracing::info!("Cef initialized successfully");

	let mut winit_app = WinitApp::new(cef_context, send, event_loop.create_proxy());

	event_loop.run_app(&mut winit_app).unwrap();
}
