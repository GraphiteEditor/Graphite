use ipc_channel::ipc::{IpcOneShotServer, IpcReceiver, IpcSender};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::HostConfig;
use super::messages::{EventMessage, HostControlMessage};
use crate::consts::{HOST_HELLO_TIMEOUT, HOST_SHUTDOWN_TIMEOUT};
use crate::events::EventQueue;
use crate::frames::FrameSurface;
#[cfg(feature = "accelerated_paint")]
use crate::frames::plane;
use crate::frames::receive::{FrameConsumer, PendingFrame, SegmentTable};
#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::platform;
use crate::{UiError, UiEvent};

pub(crate) struct HostHandle {
	sender: IpcSender<HostControlMessage>,
	receivers: Mutex<Option<InstanceReceivers>>,
	child: Arc<Mutex<Child>>,
	shutting_down: Arc<AtomicBool>,
	died_reported: Arc<AtomicBool>,
	#[cfg_attr(any(not(feature = "accelerated_paint"), target_os = "windows"), expect(dead_code))]
	host_acceleration: bool,
	#[cfg(target_os = "windows")]
	_job: Option<platform::win::KillOnCloseJob>,
}

struct InstanceReceivers {
	events: IpcReceiver<EventMessage>,
	#[cfg(all(any(target_os = "linux", target_os = "macos"), feature = "accelerated_paint"))]
	frame_plane: Option<plane::PlaneReceiver>,
}

impl HostHandle {
	pub(crate) fn send(&self, message: HostControlMessage) {
		if let Err(e) = self.sender.send(message) {
			tracing::debug!("Failed to send message to CEF host: {e}");
		}
	}

	pub(crate) fn shutdown(&self, shutdown_complete_receiver: &mpsc::Receiver<()>) {
		self.shutting_down.store(true, Ordering::SeqCst);
		let deadline = Instant::now() + HOST_SHUTDOWN_TIMEOUT;

		if self.sender.send(HostControlMessage::Shutdown).is_ok() {
			match shutdown_complete_receiver.recv_timeout(HOST_SHUTDOWN_TIMEOUT) {
				Ok(()) => tracing::debug!("CEF host completed shutdown"),
				Err(RecvTimeoutError::Timeout) => tracing::warn!("Timed out waiting for the CEF host to shut down"),
				Err(RecvTimeoutError::Disconnected) => tracing::debug!("CEF host connection closed during shutdown"),
			}
		}

		loop {
			match self.child.lock() {
				Ok(mut child) => match child.try_wait() {
					Ok(None) => {}
					Ok(Some(_)) | Err(_) => return,
				},
				Err(_) => return,
			}
			if Instant::now() >= deadline {
				break;
			}
			std::thread::sleep(Duration::from_millis(25));
		}

		tracing::warn!("CEF host did not exit in time, killing it");
		if let Ok(mut child) = self.child.lock() {
			let _ = child.kill();
			let _ = child.wait();
		}
	}
}

impl Drop for HostHandle {
	fn drop(&mut self) {
		if !self.shutting_down.load(Ordering::SeqCst) {
			let _ = self.sender.send(HostControlMessage::Shutdown);
		}
	}
}

