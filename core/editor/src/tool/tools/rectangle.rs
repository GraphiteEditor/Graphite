use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use document_core::{layers::style, Operation};
use glam::{DAffine2, DVec2};

#[derive(Default)]
pub struct Rectangle {
	fsm_state: RectangleToolFsmState,
	data: RectangleToolData,
}

#[impl_message(Message, ToolMessage, Rectangle)]
#[derive(PartialEq, Clone, Debug, Hash)]
pub enum RectangleMessage {
	DragStart,
	DragStop,
	MouseMove,
	Abort,
	Center,
	UnCenter,
	LockAspectRatio,
	UnlockAspectRatio,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Rectangle {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use RectangleToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(RectangleMessageDiscriminant; DragStart, Center, UnCenter, LockAspectRatio, UnlockAspectRatio),
			Dragging => actions!(RectangleMessageDiscriminant; DragStop, Center, UnCenter, LockAspectRatio, UnlockAspectRatio, MouseMove, Abort),
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
#[derive(Clone, Debug, Default, Hash, Eq, PartialEq)]
struct RectangleToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	constrain_to_square: bool,
	center_around_cursor: bool,
	shape_id: Option<LayerId>,
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
		let transform = document.document.root.transform;
		use RectangleMessage::*;
		use RectangleToolFsmState::*;
		if let ToolMessage::Rectangle(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;
					responses.push_back(DocumentMessage::StartTransaction.into());
					data.shape_id = Some(generate_hash(&*responses, input, document.document.hash()));
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					responses.push_back(create_layer(data, tool_data));
					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse.position;
					responses.push_back(super::make_transform(
						data.shape_id.unwrap(),
						data.constrain_to_square,
						data.center_around_cursor,
						data.drag_start,
						data.drag_current,
						transform,
					));

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse.position;

					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					match data.drag_start == data.drag_current {
						true => responses.push_back(DocumentMessage::AbortTransaction.into()),
						false => responses.push_back(DocumentMessage::CommitTransaction.into()),
					}

					data.shape_id = None;
					Ready
				}
				// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
				(Dragging, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());
					data.shape_id = None;

					Ready
				}
				(Ready, LockAspectRatio) => update_state_no_op(&mut data.constrain_to_square, true, Ready),
				(Ready, UnlockAspectRatio) => update_state_no_op(&mut data.constrain_to_square, false, Ready),
				(Dragging, LockAspectRatio) => update_state(|data| &mut data.constrain_to_square, true, data, responses, Dragging, transform),
				(Dragging, UnlockAspectRatio) => update_state(|data| &mut data.constrain_to_square, false, data, responses, Dragging, transform),

				(Ready, Center) => update_state_no_op(&mut data.center_around_cursor, true, Ready),
				(Ready, UnCenter) => update_state_no_op(&mut data.center_around_cursor, false, Ready),
				(Dragging, Center) => update_state(|data| &mut data.center_around_cursor, true, data, responses, Dragging, transform),
				(Dragging, UnCenter) => update_state(|data| &mut data.center_around_cursor, false, data, responses, Dragging, transform),
				_ => self,
			}
		} else {
			self
		}
	}
}

fn update_state_no_op(state: &mut bool, value: bool, new_state: RectangleToolFsmState) -> RectangleToolFsmState {
	*state = value;
	new_state
}

fn update_state(
	state: fn(&mut RectangleToolData) -> &mut bool,
	value: bool,
	data: &mut RectangleToolData,
	responses: &mut VecDeque<Message>,
	new_state: RectangleToolFsmState,
	transform: DAffine2,
) -> RectangleToolFsmState {
	*(state(data)) = value;

	responses.push_back(super::make_transform(
		data.shape_id.unwrap(),
		data.constrain_to_square,
		data.center_around_cursor,
		data.drag_start,
		data.drag_current,
		transform,
	));

	new_state
}

fn create_layer(data: &RectangleToolData, tool_data: &DocumentToolData) -> Message {
	Operation::AddEllipse {
		path: vec![data.shape_id.unwrap()],
		insert_index: -1,
		transform: DAffine2::ZERO.to_cols_array(),
		style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
	}
	.into()
}
