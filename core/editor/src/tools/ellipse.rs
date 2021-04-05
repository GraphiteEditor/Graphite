use crate::events::MouseKeys;
use crate::events::{Event, Response};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::Operation;

#[derive(Default)]
pub struct Ellipse {
	state: EllipseToolState,
}

impl Tool for Ellipse {
	fn handle_input(&mut self, event: &Event, document: &Document) -> (Vec<Response>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.state = self.state.transition(event, document, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EllipseToolState {
	Ready,
	LmbDown,
	TransformSelected,
}

impl Default for EllipseToolState {
	fn default() -> Self {
		EllipseToolState::Ready
	}
}

impl Fsm for EllipseToolState {
	fn transition(self, event: &Event, document: &Document, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(EllipseToolState::Ready, Event::MouseDown(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => EllipseToolState::LmbDown,

			// TODO - Check for left mouse button
			(EllipseToolState::LmbDown, Event::MouseUp(mouse_state)) => {
				operations.push(Operation::AddCircle {
					cx: mouse_state.position.x as f64,
					cy: mouse_state.position.y as f64,
					r: 10.0,
				});

				EllipseToolState::Ready
			}

			_ => self,
		}
	}
}