pub(crate) fn spawn_host(acceleration: bool) -> Result<HostHandle, UiError> {
	let (server, server_name) = IpcOneShotServer::<EventMessage>::new().map_err(|e| UiError::Bootstrap(format!("failed to create the bootstrap server: {e}")))?;

	let executable = std::env::current_exe().map_err(|e| UiError::Bootstrap(format!("failed to get the current executable path: {e}")))?;
	let mut command = Command::new(executable);

	#[cfg_attr(not(all(any(target_os = "linux", target_os = "macos"), feature = "accelerated_paint")), expect(unused_mut))]
	let mut config = HostConfig {
		server: server_name,
		main_pid: std::process::id(),
		acceleration,
		#[cfg(target_os = "linux")]
		frame_socket_fd: None,
		#[cfg(target_os = "macos")]
		frame_service: None,
	};

	#[cfg(all(target_os = "linux", feature = "accelerated_paint"))]
	let frame_socket = if acceleration {
		match plane::socketpair() {
			Ok((main_end, host_end)) => {
				config.frame_socket_fd = Some(plane::FRAME_SOCKET_CHILD_FD);
				Some((main_end, host_end))
			}
			Err(e) => {
				tracing::error!("Failed to create the accelerated frame socket, falling back to software frames: {e}");
				None
			}
		}
	} else {
		None
	};

	#[cfg(all(target_os = "macos", feature = "accelerated_paint"))]
	let frame_service = if acceleration {
		let name = format!("art.graphite.Graphite.cef-frames.{}.{:x}", std::process::id(), rand::random::<u64>());
		match plane::create_service(&name) {
			Ok(port) => {
				config.frame_service = Some(name);
				Some(port)
			}
			Err(e) => {
				tracing::error!("Failed to create the accelerated frame service, falling back to software frames: {e}");
				None
			}
		}
	} else {
		None
	};

	command.arg(config.to_arg());

	#[cfg(target_os = "linux")]
	platform::linux::setup_command(
		&mut command,
		#[cfg(feature = "accelerated_paint")]
		frame_socket.as_ref().map(|(_, host_end)| {
			use std::os::fd::AsRawFd;
			host_end.as_raw_fd()
		}),
	);

	let mut child = command.spawn().map_err(UiError::Spawn)?;

	#[cfg(target_os = "windows")]
	let job = match platform::win::KillOnCloseJob::assign(&child) {
		Ok(job) => Some(job),
		Err(e) => {
			tracing::error!("Failed to assign the CEF host to a job object (orphan prevention degraded): {e}");
			None
		}
	};

	#[cfg(all(target_os = "linux", feature = "accelerated_paint"))]
	let frame_plane = frame_socket.map(|(main_end, host_end)| {
		drop(host_end);
		plane::PlaneReceiver::new(main_end)
	});

	let (hello_tx, hello_rx) = mpsc::channel();
	let accept_thread = std::thread::Builder::new().name("cef-host-accept".to_string()).spawn(move || {
		let _ = hello_tx.send(server.accept());
	});
	if let Err(e) = accept_thread {
		let _ = child.kill();
		let _ = child.wait();
		return Err(UiError::Bootstrap(format!("failed to spawn the host accept thread: {e}")));
	}

	let deadline = Instant::now() + HOST_HELLO_TIMEOUT;
	let (event_receiver, hello) = loop {
		match hello_rx.recv_timeout(Duration::from_millis(100)) {
			Ok(Ok(accepted)) => break accepted,
			Ok(Err(e)) => {
				let _ = child.kill();
				let _ = child.wait();
				return Err(UiError::Handshake(format!("failed to accept the host connection: {e}")));
			}
			Err(RecvTimeoutError::Timeout) => {
				if let Ok(Some(status)) = child.try_wait() {
					return Err(UiError::HostExited(status.to_string()));
				}
				if Instant::now() >= deadline {
					let _ = child.kill();
					let _ = child.wait();
					return Err(UiError::HandshakeTimeout);
				}
			}
			Err(RecvTimeoutError::Disconnected) => {
				let _ = child.kill();
				let _ = child.wait();
				return Err(UiError::Handshake("the accept thread disappeared".to_string()));
			}
		}
	};

	let EventMessage::Hello {
		pid,
		control_sender,
		acceleration: host_acceleration,
	} = hello
	else {
		let _ = child.kill();
		let _ = child.wait();
		return Err(UiError::Handshake("the first message from the host was not Hello".to_string()));
	};
	tracing::info!("CEF host process connected (pid {pid})");
	if acceleration && !host_acceleration {
		tracing::warn!("UI acceleration was requested but the CEF host could not set up its frame plane; falling back to software frames");
	}

	Ok(HostHandle {
		sender: control_sender,
		receivers: Mutex::new(Some(InstanceReceivers {
			events: event_receiver,
			#[cfg(all(target_os = "linux", feature = "accelerated_paint"))]
			frame_plane,
			#[cfg(all(target_os = "macos", feature = "accelerated_paint"))]
			frame_plane: frame_service.map(plane::PlaneReceiver::new),
		})),
		child: Arc::new(Mutex::new(child)),
		shutting_down: Arc::new(AtomicBool::new(false)),
		died_reported: Arc::new(AtomicBool::new(false)),
		host_acceleration,
		#[cfg(target_os = "windows")]
		_job: job,
	})
}

pub(crate) fn start_instance(handle: &HostHandle, surface: FrameSurface, events: EventQueue) -> Result<mpsc::Receiver<()>, UiError> {
	let receive_side = match handle.receivers.lock() {
		Ok(mut receive_side) => receive_side.take(),
		Err(_) => None,
	};
	let Some(receive_side) = receive_side else {
		return Err(UiError::InstanceLimit);
	};

	let (shutdown_complete_sender, shutdown_complete_receiver) = mpsc::channel();
	let consumer = FrameConsumer::new(surface, events.clone(), handle.sender.clone());

	#[cfg(all(any(target_os = "linux", target_os = "macos"), feature = "accelerated_paint"))]
	if let Some(receiver) = receive_side.frame_plane
		&& handle.host_acceleration
	{
		let consumer = consumer.clone();
		std::thread::Builder::new()
			.name("cef-frames".to_string())
			.spawn(move || crate::frames::receive::plane_receiver_loop(receiver, consumer))
			.map_err(|e| UiError::Bootstrap(format!("failed to spawn the frame receiver thread: {e}")))?;
	}

	{
		let receiver = receive_side.events;
		let shutting_down = handle.shutting_down.clone();
		let died_reported = handle.died_reported.clone();
		let events = events.clone();
		std::thread::Builder::new()
			.name("cef-host".to_string())
			.spawn(move || event_receiver_loop(receiver, consumer, events, shutting_down, died_reported, shutdown_complete_sender))
			.map_err(|e| UiError::Bootstrap(format!("failed to spawn the host event receiver thread: {e}")))?;
	}

	{
		let child = handle.child.clone();
		let shutting_down = handle.shutting_down.clone();
		let died_reported = handle.died_reported.clone();
		let events = events.clone();
		std::thread::Builder::new()
			.name("cef-host-supervisor".to_string())
			.spawn(move || {
				loop {
					let status = match child.lock() {
						Ok(mut child) => child.try_wait(),
						Err(_) => return,
					};
					match status {
						Ok(None) => std::thread::sleep(Duration::from_millis(100)),
						Ok(Some(status)) => {
							if shutting_down.load(Ordering::SeqCst) {
								tracing::debug!("CEF host exited during shutdown: {status}");
							} else {
								report_host_died(&died_reported, &events, &format!("CEF host process exited unexpectedly: {status}"));
							}
							return;
						}
						Err(_) => return,
					}
				}
			})
			.map_err(|e| UiError::Bootstrap(format!("failed to spawn the host supervisor thread: {e}")))?;
	}

	Ok(shutdown_complete_receiver)
}

