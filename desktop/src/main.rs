use std::process::exit;
use std::time::{Duration, Instant};
use std::{fmt::Debug, thread};

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
	WorkCef,
	KeepProcessAliveWhenResizing(WindowSize),
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
	let event_loop_proxy = event_loop.create_proxy();

	let (window_size_sender, window_size_receiver) = std::sync::mpsc::channel();
	let (poll_cef_sender, poll_cef_receiver) = std::sync::mpsc::channel();

	let cef_context = match cef_context.init(cef::CefHandler::new(window_size_receiver, event_loop_proxy.clone(), poll_cef_sender)) {
		Ok(c) => c,
		Err(cef::InitError::InitializationFailed) => {
			tracing::error!("Cef initialization failed");
			exit(1);
		}
	};

	tracing::info!("Cef initialized successfully");
	let poll_cef_elp = event_loop_proxy.clone();
	let handle = thread::spawn(move || {
		loop {
			match poll_cef_receiver.recv_timeout(Duration::from_millis(100)) {
				Ok(scheduled_instant) => {
					if scheduled_instant > Instant::now() {
						thread::sleep(scheduled_instant - Instant::now());
					}
					let _ = poll_cef_elp.send_event(CustomEvent::WorkCef);
				}
				Err(_) => {
					let _ = poll_cef_elp.send_event(CustomEvent::WorkCef);
				}
			}
		}
	});

	let mut winit_app = WinitApp::new(cef_context, window_size_sender, event_loop_proxy.clone(), handle);

	event_loop.run_app(&mut winit_app).unwrap();
}
