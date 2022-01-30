use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::MouseMotion;
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo};
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData, ToolType};
use crate::viewport_tools::tool_options::ToolOptions;

use graphene::layers::style;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Freehand {
	fsm_state: FreehandToolFsmState,
	data: FreehandToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Freehand)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum FreehandMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FreehandToolFsmState {
	Ready,
	Drawing,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Freehand {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use FreehandToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(FreehandMessageDiscriminant; DragStart, DragStop, Abort),
			Drawing => actions!(FreehandMessageDiscriminant; DragStop, PointerMove, Abort),
		}
	}
}

impl Default for FreehandToolFsmState {
	fn default() -> Self {
		FreehandToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct FreehandToolData {
	points: Vec<DVec2>,
	weight: u32,
	path: Option<Vec<LayerId>>,
}

impl Fsm for FreehandToolFsmState {
	type ToolData = FreehandToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use FreehandMessage::*;
		use FreehandToolFsmState::*;

		let transform = document.graphene_document.root.transform;

		if let ToolMessage::Freehand(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					data.path = Some(vec![generate_uuid()]);

					let pos = transform.inverse().transform_point2(input.mouse.position);

					data.points.push(pos);

					data.weight = match tool_data.tool_options.get(&ToolType::Freehand) {
						Some(&ToolOptions::Freehand { weight }) => weight,
						_ => 5,
					};

					responses.push_back(make_operation(data, tool_data));

					Drawing
				}
				(Drawing, PointerMove) => {
					let pos = transform.inverse().transform_point2(input.mouse.position);

					if data.points.last() != Some(&pos) {
						data.points.push(pos);
					}

					responses.push_back(remove_preview(data));
					responses.push_back(make_operation(data, tool_data));

					Drawing
				}
				(Drawing, DragStop) | (Drawing, Abort) => {
					if data.points.len() >= 2 {
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(remove_preview(data));
						responses.push_back(make_operation(data, tool_data));
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					data.path = None;
					data.points.clear();

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
			FreehandToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![],
				mouse: Some(MouseMotion::LmbDrag),
				label: String::from("Draw Polyline"),
				plus: false,
			}])]),
			FreehandToolFsmState::Drawing => HintData(vec![]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

fn remove_preview(data: &FreehandToolData) -> Message {
	Operation::DeleteLayer { path: data.path.clone().unwrap() }.into()
}

fn make_operation(data: &FreehandToolData, tool_data: &DocumentToolData) -> Message {
	let points: Vec<(f64, f64)> = data.points.iter().map(|p| (p.x, p.y)).collect();

	Operation::AddPolyline {
		path: data.path.clone().unwrap(),
		insert_index: -1,
		transform: DAffine2::IDENTITY.to_cols_array(),
		points,
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, data.weight as f32)), Some(style::Fill::none())),
	}
	.into()
}
