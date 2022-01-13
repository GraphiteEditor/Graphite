use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessor;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::tool::snapping::SnapHandler;
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

#[remain::sorted]
#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PenMessage {
	Abort,
	Confirm,
	DragStart,
	DragStop,
	PointerMove,
	Undo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenToolFsmState {
	Ready,
	Drawing,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Pen {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use PenToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(PenMessageDiscriminant; Undo, DragStart, DragStop, Confirm, Abort),
			Drawing => actions!(PenMessageDiscriminant; DragStop, PointerMove, Confirm, Abort),
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
	layer_exists: bool,
	snap_handler: SnapHandler,
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

		use PenMessage::*;
		use PenToolFsmState::*;
		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					data.path = Some(vec![generate_uuid()]);
					data.layer_exists = false;

					data.snap_handler.start_snap(document, document.all_layers_sorted(), &[]);
					let snapped_position = data.snap_handler.snap_position(document, input.mouse.position);

					let pos = transform.inverse() * DAffine2::from_translation(snapped_position);

					data.points.push(pos);
					data.next_point = pos;

					data.weight = match tool_data.tool_options.get(&ToolType::Pen) {
						Some(&ToolOptions::Pen { weight }) => weight,
						_ => 5,
					};

					responses.push_back(make_operation(data, tool_data, true));

					Drawing
				}
				(Drawing, DragStop) => {
					let snapped_position = data.snap_handler.snap_position(document, input.mouse.position);
					let pos = transform.inverse() * DAffine2::from_translation(snapped_position);

					// TODO: introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.points.last() != Some(&pos) {
						data.points.push(pos);
						data.next_point = pos;
					}

					responses.push_back(remove_preview(data));
					responses.push_back(make_operation(data, tool_data, true));

					Drawing
				}
				(Drawing, PointerMove) => {
					let snapped_position = data.snap_handler.snap_position(document, input.mouse.position);
					let pos = transform.inverse() * DAffine2::from_translation(snapped_position);
					data.next_point = pos;

					responses.push_back(remove_preview(data));
					responses.push_back(make_operation(data, tool_data, true));

					Drawing
				}
				(Drawing, Confirm) | (Drawing, Abort) => {
					if data.points.len() >= 2 {
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(remove_preview(data));
						responses.push_back(make_operation(data, tool_data, false));
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					data.path = None;
					data.points.clear();
					data.snap_handler.cleanup();

					Ready
				}
				_ => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			PenToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![],
				mouse: Some(MouseMotion::Lmb),
				label: String::from("Draw Path"),
				plus: false,
			}])]),
			PenToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Extend Path"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyEnter])],
					mouse: None,
					label: String::from("End Path"),
					plus: false,
				}]),
			]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}
}

fn remove_preview(data: &PenToolData) -> Message {
	Operation::DeleteLayer { path: data.path.clone().unwrap() }.into()
}

fn make_operation(data: &PenToolData, tool_data: &DocumentToolData, show_preview: bool) -> Message {
	let mut points: Vec<(f64, f64)> = data.points.iter().map(|p| (p.translation.x, p.translation.y)).collect();
	if show_preview {
		points.push((data.next_point.translation.x, data.next_point.translation.y))
	}

	Operation::AddPen {
		path: data.path.clone().unwrap(),
		insert_index: -1,
		transform: DAffine2::IDENTITY.to_cols_array(),
		points,
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, data.weight as f32)), Some(style::Fill::none())),
	}
	.into()
}
