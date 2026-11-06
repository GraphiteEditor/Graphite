use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::heart_handles::{HeartHandles, HeartHandlesState};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::{ShapeGizmoHandler, extract_heart_parameters};
use crate::messages::tool::common_functionality::snap_session::{SnapSession, SnapView};
use crate::messages::tool::tool_messages::tool_prelude::*;
use graphene_std::vector::algorithms::shapes::{HEART_SIZE, HeartProportions, heart_bezpath};
use graphene_std::vector::misc::point_to_dvec2;
use kurbo::ParamCurve;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct HeartGizmoHandler {
	heart_handles: HeartHandles,
}

impl ShapeGizmoHandler for HeartGizmoHandler {
	fn is_any_gizmo_hovered(&self) -> bool {
		self.heart_handles.hovered()
	}

	fn handle_state(&mut self, selected_heart_layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, _responses: &mut VecDeque<Message>) {
		self.heart_handles.handle_actions(selected_heart_layer, mouse_position, document);
	}

	fn handle_click(&mut self) {
		if self.heart_handles.hovered() {
			self.heart_handles.update_state(HeartHandlesState::Dragging);
		}
	}

	fn handle_update(&mut self, drag_start: DVec2, view: SnapView, _snap_session: &mut SnapSession, responses: &mut VecDeque<Message>) {
		if self.heart_handles.is_dragging() {
			self.heart_handles.update(view, drag_start, responses);
		}
	}

	fn overlays(
		&self,
		document: &DocumentMessageHandler,
		selected_heart_layer: Option<LayerNodeIdentifier>,
		_input: &InputPreprocessorMessageHandler,
		_shape_editor: &mut &mut ShapeState,
		_mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		self.heart_handles.overlays(document, selected_heart_layer, overlay_context);
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		_input: &InputPreprocessorMessageHandler,
		_shape_editor: &mut &mut ShapeState,
		_mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if self.heart_handles.is_dragging() {
			self.heart_handles.overlays(document, None, overlay_context);
		}
	}

	fn cleanup(&mut self) {
		self.heart_handles.cleanup();
	}

	fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		(self.heart_handles.hovered() || self.heart_handles.is_dragging()).then(|| self.heart_handles.cursor())
	}
}

/// The anchors of a heart as offsets from the center of its box in units of half its size: the cleft, the right widest point, the tip, and the left widest point.
pub fn heart_points(proportions: &HeartProportions) -> Vec<DVec2> {
	heart_bezpath(proportions).segments().map(|segment| point_to_dvec2(segment.start()) / (HEART_SIZE / 2.)).collect()
}

#[derive(Default)]
pub struct Heart;

impl Heart {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::heart::IDENTIFIER).expect("Heart node can't be found");
		node_type.default_node_template()
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
		let [center, lock_ratio, _] = modifier;
		let points = extract_heart_parameters(layer, document).map(|proportions| heart_points(&proportions)).unwrap_or_default();

		if let Some([start, end]) = shape_tool_data.data.calculate_points(snap_session, view, center, lock_ratio, &points) {
			// The node draws the heart at a set size, which the transform scales to fill the box
			let scale = (start - end).abs() / viewport_zoom(document) / HEART_SIZE;
			responses.add(window_aligned_transform_set(document, layer, start.midpoint(end), scale));
		}
	}
}
