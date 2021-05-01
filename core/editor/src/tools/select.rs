use crate::events::{Event, Response};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

impl Tool for Select {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<Response>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectToolFsmState {
	Ready,
	LmbDown,
	TransformSelected,
}

impl Default for SelectToolFsmState {
	fn default() -> Self {
		SelectToolFsmState::Ready
	}
}

#[derive(Default)]
struct SelectToolData;

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;

	fn transition(self, event: &Event, _document: &Document, _tool_data: &DocumentToolData, _data: &mut Self::ToolData, _responses: &mut Vec<Response>, _operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(SelectToolFsmState::Ready, Event::LmbDown(_mouse_state)) => SelectToolFsmState::LmbDown,

			(SelectToolFsmState::LmbDown, Event::LmbUp(_mouse_state)) => SelectToolFsmState::Ready,

			(SelectToolFsmState::LmbDown, Event::MouseMove(_mouse_state)) => SelectToolFsmState::TransformSelected,

			(SelectToolFsmState::TransformSelected, Event::MouseMove(_mouse_state)) => self,

			(SelectToolFsmState::TransformSelected, Event::LmbUp(_mouse_state)) => SelectToolFsmState::Ready,

			_ => self,
		}
	}
}
