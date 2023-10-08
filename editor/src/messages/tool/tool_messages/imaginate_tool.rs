use super::tool_prelude::*;
use crate::messages::portfolio::document::node_graph::{self, IMAGINATE_NODE};
use crate::messages::tool::common_functionality::path_outline::PathOutline;
use crate::messages::tool::common_functionality::resize::Resize;

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
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	SelectionChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	Resize {
		center: Key,
		lock_ratio: Key,
	},
}

impl LayoutHolder for ImaginateTool {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for ImaginateTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
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
			document_dirty: Some(ImaginateToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(ImaginateToolMessage::Abort.into()),
			selection_changed: Some(ImaginateToolMessage::SelectionChanged.into()),
			..Default::default()
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
	path_outlines: PathOutline,
}

impl Fsm for ImaginateToolFsmState {
	type ToolData = ImaginateToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData { document, input, render_data, .. }: &mut ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let shape_data = &mut tool_data.data;

		let ToolMessage::Imaginate(event) = event else {
			return self;
		};
		match (self, event) {
			(_, ImaginateToolMessage::DocumentIsDirty | ImaginateToolMessage::SelectionChanged) => {
				tool_data.path_outlines.clear_selected(responses);
				//tool_data.path_outlines.update_selected(document.selected_visible_layers(), document, responses, render_data);

				self
			}
			(ImaginateToolFsmState::Ready, ImaginateToolMessage::DragStart) => {
				tool_data.path_outlines.clear_selected(responses);

				shape_data.start(responses, document, input, render_data);
				responses.add(DocumentMessage::StartTransaction);
				shape_data.path = Some(document.get_path_for_new_layer());
				responses.add(DocumentMessage::DeselectAllLayers);

				use graph_craft::document::*;

				// Utility function to offset the position of each consecutive node
				let mut pos = 8;
				let mut next_pos = || {
					pos += 8;
					graph_craft::document::DocumentNodeMetadata::position((pos, 4))
				};

				// Get the node type for the Transform and Imaginate nodes
				let Some(transform_node_type) = crate::messages::portfolio::document::node_graph::resolve_document_node_type("Transform") else {
					warn!("Transform node should be in registry");
					return ImaginateToolFsmState::Drawing;
				};
				let imaginate_node_type = &*IMAGINATE_NODE;

				// Give them a unique ID
				let [transform_node_id, imaginate_node_id] = [100, 101];

				// Create the network based on the Input -> Output passthrough default network
				let mut network = node_graph::new_image_network(16, imaginate_node_id);

				// Insert the nodes into the default network
				network
					.nodes
					.insert(transform_node_id, transform_node_type.to_document_node_default_inputs([Some(NodeInput::node(0, 0))], next_pos()));
				network.nodes.insert(
					imaginate_node_id,
					imaginate_node_type.to_document_node_default_inputs([Some(graph_craft::document::NodeInput::node(transform_node_id, 0))], next_pos()),
				);

				// Add a layer with a frame to the document
				responses.add(Operation::AddFrame {
					path: shape_data.path.clone().unwrap(),
					insert_index: -1,
					transform: DAffine2::ZERO.to_cols_array(),
					network,
				});
				responses.add(NodeGraphMessage::ShiftNode { node_id: imaginate_node_id });

				ImaginateToolFsmState::Drawing
			}
			(state, ImaginateToolMessage::Resize { center, lock_ratio }) => {
				let message = shape_data.calculate_transform(responses, document, input, center, lock_ratio, true);
				responses.try_add(message);

				state
			}
			(ImaginateToolFsmState::Drawing, ImaginateToolMessage::DragStop) => {
				if let Some(layer_path) = &shape_data.path {
					responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path: layer_path.to_vec() });
				}

				input.mouse.finish_transaction(shape_data.viewport_drag_start(document), responses);
				shape_data.cleanup(responses);

				ImaginateToolFsmState::Ready
			}
			(ImaginateToolFsmState::Drawing, ImaginateToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				shape_data.cleanup(responses);
				tool_data.path_outlines.clear_selected(responses);

				ImaginateToolFsmState::Ready
			}
			(_, ImaginateToolMessage::Abort) => {
				tool_data.path_outlines.clear_selected(responses);

				ImaginateToolFsmState::Ready
			}
			_ => self,
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

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}
