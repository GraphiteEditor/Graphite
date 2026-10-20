use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::circle_arc_radius_handle::{RadiusHandle, RadiusHandleState};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::sweep_angle_gizmo::{SweepAngleGizmo, SweepAngleGizmoState};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shapes::shape_utility::{ShapeGizmoHandler, arc_end_points_ignore_layer, arc_outline, extract_arc_parameters};
use crate::messages::tool::common_functionality::snap_session::{SnapSession, SnapView};
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::misc::ArcType;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct ArcGizmoHandler {
	sweep_angle_gizmo: SweepAngleGizmo,
	arc_radius_handle: RadiusHandle,
}

impl ArcGizmoHandler {
	pub fn new() -> Self {
		Self { ..Default::default() }
	}
}

impl ShapeGizmoHandler for ArcGizmoHandler {
	fn handle_state(&mut self, selected_shape_layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.sweep_angle_gizmo.handle_actions(selected_shape_layer, document, mouse_position);
		self.arc_radius_handle.handle_actions(selected_shape_layer, document, mouse_position, responses);
	}

	fn is_any_gizmo_hovered(&self) -> bool {
		self.sweep_angle_gizmo.hovered() || self.arc_radius_handle.hovered()
	}

	fn handle_click(&mut self) {
		// If hovering over both the gizmos give priority to sweep angle gizmo
		if self.sweep_angle_gizmo.hovered() && self.arc_radius_handle.hovered() {
			self.sweep_angle_gizmo.update_state(SweepAngleGizmoState::Dragging);
			self.arc_radius_handle.update_state(RadiusHandleState::Inactive);
			return;
		}

		if self.sweep_angle_gizmo.hovered() {
			self.sweep_angle_gizmo.update_state(SweepAngleGizmoState::Dragging);
		}

		if self.arc_radius_handle.hovered() {
			self.arc_radius_handle.update_state(RadiusHandleState::Dragging);
		}
	}

	fn handle_update(&mut self, drag_start: DVec2, view: SnapView, snap_session: &mut SnapSession, responses: &mut VecDeque<Message>) {
		if self.sweep_angle_gizmo.is_dragging_or_snapped() {
			self.sweep_angle_gizmo.update_arc(view, snap_session, responses);
		}

		if self.arc_radius_handle.is_dragging() {
			self.arc_radius_handle.update_inner_radius(view, snap_session, responses, drag_start);
		}
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		_shape_editor: &mut &mut crate::messages::tool::common_functionality::shape_editor::ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut crate::messages::portfolio::document::overlays::utility_types::OverlayContext,
	) {
		if self.sweep_angle_gizmo.is_dragging_or_snapped() {
			self.sweep_angle_gizmo.overlays(None, document, input, mouse_position, overlay_context);
			arc_outline(self.sweep_angle_gizmo.layer, document, overlay_context);
		}

		if self.arc_radius_handle.is_dragging() {
			self.sweep_angle_gizmo.overlays(self.arc_radius_handle.layer, document, input, mouse_position, overlay_context);
			self.arc_radius_handle.overlays(document, overlay_context);
		}
	}

	fn overlays(
		&self,
		document: &DocumentMessageHandler,
		selected_shape_layer: Option<LayerNodeIdentifier>,
		input: &InputPreprocessorMessageHandler,
		_shape_editor: &mut &mut crate::messages::tool::common_functionality::shape_editor::ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut crate::messages::portfolio::document::overlays::utility_types::OverlayContext,
	) {
		// If hovering over both the gizmos give priority to sweep angle gizmo
		if self.sweep_angle_gizmo.hovered() && self.arc_radius_handle.hovered() {
			self.sweep_angle_gizmo.overlays(selected_shape_layer, document, input, mouse_position, overlay_context);
			return;
		}

		if self.arc_radius_handle.hovered() {
			let layer = self.arc_radius_handle.layer;

			self.arc_radius_handle.overlays(document, overlay_context);
			self.sweep_angle_gizmo.overlays(layer, document, input, mouse_position, overlay_context);
		}

		self.sweep_angle_gizmo.overlays(selected_shape_layer, document, input, mouse_position, overlay_context);
		self.arc_radius_handle.overlays(document, overlay_context);

		arc_outline(selected_shape_layer.or(self.sweep_angle_gizmo.layer), document, overlay_context);
	}

	fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		if self.sweep_angle_gizmo.hovered() || self.sweep_angle_gizmo.is_dragging_or_snapped() {
			return Some(MouseCursorIcon::Default);
		}

		if self.arc_radius_handle.hovered() || self.arc_radius_handle.is_dragging() {
			return Some(MouseCursorIcon::EWResize);
		}

		None
	}

	fn cleanup(&mut self) {
		self.sweep_angle_gizmo.cleanup();
		self.arc_radius_handle.cleanup();
	}
}
#[derive(Default)]
pub struct Arc;

impl Arc {
	pub fn create_node(arc_type: ArcType) -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::arc::IDENTIFIER).expect("Ellipse node does not exist");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::Number(0.5), false)),
			Some(NodeInput::value(TaggedValue::Number(0.), false)),
			Some(NodeInput::value(TaggedValue::Number(270.), false)),
			Some(NodeInput::value(TaggedValue::ArcType(arc_type), false)),
		])
	}

	pub fn update_shape(
		view: SnapView,
		snap_session: &mut SnapSession,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let document = view.document;
		let (center, lock_ratio) = (modifier[0], modifier[1]);

		// The arc's two ends snap, since the box around its whole circle is not drawn
		let ends = extract_arc_parameters(Some(layer), document)
			.and_then(|(_, start_angle, sweep_angle, _)| arc_end_points_ignore_layer(1., start_angle, sweep_angle, None))
			.map(|(start, end)| vec![start, end])
			.unwrap_or_default();
		if let Some([start, end]) = shape_tool_data.data.calculate_points(snap_session, view, center, lock_ratio, &ends) {
			let Some(node_id) = graph_modification_utils::get_arc_id(layer, &document.network_interface) else {
				return;
			};

			let dimensions = (start - end).abs();
			let mut aspect = DVec2::ONE;

			// We keep the smaller dimension's scale at 1 and scale the other dimension accordingly
			let radius = if dimensions.x > dimensions.y {
				aspect.x = dimensions.x / dimensions.y;
				dimensions.y / 2.
			} else {
				aspect.y = dimensions.y / dimensions.x;
				dimensions.x / 2.
			};

			let radius = radius / viewport_zoom(document);

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, graphene_std::vector::generator_nodes::arc::RadiusInput),
				input: NodeInput::value(TaggedValue::Number(radius), false),
			});

			responses.add(window_aligned_transform_set(document, layer, start.midpoint(end), aspect));
		}
	}
}
