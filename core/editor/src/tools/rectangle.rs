use crate::events::{Event, ToolResponse};
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
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<ToolResponse>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, &mut responses, &mut operations);

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

	fn transition(self, event: &Event, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, _responses: &mut Vec<ToolResponse>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(RectangleToolFsmState::Ready, Event::LmbDown(mouse_state)) => {
				data.drag_start = mouse_state.position;
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
				operations.push(Operation::ClearWorkingFolder);
				let start = data.drag_start;
				let end = mouse_state;
				operations.push(Operation::AddRect {
					path: vec![],
					insert_index: -1,
					x0: start.x as f64,
					y0: start.y as f64,
					x1: end.x as f64,
					y1: end.y as f64,
					style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
				});

				RectangleToolFsmState::LmbDown
			}
			(RectangleToolFsmState::LmbDown, Event::LmbUp(mouse_state)) => {
				let r = data.drag_start.distance(&mouse_state.position);
				log::info!("draw rectangle with radius: {:.2}", r);
				operations.push(Operation::ClearWorkingFolder);
				let start = data.drag_start;
				let end = mouse_state.position;
				operations.push(Operation::AddRect {
					path: vec![],
					insert_index: -1,
					x0: start.x as f64,
					y0: start.y as f64,
					x1: end.x as f64,
					y1: end.y as f64,
					style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
				});
				operations.push(Operation::CommitTransaction);

				RectangleToolFsmState::Ready
			}

			_ => self,
		}
	}
}
