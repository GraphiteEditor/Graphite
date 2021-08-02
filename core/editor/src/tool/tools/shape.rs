use crate::input::InputPreprocessor;
use crate::tool::{DocumentToolData, Fsm, ShapeType, ToolActionHandlerData, ToolOptions, ToolType};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use document_core::layers::LayerData;
use document_core::{layers::style, Operation};
use glam::DAffine2;

use super::resize::*;

#[derive(Default)]
pub struct Shape {
	fsm_state: ShapeToolFsmState,
	data: ShapeToolData,
}

#[impl_message(Message, ToolMessage, Shape)]
#[derive(PartialEq, Clone, Debug, Hash)]
pub enum ShapeMessage {
	DragStart,
	DragStop,
	Resize(ResizeMessage),
	Abort,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Shape {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use ShapeToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(ShapeMessageDiscriminant; DragStart, Resize),
			Dragging => actions!(ShapeMessageDiscriminant; DragStop,  Abort, Resize),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeToolFsmState {
	Ready,
	Dragging,
}

impl Default for ShapeToolFsmState {
	fn default() -> Self {
		ShapeToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct ShapeToolData {
	sides: u8,
	data: Resize,
}

impl Fsm for ShapeToolFsmState {
	type ToolData = ShapeToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let mut shape_data = &mut data.data;
		use ShapeMessage::*;
		use ShapeToolFsmState::*;
		if let ToolMessage::Shape(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.drag_start = input.mouse.position;
					responses.push_back(DocumentMessage::StartTransaction.into());
					shape_data.path = Some(vec![generate_hash(&*responses, input, document.document.hash())]);
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					data.sides = match tool_data.tool_options.get(&ToolType::Shape) {
						Some(&ToolOptions::Shape {
							shape_type: ShapeType::Polygon { vertices },
						}) => vertices as u8,
						_ => 6,
					};

					responses.push_back(
						Operation::AddShape {
							path: shape_data.path.clone().unwrap(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							sides: data.sides,
							style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
						}
						.into(),
					);

					Dragging
				}
				(state, Resize(message)) => {
					shape_data.process_action(message, input, responses);

					state
				}
				(Dragging, DragStop) => {
					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					match shape_data.drag_start == input.mouse.position {
						true => responses.push_back(DocumentMessage::AbortTransaction.into()),
						false => responses.push_back(DocumentMessage::CommitTransaction.into()),
					}

					shape_data.path = None;
					Ready
				}
				(Dragging, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());
					shape_data.path = None;

					Ready
				}
				_ => self,
			}
		} else {
			self
		}
	}
}
