use ipc_channel::ipc::IpcSender;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use super::remote::messages::EventMessage;
use super::view::{ViewInfo, ViewInfoReceiver, ViewInfoUpdate};
use crate::Cursor;

#[derive(Clone)]
pub(crate) struct BrowserDelegate(Arc<Inner>);

struct Inner {
	sender: Arc<Mutex<IpcSender<EventMessage>>>,
	view_info: Mutex<ViewInfoReceiver>,
}

impl BrowserDelegate {
	pub(crate) fn new(sender: Arc<Mutex<IpcSender<EventMessage>>>, view_info_receiver: Receiver<ViewInfoUpdate>) -> Self {
		Self(Arc::new(Inner {
			sender,
			view_info: Mutex::new(ViewInfoReceiver::new(view_info_receiver)),
		}))
	}

	fn send(&self, message: EventMessage) {
		let Ok(sender) = self.0.sender.lock() else {
			tracing::error!("Failed to lock host message sender");
			return;
		};
		if let Err(e) = sender.send(message) {
			tracing::debug!("Failed to send message to main process: {e}");
		}
	}

	pub(crate) fn view_info(&self) -> ViewInfo {
		let Ok(mut guard) = self.0.view_info.lock() else {
			tracing::error!("Failed to lock the view info mirror");
			return ViewInfo::new();
		};
		guard.current()
	}

	pub(crate) fn load_resource(&self, path: PathBuf) -> Option<crate::resources::Resource> {
		crate::resources::load(path)
	}

	pub(crate) fn cursor_change(&self, cursor: Cursor) {
		self.send(EventMessage::CursorChange(cursor));
	}

	pub(crate) fn initialized_web_communication(&self) {
		self.send(EventMessage::WebCommunicationInitialized);
	}

	pub(crate) fn receive_web_message(&self, message: &[u8]) {
		self.send(EventMessage::WebMessage(message.to_vec()));
	}
}
