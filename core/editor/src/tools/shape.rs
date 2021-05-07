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
	drag_current: ViewportPosition,
	constrain_to_square: bool,
	center_around_cursor: bool,
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
				data.drag_current = mouse_state.position;

				data.sides = 6;

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
				data.drag_current = *mouse_state;
				operations.push(Operation::ClearWorkingFolder);
				operations.push(make_operation(data, tool_data, canvas_transform));

				ShapeToolFsmState::LmbDown
			}
			(ShapeToolFsmState::LmbDown, Event::LmbUp(mouse_state)) => {
				data.drag_current = mouse_state.position;
				operations.push(Operation::ClearWorkingFolder);
				if data.drag_start != data.drag_current {
					operations.push(make_operation(data, tool_data, canvas_transform));
					operations.push(Operation::CommitTransaction);
				}

				ShapeToolFsmState::Ready
			}

			(state, Event::KeyDown(Key::KeyShift)) => {
				data.constrain_to_square = true;

				if state == ShapeToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data, canvas_transform));
				}

				self
			}

			(state, Event::KeyUp(Key::KeyShift)) => {
				data.constrain_to_square = false;

				if state == ShapeToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data, canvas_transform));
				}

				self
			}

			(state, Event::KeyDown(Key::KeyAlt)) => {
				data.center_around_cursor = true;

				if state == ShapeToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data, canvas_transform));
				}

				self
			}

			(state, Event::KeyUp(Key::KeyAlt)) => {
				data.center_around_cursor = false;

				if state == ShapeToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data, canvas_transform));
				}

				self
			}

			_ => self,
		}
	}
}

fn make_operation(data: &ShapeToolData, tool_data: &DocumentToolData, canvas_transform: &CanvasTransform) -> Operation {
	let start = data.drag_start.to_canvas_position(canvas_transform);
	let end = data.drag_current.to_canvas_position(canvas_transform);
	let x0 = start.x;
	let y0 = start.y;
	let x1 = end.x;
	let y1 = end.y;

	let (x0, y0, x1, y1) = if data.constrain_to_square {
		let (x_dir, y_dir) = ((x1 - x0).signum(), (y1 - y0).signum());
		let max_dist = f64::max((x1 - x0).abs(), (y1 - y0).abs());
		if data.center_around_cursor {
			(x0 - max_dist * x_dir, y0 - max_dist * y_dir, x0 + max_dist * x_dir, y0 + max_dist * y_dir)
		} else {
			(x0, y0, x0 + max_dist * x_dir, y0 + max_dist * y_dir)
		}
	} else {
		let (x0, y0) = if data.center_around_cursor { (x0 - 2.0 * (x1 - x0), y0 - 2.0 * (y1 - y0)) } else { (x0, y0) };
		(x0, y0, x1, y1)
	};

	Operation::AddShape {
		path: vec![],
		insert_index: -1,
		x0,
		y0,
		x1,
		y1,
		sides: data.sides,
		style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
	}
}
