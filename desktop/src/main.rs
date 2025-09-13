use std::process::exit;
use std::time::Instant;

use cef::CefHandler;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

pub(crate) mod consts;

mod cef;

mod native_window;

mod render;

mod app;
use app::WinitApp;

mod dirs;
mod persist;

use graphite_desktop_wrapper::messages::DesktopWrapperMessage;
use graphite_desktop_wrapper::{NodeGraphExecutionResult, WgpuContext};

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

	let wgpu_context = futures::executor::block_on(WgpuContext::new()).unwrap();

	let event_loop = EventLoop::new().unwrap();
	let (custom_event_sender, custom_event_receiver) = std::sync::mpsc::channel();
	let custom_event_scheduler = event_loop.create_scheduler(custom_event_sender);

	let (window_size_sender, window_size_receiver) = std::sync::mpsc::channel();

	let cef_handler = cef::CefHandler::new(wgpu_context.clone(), custom_event_scheduler.clone(), window_size_receiver);
	let cef_context = match cef_context_builder.initialize(cef_handler) {
		Ok(c) => c,
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

	tracing::info!("CEF initialized successfully");

	let mut winit_app = WinitApp::new(Box::new(cef_context), window_size_sender, wgpu_context, custom_event_receiver, custom_event_scheduler);

	event_loop.run_app(&mut winit_app).unwrap();
}

pub(crate) enum CustomEvent {
	UiUpdate(wgpu::Texture),
	ScheduleBrowserWork(Instant),
	WebCommunicationInitialized,
	DesktopWrapperMessage(DesktopWrapperMessage),
	NodeGraphExecutionResult(NodeGraphExecutionResult),
	CloseWindow,
}

#[derive(Clone)]
struct CustomEventScheduler {
	proxy: winit::event_loop::EventLoopProxy,
	sender: std::sync::mpsc::Sender<CustomEvent>,
}
impl CustomEventScheduler {
	fn schedule(&self, event: CustomEvent) {
		let _ = self.sender.send(event);
		self.proxy.wake_up();
	}
}

trait CustomEventEventLoopExt {
	fn create_scheduler(&self, sender: std::sync::mpsc::Sender<CustomEvent>) -> CustomEventScheduler;
}
impl CustomEventEventLoopExt for winit::event_loop::EventLoop {
	fn create_scheduler(&self, sender: std::sync::mpsc::Sender<CustomEvent>) -> CustomEventScheduler {
		CustomEventScheduler { proxy: self.create_proxy(), sender }
	}
}
