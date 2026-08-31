use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Teardrop;

impl Teardrop {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::teardrop::IDENTIFIER).expect("Teardrop node can't be found");
		node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(0.5), false)), Some(NodeInput::value(TaggedValue::F64(0.5), false))])
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
			let Some(node_id) = graph_modification_utils::get_teardrop_id(layer, &document.network_interface) else {
				return;
			};

			let radius = ((start - end) / 2. / viewport_zoom(document)).abs();

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, graphene_std::vector::generator_nodes::teardrop::WidthInput),
				input: NodeInput::value(TaggedValue::F64(radius.x), false),
			});
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, graphene_std::vector::generator_nodes::teardrop::HeightInput),
				input: NodeInput::value(TaggedValue::F64(radius.y), false),
			});
			responses.add(window_aligned_transform_set(document, layer, start.midpoint(end), DVec2::ONE));
		}
	}
}
