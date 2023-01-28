use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::Operation;

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
		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, true);
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum NodeGraphToolFsmState {
	#[default]
	Ready,
	Drawing,
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
					shape_data.start(responses, document, input, font_cache);
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(NodeGraphMessage::SetDrawing { new_drawing: true }.into());
					shape_data.path = Some(document.get_path_for_new_layer());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					let network = graph_craft::document::NodeNetwork::new_network(20, 0);

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
					input.mouse.finish_transaction(shape_data.viewport_drag_start(document), responses);
					responses.push_back(NodeGraphMessage::SetDrawing { new_drawing: false }.into());
					shape_data.cleanup(responses);

					Ready
				}
				(Drawing, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());
					responses.push_back(NodeGraphMessage::SetDrawing { new_drawing: false }.into());

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
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Repaint Frame"),
				HintInfo::keys([Key::Shift], "Constrain Square").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			NodeGraphToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair }.into());
	}
}
