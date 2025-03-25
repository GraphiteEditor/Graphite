use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Ellipse;

impl Ellipse {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_document_node_type("Ellipse").expect("Ellipse node does not exist");
		node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(0.5), false)), Some(NodeInput::value(TaggedValue::F64(0.5), false))])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) -> bool {
		let (center, lock_ratio) = (modifier[0], modifier[1]);
		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, center, lock_ratio) {
			let Some(node_id) = graph_modification_utils::get_ellipse_id(layer, &document.network_interface) else {
				return true;
			};

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 1),
				input: NodeInput::value(TaggedValue::F64(((start.x - end.x) / 2.).abs()), false),
			});
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 2),
				input: NodeInput::value(TaggedValue::F64(((start.y - end.y) / 2.).abs()), false),
			});
			responses.add(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_translation(start.midpoint(end)),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			});
		}
		false
	}
}
