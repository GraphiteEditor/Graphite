use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_document_node_type};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, NodeGraphLayer};
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_spiral_parameters;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::tool_messages::shape_tool::ShapeOptionsUpdate;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::algorithms::shapes::{calculate_growth_factor, spiral_point};
use graphene_std::vector::misc::SpiralType;
use std::collections::VecDeque;

/// Calculates the position of a spiral endpoint at a given angle offset (0 = start, TAU = end).
pub fn calculate_spiral_endpoints(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, viewport: DAffine2, theta: f64) -> Option<DVec2> {
	let (spiral_type, start_angle, a, outer_radius, turns, _) = extract_spiral_parameters(layer, document)?;
	let b = calculate_growth_factor(a, turns, outer_radius, spiral_type);
	let theta = turns * theta + start_angle.to_radians();

	Some(viewport.transform_point2(spiral_point(theta, a, b, spiral_type)))
}

#[derive(Default)]
pub struct Spiral;

impl Spiral {
	pub fn create_node(spiral_type: SpiralType, turns: f64) -> NodeTemplate {
		let inner_radius = match spiral_type {
			SpiralType::Archimedean => 0.,
			SpiralType::Logarithmic => 0.1,
		};

		let identifier = DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::spiral::IDENTIFIER);
		let node_type = resolve_document_node_type(&identifier).expect("Spiral node can't be found");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::SpiralType(spiral_type), false)),
			Some(NodeInput::value(TaggedValue::F64(turns), false)),
			Some(NodeInput::value(TaggedValue::F64(0.), false)),
			Some(NodeInput::value(TaggedValue::F64(inner_radius), false)),
			Some(NodeInput::value(TaggedValue::F64(0.1), false)),
			Some(NodeInput::value(TaggedValue::F64(90.), false)),
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		viewport: &ViewportMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		responses: &mut VecDeque<Message>,
	) {
		use graphene_std::vector::generator_nodes::spiral::*;

		let viewport_drag_start = shape_tool_data.data.viewport_drag_start(document);

		let ignore = vec![layer];
		let snap_data = SnapData::ignore(document, ipp, viewport, &ignore);
		let config = SnapTypeConfiguration::default();
		let document_mouse = document.metadata().document_to_viewport.inverse().transform_point2(ipp.mouse.position);
		let snapped = shape_tool_data.data.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), config);
		let snapped_viewport_point = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);
		shape_tool_data.data.snap_manager.update_indicator(snapped);

		let dragged_distance = (viewport_drag_start - snapped_viewport_point).length() / viewport_zoom(document);

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		let Some(parameters) = NodeGraphLayer::new(layer, &document.network_interface).find_node_parameters(graphene_std::vector::generator_nodes::spiral::IDENTIFIER) else {
			return;
		};

		let Some(&TaggedValue::SpiralType(spiral_type)) = parameters.value(SpiralTypeInput) else {
			return;
		};

		let new_radius = match spiral_type {
			SpiralType::Archimedean => dragged_distance,
			SpiralType::Logarithmic => (dragged_distance).max(0.1),
		};

		responses.add(window_aligned_transform_set(document, layer, viewport_drag_start, DVec2::ONE));

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, OuterRadiusInput),
			input: NodeInput::value(TaggedValue::F64(new_radius), false),
		});
	}

	/// Updates the number of turns of a Spiral node and recalculates its radius based on drag distance.
	/// Also updates the Shape tool's turns UI widget to reflect the change.
	pub fn update_turns(decrease: bool, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		use graphene_std::vector::generator_nodes::spiral::*;

		let Some(parameters) = NodeGraphLayer::new(layer, &document.network_interface).find_node_parameters(graphene_std::vector::generator_nodes::spiral::IDENTIFIER) else {
			return;
		};

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		let Some(&TaggedValue::F64(mut turns)) = parameters.value(TurnsInput) else {
			return;
		};

		if decrease {
			turns = (turns - 1.).max(1.);
		} else {
			turns += 1.;
		}

		responses.add(ShapeToolMessage::UpdateOptions {
			options: ShapeOptionsUpdate::Turns(turns),
		});

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, TurnsInput),
			input: NodeInput::value(TaggedValue::F64(turns), false),
		});
	}
}
