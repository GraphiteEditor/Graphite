use std::fmt::Debug;
use std::process::exit;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tracing_subscriber::EnvFilter;
use winit::event_loop::{EventLoop, EventLoopProxy};

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

#[derive(Clone)]
struct CefHandler {
	window_size_receiver: Arc<Mutex<WindowSizeReceiver>>,
	event_loop_proxy: EventLoopProxy<CustomEvent>,
}
struct WindowSizeReceiver {
	receiver: Receiver<WindowSize>,
	window_size: WindowSize,
}
impl WindowSizeReceiver {
	fn new(window_size_receiver: Receiver<WindowSize>) -> Self {
		Self {
			window_size: WindowSize { width: 1, height: 1 },
			receiver: window_size_receiver,
		}
	}
}
impl CefHandler {
	fn new(window_size_receiver: Receiver<WindowSize>, event_loop_proxy: EventLoopProxy<CustomEvent>) -> Self {
		Self {
			window_size_receiver: Arc::new(Mutex::new(WindowSizeReceiver::new(window_size_receiver))),
			event_loop_proxy,
		}
	}
}

impl cef::CefEventHandler for CefHandler {
	fn window_size(&self) -> cef::WindowSize {
		let Ok(mut guard) = self.window_size_receiver.lock() else {
			tracing::error!("Failed to lock window_size_receiver");
			return cef::WindowSize::new(1, 1);
		};
		let WindowSizeReceiver { receiver, window_size } = &mut *guard;
		for new_window_size in receiver.try_iter() {
			*window_size = new_window_size;
		}
		*window_size
	}
	fn draw(&self, frame_buffer: FrameBuffer) {
		let _ = self.event_loop_proxy.send_event(CustomEvent::UiUpdate(frame_buffer));
	}

	fn schedule_cef_message_loop_work(&self, scheduled_time: std::time::Instant) {
		let _ = self.event_loop_proxy.send_event(CustomEvent::ScheduleBrowserWork(scheduled_time));
	}
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

	let cef_context = match cef_context.init(CefHandler::new(recv, event_loop.create_proxy())) {
		Ok(c) => c,
		Err(cef::InitError::InitializationFailed) => {
			tracing::error!("Cef initialization failed");
			exit(1);
		}
	};

	tracing::info!("Cef initialized successfully");

	let mut winit_app = WinitApp::new(cef_context, send);

	event_loop.run_app(&mut winit_app).unwrap();
}
