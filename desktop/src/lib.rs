use clap::Parser;
use std::process::exit;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

pub(crate) mod consts;

mod app;
mod cef;
mod cli;
mod dirs;
mod event;
mod persist;
mod render;
mod window;

mod gpu_context;

use app::App;
use cef::CefHandler;
use cli::Cli;
use event::CreateAppEventSchedulerEventLoopExt;

pub fn start() {
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	let cef_context_builder = cef::CefContextBuilder::<CefHandler>::new();

	if cef_context_builder.is_sub_process() {
		// We are in a CEF subprocess
		// This will block until the CEF subprocess quits
		let error = cef_context_builder.execute_sub_process();
		tracing::warn!("Cef subprocess failed with error: {error}");
		return;
	}

	let cli = Cli::parse();

	let wgpu_context = futures::executor::block_on(gpu_context::create_wgpu_context());

	let event_loop = EventLoop::new().unwrap();
	let (app_event_sender, app_event_receiver) = std::sync::mpsc::channel();
	let app_event_scheduler = event_loop.create_app_event_scheduler(app_event_sender);

	let (window_size_sender, window_size_receiver) = std::sync::mpsc::channel();

	let cef_handler = cef::CefHandler::new(wgpu_context.clone(), app_event_scheduler.clone(), window_size_receiver);
	let cef_context = match cef_context_builder.initialize(cef_handler, cli.disable_ui_acceleration) {
		Ok(c) => {
			tracing::info!("CEF initialized successfully");
			c
		}
		Err(cef::InitError::AlreadyRunning) => {
			tracing::error!("Another instance is already running, Exiting.");
			exit(0);
		}
		Err(cef::InitError::InitializationFailed(code)) => {
			tracing::error!("Cef initialization failed with code: {code}");
			exit(1);
		}
		Err(cef::InitError::BrowserCreationFailed) => {
			tracing::error!("Failed to create CEF browser");
			exit(1);
		}
		Err(cef::InitError::RequestContextCreationFailed) => {
			tracing::error!("Failed to create CEF request context");
			exit(1);
		}
	};

	let mut app = App::new(Box::new(cef_context), window_size_sender, wgpu_context, app_event_receiver, app_event_scheduler, cli.files);

	event_loop.run_app(&mut app).unwrap();
}

pub fn start_helper() {
	let cef_context_builder = cef::CefContextBuilder::<CefHandler>::new_helper();
	assert!(cef_context_builder.is_sub_process());
	cef_context_builder.execute_sub_process();
}
