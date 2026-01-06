use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Arrow;

impl Arrow {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_document_node_type("Arrow").expect("Arrow node does not exist");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false)),          // start
			Some(NodeInput::value(TaggedValue::DVec2(DVec2::new(100., 0.)), false)), // end
			Some(NodeInput::value(TaggedValue::F64(10.), false)),                    // shaft_width
			Some(NodeInput::value(TaggedValue::F64(30.), false)),                    // head_width
			Some(NodeInput::value(TaggedValue::F64(20.), false)),                    // head_length
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let [center, lock_ratio, _] = modifier;

		let Some([start, end]) = tool_data.data.calculate_points(document, input, &document.viewport_message_handler, lock_ratio, center) else {
			return;
		};

		let delta = end - start;
		let length = delta.length();
		if length < 1e-6 {
			return;
		}

		let Some(node_id) = graph_modification_utils::get_arrow_id(layer, &document.network_interface) else {
			return;
		};

		// Calculate proportional dimensions
		let shaft_width = length * 0.1;
		let head_width = length * 0.3;
		let head_length = length * 0.2;

		// Update Arrow node parameters - now using start/end points instead of transform
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::DVec2(start), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 2),
			input: NodeInput::value(TaggedValue::DVec2(end), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 3),
			input: NodeInput::value(TaggedValue::F64(shaft_width), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 4),
			input: NodeInput::value(TaggedValue::F64(head_width), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 5),
			input: NodeInput::value(TaggedValue::F64(head_length), false),
		});
	}

	pub fn overlays(_document: &DocumentMessageHandler, _tool_data: &ShapeToolData, _overlay_context: &mut OverlayContext) {}
}
