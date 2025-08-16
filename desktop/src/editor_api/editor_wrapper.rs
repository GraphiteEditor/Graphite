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
	queue: VecDeque<EditorMessage>,
	messages: Vec<Message>,
	responses: Vec<NativeMessage>,
}

impl EditorApi for EditorWrapper {
	fn dispatch(&mut self, message: EditorMessage) -> Vec<NativeMessage> {
		self.queue_editor_message(message);

		while !self.queue.is_empty() {
			while let Some(message) = self.queue.pop_front() {
				self.handle_editor_message(message);
			}
			let frontend_messages = self
				.editor
				.handle_message(Message::Batched {
					messages: std::mem::take(&mut self.messages).into_boxed_slice(),
				})
				.into_iter()
				.filter_map(|m| intercept_frontend_message::intercept_frontend_message(m, &mut self.responses))
				.collect::<Vec<_>>();
			self.respond(NativeMessage::ToFrontend(ron::to_string(&frontend_messages).unwrap().into_bytes()));
		}

		std::mem::take(&mut self.responses)
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
			messages: Vec::new(),
			responses: Vec::new(),
		}
	}

	pub fn resume(&self, wgpu_context: WgpuContext) {
		let application_io = WasmApplicationIo::new_with_context(wgpu_context);
		futures::executor::block_on(graphite_editor::node_graph_executor::replace_application_io(application_io));
	}

	fn handle_editor_message(&mut self, message: EditorMessage) {
		handle_editor_message::handle_editor_message(self, message);
	}

	pub fn respond(&mut self, response: NativeMessage) {
		self.responses.push(response);
	}

	pub(super) fn queue_editor_message(&mut self, message: EditorMessage) {
		self.queue.push_back(message);
	}

	pub(super) fn queue_message(&mut self, message: Message) {
		if let Some(message) = intercept_message::intercept_message(message, &mut self.responses) {
			self.messages.push(message);
		}
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
