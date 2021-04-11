use crate::events::{Event, Response};
use crate::events::{Key, MouseKeys, ViewportPosition};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::Operation;

#[derive(Default)]
pub struct Rectangle {
	fsm_state: RectangleToolFsmState,
	data: RectangleToolData,
}

impl Tool for Rectangle {
	fn handle_input(&mut self, event: &Event, document: &Document) -> (Vec<Response>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, &mut self.data, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RectangleToolFsmState {
	Ready,
	LmbDown,
}

impl Default for RectangleToolFsmState {
	fn default() -> Self {
		RectangleToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct RectangleToolData {
	drag_start: ViewportPosition,
}

impl Fsm for RectangleToolFsmState {
	type ToolData = RectangleToolData;

	fn transition(self, event: &Event, document: &Document, data: &mut Self::ToolData, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(RectangleToolFsmState::Ready, Event::MouseDown(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => {
				data.drag_start = mouse_state.position;
				RectangleToolFsmState::LmbDown
			}
			(RectangleToolFsmState::Ready, Event::KeyDown(Key::KeyZ)) => {
				if let Some(id) = document.root.list_layers().last() {
					operations.push(Operation::DeleteLayer { path: vec![*id] })
				}
				RectangleToolFsmState::Ready
			}

			// TODO - Check for left mouse button
			(RectangleToolFsmState::LmbDown, Event::MouseUp(mouse_state)) => {
				let r = data.drag_start.distance(&mouse_state.position);
				log::info!("draw rectangle with radius: {:.2}", r);
				let start = data.drag_start;
				let end = mouse_state.position;
				operations.push(Operation::AddRect {
					path: vec![],
					insert_index: -1,
					x0: start.x as f64,
					y0: start.y as f64,
					x1: end.x as f64,
					y1: end.y as f64,
				});

				RectangleToolFsmState::Ready
			}

			_ => self,
		}
	}
}
