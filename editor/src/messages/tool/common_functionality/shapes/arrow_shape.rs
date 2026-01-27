use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Arrow;

impl Arrow {
	pub fn create_node(document: &DocumentMessageHandler, drag_start: DVec2) -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector_nodes::arrow::IDENTIFIER).expect("Arrow node does not exist");
		let viewport_pos = document.metadata().document_to_viewport.transform_point2(drag_start);
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::DVec2(viewport_pos), false)), // start
			Some(NodeInput::value(TaggedValue::DVec2(viewport_pos), false)), // end
			Some(NodeInput::value(TaggedValue::F64(10.), false)),            // shaft_width
			Some(NodeInput::value(TaggedValue::F64(30.), false)),            // head_width
			Some(NodeInput::value(TaggedValue::F64(20.), false)),            // head_length
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		_viewport: &ViewportMessageHandler,
		layer: LayerNodeIdentifier,
		tool_data: &mut ShapeToolData,
		_modifier: ShapeToolModifierKey,
		shaft_width: f64,
		head_width: f64,
		head_length: f64,
		responses: &mut VecDeque<Message>,
	) {
		// Track current mouse position in viewport space
		tool_data.line_data.drag_current = input.mouse.position;

		// Convert both points to document space (matching Line tool pattern)
		let document_to_viewport = document.metadata().document_to_viewport;
		let start_document = tool_data.data.drag_start;
		let end_document = document_to_viewport.inverse().transform_point2(tool_data.line_data.drag_current);

		// Calculate length in document space for validation
		let delta = end_document - start_document;
		let length_document = delta.length();
		if length_document < 1e-6 {
			return;
		}

		let Some(node_id) = graph_modification_utils::get_arrow_id(layer, &document.network_interface) else {
			return;
		};

		// Use fixed dimensions from tool options - only length changes during drag
		// Update Arrow node parameters with document space coordinates (like Line tool)
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::DVec2(start_document), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 2),
			input: NodeInput::value(TaggedValue::DVec2(end_document), false),
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

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn overlays(_document: &DocumentMessageHandler, _tool_data: &ShapeToolData, _overlay_context: &mut OverlayContext) {}
}
