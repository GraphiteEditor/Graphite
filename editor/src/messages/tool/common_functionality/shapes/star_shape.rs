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
		_input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		self.number_of_points_dial.overlays(document, selected_star_layer, shape_editor, mouse_position, overlay_context);
		self.point_radius_handle.overlays(selected_star_layer, document, overlay_context);

		star_outline(selected_star_layer, document, overlay_context);
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
pub struct Star;

impl Star {
	pub fn create_node(vertices: u32) -> NodeTemplate {
		let identifier = DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::star::IDENTIFIER);
		let node_type = resolve_document_node_type(&identifier).expect("Star node can't be found");
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
		viewport: &ViewportMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let [center, lock_ratio, _] = modifier;

		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, viewport, center, lock_ratio) {
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

#[cfg(test)]
mod test_star {
	use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
	use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeType;
	use crate::messages::tool::tool_messages::shape_tool::ShapeOptionsUpdate;
	use crate::test_utils::test_prelude::*;
	use graph_craft::document::value::TaggedValue;

	/// Switch to Star shape type, then manually drag to avoid drag_tool re-selecting and resetting options.
	async fn draw_star(editor: &mut EditorTestUtils, x1: f64, y1: f64, x2: f64, y2: f64, modifier_keys: ModifierKeys) {
		editor.select_tool(ToolType::Shape).await;
		editor
			.handle_message(ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Star),
			})
			.await;
		editor.move_mouse(x1, y1, modifier_keys, MouseKeys::empty()).await;
		editor.left_mousedown(x1, y1, modifier_keys).await;
		editor.move_mouse(x2, y2, modifier_keys, MouseKeys::LEFT).await;
		editor.left_mouseup(x2, y2, modifier_keys).await;
	}

	/// Returns (sides, outer_radius, inner_radius) from the first star node in the document.
	fn get_star_inputs(editor: &EditorTestUtils) -> Option<(u32, f64, f64)> {
		let document = editor.active_document();
		document.metadata().all_layers().find_map(|layer| {
			let inputs = NodeGraphLayer::new(layer, &document.network_interface)
				.find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::star::IDENTIFIER))?;
			let Some(&TaggedValue::U32(sides)) = inputs.get(1).and_then(|i| i.as_value()) else {
				return None;
			};
			let Some(&TaggedValue::F64(outer_radius)) = inputs.get(2).and_then(|i| i.as_value()) else {
				return None;
			};
			let Some(&TaggedValue::F64(inner_radius)) = inputs.get(3).and_then(|i| i.as_value()) else {
				return None;
			};
			Some((sides, outer_radius, inner_radius))
		})
	}

	#[tokio::test]
	async fn star_draw_simple() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_star(&mut editor, 0., 0., 100., 100., ModifierKeys::empty()).await;

		assert_eq!(editor.active_document().metadata().all_layers().count(), 1);
		let (sides, outer_radius, _) = get_star_inputs(&editor).expect("Star node should exist after draw");
		assert!(sides >= 2, "Star should have at least 2 points, got {sides}");
		assert!(outer_radius > 0., "Outer radius should be positive, got {outer_radius}");
	}

	#[tokio::test]
	async fn star_inner_radius_is_half_outer() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_star(&mut editor, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let (_, outer_radius, inner_radius) = get_star_inputs(&editor).expect("Star node should exist");
		assert!(
			(inner_radius - outer_radius / 2.).abs() < 1e-10,
			"Inner radius {inner_radius} should equal outer_radius/2 = {}",
			outer_radius / 2.
		);
	}

	#[tokio::test]
	async fn star_draw_correct_outer_radius() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		// 100x100 drag: dimensions=(100,100), x==y, radius = x/2 = 50
		draw_star(&mut editor, 0., 0., 100., 100., ModifierKeys::empty()).await;

		let (_, outer_radius, _) = get_star_inputs(&editor).expect("Star node should exist");
		assert!((outer_radius - 50.).abs() < 1., "Expected outer radius ~50 for 100x100 drag, got {outer_radius}");
	}

	#[tokio::test]
	async fn star_cancel_rmb_no_layer() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.select_tool(ToolType::Shape).await;
		editor
			.handle_message(ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Star),
			})
			.await;
		editor.drag_tool_cancel_rmb(ToolType::Shape).await;

		assert_eq!(
			editor.active_document().metadata().all_layers().count(),
			0,
			"RMB-cancelled star should not create a layer"
		);
	}
}
