use crate::consts::{GRID_COLUMNS_INDEX, GRID_ROW_COLUMN_GIZMO_OFFSET, GRID_ROW_INDEX};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::prelude::{GraphOperationMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_grid_parameters;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::misc::{GridType, dvec2_to_point, get_line_endpoints};
use kurbo::{Line, ParamCurveNearest};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RowColumnGizmoState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RowColumnGizmoType {
	#[default]
	None,
	Top,
	Down,
	Left,
	Right,
}

impl RowColumnGizmoType {
	pub fn get_line_points(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
		match grid_type {
			GridType::Rectangular => match self {
				Self::Top => get_rectangle_top_line_points(columns, rows, spacing),
				Self::Right => get_rectangle_right_line_points(columns, rows, spacing),
				Self::Down => get_rectangle_bottom_line_points(columns, rows, spacing),
				Self::Left => get_rectangle_left_line_points(columns, rows, spacing),
				Self::None => panic!("RowColumnGizmoType::None does not have line points"),
			},
			GridType::Isometric => match self {
				Self::Top => calculate_isometric_top_line_points(columns, spacing, angles),
				Self::Right => calculate_isometric_right_line_points(columns, rows, spacing, angles),
				Self::Down => calculate_isometric_bottom_line_points(columns, rows, spacing, angles),
				Self::Left => calculate_isometric_left_line_points(rows, spacing, angles),
				Self::None => panic!("RowColumnGizmoType::None does not have line points"),
			},
		}
	}
	fn line(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2) -> Line {
		let (p0, p1) = self.get_line_points(grid_type, columns, rows, spacing, angles);
		convert_to_gizmo_line(viewport.transform_point2(p0), viewport.transform_point2(p1))
	}
	fn opposite(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2) -> Line {
		let opposite_gizmo_type = match self {
			Self::Top => Self::Down,
			Self::Right => Self::Left,
			Self::Down => Self::Top,
			Self::Left => Self::Right,
			Self::None => panic!("RowColumnGizmoType::None does not have opposite"),
		};

		opposite_gizmo_type.line(grid_type, columns, rows, spacing, angles, viewport)
	}
	fn direction(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2) -> DVec2 {
		match self {
			RowColumnGizmoType::Top => {
				if grid_type == GridType::Rectangular {
					calculate_rectangle_top_direction(columns, rows, spacing, viewport)
				} else {
					-calculate_isometric_top_direction(angles, spacing, Some(viewport))
				}
			}
			RowColumnGizmoType::Down => {
				if grid_type == GridType::Rectangular {
					-calculate_rectangle_top_direction(columns, rows, spacing, viewport)
				} else {
					calculate_isometric_top_direction(angles, spacing, Some(viewport))
				}
			}
			RowColumnGizmoType::Right => {
				if grid_type == GridType::Rectangular {
					calculate_rectangle_side_direction(columns, rows, spacing, viewport)
				} else {
					calculate_isometric_side_direction(angles, spacing, Some(viewport))
				}
			}
			RowColumnGizmoType::Left => {
				if grid_type == GridType::Rectangular {
					-calculate_rectangle_side_direction(columns, rows, spacing, viewport)
				} else {
					-calculate_isometric_side_direction(angles, spacing, Some(viewport))
				}
			}
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a line"),
		}
	}

	fn initial_dimension(&self, rows: u32, columns: u32) -> u32 {
		match self {
			RowColumnGizmoType::Top | RowColumnGizmoType::Down => rows,
			RowColumnGizmoType::Right | RowColumnGizmoType::Left => columns,
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	fn spacing(&self, spacing: DVec2) -> f64 {
		match self {
			RowColumnGizmoType::Top | RowColumnGizmoType::Down => spacing.y,
			RowColumnGizmoType::Right | RowColumnGizmoType::Left => spacing.x,
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	fn index(&self) -> usize {
		match self {
			RowColumnGizmoType::Top | RowColumnGizmoType::Down => GRID_ROW_INDEX,
			RowColumnGizmoType::Right | RowColumnGizmoType::Left => GRID_COLUMNS_INDEX,
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	fn mouse_icon(&self) -> MouseCursorIcon {
		match self {
			RowColumnGizmoType::Top | RowColumnGizmoType::Down => MouseCursorIcon::NSResize,
			RowColumnGizmoType::Right | RowColumnGizmoType::Left => MouseCursorIcon::EWResize,
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	pub fn all() -> [Self; 4] {
		[Self::Top, Self::Right, Self::Down, Self::Left]
	}
}

#[derive(Clone, Debug, Default)]
pub struct RowColumnGizmo {
	pub layer: Option<LayerNodeIdentifier>,
	pub gizmo_type: RowColumnGizmoType,
	initial_rows: u32,
	initial_columns: u32,
	spacing: DVec2,
	gizmo_state: RowColumnGizmoState,
}

impl RowColumnGizmo {
	pub fn cleanup(&mut self) {
		self.layer = None;
	}

	pub fn update_state(&mut self, state: RowColumnGizmoState) {
		self.gizmo_state = state;
	}

	pub fn is_hovered(&self) -> bool {
		self.gizmo_state == RowColumnGizmoState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.gizmo_state == RowColumnGizmoState::Dragging
	}

	fn initial_dimension(&self) -> u32 {
		match &self.gizmo_type {
			RowColumnGizmoType::Top | RowColumnGizmoType::Down => self.initial_rows,
			RowColumnGizmoType::Right | RowColumnGizmoType::Left => self.initial_columns,
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(layer, document) else {
			return;
		};
		let viewport = document.metadata().transform_to_viewport(layer);
		if let Some(gizmo_type) = check_if_over_gizmo(grid_type, columns, rows, spacing, angles, mouse_position, viewport) {
			self.layer = Some(layer);
			self.gizmo_type = gizmo_type;
			self.initial_rows = rows;
			self.initial_columns = columns;
			self.spacing = spacing;
			self.update_state(RowColumnGizmoState::Hover);
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: self.gizmo_type.mouse_icon() });
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, layer: Option<LayerNodeIdentifier>, _shape_editor: &mut &mut ShapeState, _mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		let Some(layer) = layer.or(self.layer) else { return };
		let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(layer, document) else {
			return;
		};
		let viewport = document.metadata().transform_to_viewport(layer);

		if !matches!(self.gizmo_state, RowColumnGizmoState::Inactive) {
			let (p0, p1) = self.gizmo_type.get_line_points(grid_type, columns, rows, spacing, angles);
			let line = convert_to_gizmo_line(viewport.transform_point2(p0), viewport.transform_point2(p1));
			let (p0, p1) = get_line_endpoints(line);
			overlay_context.dashed_line(p0, p1, None, None, Some(5.), Some(5.), Some(0.5));

			if matches!(self.gizmo_state, RowColumnGizmoState::Hover) {
				let line = self.gizmo_type.opposite(grid_type, columns, rows, spacing, angles, viewport);
				let (p0, p1) = get_line_endpoints(line);
				overlay_context.dashed_line(p0, p1, None, None, Some(5.), Some(5.), Some(0.5));
			}
		}
	}

	pub fn update(&self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else { return };
		let viewport = document.metadata().transform_to_viewport(layer);

		let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(layer, document) else {
			return;
		};
		let direction = self.gizmo_type.direction(grid_type, columns, rows, spacing, angles, viewport);
		let delta_vector = input.mouse.position - drag_start;

		let viewport_spacing = get_viewport_grid_spacing(grid_type, angles, self.spacing, viewport);
		let delta = delta_vector.dot(direction);

		let dimensions_to_add = (delta / (self.gizmo_type.spacing(viewport_spacing))).floor() as i32;
		let new_dimension = (self.initial_dimension() as i32 + dimensions_to_add).max(1) as u32;

		let Some(node_id) = graph_modification_utils::get_grid_id(layer, &document.network_interface) else {
			return;
		};

		let dimensions_delta = new_dimension as i32 - self.gizmo_type.initial_dimension(rows, columns) as i32;
		let transform = self.transform_grid(dimensions_delta, grid_type, columns, rows, viewport_spacing, angles, viewport);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, self.gizmo_type.index()),
			input: NodeInput::value(TaggedValue::U32((self.initial_dimension() as i32 + dimensions_to_add).max(1) as u32), false),
		});

		responses.add(GraphOperationMessage::TransformChange {
			layer,
			transform: transform,
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	fn transform_grid(&self, dimensions_delta: i32, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2) -> DAffine2 {
		match self.gizmo_type {
			RowColumnGizmoType::Top => {
				let move_up_by = self.gizmo_type.direction(grid_type, columns, rows, spacing, angles, viewport) * dimensions_delta as f64 * spacing.y;
				DAffine2::from_translation(move_up_by)
			}
			RowColumnGizmoType::Left => {
				let move_left_by = self.gizmo_type.direction(grid_type, columns, rows, spacing, angles, viewport) * dimensions_delta as f64 * spacing.x;
				DAffine2::from_translation(move_left_by)
			}
			RowColumnGizmoType::Down | RowColumnGizmoType::Right | RowColumnGizmoType::None => DAffine2::IDENTITY,
		}
	}
}

fn check_if_over_gizmo(grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, mouse_position: DVec2, viewport: DAffine2) -> Option<RowColumnGizmoType> {
	let mouse_point = dvec2_to_point(mouse_position);
	let accuracy = 1e-6;
	let threshold = 20.;

	for gizmo_type in RowColumnGizmoType::all() {
		let line = gizmo_type.line(grid_type, columns, rows, spacing, angles, viewport);
		if line.nearest(mouse_point, accuracy).distance_sq < threshold {
			return Some(gizmo_type);
		}
	}

	None
}

fn convert_to_gizmo_line(p0: DVec2, p1: DVec2) -> Line {
	Line {
		p0: dvec2_to_point(p0),
		p1: dvec2_to_point(p1),
	}
}

/// Get corners of the rectangular-grid.
/// Returns a tuple of (topleft,topright,bottomright,bottomleft)
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

fn get_viewport_grid_spacing(grid_type: GridType, angles: DVec2, spacing: DVec2, viewport: DAffine2) -> DVec2 {
	match grid_type {
		GridType::Rectangular => {
			let p0 = DVec2::ZERO;
			let p1 = DVec2::new(spacing.x, 0.);
			let p2 = DVec2::new(0., spacing.y);

			let viewport_spacing_x = (viewport.transform_point2(p0) - viewport.transform_point2(p1)).length();
			let viewport_spacing_y = (viewport.transform_point2(p0) - viewport.transform_point2(p2)).length();

			DVec2::new(viewport_spacing_x, viewport_spacing_y)
		}
		GridType::Isometric => {
			let p0 = calculate_isometric_point(0, 0, angles, spacing);
			let p1 = calculate_isometric_point(1, 0, angles, spacing);
			let p2 = calculate_isometric_point(0, 1, angles, spacing);

			let viewport_spacing_x = viewport.transform_point2(p0).x - viewport.transform_point2(p1).x;
			let viewport_spacing_y = viewport.transform_point2(p0).y - viewport.transform_point2(p2).y;

			DVec2::new(viewport_spacing_x.abs(), viewport_spacing_y.abs())
		}
	}
}

fn get_rectangle_top_line_points(columns: u32, rows: u32, spacing: DVec2) -> (DVec2, DVec2) {
	let (top_left, top_right, _, _) = get_corners(columns, rows, spacing);

	let offset = DVec2::new(spacing.x * 0.25, 0.);
	let spacing_offset = DVec2::new(0., -GRID_ROW_COLUMN_GIZMO_OFFSET);

	(top_left + offset + spacing_offset, top_right - offset + spacing_offset)
}

fn get_rectangle_bottom_line_points(columns: u32, rows: u32, spacing: DVec2) -> (DVec2, DVec2) {
	let (_, _, bottom_right, bottom_left) = get_corners(columns, rows, spacing);

	let offset = DVec2::new(spacing.x * 0.25, 0.);
	let spacing_offset = DVec2::new(0., GRID_ROW_COLUMN_GIZMO_OFFSET);

	(bottom_left + offset + spacing_offset, bottom_right - offset + spacing_offset)
}

fn get_rectangle_right_line_points(columns: u32, rows: u32, spacing: DVec2) -> (DVec2, DVec2) {
	let (_, top_right, bottom_right, _) = get_corners(columns, rows, spacing);

	let offset = DVec2::new(0., -spacing.y * 0.25);
	let spacing_offset = DVec2::new(GRID_ROW_COLUMN_GIZMO_OFFSET, 0.);

	(top_right - offset + spacing_offset, bottom_right + offset + spacing_offset)
}

fn get_rectangle_left_line_points(columns: u32, rows: u32, spacing: DVec2) -> (DVec2, DVec2) {
	let (top_left, _, _, bottom_left) = get_corners(columns, rows, spacing);
	let offset = DVec2::new(0., -spacing.y * 0.25);

	let spacing_offset = DVec2::new(GRID_ROW_COLUMN_GIZMO_OFFSET, 0.);

	(top_left - offset - spacing_offset, bottom_left + offset - spacing_offset)
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

fn calculate_isometric_top_line_points(columns: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
	let top_left = calculate_isometric_point(0, 0, angles, spacing);
	let top_right = calculate_isometric_point(columns - 1, 0, angles, spacing);

	let offset = DVec2::new(spacing.x * 0.25, 0.);
	let isometric_spacing = calculate_isometric_offset(spacing, angles);
	let isometric_offset = DVec2::new(0., isometric_spacing.y);
	let end_isometric_offset = if columns % 2 == 0 { DVec2::ZERO } else { DVec2::new(0., isometric_spacing.y) };
	let spacing_offset = DVec2::new(0., -GRID_ROW_COLUMN_GIZMO_OFFSET);

	(top_left + offset + spacing_offset - isometric_offset, top_right - offset + spacing_offset - end_isometric_offset)
}

fn calculate_isometric_bottom_line_points(columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
	let bottom_left = calculate_isometric_point(0, rows - 1, angles, spacing);
	let bottom_right = calculate_isometric_point(columns - 1, rows - 1, angles, spacing);

	let offset = DVec2::new(spacing.x * 0.25, 0.);
	let isometric_offset = if columns % 2 == 0 {
		let offset = calculate_isometric_offset(spacing, angles);
		DVec2::new(0., offset.y)
	} else {
		DVec2::ZERO
	};
	let spacing_offset = DVec2::new(0., GRID_ROW_COLUMN_GIZMO_OFFSET);

	(bottom_left + offset + spacing_offset, bottom_right - offset + spacing_offset + isometric_offset)
}

fn calculate_isometric_offset(spacing: DVec2, angles: DVec2) -> DVec2 {
	let first_point = calculate_isometric_point(0, 0, angles, spacing);
	let second_point = calculate_isometric_point(1, 0, angles, spacing);

	DVec2::new(first_point.x - second_point.x, first_point.y - second_point.y)
}

fn calculate_isometric_right_line_points(columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
	let top_right = calculate_isometric_point(columns - 1, 0, angles, spacing);
	let bottom_right = calculate_isometric_point(columns - 1, rows - 1, angles, spacing);
	let side_direction = calculate_isometric_side_direction(angles, spacing, None);

	let offset = DVec2::new(0., -spacing.y * 0.25);
	let spacing_offset = GRID_ROW_COLUMN_GIZMO_OFFSET * side_direction;

	(top_right - offset + spacing_offset, bottom_right + offset + spacing_offset)
}

fn calculate_isometric_left_line_points(rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
	let top_left = calculate_isometric_point(0, 0, angles, spacing);
	let bottom_left = calculate_isometric_point(0, rows - 1, angles, spacing);
	let side_direction = calculate_isometric_side_direction(angles, spacing, None);

	let offset = DVec2::new(0., -spacing.y * 0.25);
	let spacing_offset = GRID_ROW_COLUMN_GIZMO_OFFSET * side_direction;

	(top_left - offset - spacing_offset, bottom_left + offset - spacing_offset)
}

fn calculate_rectangle_side_direction(columns: u32, rows: u32, spacing: DVec2, viewport: DAffine2) -> DVec2 {
	let (left, right, _, _) = get_corners(columns, rows, spacing);
	(viewport.transform_point2(right) - viewport.transform_point2(left)).try_normalize().unwrap_or(DVec2::ZERO)
}

fn calculate_rectangle_top_direction(columns: u32, rows: u32, spacing: DVec2, viewport: DAffine2) -> DVec2 {
	let (left, _, _, bottom_left) = get_corners(columns, rows, spacing);

	(viewport.transform_point2(left) - viewport.transform_point2(bottom_left)).try_normalize().unwrap_or(DVec2::ZERO)
}

fn calculate_isometric_side_direction(angles: DVec2, spacing: DVec2, viewport: Option<DAffine2>) -> DVec2 {
	let first_point = calculate_isometric_point(0, 0, angles, spacing);
	let first_row_last_column = calculate_isometric_point(2, 0, angles, spacing);

	if let Some(viewport) = viewport {
		return (viewport.transform_point2(first_row_last_column) - viewport.transform_point2(first_point))
			.try_normalize()
			.unwrap_or_default();
	}

	(first_row_last_column - first_point).try_normalize().unwrap_or_default()
}

fn calculate_isometric_top_direction(angles: DVec2, spacing: DVec2, viewport: Option<DAffine2>) -> DVec2 {
	let first_point = calculate_isometric_point(0, 0, angles, spacing);
	let first_row_last_column = calculate_isometric_point(0, 1, angles, spacing);

	if let Some(viewport) = viewport {
		return (viewport.transform_point2(first_row_last_column) - viewport.transform_point2(first_point))
			.try_normalize()
			.unwrap_or_default();
	}

	(first_point - first_row_last_column).try_normalize().unwrap_or_default()
}
