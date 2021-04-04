use crate::events::MouseKeys;
use crate::events::{Event, Response};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::Operation;

#[derive(Default)]
pub struct Select {
	state: SelectToolState,
}

impl Tool for Select {
	fn handle_input(&mut self, event: &Event, document: &Document) -> (Vec<Response>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.state = self.state.transition(event, document, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectToolState {
	Ready,
	LmbDown,
	TransformSelected,
}

impl Default for SelectToolState {
	fn default() -> Self {
		SelectToolState::Ready
	}
}

impl Fsm for SelectToolState {
	fn transition(self, event: &Event, document: &Document, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(SelectToolState::Ready, Event::MouseDown(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => SelectToolState::LmbDown,

			(SelectToolState::LmbDown, Event::MouseUp(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => SelectToolState::Ready,

			(SelectToolState::LmbDown, Event::MouseMovement(mouse_state)) => SelectToolState::TransformSelected,

			(SelectToolState::TransformSelected, Event::MouseMovement(mouse_state)) => SelectToolState::TransformSelected,

			(SelectToolState::TransformSelected, Event::MouseUp(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => SelectToolState::Ready,

			(state, _) => state,
		}
	}
}
