use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeToolModifierKey;
use crate::messages::tool::tool_messages::shape_tool::ShapeToolData;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;

#[derive(Default)]
pub struct Heart;

impl Heart {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_document_node_type("Heart").expect("Heart node can't be found");
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
		let Some(node_id) = graph_modification_utils::get_heart_id(layer, &document.network_interface) else {
			return;
		};

		let dimensions = (start - end).abs();

		let radius: f64 = if dimensions.x > dimensions.y { dimensions.y / 2. } else { dimensions.x / 2. };

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::F64(radius), false),
		});

		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., start.midpoint(end)),
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});
	}
}
