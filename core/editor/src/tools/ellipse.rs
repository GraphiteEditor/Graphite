use crate::events::{Event, Response};
use crate::events::{Key, MouseKeys, ViewportPosition};
use crate::tools::{Fsm, Tool};
use crate::Document;
use document_core::layers::style;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Ellipse {
	fsm_state: EllipseToolFsmState,
	data: EllipseToolData,
}

impl Tool for Ellipse {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<Response>, Vec<Operation>) {
		let mut responses = Vec::new();
		let mut operations = Vec::new();
		self.fsm_state = self.fsm_state.transition(event, document, tool_data, &mut self.data, &mut responses, &mut operations);

		(responses, operations)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EllipseToolFsmState {
	Ready,
	LmbDown,
}

impl Default for EllipseToolFsmState {
	fn default() -> Self {
		EllipseToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct EllipseToolData {
	drag_start: ViewportPosition,
}

impl Fsm for EllipseToolFsmState {
	type ToolData = EllipseToolData;

	fn transition(self, event: &Event, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(EllipseToolFsmState::Ready, Event::MouseDown(mouse_state)) if mouse_state.mouse_keys.contains(MouseKeys::LEFT) => {
				data.drag_start = mouse_state.position;
				operations.push(Operation::MountWorkingFolder { path: vec![] });
				EllipseToolFsmState::LmbDown
			}

			(EllipseToolFsmState::Ready, Event::KeyDown(Key::KeyZ)) => {
				if let Some(id) = document.root.list_layers().last() {
					operations.push(Operation::DeleteLayer { path: vec![*id] })
				}
				EllipseToolFsmState::Ready
			}

			(EllipseToolFsmState::LmbDown, Event::MouseMove(mouse_state)) => {
				operations.push(Operation::ClearWorkingFolder);
				operations.push(Operation::AddCircle {
					path: vec![],
					insert_index: -1,
					cx: data.drag_start.x as f64,
					cy: data.drag_start.y as f64,
					r: data.drag_start.distance(&mouse_state),
					style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color)))
				});

				EllipseToolFsmState::LmbDown
			}

			// TODO - Check for left mouse button
			(EllipseToolFsmState::LmbDown, Event::MouseUp(mouse_state)) => {
				let r = data.drag_start.distance(&mouse_state.position);
				log::info!("draw ellipse with radius: {:.2}", r);
				operations.push(Operation::ClearWorkingFolder);
				operations.push(Operation::AddCircle {
					path: vec![],
					insert_index: -1,
					cx: data.drag_start.x as f64,
					cy: data.drag_start.y as f64,
					r: data.drag_start.distance(&mouse_state.position),
					style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
				});
				operations.push(Operation::CommitTransaction);

				EllipseToolFsmState::Ready
			}

			_ => self,
		}
	}
}
