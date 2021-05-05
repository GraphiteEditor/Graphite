use crate::events::{Event, ToolResponse};
use crate::events::{Key, ViewportPosition};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::layers::style;
use document_core::Operation;

use super::DocumentToolData;

use std::f64::consts::PI;

#[derive(Default)]
pub struct Line {
	fsm_state: LineToolFsmState,
	data: LineToolData,
}

impl Tool for Line {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<ToolResponse>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineToolFsmState {
	Ready,
	LmbDown,
}

impl Default for LineToolFsmState {
	fn default() -> Self {
		LineToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct LineToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	snap_angle: bool,
}

impl Fsm for LineToolFsmState {
	type ToolData = LineToolData;

	fn transition(self, event: &Event, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, _responses: &mut Vec<ToolResponse>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(LineToolFsmState::Ready, Event::LmbDown(mouse_state)) => {
				data.drag_start = mouse_state.position;
				data.drag_current = mouse_state.position;

				operations.push(Operation::MountWorkingFolder { path: vec![] });

				LineToolFsmState::LmbDown
			}
			(LineToolFsmState::Ready, Event::KeyDown(Key::KeyZ)) => {
				if let Some(id) = document.root.list_layers().last() {
					operations.push(Operation::DeleteLayer { path: vec![*id] })
				}

				LineToolFsmState::Ready
			}
			(LineToolFsmState::LmbDown, Event::MouseMove(mouse_state)) => {
				data.drag_current = *mouse_state;

				operations.push(Operation::ClearWorkingFolder);
				operations.push(make_operation(data, tool_data));

				LineToolFsmState::LmbDown
			}
			(LineToolFsmState::LmbDown, Event::LmbUp(mouse_state)) => {
				data.drag_current = mouse_state.position;

				operations.push(Operation::ClearWorkingFolder);
				operations.push(make_operation(data, tool_data));
				operations.push(Operation::CommitTransaction);

				LineToolFsmState::Ready
			}

			(state, Event::KeyDown(Key::KeyShift)) => {
				data.snap_angle = true;

				if state == LineToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				self
			}

			(state, Event::KeyUp(Key::KeyShift)) => {
				data.snap_angle = false;

				if state == LineToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				self
			}

			_ => self,
		}
	}
}

fn make_operation(data: &LineToolData, tool_data: &DocumentToolData) -> Operation {
	let x0 = data.drag_start.x as f64;
	let y0 = data.drag_start.y as f64;
	let x1 = data.drag_current.x as f64;
	let y1 = data.drag_current.y as f64;

	let (x2, y2) = if data.snap_angle {
		let (dx, dy) = (x1 - x0, y1 - y0);
		let length = f64::hypot(dx, dy);
		let angle = f64::atan2(dx, dy);
		let snap_resolution = 12.0;
		let snapped_angle = (angle * snap_resolution / PI).round() / snap_resolution * PI;
		(x0 + f64::sin(snapped_angle) * length, y0 + f64::cos(snapped_angle) * length)
	} else {
		(x1, y1)
	};

	Operation::AddLine {
		path: vec![],
		insert_index: -1,
		x0,
		y0,
		x1: x2,
		y1: y2,
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), None),
	}
}
