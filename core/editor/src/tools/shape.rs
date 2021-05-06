use crate::events::{CanvasPosition, Key, ViewportPosition};
use crate::events::{CanvasTransform, Event, ToolResponse};
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
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData, canvas_transform: &CanvasTransform) -> (Vec<ToolResponse>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, canvas_transform, &mut responses, &mut operations);

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

	fn transition(
		self,
		event: &Event,
		document: &Document,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		canvas_transform: &CanvasTransform,
		_responses: &mut Vec<ToolResponse>,
		operations: &mut Vec<Operation>,
	) -> Self {
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
				let start = data.drag_start.to_canvas_position(canvas_transform);
				let end = mouse_state.to_canvas_position(canvas_transform);
				operations.push(Operation::AddShape {
					path: vec![],
					insert_index: -1,
					x0: start.x,
					y0: start.y,
					x1: end.x,
					y1: end.y,
					sides: 6,
					style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
				});

				ShapeToolFsmState::LmbDown
			}
			(ShapeToolFsmState::LmbDown, Event::LmbUp(mouse_state)) => {
				let r = data.drag_start.distance(&mouse_state.position);
				log::info!("Draw Shape with radius: {:.2}", r);

				let start = data.drag_start.to_canvas_position(canvas_transform);
				let end = mouse_state.position.to_canvas_position(canvas_transform);
				// TODO: Set the sides value and use it for the operation.
				// let sides = data.sides;
				let sides = 6;
				operations.push(Operation::ClearWorkingFolder);
				log::info!("Shape: start {},{} end {},{}", start.x, start.y, end.x, end.y);
				operations.push(Operation::AddShape {
					path: vec![],
					insert_index: -1,
					x0: start.x,
					y0: start.y,
					x1: end.x,
					y1: end.y,
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
