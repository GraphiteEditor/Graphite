use crate::events::{Event, Response};
use crate::events::{MouseKeys, ViewportPosition};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::Operation;

#[derive(Default)]
pub struct Ellipse {
	fsm_state: EllipseToolFsmState,
	data: EllipseToolData,
}

impl Tool for Ellipse {
	fn handle_input(&mut self, event: &Event, document: &Document) -> (Vec<Response>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, &mut self.data, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EllipseToolFsmState {
	Ready,
	LmbDown,
}

impl Default for EllipseToolFsmState {
	fn default() -> Self {
		EllipseToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct EllipseToolData {
	drag_start: ViewportPosition,
	index: u64,
}

impl Fsm for EllipseToolFsmState {
	type ToolData = EllipseToolData;

	fn transition(self, event: &Event, document: &Document, data: &mut Self::ToolData, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(EllipseToolFsmState::Ready, Event::MouseDown(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => {
				data.drag_start = mouse_state.position;
				EllipseToolFsmState::LmbDown
			}

			// TODO - Check for left mouse button
			(EllipseToolFsmState::LmbDown, Event::MouseUp(mouse_state)) => {
				let r = data.drag_start.distance(&mouse_state.position);
				log::info!("draw ellipse with radius: {:.2}", r);
				let name = format!("ellipses/ellipse-{}", data.index);
				if data.index == 0 {
					operations.push(Operation::AddFolder { path: "ellipses".to_string() });
				}
				data.index += 1;
				operations.push(Operation::AddCircle {
					path: name,
					cx: data.drag_start.x as f64,
					cy: data.drag_start.y as f64,
					r: data.drag_start.distance(&mouse_state.position),
				});

				EllipseToolFsmState::Ready
			}

			_ => self,
		}
	}
}
