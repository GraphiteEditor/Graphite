use crate::input::keyboard::Key;
use crate::input::InputPreprocessor;
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use document_core::{layers::style, Operation};
use glam::DAffine2;

use super::resize::*;

#[derive(Default)]
pub struct Ellipse {
	fsm_state: EllipseToolFsmState,
	data: EllipseToolData,
}

#[impl_message(Message, ToolMessage, Ellipse)]
#[derive(PartialEq, Clone, Debug, Hash)]
pub enum EllipseMessage {
	DragStart,
	DragStop,
	Resize { center: Key, lock_ratio: Key },
	Abort,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Ellipse {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use EllipseToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(EllipseMessageDiscriminant; DragStart),
			Dragging => actions!(EllipseMessageDiscriminant; DragStop, Abort, Resize),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EllipseToolFsmState {
	Ready,
	Dragging,
}

impl Default for EllipseToolFsmState {
	fn default() -> Self {
		EllipseToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct EllipseToolData {
	sides: u8,
	data: Resize,
}

impl Fsm for EllipseToolFsmState {
	type ToolData = EllipseToolData;

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
		use EllipseMessage::*;
		use EllipseToolFsmState::*;
		if let ToolMessage::Ellipse(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.drag_start = input.mouse.position;
					responses.push_back(DocumentMessage::StartTransaction.into());
					shape_data.path = Some(vec![generate_hash(&*responses, input, document.document.hash())]);
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					responses.push_back(
						Operation::AddEllipse {
							path: shape_data.path.clone().unwrap(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
						}
						.into(),
					);

					Dragging
				}
				(state, Resize { center, lock_ratio }) => {
					if let Some(message) = shape_data.calculate_transform(center, lock_ratio, input) {
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
