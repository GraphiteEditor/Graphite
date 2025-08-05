use std::process::exit;
use std::time::Instant;
use std::{fmt::Debug, time::Duration};

use graphite_editor::messages::prelude::Message;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

mod cef;
use cef::{Setup, WindowSize};

mod render;
use render::WgpuContext;

mod app;
use app::WinitApp;

mod dirs;

mod dialogs;

#[derive(Debug)]
pub(crate) enum CustomEvent {
	UiUpdate(wgpu::Texture),
	ScheduleBrowserWork(Instant),
	DispatchMessage(Message),
	MessageReceived(Message),
	NodeGraphRan(Option<wgpu::Texture>),
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

	let rendering_loop_proxy = event_loop.create_proxy();
	let target_fps = 60;
	std::thread::spawn(move || {
		loop {
			let last_render = Instant::now();
			let (has_run, texture) = futures::executor::block_on(graphite_editor::node_graph_executor::run_node_graph());
			if has_run {
				let _ = rendering_loop_proxy.send_event(CustomEvent::NodeGraphRan(texture.map(|t| (*t.texture).clone())));
			}
			let frame_time = Duration::from_secs_f32((target_fps as f32).recip());
			let sleep = last_render + frame_time - Instant::now();
			std::thread::sleep(sleep);
		}
	});

	let mut winit_app = WinitApp::new(cef_context, window_size_sender, wgpu_context, event_loop.create_proxy());

	event_loop.run_app(&mut winit_app).unwrap();
}
