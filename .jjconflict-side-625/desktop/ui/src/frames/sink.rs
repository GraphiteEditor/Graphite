use ipc_channel::ipc::IpcSender;
use std::sync::Mutex;

use crate::remote::messages::HostControlMessage;

pub(super) struct FrameSink {
	state: Mutex<FrameSinkState>,
}

#[derive(Default)]
struct FrameSinkState {
	newest_installed: u64,
	last_acked: u64,
}

impl FrameSink {
	pub(super) fn new() -> Self {
		Self {
			state: Mutex::new(FrameSinkState::default()),
		}
	}

	pub(super) fn deliver(&self, sender: &IpcSender<HostControlMessage>, seq: u64, install: impl FnOnce() -> bool) {
		let Ok(mut state) = self.state.lock() else {
			tracing::error!("Failed to lock the frame sink");
			return;
		};

		if seq > 1 && seq - 1 > state.last_acked {
			if let Err(e) = sender.send(HostControlMessage::FrameAck { seq: seq - 1 }) {
				tracing::debug!("Failed to ack superseded frames to CEF host: {e}");
			}
			state.last_acked = seq - 1;
		}
		if seq > state.newest_installed && install() {
			state.newest_installed = seq;
		}
		if seq > state.last_acked {
			if let Err(e) = sender.send(HostControlMessage::FrameAck { seq }) {
				tracing::debug!("Failed to ack frame to CEF host: {e}");
			}
			state.last_acked = seq;
		}
	}
}
