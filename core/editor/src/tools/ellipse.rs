use crate::events::MouseKeys;
use crate::events::{Event, Response};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::Operation;

#[derive(Default)]
pub struct Ellipse {
	fsm_state: EllipseToolFsmState,
}

impl Tool for Ellipse {
	fn handle_input(&mut self, event: &Event, document: &Document) -> (Vec<Response>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EllipseToolFsmState {
	Ready,
	LmbDown,
	TransformSelected,
}

impl Default for EllipseToolFsmState {
	fn default() -> Self {
		EllipseToolFsmState::Ready
	}
}

impl Fsm for EllipseToolFsmState {
	fn transition(self, event: &Event, document: &Document, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(EllipseToolFsmState::Ready, Event::MouseDown(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => EllipseToolFsmState::LmbDown,

			// TODO - Check for left mouse button
			(EllipseToolFsmState::LmbDown, Event::MouseUp(mouse_state)) => {
				operations.push(Operation::AddCircle {
					cx: mouse_state.position.x as f64,
					cy: mouse_state.position.y as f64,
					r: 10.0,
				});

				EllipseToolFsmState::Ready
			}

			_ => self,
		}
	}
}
