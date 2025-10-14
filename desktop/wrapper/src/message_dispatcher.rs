use graphite_editor::application::Editor;
use std::collections::VecDeque;

use super::handle_desktop_wrapper_message::handle_desktop_wrapper_message;
use super::intercept_editor_message::intercept_editor_message;
use super::intercept_frontend_message::intercept_frontend_message;
use super::messages::{DesktopFrontendMessage, DesktopWrapperMessage, EditorMessage};

pub(crate) struct DesktopWrapperMessageDispatcher<'a> {
	editor: &'a mut Editor,
	desktop_wrapper_message_queue: VecDeque<DesktopWrapperMessage>,
	editor_message_queue: Vec<EditorMessage>,
	responses: Vec<DesktopFrontendMessage>,
}

impl<'a> DesktopWrapperMessageDispatcher<'a> {
	pub(crate) fn new(editor: &'a mut Editor) -> Self {
		Self {
			editor,
			desktop_wrapper_message_queue: VecDeque::new(),
			editor_message_queue: Vec::new(),
			responses: Vec::new(),
		}
	}

	pub(crate) fn execute(mut self) -> Vec<DesktopFrontendMessage> {
		self.process_queue();
		self.responses
	}

	pub(crate) fn queue_desktop_wrapper_message(&mut self, message: DesktopWrapperMessage) {
		self.desktop_wrapper_message_queue.push_back(message);
	}

	pub(super) fn queue_editor_message(&mut self, message: EditorMessage) {
		if let Some(message) = intercept_editor_message(self, message) {
			self.editor_message_queue.push(message);
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
			self.queue_editor_message(message);
		}
	}

	fn process_queue(&mut self) {
		let mut frontend_messages = Vec::new();

		while !self.desktop_wrapper_message_queue.is_empty() || !self.editor_message_queue.is_empty() {
			while let Some(message) = self.desktop_wrapper_message_queue.pop_front() {
				handle_desktop_wrapper_message(self, message);
			}
			let current_frontend_messages = self
				.editor
				.handle_message(EditorMessage::Batched {
					messages: std::mem::take(&mut self.editor_message_queue).into_boxed_slice(),
				})
				.into_iter()
				.filter_map(|m| intercept_frontend_message(self, m));
			frontend_messages.extend(current_frontend_messages);
		}

		if !frontend_messages.is_empty() {
			self.respond(DesktopFrontendMessage::ToWeb(frontend_messages));
		}
	}
}
