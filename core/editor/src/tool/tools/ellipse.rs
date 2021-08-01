use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use document_core::{layers::style, Operation};
use glam::{DAffine2, DVec2};

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
	MouseMove,
	Abort,
	Center,
	UnCenter,
	LockAspectRatio,
	UnlockAspectRatio,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Ellipse {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use EllipseToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(EllipseMessageDiscriminant; DragStart, Center, UnCenter, LockAspectRatio, UnlockAspectRatio),
			Dragging => actions!(EllipseMessageDiscriminant; DragStop, Center, UnCenter, LockAspectRatio, UnlockAspectRatio, MouseMove, Abort),
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
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	constrain_to_circle: bool,
	center_around_cursor: bool,
	shape_id: Option<LayerId>,
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
		let transform = document.document.root.transform;
		use EllipseMessage::*;
		use EllipseToolFsmState::*;
		if let ToolMessage::Ellipse(event) = event {
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
						data.constrain_to_circle,
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
				(Dragging, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());
					data.shape_id = None;

					Ready
				}
				(Ready, LockAspectRatio) => update_state_no_op(&mut data.constrain_to_circle, true, Ready),
				(Ready, UnlockAspectRatio) => update_state_no_op(&mut data.constrain_to_circle, false, Ready),
				(Dragging, LockAspectRatio) => update_state(|data| &mut data.constrain_to_circle, true, data, responses, Dragging, transform),
				(Dragging, UnlockAspectRatio) => update_state(|data| &mut data.constrain_to_circle, false, data, responses, Dragging, transform),

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

fn update_state_no_op(state: &mut bool, value: bool, new_state: EllipseToolFsmState) -> EllipseToolFsmState {
	*state = value;
	new_state
}

fn update_state(
	state: fn(&mut EllipseToolData) -> &mut bool,
	value: bool,
	data: &mut EllipseToolData,
	responses: &mut VecDeque<Message>,
	new_state: EllipseToolFsmState,
	transform: DAffine2,
) -> EllipseToolFsmState {
	*(state(data)) = value;

	responses.push_back(super::make_transform(
		data.shape_id.unwrap(),
		data.constrain_to_circle,
		data.center_around_cursor,
		data.drag_start,
		data.drag_current,
		transform,
	));

	new_state
}

fn create_layer(data: &EllipseToolData, tool_data: &DocumentToolData) -> Message {
	Operation::AddEllipse {
		path: vec![data.shape_id.unwrap()],
		insert_index: -1,
		transform: DAffine2::ZERO.to_cols_array(),
		style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
	}
	.into()
}
