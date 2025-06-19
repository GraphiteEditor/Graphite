use super::shape_utility::{ShapeToolModifierKey, update_radius_sign};
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use core::f64;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Star;

impl Star {
	pub fn create_node(vertices: u32) -> NodeTemplate {
		let node_type = resolve_document_node_type("Star").expect(" Star node does not exist");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::U32(vertices), false)),
			Some(NodeInput::value(TaggedValue::F64(0.5), false)),
			Some(NodeInput::value(TaggedValue::F64(0.25), false)),
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let (center, lock_ratio) = (modifier[0], modifier[1]);
		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, center, lock_ratio) {
			// TODO: We need to determine how to allow the polygon node to make irregular shapes
			update_radius_sign(end, start, layer, document, responses);

			let dimensions = (start - end).abs();
			let mut scale = DVec2::ONE;
			let radius: f64;

			// We keep the smaller dimension's scale at 1 and scale the other dimension accordingly
			if dimensions.x > dimensions.y {
				scale.x = dimensions.x / dimensions.y;
				radius = dimensions.y / 2.;
			} else {
				scale.y = dimensions.y / dimensions.x;
				radius = dimensions.x / 2.;
			}

			let Some(node_id) = graph_modification_utils::get_star_id(layer, &document.network_interface) else {
				return;
			};

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 2),
				input: NodeInput::value(TaggedValue::F64(radius), false),
			});

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 3),
				input: NodeInput::value(TaggedValue::F64(radius / 2.0), false),
			});

			responses.add(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_scale_angle_translation(scale, 0.0, (start + end) / 2.0),
				transform_in: TransformIn::Viewport,
				skip_rerender: false,
			});
		}
	}
}
