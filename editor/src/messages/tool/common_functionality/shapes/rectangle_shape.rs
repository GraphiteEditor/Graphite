use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Rectangle;

impl Rectangle {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER).expect("Rectangle node can't be found");
		node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(1.), false)), Some(NodeInput::value(TaggedValue::F64(1.), false))])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		viewport: &ViewportMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let [center, lock_ratio, _] = modifier;

		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, viewport, center, lock_ratio) {
			let Some(node_id) = graph_modification_utils::get_rectangle_id(layer, &document.network_interface) else {
				return;
			};

			// Convert viewport dimensions to document space
			let document_to_viewport = document.metadata().document_to_viewport;
			let viewport_delta = end - start;
			let document_delta = document_to_viewport.inverse().transform_vector2(viewport_delta);
			let document_center = document_to_viewport.inverse().transform_point2(start.midpoint(end));

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 1),
				input: NodeInput::value(TaggedValue::F64(document_delta.x.abs()), false),
			});
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 2),
				input: NodeInput::value(TaggedValue::F64(document_delta.y.abs()), false),
			});
			responses.add(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_translation(document_center),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			});
		}
	}
}
