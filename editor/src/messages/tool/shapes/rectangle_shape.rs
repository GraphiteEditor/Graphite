use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::{DAffine2, DVec2};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use std::collections::VecDeque;

#[derive(Default)]
pub struct Rectangle;

impl Shape for Rectangle {
	fn name() -> &'static str {
		"Rectangle"
	}

	fn icon_name() -> &'static str {
		"VectorRectangleTool"
	}

	fn create_node(document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
		let node_type = resolve_document_node_type("Rectangle").expect("Rectangle node does not exist");
		let node = node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(1.), false)), Some(NodeInput::value(TaggedValue::F64(1.), false))]);
		let nodes = vec![(NodeId(0), node)];

		let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, document.new_layer_bounding_artboard(input), responses);
		responses.add(Message::StartBuffer);
		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., input.mouse.position),
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});

		layer
	}

	fn update_shape(document: &DocumentMessageHandler, _: &InputPreprocessorMessageHandler, layer: LayerNodeIdentifier, start: DVec2, end: DVec2, responses: &mut VecDeque<Message>) -> bool {
		let Some(node_id) = graph_modification_utils::get_rectangle_id(layer, &document.network_interface) else {
			return true;
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::F64((start.x - end.x).abs()), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 2),
			input: NodeInput::value(TaggedValue::F64((start.y - end.y).abs()), false),
		});
		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_translation((start + end) / 2.),
			transform_in: TransformIn::Local,
			skip_rerender: false,
		});
		false
	}
}
