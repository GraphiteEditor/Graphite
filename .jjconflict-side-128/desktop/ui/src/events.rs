use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

use crate::UiEvent;

#[derive(Clone)]
pub(crate) struct EventQueue {
	sender: std::sync::mpsc::Sender<UiEvent>,
	terminated: Arc<AtomicBool>,
}

impl EventQueue {
	pub(crate) fn new() -> (Self, Receiver<UiEvent>) {
		let (sender, receiver) = std::sync::mpsc::channel();
		(
			Self {
				sender,
				terminated: Arc::new(AtomicBool::new(false)),
			},
			receiver,
		)
	}

	pub(crate) fn send(&self, event: UiEvent) {
		let _ = self.sender.send(event);
	}

	pub(crate) fn terminate(&self, event: UiEvent) {
		let _ = self.sender.send(event);
		self.terminated.store(true, Ordering::SeqCst);
	}

	pub(crate) fn mark_terminated(&self) {
		self.terminated.store(true, Ordering::SeqCst);
	}

	pub(crate) fn is_terminated(&self) -> bool {
		self.terminated.load(Ordering::SeqCst)
	}
}