fn report_host_died(died_reported: &AtomicBool, events: &EventQueue, message: &str) {
	if died_reported.swap(true, Ordering::SeqCst) {
		return;
	}
	tracing::error!("{message}");
	events.terminate(UiEvent::Crashed);
}

fn event_receiver_loop(
	receiver: IpcReceiver<EventMessage>,
	consumer: FrameConsumer,
	events: EventQueue,
	shutting_down: Arc<AtomicBool>,
	died_reported: Arc<AtomicBool>,
	shutdown_complete_sender: mpsc::Sender<()>,
) {
	let mut newest_frame: Option<PendingFrame> = None;
	let mut segments = SegmentTable::new();

	loop {
		let message = match receiver.recv() {
			Ok(message) => message,
			Err(ipc_channel::IpcError::Io(ref io)) if io.kind() == std::io::ErrorKind::Interrupted => continue,
			Err(e) => {
				if !shutting_down.load(Ordering::SeqCst) {
					report_host_died(&died_reported, &events, &format!("Lost connection to the CEF host process: {e:?}"));
				}
				return;
			}
		};
		handle_message(message, &events, &shutting_down, &died_reported, &shutdown_complete_sender, &mut newest_frame, &mut segments);

		loop {
			match receiver.try_recv() {
				Ok(message) => handle_message(message, &events, &shutting_down, &died_reported, &shutdown_complete_sender, &mut newest_frame, &mut segments),
				Err(ipc_channel::TryRecvError::Empty) => break,
				Err(ipc_channel::TryRecvError::IpcError(ipc_channel::IpcError::Io(ref io))) if io.kind() == std::io::ErrorKind::Interrupted => break,
				Err(ipc_channel::TryRecvError::IpcError(e)) => {
					if !shutting_down.load(Ordering::SeqCst) {
						report_host_died(&died_reported, &events, &format!("Lost connection to the CEF host process: {e:?}"));
					}
					return;
				}
			}
		}

		if let Some(frame) = newest_frame.take() {
			consumer.deliver_pending(frame, &segments);
		}
	}
}

fn handle_message(
	message: EventMessage,
	events: &EventQueue,
	shutting_down: &AtomicBool,
	died_reported: &AtomicBool,
	shutdown_complete_sender: &mpsc::Sender<()>,
	newest_frame: &mut Option<PendingFrame>,
	segments: &mut SegmentTable,
) {
	match message {
		EventMessage::Hello { .. } => tracing::error!("Unexpected second Hello from the CEF host"),
		EventMessage::BrowserCreated => tracing::info!("CEF host created the browser"),
		EventMessage::InitFailed(e) => {
			tracing::error!("CEF initialization failed in the host process: {e}");
			died_reported.store(true, Ordering::SeqCst);
			events.terminate(UiEvent::Failure(e.to_string()));
		}
		EventMessage::WebCommunicationInitialized => events.send(UiEvent::Ready),
		EventMessage::WebMessage(message) => events.send(UiEvent::Message(message)),
		EventMessage::CursorChange(cursor) => events.send(UiEvent::Cursor(cursor)),
		EventMessage::AdvertiseFrameSegment { index, shm } => segments.advertise(index, shm),
		EventMessage::SoftwareFrame { seq, segment, width, height } => {
			let frame = PendingFrame::Software { seq, segment, width, height };
			if newest_frame.as_ref().is_none_or(|newest| newest.seq() < frame.seq()) {
				*newest_frame = Some(frame);
			}
		}
		#[cfg(all(target_os = "windows", feature = "accelerated_paint"))]
		EventMessage::AcceleratedFrame {
			seq,
			handle,
			width,
			height,
			format,
			content,
		} => {
			let frame = PendingFrame::Accelerated(plane::WireFrame::new(seq, handle, width, height, format, content));
			if newest_frame.as_ref().is_none_or(|newest| newest.seq() < frame.seq()) {
				*newest_frame = Some(frame);
			}
		}
		EventMessage::ShutdownComplete => {
			shutting_down.store(true, Ordering::SeqCst);
			let _ = shutdown_complete_sender.send(());
		}
	}
}
