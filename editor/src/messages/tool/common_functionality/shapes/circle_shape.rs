use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::{viewport_zoom, window_aligned_transform_set};
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeToolModifierKey;
use crate::messages::tool::tool_messages::shape_tool::ShapeToolData;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;

#[derive(Default)]
pub struct Circle;

impl Circle {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::circle::IDENTIFIER).expect("Circle can't be found");
		node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(0.), false))])
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
		let center = modifier[0];
		let [start, end] = shape_tool_data.data.calculate_circle_points(document, ipp, viewport, center);
		let Some(node_id) = graph_modification_utils::get_circle_id(layer, &document.network_interface) else {
			return;
		};

		let dimensions = ((start - end) / viewport_zoom(document)).abs();

		// We keep the smaller dimension's scale at 1 and scale the other dimension accordingly
		let radius: f64 = if dimensions.x > dimensions.y { dimensions.y / 2. } else { dimensions.x / 2. };

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::F64(radius), false),
		});

		responses.add(window_aligned_transform_set(document, layer, start.midpoint(end), DVec2::ONE));
	}
}
