use crate::consts::DRAG_THRESHOLD;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeysGroup, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use graphene::Operation;

use glam::DAffine2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct NodeGraphFrameTool {
	fsm_state: NodeGraphToolFsmState,
	tool_data: NodeGraphToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, NodeGraphFrame)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum NodeGraphFrameToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	DragStart,
	DragStop,
	Resize {
		center: Key,
		lock_ratio: Key,
	},
}

impl PropertyHolder for NodeGraphFrameTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for NodeGraphFrameTool {
	fn process_message(&mut self, message: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if message == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if message == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(message, &mut self.tool_data, tool_data, &(), responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use NodeGraphToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(NodeGraphFrameToolMessageDiscriminant;
				DragStart,
			),
			Drawing => actions!(NodeGraphFrameToolMessageDiscriminant;
				DragStop,
				Abort,
				Resize,
			),
		}
	}
}

impl ToolMetadata for NodeGraphFrameTool {
	fn icon_name(&self) -> String {
		"RasterNodesTool".into()
	}
	fn tooltip(&self) -> String {
		"Node Graph Frame Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::NodeGraphFrame
	}
}

impl ToolTransition for NodeGraphFrameTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: None,
			tool_abort: Some(NodeGraphFrameToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeGraphToolFsmState {
	Ready,
	Drawing,
}

impl Default for NodeGraphToolFsmState {
	fn default() -> Self {
		NodeGraphToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct NodeGraphToolData {
	data: Resize,
}

impl Fsm for NodeGraphToolFsmState {
	type ToolData = NodeGraphToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, _document_id, _global_tool_data, input, font_cache): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use NodeGraphFrameToolMessage::*;
		use NodeGraphToolFsmState::*;

		let mut shape_data = &mut tool_data.data;

		if let ToolMessage::NodeGraphFrame(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(responses, document, input.mouse.position, font_cache);
					responses.push_back(DocumentMessage::StartTransaction.into());
					shape_data.path = Some(document.get_path_for_new_layer());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					use graph_craft::{document::*, generic, proto::*};
					let network = NodeNetwork {
						inputs: vec![0],
						output: 1,
						nodes: [
							(
								0,
								DocumentNode {
									name: "Input".into(),
									inputs: vec![NodeInput::Network],
									implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode", &[generic!("T")])),
									metadata: DocumentNodeMetadata { position: (8, 4) },
								},
							),
							(
								1,
								DocumentNode {
									name: "Output".into(),
									inputs: vec![NodeInput::Node(0)],
									implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode", &[generic!("T")])),
									metadata: DocumentNodeMetadata { position: (20, 4) },
								},
							),
						]
						.into_iter()
						.collect(),
					};

					responses.push_back(
						Operation::AddNodeGraphFrame {
							path: shape_data.path.clone().unwrap(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							network,
						}
						.into(),
					);

					Drawing
				}
				(state, Resize { center, lock_ratio }) => {
					if let Some(message) = shape_data.calculate_transform(responses, document, center, lock_ratio, input) {
						responses.push_back(message);
					}

					state
				}
				(Drawing, DragStop) => {
					match shape_data.drag_start.distance(input.mouse.position) <= DRAG_THRESHOLD {
						true => responses.push_back(DocumentMessage::AbortTransaction.into()),
						false => responses.push_back(DocumentMessage::CommitTransaction.into()),
					}

					shape_data.cleanup(responses);

					Ready
				}
				(Drawing, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());

					shape_data.cleanup(responses);

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
			NodeGraphToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![],
					key_groups_mac: None,
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Draw Repaint Frame"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Shift]).into()],
					key_groups_mac: None,
					mouse: None,
					label: String::from("Constrain Square"),
					plus: true,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Alt]).into()],
					key_groups_mac: None,
					mouse: None,
					label: String::from("From Center"),
					plus: true,
				},
			])]),
			NodeGraphToolFsmState::Drawing => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Shift]).into()],
					key_groups_mac: None,
					mouse: None,
					label: String::from("Constrain Square"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Alt]).into()],
					key_groups_mac: None,
					mouse: None,
					label: String::from("From Center"),
					plus: false,
				},
			])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair }.into());
	}
}
