use ipc_channel::ipc::{IpcReceiver, IpcSender};
use std::sync::{Arc, Mutex};

use super::HostConfig;
use super::messages::{EventMessage, HostControlMessage};
use crate::context::{CefContext, CefContextHandle};
use crate::delegate::BrowserDelegate;
use crate::frames::FrameStreamer;
#[cfg(feature = "accelerated_paint")]
use crate::frames::plane::PlaneSender;
use crate::frames::sequence::SequenceState;
#[cfg(target_os = "macos")]
use crate::platform::mac;

pub(crate) fn run() {
	// Ignore SIGINT, the controlling process is responsible for shutting down the host
	#[cfg(any(target_os = "linux", target_os = "macos"))]
	unsafe {
		libc::signal(libc::SIGINT, libc::SIG_IGN);
	}

	let args: Vec<String> = std::env::args().collect();
	let config = HostConfig::from_args(&args).expect("CEF host started without a valid host config argument");
	let acceleration_requested = config.acceleration;

	#[cfg(target_os = "macos")]
	mac::spawn_parent_watchdog(config.main_pid);

	let bootstrap = IpcSender::<EventMessage>::connect(config.server.clone()).expect("Failed to connect to the main process bootstrap server");
	let event_sender = Arc::new(Mutex::new(bootstrap));
	let (control_sender, control_receiver) = ipc_channel::ipc::channel::<HostControlMessage>().expect("Failed to create control channel");

	#[cfg(feature = "accelerated_paint")]
	let plane = if acceleration_requested { PlaneSender::from_config(&config, event_sender.clone()) } else { None };
	#[cfg(feature = "accelerated_paint")]
	let acceleration = plane.is_some();
	#[cfg(not(feature = "accelerated_paint"))]
	let acceleration = {
		if acceleration_requested {
			tracing::error!("UI acceleration requested but the accelerated_paint feature is disabled; using software frames");
		}
		false
	};

	event_sender
		.lock()
		.expect("The host message sender cannot be poisoned before threads exist")
		.send(EventMessage::Hello {
			pid: std::process::id(),
			control_sender,
			acceleration,
		})
		.expect("Failed to send Hello to the main process");

	let sequence = Arc::new(SequenceState::new());
	let frames = FrameStreamer::new(
		event_sender.clone(),
		sequence.clone(),
		#[cfg(feature = "accelerated_paint")]
		plane,
	);
	let (view_info_sender, view_info_receiver) = std::sync::mpsc::channel();
	let delegate = BrowserDelegate::new(event_sender.clone(), view_info_receiver);

	let context = match CefContext::create(delegate, frames, view_info_sender, acceleration) {
		Ok(context) => {
			if let Ok(sender) = event_sender.lock() {
				let _ = sender.send(EventMessage::BrowserCreated);
			}
			context
		}
		Err(e) => {
			tracing::error!("CEF initialization failed in host process: {e}");
			if let Ok(sender) = event_sender.lock() {
				let _ = sender.send(EventMessage::InitFailed(e));
			}
			std::process::exit(1);
		}
	};

	let outcome = context.run(move |handle| control_loop(&control_receiver, &handle, sequence.as_ref()));

	match outcome {
		ControlOutcome::Shutdown => {
			tracing::debug!("Shut down CEF host");
			if let Ok(sender) = event_sender.lock() {
				let _ = sender.send(EventMessage::ShutdownComplete);
			}
		}
		ControlOutcome::Disconnected => std::process::exit(0),
	}
}

enum ControlOutcome {
	Shutdown,
	Disconnected,
}

fn control_loop(receiver: &IpcReceiver<HostControlMessage>, context: &CefContextHandle, sequence: &SequenceState) -> ControlOutcome {
	loop {
		match receiver.recv() {
			Ok(HostControlMessage::Input(events)) => context.apply_input(events),
			Ok(HostControlMessage::UpdateViewInfo(update)) => context.update_view_info(update),
			Ok(HostControlMessage::RefreshViewInfo) => context.refresh_view_info(),
			Ok(HostControlMessage::SendWebMessage(message)) => context.send_web_message(message),
			Ok(HostControlMessage::FrameAck { seq }) => sequence.ack(seq),
			Ok(HostControlMessage::Shutdown) => return ControlOutcome::Shutdown,
			Err(ipc_channel::IpcError::Io(ref io)) if io.kind() == std::io::ErrorKind::Interrupted => {}
			Err(e) => {
				tracing::warn!("Control channel closed ({e:?}), shutting down CEF host");
				return ControlOutcome::Disconnected;
			}
		}
	}
}
