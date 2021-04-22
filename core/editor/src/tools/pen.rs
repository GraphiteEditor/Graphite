use crate::events::{Event, Response};
use crate::events::{Key, MouseKeys, ViewportPosition};
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
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<Response>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, &mut responses, &mut operations);

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
	points: Vec<ViewportPosition>,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;

	fn transition(self, event: &Event, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, _responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(PenToolFsmState::Ready, Event::MouseDown(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => {
				operations.push(Operation::MountWorkingFolder { path: vec![] });
				data.points.push(mouse_state.position);
				PenToolFsmState::LmbDown
			}
			(PenToolFsmState::Ready, Event::KeyDown(Key::KeyZ)) => {
				if let Some(id) = document.root.list_layers().last() {
					operations.push(Operation::DeleteLayer { path: vec![*id] })
				}
				PenToolFsmState::Ready
			}
			// TODO - Check for left mouse button
			(PenToolFsmState::LmbDown, Event::MouseDown(mouse_state)) => {
				data.points.push(mouse_state.position);
				PenToolFsmState::LmbDown
			}
			(PenToolFsmState::LmbDown, Event::MouseMove(mouse_state)) => {
				let mut points: Vec<_> = data.points.iter().map(|p| (p.x as f64, p.y as f64)).collect();
				points.push((mouse_state.x as f64, mouse_state.y as f64));

				operations.push(Operation::ClearWorkingFolder);
				operations.push(Operation::AddPen {
					path: vec![],
					insert_index: -1,
					points,
					style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), Some(style::Fill::none())),
				});
				PenToolFsmState::LmbDown
			}
			(PenToolFsmState::LmbDown, Event::KeyDown(Key::KeyEnter)) => {
				let points = data.points.drain(..).map(|p| (p.x as f64, p.y as f64)).collect();
				operations.push(Operation::ClearWorkingFolder);
				operations.push(Operation::AddPen {
					path: vec![],
					insert_index: -1,
					points,
					style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), Some(style::Fill::none())),
				});
				operations.push(Operation::CommitTransaction);
				PenToolFsmState::Ready
			}

			_ => self,
		}
	}
}
