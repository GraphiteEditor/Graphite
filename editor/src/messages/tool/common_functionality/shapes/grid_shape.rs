use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::grid_rows_columns_gizmo::{RowColumnGizmo, RowColumnGizmoState};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::ShapeGizmoHandler;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::NodeInputDecleration;
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

	fn handle_state(&mut self, selected_grid_layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, _responses: &mut VecDeque<Message>) {
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
			return Some(self.row_column_gizmo.gizmo_type.mouse_icon());
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
		use graphene_std::vector::generator_nodes::grid::*;

		let [center, lock_ratio, _] = modifier;
		let is_isometric = grid_type == GridType::Isometric;

		let Some(node_id) = graph_modification_utils::get_grid_id(layer, &document.network_interface) else {
			return;
		};

		let start = shape_tool_data.data.viewport_drag_start(document);
		let end = ipp.mouse.position;

		let (translation, dimensions, angle) = calculate_grid_params(start, end, is_isometric, ipp.keyboard.key(center), ipp.keyboard.key(lock_ratio));

		// Set dimensions/spacing
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, SpacingInput::<f64>::INDEX),
			input: NodeInput::value(TaggedValue::DVec2(dimensions), false),
		});

		// Set angle for isometric grids
		if let Some(angle_deg) = angle {
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, AnglesInput::INDEX),
				input: NodeInput::value(TaggedValue::DVec2(DVec2::splat(angle_deg)), false),
			});
		}

		// Set transform
		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., translation),
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});
	}
}

fn calculate_grid_params(start: DVec2, end: DVec2, is_isometric: bool, center: bool, lock_ratio: bool) -> (DVec2, DVec2, Option<f64>) {
	let raw_dimensions = (start - end).abs();
	let mouse_delta = end - start;
	let dimensions;
	let mut translation = start;
	let mut angle = None;

	match (center, lock_ratio) {
		// Both center and lock_ratio: centered + square/fixed-angle grid
		(true, true) => {
			if is_isometric {
				// Fix angle at 30° - standardized isometric view
				angle = Some(30.);

				// Calculate the width based on given height and angle 30°
				let width = calculate_isometric_x_position(raw_dimensions.y / 9., 30_f64.to_radians(), 30_f64.to_radians()).abs();

				// To make draw from center: shift x by half of width and y by half of height (mouse_delta.y)
				translation -= DVec2::new(width / 2., mouse_delta.y / 2.);
				dimensions = DVec2::splat(raw_dimensions.y) / 9.;

				// Adjust for negative upward drag - compensate for coordinate system
				if end.y < start.y {
					translation -= DVec2::new(0., start.y - end.y);
				}
			} else {
				// We want to make both dimensions the same so we choose whichever is bigger and shift to make center
				let max = raw_dimensions.x.max(raw_dimensions.y);
				let distance_to_center = max;
				translation = start - distance_to_center;
				dimensions = 2. * DVec2::splat(max) / 9.; // 2x because centering halves the effective area
			}
		}

		// Only center: centered grid with free aspect ratio
		(true, false) => {
			if is_isometric {
				// Calculate angle from mouse movement - dynamic angle based on drag direction
				angle = Some((raw_dimensions.y / (mouse_delta.x * 2.)).atan().to_degrees());

				// To make draw from center: shift by half of mouse movement
				translation -= mouse_delta / 2.;
				dimensions = DVec2::splat(raw_dimensions.y) / 9.;

				// Adjust for upward drag - maintain proper grid positioning
				if end.y < start.y {
					translation -= DVec2::new(0., start.y - end.y);
				}
			} else {
				// Logic: Rectangular centered grid using exact drag proportions
				let distance_to_center = raw_dimensions;
				translation = start - distance_to_center;
				dimensions = 2. * raw_dimensions / 9.; // 2x for centering
			}
		}

		// Only lock_ratio: square/fixed-angle grid from drag start point
		(false, true) => {
			let max: f64;
			if is_isometric {
				dimensions = DVec2::splat(raw_dimensions.y) / 9.;

				// Use 30° for angle - consistent isometric standard
				angle = Some(30.);
				max = raw_dimensions.y;
			} else {
				// Logic: Force square grid by using larger dimension
				max = raw_dimensions.x.max(raw_dimensions.y);
				dimensions = DVec2::splat(max) / 9.;
			}

			// Adjust for negative drag directions - maintain grid at intended position
			if end.y < start.y {
				translation -= DVec2::new(0., max);
			}
			if end.x < start.x {
				translation -= DVec2::new(max, 0.);
			}
		}

		// Neither center nor lock_ratio: free-form grid following exact user input
		(false, false) => {
			if is_isometric {
				// Calculate angle from mouse movement - fully dynamic
				// Logic: angle represents user's exact intended perspective
				angle = Some((raw_dimensions.y / (mouse_delta.x * 2.)).atan().to_degrees());
				dimensions = DVec2::splat(raw_dimensions.y) / 9.;
			} else {
				// Use exact drag dimensions for grid spacing - what you drag is what you get
				// Logic: Direct mapping of user gesture to grid parameters
				dimensions = raw_dimensions / 9.;

				// Adjust for leftward drag - keep grid positioned correctly
				if end.x < start.x {
					translation -= DVec2::new(start.x - end.x, 0.);
				}
			}

			// Adjust for upward drag (common to both grid types)
			// Logic: compensate for coordinate system where Y increases downward
			if end.y < start.y {
				translation -= DVec2::new(0., start.y - end.y);
			}
		}
	}

	(translation, dimensions, angle)
}

fn calculate_isometric_x_position(y_spacing: f64, rad_a: f64, rad_b: f64) -> f64 {
	let spacing_x = y_spacing / (rad_a.tan() + rad_b.tan());
	spacing_x * 9.
}
