use crate::events::{CanvasPosition, CanvasTransform, Key};
use crate::events::{Event, ToolResponse};
use crate::tools::{Fsm, Tool};
use crate::Document;

use document_core::layers::style;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Pen {
	fsm_state: PenToolFsmState,
	data: PenToolData,
}

impl Tool for Pen {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData, canvas_transform: &CanvasTransform) -> (Vec<ToolResponse>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, canvas_transform, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenToolFsmState {
	Ready,
	LmbDown,
}

impl Default for PenToolFsmState {
	fn default() -> Self {
		PenToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct PenToolData {
	points: Vec<CanvasPosition>,
	next_point: CanvasPosition,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;

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
			(PenToolFsmState::Ready, Event::LmbDown(mouse_state)) => {
				operations.push(Operation::MountWorkingFolder { path: vec![] });

				let canvas_position = mouse_state.position.to_canvas_position(canvas_transform);
				data.points.push(canvas_position);
				data.next_point = canvas_position;

				PenToolFsmState::LmbDown
			}
			(PenToolFsmState::Ready, Event::KeyDown(Key::KeyZ)) => {
				if let Some(id) = document.root.list_layers().last() {
					operations.push(Operation::DeleteLayer { path: vec![*id] })
				}

				PenToolFsmState::Ready
			}
			(PenToolFsmState::LmbDown, Event::LmbUp(mouse_state)) => {
				let canvas_position = mouse_state.position.to_canvas_position(canvas_transform);
				if data.points.last() != Some(&canvas_position) {
					data.points.push(canvas_position);
					data.next_point = canvas_position;
				}

				PenToolFsmState::LmbDown
			}
			(PenToolFsmState::LmbDown, Event::MouseMove(mouse_state)) => {
				data.next_point = mouse_state.to_canvas_position(canvas_transform);

				operations.push(Operation::ClearWorkingFolder);
				operations.push(make_operation(data, tool_data, true));

				PenToolFsmState::LmbDown
			}
			// TODO - simplify with or_patterns when rust 1.53.0 is stable  (https://github.com/rust-lang/rust/issues/54883)
			(PenToolFsmState::LmbDown, Event::KeyDown(Key::KeyEnter)) | (PenToolFsmState::LmbDown, Event::KeyDown(Key::KeyEscape)) | (PenToolFsmState::LmbDown, Event::RmbDown(_)) => {
				operations.push(Operation::ClearWorkingFolder);

				if data.points.len() >= 2 {
					operations.push(make_operation(data, tool_data, false));
					operations.push(Operation::CommitTransaction);
				} else {
					operations.push(Operation::DiscardWorkingFolder);
				}

				data.points.clear();

				PenToolFsmState::Ready
			}
			_ => self,
		}
	}
}

fn make_operation(data: &PenToolData, tool_data: &DocumentToolData, show_preview: bool) -> Operation {
	let mut points: Vec<(f64, f64)> = data.points.iter().map(|p| (p.x, p.y)).collect();
	if show_preview {
		points.push((data.next_point.x, data.next_point.y))
	}
	Operation::AddPen {
		path: vec![],
		insert_index: -1,
		points,
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), Some(style::Fill::none())),
	}
}
