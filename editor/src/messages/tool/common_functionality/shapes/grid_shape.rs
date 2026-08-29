use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::consts::GRID_ROW_COLUMN_GIZMO_OFFSET;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::ParameterRef;
use graphene_std::vector::misc::GridType;
use graphene_std::vector::misc::dvec2_to_point;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Grid;

impl Grid {
	pub fn create_node(grid_type: GridType) -> NodeTemplate {
		let node_type = resolve_proto_node_type(graphene_std::vector::generator_nodes::grid::IDENTIFIER).expect("Grid can't be found");
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

		let dimensions = dimensions / viewport_zoom(document);

		// Set dimensions/spacing
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, SpacingInput),
			input: NodeInput::value(TaggedValue::DVec2(dimensions), false),
		});

		// Set angle for isometric grids
		if let Some(angle_deg) = angle {
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, AnglesInput),
				input: NodeInput::value(TaggedValue::DVec2(DVec2::splat(angle_deg)), false),
			});
		}

		// Set transform
		responses.add(window_aligned_transform_set(document, layer, translation, DVec2::ONE));
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

// Where a grid's four draggable edges sit, in both the rectangular and the isometric layout. This is the
// grid's own geometry rather than gizmo machinery, so it lives with the shape; the interaction that reads it
// is declared in the gizmo registry.

fn convert_to_gizmo_line(p0: DVec2, p1: DVec2) -> kurbo::Line {
	kurbo::Line {
		p0: dvec2_to_point(p0),
		p1: dvec2_to_point(p1),
	}
}

/// The corners of a rectangular grid, as (top left, top right, bottom right, bottom left).
fn get_corners(columns: u32, rows: u32, spacing: DVec2) -> (DVec2, DVec2, DVec2, DVec2) {
	let (width, height) = (spacing.x, spacing.y);

	let x_distance = (columns - 1) as f64 * width;
	let y_distance = (rows - 1) as f64 * height;

	let point0 = DVec2::ZERO;
	let point1 = DVec2::new(x_distance, 0.);
	let point2 = DVec2::new(x_distance, y_distance);
	let point3 = DVec2::new(0., y_distance);

	(point0, point1, point2, point3)
}

fn get_rectangle_top_line_points(columns: u32, rows: u32, spacing: DVec2) -> (DVec2, DVec2) {
	let (top_left, top_right, _, _) = get_corners(columns, rows, spacing);
	let offset = if columns == 1 || rows == 1 {
		DVec2::ZERO
	} else if columns == 2 {
		DVec2::new(spacing.x * 0.25, 0.)
	} else {
		DVec2::new(spacing.x * 0.5, 0.)
	};

	(top_left + offset, top_right - offset)
}

fn get_rectangle_bottom_line_points(columns: u32, rows: u32, spacing: DVec2) -> (DVec2, DVec2) {
	let (_, _, bottom_right, bottom_left) = get_corners(columns, rows, spacing);
	let offset = if columns == 1 || rows == 1 {
		DVec2::ZERO
	} else if columns == 2 {
		DVec2::new(spacing.x * 0.25, 0.)
	} else {
		DVec2::new(spacing.x * 0.5, 0.)
	};

	(bottom_left + offset, bottom_right - offset)
}

fn get_rectangle_right_line_points(columns: u32, rows: u32, spacing: DVec2) -> (DVec2, DVec2) {
	let (_, top_right, bottom_right, _) = get_corners(columns, rows, spacing);
	let offset = if columns == 1 || rows == 1 {
		DVec2::ZERO
	} else if rows == 2 {
		DVec2::new(0., -spacing.y * 0.25)
	} else {
		DVec2::new(0., -spacing.y * 0.5)
	};

	(top_right - offset, bottom_right + offset)
}

fn get_rectangle_left_line_points(columns: u32, rows: u32, spacing: DVec2) -> (DVec2, DVec2) {
	let (top_left, _, _, bottom_left) = get_corners(columns, rows, spacing);
	let offset = if columns == 1 || rows == 1 {
		DVec2::ZERO
	} else if rows == 2 {
		DVec2::new(0., -spacing.y * 0.25)
	} else {
		DVec2::new(0., -spacing.y * 0.5)
	};

	(top_left - offset, bottom_left + offset)
}

fn calculate_isometric_point(column: u32, row: u32, angles: DVec2, spacing: DVec2) -> DVec2 {
	let tan_a = angles.x.to_radians().tan();
	let tan_b = angles.y.to_radians().tan();

	let spacing = DVec2::new(spacing.y / (tan_a + tan_b), spacing.y);

	let a_angles_eaten = column.div_ceil(2) as f64;
	let b_angles_eaten = (column / 2) as f64;

	let offset_y_fraction = b_angles_eaten * tan_b - a_angles_eaten * tan_a;

	DVec2::new(spacing.x * column as f64, spacing.y * row as f64 + offset_y_fraction * spacing.x)
}

fn calculate_isometric_top_line_points(columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
	let top_left = calculate_isometric_point(0, 0, angles, spacing);
	let top_right = calculate_isometric_point(columns - 1, 0, angles, spacing);

	let offset = if columns == 1 || rows == 1 { DVec2::ZERO } else { DVec2::new(spacing.x * 0.5, 0.) };
	let isometric_spacing = calculate_isometric_offset(spacing, angles);
	let isometric_offset = DVec2::new(0., isometric_spacing.y);
	let end_isometric_offset = if columns.is_multiple_of(2) { DVec2::ZERO } else { DVec2::new(0., isometric_spacing.y) };

	(top_left + offset - isometric_offset, top_right - offset - end_isometric_offset)
}

