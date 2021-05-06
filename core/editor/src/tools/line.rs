use crate::events::{CanvasTransform, Event, ToolResponse};
use crate::events::{Key, ViewportPosition};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::layers::style;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Line {
	fsm_state: LineToolFsmState,
	data: LineToolData,
}

impl Tool for Line {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData, canvas_transform: &CanvasTransform) -> (Vec<ToolResponse>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, canvas_transform, &mut responses, &mut operations);

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
}

impl Fsm for LineToolFsmState {
	type ToolData = LineToolData;

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
			(LineToolFsmState::Ready, Event::LmbDown(mouse_state)) => {
				data.drag_start = mouse_state.position;
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
				operations.push(Operation::ClearWorkingFolder);
				let start = data.drag_start.to_canvas_position(canvas_transform);
				let end = mouse_state.to_canvas_position(canvas_transform);
				operations.push(Operation::AddLine {
					path: vec![],
					insert_index: -1,
					x0: start.x,
					y0: start.y,
					x1: end.x,
					y1: end.y,
					style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), None),
				});

				LineToolFsmState::LmbDown
			}
			(LineToolFsmState::LmbDown, Event::LmbUp(mouse_state)) => {
				let distance = data.drag_start.distance(&mouse_state.position);
				log::info!("draw Line with distance: {:.2}", distance);
				operations.push(Operation::ClearWorkingFolder);
				let start = data.drag_start.to_canvas_position(canvas_transform);
				let end = mouse_state.position.to_canvas_position(canvas_transform);
				operations.push(Operation::AddLine {
					path: vec![],
					insert_index: -1,
					x0: start.x,
					y0: start.y,
					x1: end.x,
					y1: end.y,
					style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), None),
				});
				operations.push(Operation::CommitTransaction);

				LineToolFsmState::Ready
			}

			_ => self,
		}
	}
}
