use std::process::exit;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

use graphite_desktop_wrapper::WgpuContext;

pub(crate) mod consts;

mod app;
mod cef;
mod dirs;
mod event;
mod native_window;
mod persist;
mod render;

use app::App;
use cef::CefHandler;
use event::CreateAppEventSchedulerEventLoopExt;

fn main() {
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	let cef_context_builder = cef::CefContextBuilder::<CefHandler>::new();

	if cef_context_builder.is_sub_process() {
		// We are in a CEF subprocess
		// This will block until the CEF subprocess quits
		let error = cef_context_builder.execute_sub_process();
		tracing::error!("Cef subprocess failed with error: {error}");
		return;
	}

	let wgpu_context = futures::executor::block_on(init_wgpu_context());

	let event_loop = EventLoop::new().unwrap();
	let (app_event_sender, app_event_receiver) = std::sync::mpsc::channel();
	let app_event_scheduler = event_loop.create_app_event_scheduler(app_event_sender);

	let (window_size_sender, window_size_receiver) = std::sync::mpsc::channel();

	let cef_handler = cef::CefHandler::new(wgpu_context.clone(), app_event_scheduler.clone(), window_size_receiver);
	let cef_context = match cef_context_builder.initialize(cef_handler) {
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

	let mut app = App::new(Box::new(cef_context), window_size_sender, wgpu_context, app_event_receiver, app_event_scheduler);

	event_loop.run_app(&mut app).unwrap();
}

async fn init_wgpu_context() -> WgpuContext {
	// TODO: make this configurable via cli flags instead
	let adapter_override = std::env::var("GRAPHITE_WGPU_ADAPTER").ok().map(|s| usize::from_str_radix(&s, 10).ok()).flatten();

	let instance_descriptor = wgpu::InstanceDescriptor {
		backends: wgpu::Backends::all(),
		..Default::default()
	};
	let instance = wgpu::Instance::new(&instance_descriptor);

	let mut adapters = instance.enumerate_adapters(wgpu::Backends::all());

	// TODO: add a cli flag to list adapters and exit instead of always printing
	let adapters_fmt = adapters
		.iter()
		.enumerate()
		.map(|(i, a)| {
			let info = a.get_info();
			format!(
				"\nAdapter {}:\n  Name: {}\n  Backend: {:?}\n  Driver: {}\n  Device ID: {}\n  Vendor ID: {}",
				i, info.name, info.backend, info.driver, info.device, info.vendor
			)
		})
		.collect::<Vec<_>>()
		.join("\n");
	println!("\nAvailable wgpu adapters:\n {}\n", adapters_fmt);

	let adapter_index = if let Some(index) = adapter_override
		&& index < adapters.len()
	{
		index
	} else if cfg!(target_os = "windows") {
		match adapters.iter().enumerate().find(|(_, a)| a.get_info().backend == wgpu::Backend::Dx12) {
			Some((index, _)) => index,
			None => 0,
		}
	} else {
		0 // Same behavior as requests adapter
	};

	tracing::info!("Using WGPU adapter {adapter_index}");

	let adapter = adapters.remove(adapter_index);

	WgpuContext::new_with_instance_and_adapter(instance, adapter)
		.await
		.expect("Failed to create WGPU context with specified adapter")
}
