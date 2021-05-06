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
		let stroke = style::Stroke::new(tool_data.primary_color, 5.);
		let fill = style::Fill::none();
		let style = style::PathStyle::new(Some(stroke), Some(fill));

		match (self, event) {
			(PenToolFsmState::Ready, Event::LmbDown(mouse_state)) => {
				operations.push(Operation::MountWorkingFolder { path: vec![] });
				data.points.push(mouse_state.position.to_canvas_position(canvas_transform));
				PenToolFsmState::LmbDown
			}
			(PenToolFsmState::Ready, Event::KeyDown(Key::KeyZ)) => {
				if let Some(id) = document.root.list_layers().last() {
					operations.push(Operation::DeleteLayer { path: vec![*id] })
				}
				PenToolFsmState::Ready
			}
			(PenToolFsmState::LmbDown, Event::LmbUp(mouse_state)) => {
				data.points.push(mouse_state.position.to_canvas_position(canvas_transform));
				PenToolFsmState::LmbDown
			}
			(PenToolFsmState::LmbDown, Event::MouseMove(mouse_state)) => {
				let mut points: Vec<_> = data.points.iter().map(|p: &CanvasPosition| (p.x, p.y)).collect();
				let pos = mouse_state.to_canvas_position(canvas_transform);
				points.push((pos.x, pos.y));

				operations.push(Operation::ClearWorkingFolder);
				operations.push(Operation::AddPen {
					path: vec![],
					insert_index: -1,
					points,
					style,
				});
				PenToolFsmState::LmbDown
			}
			(PenToolFsmState::LmbDown, Event::KeyDown(Key::KeyEnter)) => {
				let points = data.points.drain(..).map(|p| (p.x, p.y)).collect();
				operations.push(Operation::ClearWorkingFolder);
				operations.push(Operation::AddPen {
					path: vec![],
					insert_index: -1,
					points,
					style,
				});
				operations.push(Operation::CommitTransaction);
				PenToolFsmState::Ready
			}

			_ => self,
		}
	}
}
