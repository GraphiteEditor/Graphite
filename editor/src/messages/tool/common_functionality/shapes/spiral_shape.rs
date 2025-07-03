use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::tool_messages::shape_tool::ShapeOptionsUpdate;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeId;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::misc::SpiralType;
use std::collections::VecDeque;
use std::f64::consts::TAU;

#[derive(Default)]
pub struct Spiral;

impl Spiral {
	pub fn create_node(spiral_type: SpiralType, turns: f64) -> NodeTemplate {
		let node_type = resolve_document_node_type("Spiral").expect("Spiral node can't be found");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::SpiralType(spiral_type), false)),
			Some(NodeInput::value(TaggedValue::F64(0.001), false)),
			Some(NodeInput::value(TaggedValue::F64(0.1), false)),
			None,
			Some(NodeInput::value(TaggedValue::F64(0.1), false)),
			Some(NodeInput::value(TaggedValue::F64(turns), false)),
		])
	}

	pub fn update_shape(document: &DocumentMessageHandler, ipp: &InputPreprocessorMessageHandler, layer: LayerNodeIdentifier, shape_tool_data: &mut ShapeToolData, responses: &mut VecDeque<Message>) {
		let viewport_drag_start = shape_tool_data.data.viewport_drag_start(document);

		let ignore = vec![layer];
		let snap_data = SnapData::ignore(document, ipp, &ignore);
		let config = SnapTypeConfiguration::default();
		let document_mouse = document.metadata().document_to_viewport.inverse().transform_point2(ipp.mouse.position);
		let snapped = shape_tool_data.data.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), config);
		let snapped_viewport_point = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);
		shape_tool_data.data.snap_manager.update_indicator(snapped);

		let dragged_distance = (viewport_drag_start - snapped_viewport_point).length();

		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Spiral") else {
			return;
		};

		let Some(&TaggedValue::F64(turns)) = node_inputs.get(6).unwrap().as_value() else {
			return;
		};

		Self::update_radius(node_id, dragged_distance, turns, responses);

		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., viewport_drag_start),
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});
	}

	pub fn update_radius(node_id: NodeId, drag_length: f64, turns: f64, responses: &mut VecDeque<Message>) {
		let archimedean_radius = drag_length / (turns * TAU);
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 5),
			input: NodeInput::value(TaggedValue::F64(archimedean_radius), false),
		});

		// 0.2 is the default parameter
		let factor = (0.2 * turns * TAU).exp();
		let logarithmic_radius = drag_length / factor;
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 2),
			input: NodeInput::value(TaggedValue::F64(logarithmic_radius), false),
		});
	}

	/// Updates the number of turns of a spiral node and recalculates its radius based on drag distance.
	/// Also updates the Shape Tool's turns UI widget to reflect the change.
	pub fn update_turns(drag_start: DVec2, decrease: bool, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(node_id) = graph_modification_utils::get_spiral_id(layer, &document.network_interface) else {
			return;
		};

		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Spiral") else {
			return;
		};

		let Some(&TaggedValue::F64(n)) = node_inputs.get(6).unwrap().as_value() else { return };

		let input: NodeInput;
		let turns: f64;
		if decrease {
			turns = (n - 1.).max(1.);
			input = NodeInput::value(TaggedValue::F64(turns), false);
			responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Turns(turns)));
		} else {
			turns = n + 1.;
			input = NodeInput::value(TaggedValue::F64(turns), false);
			responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Turns(turns)));
		}

		let drag_length = drag_start.distance(ipp.mouse.position);

		Self::update_radius(node_id, drag_length, turns, responses);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 6),
			input,
		});
	}
}
