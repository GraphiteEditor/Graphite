use crate::document::Document;

use super::{input_manager::InputPreprocessor, Action, ActionHandler, Operation, Response};
use crate::tools::ToolFsmState;

pub struct GlobalEventHandler {
	documents: Vec<Document>,
	active_document: usize,
	tool_state: ToolFsmState,
}

impl GlobalEventHandler {
	fn new() -> Self {
		Self {
			documents: vec![Document::default()],
			active_document: 0,
			tool_state: ToolFsmState::default(),
		}
	}
	fn active_document(&self) -> &Document {
		&self.documents[self.active_document]
	}
	fn active_document_mut(&mut self) -> &mut Document {
		&mut self.documents[self.active_document]
	}
}

impl ActionHandler<()> for GlobalEventHandler {
	fn process_action(&mut self, _data: (), input: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		let mut consumed = true;

		// process action before passing them further down
		use Action::*;
		match action {
			SelectDocument(id) => self.active_document = *id,
			_ => consumed = false,
		}

		// pass action to the next level if it was not consumed
		if !consumed {
			let doc = &mut self.documents[self.active_document];
			let tool = self.tool_state.tool_data.active_tool().unwrap().as_mut();
			let document_tool_data = &self.tool_state.document_tool_data;
			consumed = doc.handler.process_action((&mut doc.document, tool, document_tool_data), &input, action, responses, operations)
		}

		// post process action if it was not consumed
		if !consumed {}

		consumed
	}
	fn actions(&self) -> &[(&str, Action)] {
		&[("", Action::Save)]
	}
}
