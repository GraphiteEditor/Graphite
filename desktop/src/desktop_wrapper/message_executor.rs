use graphite_editor::{application::Editor, messages::prelude::Message};
use std::collections::VecDeque;

use super::handle_desktop_wrapper_message::handle_desktop_wrapper_message;
use super::intercept_frontend_message::intercept_frontend_message;
use super::intercept_message::intercept_message;
use super::messages::{DesktopFrontendMessage, DesktopWrapperMessage};

pub(crate) struct DesktopWrapperMessageExecutor<'a> {
	editor: &'a mut Editor,
	queue: VecDeque<DesktopWrapperMessage>,
	messages: Vec<Message>,
	responses: Vec<DesktopFrontendMessage>,
}

impl<'a> DesktopWrapperMessageExecutor<'a> {
	pub(crate) fn new(editor: &'a mut Editor) -> Self {
		Self {
			editor,
			queue: VecDeque::new(),
			messages: Vec::new(),
			responses: Vec::new(),
		}
	}

	pub(crate) fn execute(mut self) -> Vec<DesktopFrontendMessage> {
		self.process_queue();
		self.responses
	}

	pub(crate) fn queue(&mut self, message: DesktopWrapperMessage) {
		self.queue.push_back(message);
	}

	pub(super) fn queue_message(&mut self, message: Message) {
		if let Some(message) = intercept_message(self, message) {
			self.messages.push(message);
		}
	}

	pub(super) fn respond(&mut self, response: DesktopFrontendMessage) {
		self.responses.push(response);
	}

	pub(super) fn poll_node_graph_evaluation(&mut self) {
		let mut responses = VecDeque::new();
		if let Err(e) = self.editor.poll_node_graph_evaluation(&mut responses) {
			if e != "No active document" {
				tracing::error!("Error poling node graph: {}", e);
			}
		}
		while let Some(message) = responses.pop_front() {
			self.queue_message(message);
		}
	}

	fn process_queue(&mut self) {
		while !self.queue.is_empty() || !self.messages.is_empty() {
			while let Some(message) = self.queue.pop_front() {
				handle_desktop_wrapper_message(self, message);
			}
			let frontend_messages = self
				.editor
				.handle_message(Message::Batched {
					messages: std::mem::take(&mut self.messages).into_boxed_slice(),
				})
				.into_iter()
				.filter_map(|m| intercept_frontend_message(self, m))
				.collect::<Vec<_>>();
			self.respond(DesktopFrontendMessage::ToWeb(ron::to_string(&frontend_messages).unwrap().into_bytes()));
		}
	}
}
