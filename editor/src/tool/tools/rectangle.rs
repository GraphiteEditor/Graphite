use crate::input::keyboard::Key;
use crate::input::InputPreprocessor;
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use glam::DAffine2;
use graphene::{layers::style, Operation};
use serde::{Deserialize, Serialize};

use super::resize::*;

#[derive(Default)]
pub struct Rectangle {
	fsm_state: RectangleToolFsmState,
	data: RectangleToolData,
}

#[impl_message(Message, ToolMessage, Rectangle)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum RectangleMessage {
	DragStart,
	DragStop,
	Resize { center: Key, lock_ratio: Key },
	Abort,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Rectangle {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use RectangleToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(RectangleMessageDiscriminant; DragStart),
			Dragging => actions!(RectangleMessageDiscriminant; DragStop, Abort, Resize),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RectangleToolFsmState {
	Ready,
	Dragging,
}

impl Default for RectangleToolFsmState {
	fn default() -> Self {
		RectangleToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct RectangleToolData {
	data: Resize,
}

impl Fsm for RectangleToolFsmState {
	type ToolData = RectangleToolData;

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
		use RectangleMessage::*;
		use RectangleToolFsmState::*;
		if let ToolMessage::Rectangle(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(document, input.mouse.position);
					responses.push_back(DocumentMessage::StartTransaction.into());
					shape_data.path = Some(vec![generate_uuid()]);
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					responses.push_back(
						Operation::AddRect {
							path: shape_data.path.clone().unwrap(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							style: style::PathStyle::with_mode(None, Some(style::Fill::new(tool_data.primary_color)), document.graphene_document.view_mode),
						}
						.into(),
					);

					Dragging
				}
				(state, Resize { center, lock_ratio }) => {
					if let Some(message) = shape_data.calculate_transform(document, center, lock_ratio, input) {
						responses.push_back(message);
					}

					state
				}
				(Dragging, DragStop) => {
					// TODO: introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					match shape_data.drag_start == input.mouse.position {
						true => responses.push_back(DocumentMessage::AbortTransaction.into()),
						false => responses.push_back(DocumentMessage::CommitTransaction.into()),
					}

					shape_data.cleanup();
					Ready
				}
				(Dragging, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());
					shape_data.cleanup();

					Ready
				}
				_ => self,
			}
		} else {
			self
		}
	}
}
