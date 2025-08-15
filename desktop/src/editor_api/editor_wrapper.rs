use graph_craft::wasm_application_io::WasmApplicationIo;
use graphite_editor::{application::Editor, messages::prelude::Message};
use std::collections::VecDeque;

use crate::editor_api::messages::{EditorMessage, NativeMessage};
use crate::editor_api::{EditorApi, WgpuContext};

#[path = "handle_editor_message.rs"]
mod handle_editor_message;

#[path = "intercept_frontend_message.rs"]
mod intercept_frontend_message;
#[path = "intercept_message.rs"]
mod intercept_message;

pub struct EditorWrapper {
	editor: Editor,
	queue: VecDeque<QueuedMessage>,
}

#[allow(clippy::large_enum_variant)]
enum QueuedMessage {
	Message(Message),
	EditorMessage(EditorMessage),
}

impl EditorApi for EditorWrapper {
	fn dispatch(&mut self, message: EditorMessage) -> Vec<NativeMessage> {
		self.queue_editor_message(message);

		let mut responses = Vec::new();

		while let Some(queued_message) = self.queue.pop_front() {
			match queued_message {
				QueuedMessage::Message(message) => self.handle_message(message, &mut responses),
				QueuedMessage::EditorMessage(editor_message) => self.handle_editor_message(editor_message, &mut responses),
			}
		}

		responses
	}

	fn poll() -> Vec<NativeMessage> {
		let mut responses = Vec::new();

		let (has_run, texture) = futures::executor::block_on(graphite_editor::node_graph_executor::run_node_graph());
		if has_run {
			responses.push(NativeMessage::Loopback(EditorMessage::PoolNodeGraphEvaluation));
		}
		if let Some(texture) = texture {
			responses.push(NativeMessage::UpdateViewport(texture.texture));
			responses.push(NativeMessage::RequestRedraw);
		}

		responses
	}
}

impl EditorWrapper {
	pub fn new() -> Self {
		Self {
			editor: Editor::new(),
			queue: VecDeque::new(),
		}
	}

	pub fn resume(&self, wgpu_context: WgpuContext) {
		let application_io = WasmApplicationIo::new_with_context(wgpu_context);
		futures::executor::block_on(graphite_editor::node_graph_executor::replace_application_io(application_io));
	}

	fn handle_message(&mut self, message: Message, responses: &mut Vec<NativeMessage>) {
		if let Some(message) = intercept_message::intercept_message(message, responses) {
			let messages = self.editor.handle_message(message);
			let frontend_messages = messages
				.into_iter()
				.filter_map(|m| intercept_frontend_message::intercept_frontend_message(m, responses))
				.collect::<Vec<_>>();
			responses.push(NativeMessage::ToFrontend(ron::to_string(&frontend_messages).unwrap().into_bytes()));
		}
	}

	fn handle_editor_message(&mut self, message: EditorMessage, responses: &mut Vec<NativeMessage>) {
		handle_editor_message::handle_editor_message(self, message, responses);
	}

	pub(super) fn queue_editor_message(&mut self, message: EditorMessage) {
		self.queue.push_back(QueuedMessage::EditorMessage(message));
	}

	pub(super) fn queue_message(&mut self, message: Message) {
		self.queue.push_back(QueuedMessage::Message(message));
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
}
