use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::consts::{GRID_ANGLE_INDEX, GRID_SPACING_INDEX};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::grid_row_columns_gizmo::RowColumnGizmo;
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::grid_row_columns_gizmo::RowColumnGizmoState;

use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeGizmoHandler;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::misc::GridType;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct GridGizmoHandler {
	row_column_gizmo: RowColumnGizmo,
}

impl ShapeGizmoHandler for GridGizmoHandler {
	fn is_any_gizmo_hovered(&self) -> bool {
		self.row_column_gizmo.is_hovered()
	}

	fn handle_state(&mut self, selected_grid_layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.row_column_gizmo.handle_actions(selected_grid_layer, mouse_position, document);
	}

	fn handle_click(&mut self) {
		if self.row_column_gizmo.is_hovered() {
			self.row_column_gizmo.update_state(RowColumnGizmoState::Dragging);
		}
	}

	fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		if self.row_column_gizmo.is_dragging() {
			self.row_column_gizmo.update(document, input, responses, drag_start);
		}
	}

	fn overlays(
		&self,
		document: &DocumentMessageHandler,
		selected_grid_layer: Option<LayerNodeIdentifier>,
		_input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		self.row_column_gizmo.overlays(document, selected_grid_layer, shape_editor, mouse_position, overlay_context);
	}

	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		_input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		if self.row_column_gizmo.is_dragging() {
			self.row_column_gizmo.overlays(document, None, shape_editor, mouse_position, overlay_context);
		}
	}

	fn cleanup(&mut self) {
		self.row_column_gizmo.cleanup();
	}

	fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon> {
		if self.row_column_gizmo.is_hovered() || self.row_column_gizmo.is_dragging() {
			Some(self.row_column_gizmo.gizmo_type.mouse_icon());
		}

		None
	}
}

#[derive(Default)]
pub struct Grid;

impl Grid {
	pub fn create_node(grid_type: GridType) -> NodeTemplate {
		let node_type = resolve_document_node_type("Grid").expect("Grid can't be found");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::GridType(grid_type), false)),
			Some(NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false)),
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		grid_type: GridType,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		match grid_type {
			GridType::Rectangular => {
				Self::draw_rectangular_grid(document, layer, shape_tool_data, ipp, responses, modifier);
			}
			GridType::Isometric => {
				Self::draw_isometric_grid(document, layer, shape_tool_data, ipp, responses, modifier);
			}
		}
	}

	pub fn draw_rectangular_grid(
		document: &DocumentMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		ipp: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
		modifier: ShapeToolModifierKey,
	) {
		let [center, lock_ratio, _] = modifier;
		let mut translation = shape_tool_data.data.viewport_drag_start(document);

		let start = shape_tool_data.data.viewport_drag_start(document);
		let end = ipp.mouse.position;
		let mut dimensions = (start - end).abs();

		if ipp.keyboard.key(center) && ipp.keyboard.key(lock_ratio) {
			let max = dimensions.x.max(dimensions.y);
			let distance_to_make_center = max;
			translation = shape_tool_data.data.viewport_drag_start(document) - distance_to_make_center;
			dimensions = 2. * DVec2::splat(max) / 9.;
		} else if ipp.keyboard.key(lock_ratio) {
			let max = dimensions.x.max(dimensions.y);
			dimensions = DVec2::splat(max) / 9.;

			if end.y < start.y {
				translation -= DVec2::new(0., max)
			}

			if end.x < start.x {
				translation -= DVec2::new(max, 0.)
			}
		} else if ipp.keyboard.key(center) {
			let distance_to_make_center = dimensions;
			translation = shape_tool_data.data.viewport_drag_start(document) - distance_to_make_center;
			dimensions = 2. * dimensions / 9.;
		} else {
			dimensions = dimensions / 9.;
			if end.x < start.x {
				translation -= DVec2::new(start.x - end.x, 0.)
			}

			if end.y < start.y {
				translation -= DVec2::new(0., start.y - end.y)
			}
		};

		let Some(node_id) = graph_modification_utils::get_grid_id(layer, &document.network_interface) else {
			return;
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 2),
			input: NodeInput::value(TaggedValue::DVec2(dimensions), false),
		});

		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., translation),
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});
	}

	pub fn draw_isometric_grid(
		document: &DocumentMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		ipp: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
		modifier: ShapeToolModifierKey,
	) {
		let [center, lock_ratio, _] = modifier;
		let mut translation = shape_tool_data.data.viewport_drag_start(document);
		let Some(node_id) = graph_modification_utils::get_grid_id(layer, &document.network_interface) else {
			return;
		};

		let start = shape_tool_data.data.viewport_drag_start(document);
		let end = ipp.mouse.position;
		let mut dimensions = (start - end).abs();

		if ipp.keyboard.key(center) && ipp.keyboard.key(lock_ratio) {
			let distance_to_make_center = DVec2::splat(dimensions.y);
			translation = shape_tool_data.data.viewport_drag_start(document) - distance_to_make_center;
			dimensions = 2. * DVec2::splat(dimensions.y) / 9.;
		} else if ipp.keyboard.key(center) {
			// let mouse_x_position = end.x - start.x;
			// let angle = ((dimensions.y) / (mouse_x_position * 2.)).atan();
			// responses.add(NodeGraphMessage::SetInput {
			// 	input_connector: InputConnector::node(node_id, GRID_ANGLE_INDEX),
			// 	input: NodeInput::value(TaggedValue::DVec2(DVec2::splat((angle).to_degrees())), false),
			// });
			// translation -= DVec2::new((dimensions).x * 2., (dimensions).y * 2.);

			let mouse_x_position = end.x - start.x;
			let angle = ((dimensions.y) / (mouse_x_position * 2.)).atan();
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, GRID_ANGLE_INDEX),
				input: NodeInput::value(TaggedValue::DVec2(DVec2::splat((angle).to_degrees())), false),
			});

			translation -= (end - start) / 2.;
			dimensions = DVec2::splat(dimensions.y) / 9.;

			if end.y < start.y {
				translation -= DVec2::new(0., start.y - end.y)
			}
		} else if ipp.keyboard.key(lock_ratio) {
			let max = dimensions.x.max(dimensions.y);
			dimensions = DVec2::splat(max) / 9.;

			if end.y < start.y {
				translation -= DVec2::new(0., max)
			}

			if end.x < start.x {
				translation -= DVec2::new(max, 0.)
			}

			let mouse_x_position = end.x - start.x;
			let angle = (0.5 as f64).atan();
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, GRID_ANGLE_INDEX),
				input: NodeInput::value(TaggedValue::DVec2(DVec2::splat((angle).to_degrees())), false),
			});
		} else {
			let mouse_x_position = end.x - start.x;
			let angle = ((dimensions.y) / (mouse_x_position * 2.)).atan();
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, GRID_ANGLE_INDEX),
				input: NodeInput::value(TaggedValue::DVec2(DVec2::splat((angle).to_degrees())), false),
			});

			dimensions = DVec2::splat(dimensions.y) / 9.;

			if end.y < start.y {
				translation -= DVec2::new(0., start.y - end.y)
			}
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, GRID_SPACING_INDEX),
			input: NodeInput::value(TaggedValue::DVec2(dimensions), false),
		});

		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., translation),
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});
	}

	pub fn change_row_columns() {}
}
