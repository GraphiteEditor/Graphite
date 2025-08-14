use graphite_editor::{application::Editor, messages::prelude::Message};

use crate::editor_api::EditorApi;
use crate::editor_api::messages::{EditorMessage, NativeMessage};

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
		handle_editor_message::handle_editor_message(message, &mut responses);
		let mut native_responses = Vec::new();
		for message in responses.drain(..) {
			self.handle_message(message, &mut native_responses);
		}
		native_responses
	}

	fn poll() -> Vec<NativeMessage> {
		match futures::executor::block_on(graphite_editor::node_graph_executor::run_node_graph()) {
			(has_run, Some(texture)) if has_run => vec![NativeMessage::UpdateViewport((*texture.texture).clone())],
			_ => vec![],
		}
	}
}

impl EditorWrapper {
	pub fn new() -> Self {
		Self { editor: Editor::new() }
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
}
