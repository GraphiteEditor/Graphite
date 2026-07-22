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
pub struct QrCode;

impl QrCode {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::qr_code::IDENTIFIER).expect("QR Code node can't be found");
		node_type.node_template_input_override([None, None, Some(NodeInput::value(TaggedValue::Bool(true), false)), Some(NodeInput::value(TaggedValue::F64(1.), false))])
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
			let Some(node_id) = graph_modification_utils::get_qr_code_id(layer, &document.network_interface) else {
				return;
			};

			let side = (start.x - end.x).abs().max((start.y - end.y).abs());

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 3),
				input: NodeInput::value(TaggedValue::F64(side), false),
			});
			responses.add(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_translation(start.min(end)),
				transform_in: TransformIn::Viewport,
				skip_rerender: false,
			});
		}
	}
}
