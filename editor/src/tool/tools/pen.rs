use crate::input::InputPreprocessor;
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData, ToolOptions, ToolType};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use glam::DAffine2;
use graphene::{layers::style, Operation};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Pen {
	fsm_state: PenToolFsmState,
	data: PenToolData,
}

#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PenMessage {
	Undo,
	DragStart,
	DragStop,
	PointerMove,
	Confirm,
	Abort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenToolFsmState {
	Ready,
	Dragging,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Pen {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use PenToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(PenMessageDiscriminant; Undo, DragStart, DragStop, Confirm, Abort),
			Dragging => actions!(PenMessageDiscriminant; DragStop, PointerMove, Confirm, Abort),
		}
	}
}

impl Default for PenToolFsmState {
	fn default() -> Self {
		PenToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct PenToolData {
	points: Vec<DAffine2>,
	next_point: DAffine2,
	weight: u32,
	path: Option<Vec<LayerId>>,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let transform = document.graphene_document.root.transform;
		let pos = transform.inverse() * DAffine2::from_translation(input.mouse.position);

		use PenMessage::*;
		use PenToolFsmState::*;
		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					data.path = Some(vec![generate_uuid()]);

					data.points.push(pos);
					data.next_point = pos;

					data.weight = match tool_data.tool_options.get(&ToolType::Pen) {
						Some(&ToolOptions::Pen { weight }) => weight,
						_ => 5,
					};

					Dragging
				}
				(Dragging, DragStop) => {
					// TODO: introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.points.last() != Some(&pos) {
						data.points.push(pos);
						data.next_point = pos;
					}

					responses.extend(make_operation(data, tool_data, true));

					Dragging
				}
				(Dragging, PointerMove) => {
					data.next_point = pos;

					responses.extend(make_operation(data, tool_data, true));

					Dragging
				}
				(Dragging, Confirm) => {
					if data.points.len() >= 2 {
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.extend(make_operation(data, tool_data, false));
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					data.path = None;
					data.points.clear();

					Ready
				}
				(Dragging, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());
					data.points.clear();
					data.path = None;

					Ready
				}
				_ => self,
			}
		} else {
			self
		}
	}
}

fn make_operation(data: &PenToolData, tool_data: &DocumentToolData, show_preview: bool) -> [Message; 2] {
	let mut points: Vec<(f64, f64)> = data.points.iter().map(|p| (p.translation.x, p.translation.y)).collect();
	if show_preview {
		points.push((data.next_point.translation.x, data.next_point.translation.y))
	}
	[
		Operation::DeleteLayer { path: data.path.clone().unwrap() }.into(),
		Operation::AddPen {
			path: data.path.clone().unwrap(),
			insert_index: -1,
			transform: DAffine2::IDENTITY.to_cols_array(),
			points,
			style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, data.weight as f32)), Some(style::Fill::none())),
		}
		.into(),
	]
}
