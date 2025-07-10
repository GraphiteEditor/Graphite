use super::*;
use crate::consts::{SPIRAL_OUTER_RADIUS_INDEX, SPIRAL_TYPE_INDEX};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::arc_spiral_inner_radius_handle::{RadiusGizmo, RadiusGizmoState};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::spiral_tightness_gizmo::{TightnessGizmo, TightnessGizmoState};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::spiral_turns_handle::{SpiralTurns, SpiralTurnsState};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeGizmoHandler;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::tool_messages::shape_tool::ShapeOptionsUpdate;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::misc::SpiralType;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct SpiralGizmoHandler {
	radius_handle: RadiusGizmo,
	turns_handle: SpiralTurns,
	tightness_handle: TightnessGizmo,
}

impl ShapeGizmoHandler for SpiralGizmoHandler {
	fn is_any_gizmo_hovered(&self) -> bool {
		self.radius_handle.hovered() || self.turns_handle.hovered() || self.tightness_handle.hovered()
	}

	fn handle_state(
		&mut self,
		selected_spiral_layer: LayerNodeIdentifier,
		mouse_position: DVec2,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) {
		self.radius_handle.handle_actions(selected_spiral_layer, document, input.mouse.position, responses);
		self.turns_handle.handle_actions(selected_spiral_layer, mouse_position, document, responses);
		self.tightness_handle.handle_actions(selected_spiral_layer, input.mouse.position, document, responses);
	}

	fn handle_click(&mut self) {
		if self.radius_handle.hovered() {
			self.radius_handle.update_state(RadiusGizmoState::Dragging);
			return;
		}

		if self.turns_handle.hovered() && self.tightness_handle.hovered() {
			self.turns_handle.update_state(SpiralTurnsState::Dragging);
			self.tightness_handle.update_state(TightnessGizmoState::Inactive);
			return;
		}

		if self.radius_handle.hovered() && self.tightness_handle.hovered() {
			self.radius_handle.update_state(RadiusGizmoState::Dragging);
			self.tightness_handle.update_state(TightnessGizmoState::Inactive);
			return;
		}

		if self.turns_handle.hovered() {
			self.turns_handle.update_state(SpiralTurnsState::Dragging);
		}

		if self.tightness_handle.hovered() {
			self.tightness_handle.update_state(TightnessGizmoState::Dragging);
		}
	}

	fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		if self.radius_handle.is_dragging() {
			self.radius_handle.update_inner_radius(document, input, responses, drag_start);
		}

		if self.turns_handle.is_dragging() {
			self.turns_handle.update_number_of_turns(document, input, responses);
		}

		if self.tightness_handle.is_dragging() {
			self.tightness_handle.update_number_of_turns(document, input, responses, drag_start);
		}
	}

	fn overlays(
		&self,
		document: &DocumentMessageHandler,
		selected_spiral_layer: Option<LayerNodeIdentifier>,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if self.radius_handle.hovered() && self.tightness_handle.hovered() {
			self.radius_handle.overlays(document, selected_spiral_layer, input, mouse_position, overlay_context);
			return;
		}
		self.radius_handle.overlays(document, selected_spiral_layer, input, mouse_position, overlay_context);
		self.turns_handle.overlays(document, selected_spiral_layer, shape_editor, mouse_position, overlay_context);
		self.tightness_handle.overlays(document, selected_spiral_layer, shape_editor, mouse_position, overlay_context);

		// polygon_outline(selected_polygon_layer, document, overlay_context);
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if self.radius_handle.is_dragging() {
			self.radius_handle.overlays(document, None, input, mouse_position, overlay_context);
		}

		if self.radius_handle.is_dragging() {
			self.turns_handle.overlays(document, None, shape_editor, mouse_position, overlay_context);
		}

		if self.tightness_handle.is_dragging() {
			self.tightness_handle.overlays(document, None, shape_editor, mouse_position, overlay_context);
		}
	}

	fn cleanup(&mut self) {
		// self.number_of_points_dial.cleanup();
		self.radius_handle.cleanup();
		self.turns_handle.cleanup();
		self.tightness_handle.cleanup();
	}
}

#[derive(Default)]
pub struct Spiral;

impl Spiral {
	pub fn create_node(spiral_type: SpiralType, turns: f64) -> NodeTemplate {
		let inner_radius = match spiral_type {
			SpiralType::Archimedean => 0.,
			SpiralType::Logarithmic => 0.1,
		};

		let node_type = resolve_document_node_type("Spiral").expect("Spiral node can't be found");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::SpiralType(spiral_type), false)),
			Some(NodeInput::value(TaggedValue::F64(inner_radius), false)),
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

		let Some(&TaggedValue::SpiralType(spiral_type)) = node_inputs.get(SPIRAL_TYPE_INDEX).unwrap().as_value() else {
			return;
		};

		let new_radius = match spiral_type {
			SpiralType::Archimedean => dragged_distance,
			SpiralType::Logarithmic => (dragged_distance).max(0.1),
		};

		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., viewport_drag_start),
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, SPIRAL_OUTER_RADIUS_INDEX),
			input: NodeInput::value(TaggedValue::F64(new_radius), false),
		});
	}

	/// Updates the number of turns of a spiral node and recalculates its radius based on drag distance.
	/// Also updates the Shape Tool's turns UI widget to reflect the change.
	pub fn update_turns(decrease: bool, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Spiral") else {
			return;
		};

		let Some(&TaggedValue::F64(n)) = node_inputs.get(6).unwrap().as_value() else { return };

		let turns: f64;
		if decrease {
			turns = (n - 1.).max(1.);
			responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Turns(turns)));
		} else {
			turns = n + 1.;
			responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Turns(turns)));
		}
	}
}
