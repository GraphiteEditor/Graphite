use crate::messages::message::Message;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeToolModifierKey;
use crate::messages::tool::tool_messages::shape_tool::ShapeToolData;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

/// The heart's size is adjusted via a registry-driven radius gizmo (see the [gizmo registry]), while its
/// parametric controls (cleavage, lobes, shoulder, etc.) are adjusted via the Properties panel.
///
/// [gizmo registry]: crate::messages::tool::common_functionality::gizmos::gizmo_registry
#[derive(Default)]
pub struct Heart;

impl Heart {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::heart::IDENTIFIER).expect("Heart node can't be found");
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
		let [center, lock_ratio, _] = modifier;

		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, viewport, center, lock_ratio) {
			let Some(node_id) = graph_modification_utils::get_heart_id(layer, &document.network_interface) else {
				return;
			};

			let dimensions = (start - end).abs();

			let mut scale = DVec2::ONE;
			let radius: f64;
			if dimensions.x > dimensions.y {
				scale.x = dimensions.x / dimensions.y;
				radius = dimensions.y / 2.;
			} else {
				scale.y = dimensions.y / dimensions.x;
				radius = dimensions.x / 2.;
			}

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 1),
				input: NodeInput::value(TaggedValue::F64(radius), false),
			});

			responses.add(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_scale_angle_translation(scale, 0., (start + end) / 2.),
				transform_in: TransformIn::Viewport,
				skip_rerender: false,
			});
		}
	}
}
