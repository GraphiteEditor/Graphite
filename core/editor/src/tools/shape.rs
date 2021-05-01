use crate::events::{Event, ToolResponse};
use crate::events::{Key, ViewportPosition};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::layers::style;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Shape {
	fsm_state: ShapeToolFsmState,
	data: ShapeToolData,
}

impl Tool for Shape {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<ToolResponse>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeToolFsmState {
	Ready,
	LmbDown,
}

impl Default for ShapeToolFsmState {
	fn default() -> Self {
		ShapeToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct ShapeToolData {
	drag_start: ViewportPosition,
	sides: u8,
}

impl Fsm for ShapeToolFsmState {
	type ToolData = ShapeToolData;

	fn transition(self, event: &Event, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, _responses: &mut Vec<ToolResponse>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(ShapeToolFsmState::Ready, Event::LmbDown(mouse_state)) => {
				data.drag_start = mouse_state.position;
				operations.push(Operation::MountWorkingFolder { path: vec![] });
				ShapeToolFsmState::LmbDown
			}
			(ShapeToolFsmState::Ready, Event::KeyDown(Key::KeyZ)) => {
				if let Some(id) = document.root.list_layers().last() {
					operations.push(Operation::DeleteLayer { path: vec![*id] })
				}
				ShapeToolFsmState::Ready
			}
			(ShapeToolFsmState::LmbDown, Event::MouseMove(mouse_state)) => {
				operations.push(Operation::ClearWorkingFolder);
				let start = data.drag_start;
				let end = mouse_state;
				operations.push(Operation::AddShape {
					path: vec![],
					insert_index: -1,
					x0: start.x as f64,
					y0: start.y as f64,
					x1: end.x as f64,
					y1: end.y as f64,
					sides: 6,
					style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
				});

				ShapeToolFsmState::LmbDown
			}
			(ShapeToolFsmState::LmbDown, Event::LmbUp(mouse_state)) => {
				let r = data.drag_start.distance(&mouse_state.position);
				log::info!("Draw Shape with radius: {:.2}", r);

				let start = data.drag_start;
				let end = mouse_state.position;
				// TODO: Set the sides value and use it for the operation.
				// let sides = data.sides;
				let sides = 6;
				operations.push(Operation::ClearWorkingFolder);
				operations.push(Operation::AddShape {
					path: vec![],
					insert_index: -1,
					x0: start.x as f64,
					y0: start.y as f64,
					x1: end.x as f64,
					y1: end.y as f64,
					sides,
					style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
				});
				operations.push(Operation::CommitTransaction);

				ShapeToolFsmState::Ready
			}

			_ => self,
		}
	}
}
