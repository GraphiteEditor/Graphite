use crate::events::{CanvasTransform, Event, ToolResponse};
use crate::events::{Key, ViewportPosition};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::layers::style;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Rectangle {
	fsm_state: RectangleToolFsmState,
	data: RectangleToolData,
}

impl Tool for Rectangle {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData, canvas_transform: &CanvasTransform) -> (Vec<ToolResponse>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, canvas_transform, &mut responses, &mut operations);

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
	drag_current: ViewportPosition,
	constrain_to_square: bool,
	center_around_cursor: bool,
}

impl Fsm for RectangleToolFsmState {
	type ToolData = RectangleToolData;

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
			(RectangleToolFsmState::Ready, Event::LmbDown(mouse_state)) => {
				data.drag_start = mouse_state.position;
				data.drag_current = mouse_state.position;
				operations.push(Operation::MountWorkingFolder { path: vec![] });
				RectangleToolFsmState::LmbDown
			}
			(RectangleToolFsmState::Ready, Event::KeyDown(Key::KeyZ)) => {
				if let Some(id) = document.root.list_layers().last() {
					operations.push(Operation::DeleteLayer { path: vec![*id] })
				}
				RectangleToolFsmState::Ready
			}
			(RectangleToolFsmState::LmbDown, Event::MouseMove(mouse_state)) => {
				data.drag_current = *mouse_state;

				operations.push(Operation::ClearWorkingFolder);
				operations.push(make_operation(data, tool_data, canvas_transform));

				RectangleToolFsmState::LmbDown
			}
			(RectangleToolFsmState::LmbDown, Event::LmbUp(mouse_state)) => {
				data.drag_current = mouse_state.position;

				operations.push(Operation::ClearWorkingFolder);
				// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
				if data.drag_start != data.drag_current {
					operations.push(make_operation(data, tool_data, canvas_transform));
					operations.push(Operation::CommitTransaction);
				}

				RectangleToolFsmState::Ready
			}
			// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
			(RectangleToolFsmState::LmbDown, Event::KeyUp(Key::KeyEscape)) | (RectangleToolFsmState::LmbDown, Event::RmbDown(_)) => {
				operations.push(Operation::DiscardWorkingFolder);

				RectangleToolFsmState::Ready
			}
			(state, Event::KeyDown(Key::KeyShift)) => {
				data.constrain_to_square = true;

				if state == RectangleToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data, canvas_transform));
				}

				self
			}
			(state, Event::KeyUp(Key::KeyShift)) => {
				data.constrain_to_square = false;

				if state == RectangleToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data, canvas_transform));
				}

				self
			}
			(state, Event::KeyDown(Key::KeyAlt)) => {
				data.center_around_cursor = true;

				if state == RectangleToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data, canvas_transform));
				}

				self
			}
			(state, Event::KeyUp(Key::KeyAlt)) => {
				data.center_around_cursor = false;

				if state == RectangleToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data, canvas_transform));
				}

				self
			}
			_ => self,
		}
	}
}

fn make_operation(data: &RectangleToolData, tool_data: &DocumentToolData, canvas_transform: &CanvasTransform) -> Operation {
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
		let (x0, y0) = if data.center_around_cursor {
			let delta_x = x1 - x0;
			let delta_y = y1 - y0;

			(x0 - delta_x, y0 - delta_y)
		} else {
			(x0, y0)
		};
		(x0, y0, x1, y1)
	};

	Operation::AddRect {
		path: vec![],
		insert_index: -1,
		x0,
		y0,
		x1,
		y1,
		style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
	}
}
