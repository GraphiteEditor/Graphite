use crate::messages::message::Message;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::{viewport_zoom, window_aligned_transform_set};
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeToolModifierKey;
use crate::messages::tool::tool_messages::shape_tool::ShapeToolData;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

/// The Heart drawing mode. Its canvas gizmos are declared in the
/// [gizmo registry](crate::messages::tool::common_functionality::gizmos::gizmo_registry); the rest of its
/// shaping parameters are set from the Properties panel.
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

			// In document units, as every other generator does, or the heart comes out the wrong size at any
			// zoom but 100%.
			let dimensions = ((start - end) / viewport_zoom(document)).abs();

			// A drag that is exactly horizontal or vertical, or has not moved, leaves one dimension at zero.
			// Dividing by it would write an infinite or NaN scale, so skip the frame and keep the last size.
			if dimensions.x == 0. || dimensions.y == 0. {
				return;
			}

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
				input_connector: InputConnector::node(node_id, graphene_std::vector::generator_nodes::heart::RadiusInput),
				input: NodeInput::value(TaggedValue::F64(radius), false),
			});

			// Through the shared helper, as every other aspect-stretched shape does. A hand-built transform
			// sends a bare aspect ratio through `TransformIn::Viewport`, which divides by the zoom, leaving a
			// document scale of `aspect / zoom`. The helper multiplies by the zoom first.
			responses.add(window_aligned_transform_set(document, layer, (start + end) / 2., scale));
		}
	}
}
