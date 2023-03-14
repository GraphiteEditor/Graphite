use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::portfolio::document::node_graph::{self, IMAGINATE_NODE};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::Operation;

use glam::DAffine2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct ImaginateTool {
	fsm_state: ImaginateToolFsmState,
	tool_data: ImaginateToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Imaginate)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum ImaginateToolMessage {
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

impl PropertyHolder for ImaginateTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for ImaginateTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, true);
	}

	fn actions(&self) -> ActionList {
		use ImaginateToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(ImaginateToolMessageDiscriminant;
				DragStart,
			),
			Drawing => actions!(ImaginateToolMessageDiscriminant;
				DragStop,
				Abort,
				Resize,
			),
		}
	}
}

impl ToolMetadata for ImaginateTool {
	fn icon_name(&self) -> String {
		"RasterImaginateTool".into()
	}
	fn tooltip(&self) -> String {
		"Imaginate Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Imaginate
	}
}

impl ToolTransition for ImaginateTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: None,
			tool_abort: Some(ImaginateToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ImaginateToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
struct ImaginateToolData {
	data: Resize,
}

impl Fsm for ImaginateToolFsmState {
	type ToolData = ImaginateToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, _document_id, _global_tool_data, input, render_data): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use ImaginateToolFsmState::*;
		use ImaginateToolMessage::*;

		let mut shape_data = &mut tool_data.data;

		if let ToolMessage::Imaginate(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(responses, document, input, render_data);
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(NodeGraphMessage::SetDrawing { new_drawing: true }.into());
					shape_data.path = Some(document.get_path_for_new_layer());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					use graph_craft::document::*;

					let imaginate_node_type = &*IMAGINATE_NODE;

					let mut imaginate_inputs: Vec<NodeInput> = imaginate_node_type.inputs.iter().map(|input| input.default.clone()).collect();
					imaginate_inputs[0] = NodeInput::node(0, 0);

					let imaginate_node_id = 100;
					let mut network = node_graph::new_image_network(16, imaginate_node_id);
					network.nodes.insert(
						imaginate_node_id,
						imaginate_node_type.to_document_node(imaginate_inputs, graph_craft::document::DocumentNodeMetadata::position((16, 4))),
					);

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
			ImaginateToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Repaint Frame"),
				HintInfo::keys([Key::Shift], "Constrain Square").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			ImaginateToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair }.into());
	}
}
