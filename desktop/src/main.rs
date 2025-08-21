use std::process::exit;
use std::time::Instant;

use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

pub(crate) mod consts;

mod cef;
use cef::Setup;

mod render;

mod app;
use app::WinitApp;

mod dirs;

use graphite_desktop_wrapper::messages::DesktopWrapperMessage;
use graphite_desktop_wrapper::{NodeGraphExecutionResult, WgpuContext};

pub(crate) enum CustomEvent {
	UiUpdate(wgpu::Texture),
	ScheduleBrowserWork(Instant),
	DesktopWrapperMessage(DesktopWrapperMessage),
	NodeGraphExecutionResult(NodeGraphExecutionResult),
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

	let (window_size_sender, window_size_receiver) = std::sync::mpsc::channel();

	let wgpu_context = futures::executor::block_on(WgpuContext::new()).unwrap();
	let cef_context = match cef_context.init(cef::CefHandler::new(window_size_receiver, event_loop.create_proxy(), wgpu_context.clone())) {
		Ok(c) => c,
		Err(cef::InitError::AlreadyRunning) => {
			tracing::error!("Another instance is already running, Exiting.");
			exit(0);
		}
		Err(cef::InitError::InitializationFailed(code)) => {
			tracing::error!("Cef initialization failed with code: {code}");
			exit(1);
		}
	};

	tracing::info!("Cef initialized successfully");

	let mut winit_app = WinitApp::new(cef_context, window_size_sender, wgpu_context, event_loop.create_proxy());

	event_loop.run_app(&mut winit_app).unwrap();
}
