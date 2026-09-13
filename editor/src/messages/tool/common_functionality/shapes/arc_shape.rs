use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::misc::ArcType;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Arc;

impl Arc {
	pub fn create_node(arc_type: ArcType) -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::arc::IDENTIFIER).expect("Ellipse node does not exist");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::F64(0.5), false)),
			Some(NodeInput::value(TaggedValue::F64(0.), false)),
			Some(NodeInput::value(TaggedValue::F64(270.), false)),
			Some(NodeInput::value(TaggedValue::ArcType(arc_type), false)),
		])
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
		let (center, lock_ratio) = (modifier[0], modifier[1]);
		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, viewport, center, lock_ratio) {
			let Some(node_id) = graph_modification_utils::get_arc_id(layer, &document.network_interface) else {
				return;
			};

			let dimensions = (start - end).abs();
			let mut aspect = DVec2::ONE;
			let radius: f64;

			// We keep the smaller dimension's scale at 1 and scale the other dimension accordingly
			if dimensions.x > dimensions.y {
				aspect.x = dimensions.x / dimensions.y;
				radius = dimensions.y / 2.;
			} else {
				aspect.y = dimensions.y / dimensions.x;
				radius = dimensions.x / 2.;
			}

			let radius = radius / viewport_zoom(document);

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, graphene_std::vector::generator_nodes::arc::RadiusInput),
				input: NodeInput::value(TaggedValue::F64(radius), false),
			});

			responses.add(window_aligned_transform_set(document, layer, start.midpoint(end), aspect));
		}
	}
}
