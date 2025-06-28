use super::shape_utility::{ShapeToolModifierKey, update_radius_sign};
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::number_of_points_dial::{NumberOfPointsDial, NumberOfPointsDialState};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::point_radius_handle::{PointRadiusHandle, PointRadiusHandleState};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::{ShapeGizmoHandler, star_outline};
use crate::messages::tool::tool_messages::tool_prelude::*;
use core::f64;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct StarGizmoHandler {
	number_of_points_dial: NumberOfPointsDial,
	point_radius_handle: PointRadiusHandle,
}

impl ShapeGizmoHandler for StarGizmoHandler {
	fn is_any_gizmo_hovered(&self) -> bool {
		self.number_of_points_dial.is_hovering() || self.point_radius_handle.hovered()
	}

	fn handle_state(&mut self, selected_star_layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.number_of_points_dial.handle_actions(selected_star_layer, mouse_position, document, responses);
		self.point_radius_handle.handle_actions(selected_star_layer, document, mouse_position, responses);
	}

	fn handle_click(&mut self) {
		if self.number_of_points_dial.is_hovering() {
			self.number_of_points_dial.update_state(NumberOfPointsDialState::Dragging);
			return;
		}

		if self.point_radius_handle.hovered() {
			self.point_radius_handle.update_state(PointRadiusHandleState::Dragging);
		}
	}

	fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		if self.number_of_points_dial.is_dragging() {
			self.number_of_points_dial.update_number_of_sides(document, input, responses, drag_start);
		}

		if self.point_radius_handle.is_dragging_or_snapped() {
			self.point_radius_handle.update_inner_radius(document, input, responses, drag_start);
		}
	}

	fn overlays(
		&self,
		document: &DocumentMessageHandler,
		selected_star_layer: Option<LayerNodeIdentifier>,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		self.number_of_points_dial.overlays(document, selected_star_layer, shape_editor, mouse_position, overlay_context);
		self.point_radius_handle.overlays(selected_star_layer, document, input, mouse_position, overlay_context);

		star_outline(selected_star_layer, document, overlay_context);
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if self.number_of_points_dial.is_dragging() {
			self.number_of_points_dial.overlays(document, None, shape_editor, mouse_position, overlay_context);
		}

		if self.point_radius_handle.is_dragging_or_snapped() {
			self.point_radius_handle.overlays(None, document, input, mouse_position, overlay_context);
		}
	}

	fn cleanup(&mut self) {
		self.number_of_points_dial.cleanup();
		self.point_radius_handle.cleanup();
	}
}

#[derive(Default)]
pub struct Star;

impl Star {
	pub fn create_node(vertices: u32) -> NodeTemplate {
		let node_type = resolve_document_node_type("Star").expect("Star node can't be found");
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
		let [center, lock_ratio, _, _] = modifier;

		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, center, lock_ratio) {
			// TODO: We need to determine how to allow the polygon node to make irregular shapes
			update_radius_sign(end, start, layer, document, responses);

			let dimensions = (start - end).abs();

			// We keep the smaller dimension's scale at 1 and scale the other dimension accordingly
			let mut scale = DVec2::ONE;
			let radius: f64;
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
				input: NodeInput::value(TaggedValue::F64(radius / 2.), false),
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
