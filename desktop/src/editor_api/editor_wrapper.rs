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
}

impl EditorApi for EditorWrapper {
	fn dispatch(&mut self, message: EditorMessage) -> Vec<NativeMessage> {
		let mut responses = Vec::new();
		handle_editor_message::handle_editor_message(self, message, &mut responses);
		let mut native_responses = Vec::new();
		for message in responses.drain(..) {
			self.handle_message(message, &mut native_responses);
		}
		native_responses
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
		Self { editor: Editor::new() }
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

	pub(super) fn poll_node_graph_evaluation(&mut self, responses: &mut Vec<Message>) {
		let mut node_graph_responses = VecDeque::new();
		let err = self.editor.poll_node_graph_evaluation(&mut node_graph_responses);
		if let Err(e) = err {
			if e != "No active document" {
				tracing::error!("Error poling node graph: {}", e);
			}
		}
		responses.extend(node_graph_responses.drain(..));
	}
}
