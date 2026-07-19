use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::remote::messages::HostControlMessage;
use crate::remote::spawn::HostHandle;

mod consts;
mod context;
mod delegate;
mod dirs;
mod events;
mod frames;
mod input;
mod internal;
mod ipc;
mod platform;
mod remote;
mod resources;
mod utility;
mod view;

pub use consts::{PINCH_ZOOM_SPEED, SCROLL_LINE_HEIGHT, SCROLL_LINE_WIDTH, SCROLL_SPEED_X, SCROLL_SPEED_Y};

pub struct UiContext<S: Stage = Started> {
	inner: S::ContextData,
}

impl UiContext<Setup> {
	pub fn setup() -> UiSetupResult {
		#[cfg(target_os = "macos")]
		ipc_channel::set_bootstrap_prefix(consts::IPC_BOOTSTRAP_PREFIX);

		let raw_args: Vec<String> = std::env::args().collect();
		if raw_args.iter().any(|arg| arg.starts_with(consts::BROWSER_HOST_CONFIG_FLAG)) {
			remote::host::run();
			return UiSetupResult::Helper(ExitCode::SUCCESS);
		}

		if raw_args.iter().any(|arg| arg.starts_with("--type=")) {
			return UiSetupResult::Helper(run_helper());
		}
		UiSetupResult::Ready(UiContext { inner: () })
	}

	pub fn start(self, config: UiConfig) -> Result<UiContext<Started>, UiError> {
		let acceleration = platform::accelerated_paint(matches!(config.acceleration, Acceleration::Disabled));
		let handle = remote::spawn::spawn_host(acceleration)?;
		Ok(UiContext { inner: Arc::new(handle) })
	}
}

#[must_use]
pub enum UiSetupResult {
	Ready(UiContext<Setup>),
	Failed,
	Helper(ExitCode),
}

impl UiContext<Started> {
	pub fn instance(&self, device: &wgpu::Device, queue: &wgpu_sync::Queue) -> Result<UiInstance, UiError> {
		let surface = frames::FrameSurface::new(device.clone(), queue.clone());

		let (queue, events) = events::EventQueue::new();
		let shutdown_complete = remote::spawn::start_instance(&self.inner, surface, queue.clone())?;

		Ok(UiInstance {
			inner: Arc::new(UiInstanceInner {
				host: self.inner.clone(),
				input: Mutex::new(input::InputState::default()),
				events: Mutex::new(events),
				queue,
				shutdown_complete: Mutex::new(shutdown_complete),
				shutdown_started: AtomicBool::new(false),
			}),
		})
	}
}

impl Clone for UiContext<Started> {
	fn clone(&self) -> Self {
		UiContext { inner: self.inner.clone() }
	}
}

pub enum Setup {}
pub enum Started {}

#[expect(private_bounds)]
pub trait Stage: Sealed {}
impl Stage for Setup {}
impl Stage for Started {}
trait Sealed {
	type ContextData;
}
impl Sealed for Setup {
	type ContextData = ();
}
impl Sealed for Started {
	type ContextData = Arc<HostHandle>;
}

pub fn temp_dir_root() -> std::path::PathBuf {
	dirs::app_tmp_dir()
}

pub fn run_helper() -> ExitCode {
	context::execute_helper_process()
}

pub struct UiConfig {
	pub acceleration: Acceleration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acceleration {
	Auto,
	Disabled,
}

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum UiError {
	#[error("failed to bootstrap the UI backend: {0}")]
	Bootstrap(String),
	#[error("failed to spawn the UI backend host process: {0}")]
	Spawn(std::io::Error),
	#[error("the UI backend host process exited during startup: {0}")]
	HostExited(String),
	#[error("timed out waiting for the UI backend host process to connect")]
	HandshakeTimeout,
	#[error("UI backend handshake failed: {0}")]
	Handshake(String),
	#[error("the UI runtime already drives an instance")]
	InstanceLimit,
}

pub struct UiInstance {
	inner: Arc<UiInstanceInner>,
}

pub(crate) struct UiInstanceInner {
	host: Arc<HostHandle>,
	input: Mutex<input::InputState>,
	events: Mutex<Receiver<UiEvent>>,
	queue: events::EventQueue,
	shutdown_complete: Mutex<Receiver<()>>,
	shutdown_started: AtomicBool,
}

impl UiInstance {
	pub fn send(&self, command: UiCommand) {
		let shared = &self.inner;
		match command {
			UiCommand::Input(event) => {
				let events = {
					let Ok(mut input) = shared.input.lock() else {
						tracing::error!("Failed to lock the input state");
						return;
					};
					input::translate(&mut input, &event)
				};
				if !events.is_empty() {
					shared.host.send(HostControlMessage::Input(events));
				}
			}
			UiCommand::Resized { width, height } => shared.host.send(HostControlMessage::UpdateViewInfo(view::ViewInfoUpdate::Size { width, height })),
			UiCommand::ScaleChanged(scale) => shared.host.send(HostControlMessage::UpdateViewInfo(view::ViewInfoUpdate::Scale(scale))),
			UiCommand::Refresh => shared.host.send(HostControlMessage::RefreshViewInfo),
			UiCommand::Message(message) => shared.host.send(HostControlMessage::SendWebMessage(message)),
		}
	}

	pub fn recv(&self) -> Option<UiEvent> {
		let shared = &self.inner;
		let Ok(receiver) = shared.events.lock() else {
			return None;
		};
		loop {
			match receiver.recv_timeout(Duration::from_millis(100)) {
				Ok(event) => return Some(event),
				Err(RecvTimeoutError::Timeout) => {
					if shared.queue.is_terminated() {
						return receiver.try_recv().ok();
					}
				}
				Err(RecvTimeoutError::Disconnected) => return None,
			}
		}
	}

	pub fn shutdown(&self) {
		self.inner.shutdown();
	}
}

impl Clone for UiInstance {
	fn clone(&self) -> Self {
		UiInstance { inner: self.inner.clone() }
	}
}

impl UiInstanceInner {
	fn shutdown(&self) {
		if self.shutdown_started.swap(true, Ordering::SeqCst) {
			return;
		}
		if let Ok(receiver) = self.shutdown_complete.lock() {
			self.host.shutdown(&receiver);
		}
		self.queue.mark_terminated();
	}
}

impl Drop for UiInstanceInner {
	fn drop(&mut self) {
		self.shutdown();
	}
}

#[derive(Debug)]
pub enum UiCommand {
	Input(winit::event::WindowEvent),
	Resized { width: u32, height: u32 },
	ScaleChanged(f64),
	Refresh,
	Message(Vec<u8>),
}

#[derive(Debug)]
pub enum UiEvent {
	Ready,
	Frame(wgpu::Texture),
	Cursor(Cursor),
	Message(Vec<u8>),
	Failure(String),
	Crashed,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Cursor {
	Icon(winit::cursor::CursorIcon),
	Custom { rgba: Vec<u8>, width: u16, height: u16, hotspot_x: u16, hotspot_y: u16 },
	None,
}

impl From<winit::cursor::CursorIcon> for Cursor {
	fn from(icon: winit::cursor::CursorIcon) -> Self {
		Cursor::Icon(icon)
	}
}
