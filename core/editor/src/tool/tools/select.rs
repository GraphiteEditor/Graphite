use document_core::color::Color;
use document_core::layers::style;
use document_core::layers::style::Fill;
use document_core::layers::style::Stroke;
use document_core::Operation;
use glam::{DAffine2, DVec2};

use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{message_prelude::*, SvgDocument};

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug)]
pub enum SelectMessage {
	DragStart,
	DragStop,
	MouseMove,
	Abort,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Select {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use SelectToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(SelectMessageDiscriminant;  DragStart),
			Dragging => actions!(SelectMessageDiscriminant; DragStop, MouseMove, Abort),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectToolFsmState {
	Ready,
	Dragging,
}

impl Default for SelectToolFsmState {
	fn default() -> Self {
		SelectToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct SelectToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;

	fn transition(self, event: ToolMessage, document: &SvgDocument, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, responses: &mut VecDeque<Message>) -> Self {
		let transform = document.root.transform;
		use SelectMessage::*;
		use SelectToolFsmState::*;
		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;
					responses.push_back(Operation::MountWorkingFolder { path: vec![] }.into());
					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse.position;

					responses.push_back(Operation::ClearWorkingFolder.into());
					responses.push_back(make_operation(data, tool_data, transform));

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse.position;

					responses.push_back(Operation::ClearWorkingFolder.into());
					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.drag_start == data.drag_current {
						let point = DVec2::new(data.drag_current.x as f64, data.drag_current.y as f64);
						if let Some(intersection) = document.intersects_point_root(point).last() {
							responses.push_back(DocumentMessage::SelectLayers(vec![intersection.clone()]).into());
						} else {
							responses.push_back(DocumentMessage::SelectLayers(vec![]).into());
						}
					} else {
						let quad = [
							DVec2::new(data.drag_start.x as f64, data.drag_start.y as f64),
							DVec2::new(data.drag_current.x as f64, data.drag_start.y as f64),
							DVec2::new(data.drag_current.x as f64, data.drag_current.y as f64),
							DVec2::new(data.drag_start.x as f64, data.drag_current.y as f64),
						];
						responses.push_back(DocumentMessage::SelectLayers(document.intersects_quad_root(quad)).into());
					}

					Ready
				}
				// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
				(Dragging, Abort) => {
					responses.push_back(Operation::DiscardWorkingFolder.into());

					Ready
				}
				_ => self,
			}
		} else {
			self
		}
	}
}

fn make_operation(data: &SelectToolData, _tool_data: &DocumentToolData, transform: DAffine2) -> Message {
	let x0 = data.drag_start.x as f64;
	let y0 = data.drag_start.y as f64;
	let x1 = data.drag_current.x as f64;
	let y1 = data.drag_current.y as f64;

	Operation::AddRect {
		path: vec![],
		insert_index: -1,
		transform: (transform.inverse() * glam::DAffine2::from_scale_angle_translation(DVec2::new(x1 - x0, y1 - y0), 0., DVec2::new(x0, y0))).to_cols_array(),
		style: style::PathStyle::new(Some(Stroke::new(Color::from_rgb8(0x31, 0x94, 0xD6), 2.0)), Some(Fill::none())),
	}
	.into()
}
