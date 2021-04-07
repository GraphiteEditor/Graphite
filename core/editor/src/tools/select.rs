use crate::events::MouseKeys;
use crate::events::{Event, Response};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::Operation;

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

impl Tool for Select {
	fn handle_input(&mut self, event: &Event, document: &Document) -> (Vec<Response>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, &mut self.data, &mut responses, &mut operations);

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

	fn transition(self, event: &Event, document: &Document, data: &mut Self::ToolData, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(SelectToolFsmState::Ready, Event::MouseDown(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => SelectToolFsmState::LmbDown,

			(SelectToolFsmState::LmbDown, Event::MouseUp(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => SelectToolFsmState::Ready,

			(SelectToolFsmState::LmbDown, Event::MouseMove(mouse_state)) => SelectToolFsmState::TransformSelected,

			(SelectToolFsmState::TransformSelected, Event::MouseMove(mouse_state)) => self,

			(SelectToolFsmState::TransformSelected, Event::MouseUp(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => SelectToolFsmState::Ready,

			_ => self,
		}
	}
}