fn calculate_isometric_bottom_line_points(columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
	let bottom_left = calculate_isometric_point(0, rows - 1, angles, spacing);
	let bottom_right = calculate_isometric_point(columns - 1, rows - 1, angles, spacing);

	let offset = if columns == 1 || rows == 1 { DVec2::ZERO } else { DVec2::new(spacing.x * 0.5, 0.) };
	let isometric_offset = if columns.is_multiple_of(2) {
		let offset = calculate_isometric_offset(spacing, angles);
		DVec2::new(0., offset.y)
	} else {
		DVec2::ZERO
	};

	(bottom_left + offset, bottom_right - offset + isometric_offset)
}

fn calculate_isometric_offset(spacing: DVec2, angles: DVec2) -> DVec2 {
	let first_point = calculate_isometric_point(0, 0, angles, spacing);
	let second_point = calculate_isometric_point(1, 0, angles, spacing);

	DVec2::new(first_point.x - second_point.x, first_point.y - second_point.y)
}

fn calculate_isometric_right_line_points(columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
	let top_right = calculate_isometric_point(columns - 1, 0, angles, spacing);
	let bottom_right = calculate_isometric_point(columns - 1, rows - 1, angles, spacing);

	let offset = if columns == 1 || rows == 1 { DVec2::ZERO } else { DVec2::new(0., -spacing.y * 0.5) };

	(top_right - offset, bottom_right + offset)
}

fn calculate_isometric_left_line_points(columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
	let top_left = calculate_isometric_point(0, 0, angles, spacing);
	let bottom_left = calculate_isometric_point(0, rows - 1, angles, spacing);

	let offset = if columns == 1 || rows == 1 { DVec2::ZERO } else { DVec2::new(0., -spacing.y * 0.5) };

	(top_left - offset, bottom_left + offset)
}

/// One of the four edges a grid's rows and columns are dragged from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowColumnGizmoType {
	Top,
	Bottom,
	Left,
	Right,
}

impl RowColumnGizmoType {
	fn get_line_points(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
		match grid_type {
			GridType::Rectangular => match self {
				Self::Top => get_rectangle_top_line_points(columns, rows, spacing),
				Self::Right => get_rectangle_right_line_points(columns, rows, spacing),
				Self::Bottom => get_rectangle_bottom_line_points(columns, rows, spacing),
				Self::Left => get_rectangle_left_line_points(columns, rows, spacing),
			},
			GridType::Isometric => match self {
				Self::Top => calculate_isometric_top_line_points(columns, rows, spacing, angles),
				Self::Right => calculate_isometric_right_line_points(columns, rows, spacing, angles),
				Self::Bottom => calculate_isometric_bottom_line_points(columns, rows, spacing, angles),
				Self::Left => calculate_isometric_left_line_points(columns, rows, spacing, angles),
			},
		}
	}

	pub fn line(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2) -> kurbo::Line {
		let (p0, p1) = self.get_line_points(grid_type, columns, rows, spacing, angles);
		let direction = self.direction(viewport);
		let gap = GRID_ROW_COLUMN_GIZMO_OFFSET * viewport.inverse().transform_vector2(direction).normalize();

		convert_to_gizmo_line(viewport.transform_point2(p0 + gap), viewport.transform_point2(p1 + gap))
	}

	pub fn rect(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2) -> kurbo::Rect {
		let (p0, p1) = self.get_line_points(grid_type, columns, rows, spacing, angles);
		let direction = self.direction(viewport);
		let gap = GRID_ROW_COLUMN_GIZMO_OFFSET * direction.normalize();

		let (x0, x1) = match self {
			Self::Top | Self::Left => (viewport.transform_point2(p0 + gap), viewport.transform_point2(p1)),
			Self::Bottom | Self::Right => (viewport.transform_point2(p0), viewport.transform_point2(p1 + gap)),
		};

		kurbo::Rect::new(x0.x, x0.y, x1.x, x1.y)
	}

	/// The viewport direction this edge grows in when it is dragged outward.
	pub fn direction(&self, viewport: DAffine2) -> DVec2 {
		match self {
			Self::Top => viewport.transform_vector2(-DVec2::Y),
			Self::Bottom => viewport.transform_vector2(DVec2::Y),
			Self::Right => viewport.transform_vector2(DVec2::X),
			Self::Left => viewport.transform_vector2(-DVec2::X),
		}
	}

	/// The row or column count this edge controls.
	pub fn initial_dimension(&self, rows: u32, columns: u32) -> u32 {
		match self {
			Self::Top | Self::Bottom => rows,
			Self::Left | Self::Right => columns,
		}
	}

	/// The distance between two rows or columns along this edge's axis.
	pub fn spacing(&self, spacing: DVec2, grid_type: GridType, angles: DVec2) -> f64 {
		match self {
			Self::Top | Self::Bottom => spacing.y,
			Self::Left | Self::Right if grid_type == GridType::Rectangular => spacing.x,
			Self::Left | Self::Right => spacing.y / (angles.x.to_radians().tan() + angles.y.to_radians().tan()),
		}
	}

	/// The grid input this edge writes.
	pub fn parameter(&self) -> ParameterRef {
		use graphene_std::vector::generator_nodes::grid::*;

		match self {
			Self::Top | Self::Bottom => RowsInput.into(),
			Self::Left | Self::Right => ColumnsInput.into(),
		}
	}
}
