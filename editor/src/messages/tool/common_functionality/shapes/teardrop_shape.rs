use super::shape_utility::{ShapeToolModifierKey, update_radius_sign};
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_document_node_type};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::number_of_points_dial::{NumberOfPointsDial, NumberOfPointsDialState};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::point_radius_handle::{PointRadiusHandle, PointRadiusHandleState};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeGizmoHandler;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct TeardropGizmoHandler {
	number_of_points_dial: NumberOfPointsDial,
	point_radius_handle: PointRadiusHandle,
}

impl ShapeGizmoHandler for TeardropGizmoHandler {
	fn is_any_gizmo_hovered(&self) -> bool {
		self.number_of_points_dial.is_hovering() || self.point_radius_handle.hovered()
	}

	fn handle_state(&mut self, selected_teardrop_layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.number_of_points_dial.handle_actions(selected_teardrop_layer, mouse_position, document, responses);
		self.point_radius_handle.handle_actions(selected_teardrop_layer, document, mouse_position, responses);
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
		selected_teardrop_layer: Option<LayerNodeIdentifier>,
		_input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		self.number_of_points_dial.overlays(document, selected_teardrop_layer, shape_editor, mouse_position, overlay_context);
		self.point_radius_handle.overlays(selected_teardrop_layer, document, overlay_context);
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		_input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if self.number_of_points_dial.is_dragging() {
			self.number_of_points_dial.overlays(document, None, shape_editor, mouse_position, overlay_context);
		}

		if self.point_radius_handle.is_dragging_or_snapped() {
			self.point_radius_handle.overlays(None, document, overlay_context);
		}
	}

	fn cleanup(&mut self) {
		self.number_of_points_dial.cleanup();
		self.point_radius_handle.cleanup();
	}

	fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		if self.number_of_points_dial.is_dragging() || self.number_of_points_dial.is_hovering() {
			return Some(MouseCursorIcon::EWResize);
		}

		if self.point_radius_handle.is_dragging_or_snapped() || self.point_radius_handle.hovered() {
			return Some(MouseCursorIcon::Default);
		}

		None
	}
}

#[derive(Default)]
pub struct Teardrop;

impl Teardrop {
	pub fn create_node(_vertices: u32) -> NodeTemplate {
		let identifier = DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::teardrop::IDENTIFIER);
		let node_type = resolve_document_node_type(&identifier).expect("Teardrop node can't be found");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::F64(50.), false)),
			Some(NodeInput::value(TaggedValue::F64(50.), false)),
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
		let [center, lock_ratio, _] = modifier;

		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, viewport, center, lock_ratio) {
			update_radius_sign(end, start, layer, document, responses);

			let dimensions = (start - end).abs();
			let radius = dimensions.x / 2.0;
			let tail_length = (dimensions.y - radius).max(0.0);

			let Some(node_id) = graph_modification_utils::get_teardrop_id(layer, &document.network_interface) else {
				return;
			};

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 1),
				input: NodeInput::value(TaggedValue::F64(radius), false),
			});

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 2),
				input: NodeInput::value(TaggedValue::F64(tail_length), false),
			});

			let top = start.y.min(end.y);
			let center_x = (start.x + end.x) / 2.0;
			let center_y = top + tail_length;

			responses.add(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_translation(DVec2::new(center_x, center_y)),
				transform_in: TransformIn::Viewport,
				skip_rerender: false,
			});
		}
	}
}
