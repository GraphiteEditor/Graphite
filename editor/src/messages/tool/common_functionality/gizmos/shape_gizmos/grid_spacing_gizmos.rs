use crate::consts::{GRID_COLUMNS_INDEX, GRID_ROW_COLUMN_GIZMO_OFFSET, GRID_ROW_INDEX, GRID_SPACING_INDEX};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, FrontendMessage, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::prelude::{GraphOperationMessage, Responses};
use crate::messages::tool::common_functionality::gizmos::shape_gizmos::grid_row_columns_gizmo::{
	calculate_rectangle_side_direction, calculate_rectangle_top_direction, convert_to_gizmo_line, get_viewport_grid_spacing,
};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_grid_parameters;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::renderer::Quad;
use graphene_std::vector::misc::{GridType, dvec2_to_point, get_line_endpoints};
use kurbo::{Line, ParamCurveNearest, Rect, Shape, Triangle};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum GridSpacingGizmoState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default)]
pub struct GridSpacingGizmo {
	pub layer: Option<LayerNodeIdentifier>,
	gizmo_state: GridSpacingGizmoState,
	column_index: u32,
	row_index: u32,
	initial_spacing: DVec2,
	gizmo_type: Option<GridSpacingGizmoType>,
}

impl GridSpacingGizmo {
	pub fn cleanup(&mut self) {
		self.layer = None;
		self.gizmo_state = GridSpacingGizmoState::Inactive;
	}

	pub fn update_state(&mut self, state: GridSpacingGizmoState) {
		self.gizmo_state = state;
	}

	pub fn is_hovered(&self) -> bool {
		self.gizmo_state == GridSpacingGizmoState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.gizmo_state == GridSpacingGizmoState::Dragging
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(layer, document) else {
			return;
		};
		let stroke_width = graph_modification_utils::get_stroke_width(layer, &document.network_interface);
		let viewport = document.metadata().transform_to_viewport(layer);
		if let Some((col, row)) = check_if_over_gizmo(grid_type, columns, rows, spacing, angles, mouse_position, viewport) {
			log::info!("col {:?} , row {:?}", col, row);
			self.layer = Some(layer);
			self.column_index = col;
			self.row_index = row;
			self.initial_spacing = spacing;
			self.update_state(GridSpacingGizmoState::Hover);
			let closest_gizmo = GridSpacingGizmoType::get_closest_line(grid_type, mouse_position, col, row, spacing, angles, viewport, stroke_width);
			log::info!("gizmo type {:?}", closest_gizmo);
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: closest_gizmo.mouse_icon() });
			self.gizmo_type = Some(closest_gizmo);
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, layer: Option<LayerNodeIdentifier>, _shape_editor: &mut &mut ShapeState, _mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		let Some(layer) = layer.or(self.layer) else { return };
		let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(layer, document) else {
			return;
		};
		let viewport = document.metadata().transform_to_viewport(layer);
		let stroke_width = graph_modification_utils::get_stroke_width(layer, &document.network_interface);

		match self.gizmo_state {
			GridSpacingGizmoState::Inactive => {}
			GridSpacingGizmoState::Hover | GridSpacingGizmoState::Dragging => match grid_type {
				GridType::Rectangular => {
					if let Some(gizmo_type) = &self.gizmo_type {
						let line = gizmo_type.line(grid_type, self.column_index, self.row_index, angles, spacing, viewport, stroke_width);
						let (p0, p1) = get_line_endpoints(line);
						overlay_context.dashed_line(p0, p1, None, None, Some(5.), Some(5.), Some(0.5));
					}
				}
				GridType::Isometric => {
					if let Some(gizmo_type) = &self.gizmo_type {
						let line = gizmo_type.line(grid_type, self.column_index, self.row_index, angles, spacing, viewport, stroke_width);
						let (p0, p1) = get_line_endpoints(line);
						overlay_context.dashed_line(p0, p1, None, None, Some(5.), Some(5.), Some(0.5));
					}
				}
			},
		}
	}

	pub fn update(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else { return };
		let viewport = document.metadata().transform_to_viewport(layer);

		let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(layer, document) else {
			return;
		};

		let stroke_width = graph_modification_utils::get_stroke_width(layer, &document.network_interface);
		let Some(gizmo_type) = &self.gizmo_type else { return };
		let direction = gizmo_type.direction(self.column_index, self.row_index, angles, self.initial_spacing, viewport);
		let delta_vector = input.mouse.position - drag_start;

		let delta = delta_vector.dot(direction);

		let Some(node_id) = graph_modification_utils::get_grid_id(layer, &document.network_interface) else {
			return;
		};

		let new_spacing = gizmo_type.new_spacing(delta, self.initial_spacing);
		let spacing_delta = new_spacing - spacing;

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, GRID_SPACING_INDEX),
			input: NodeInput::value(TaggedValue::DVec2(new_spacing), false),
		});

		let transform = self.gizmo_type.as_ref().unwrap().transform_grid(spacing_delta, direction, self.column_index, self.row_index);

		responses.add(GraphOperationMessage::TransformChange {
			layer,
			transform,
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});

		log::info!("{:?}", (self.row_index, self.column_index));
		responses.add(NodeGraphMessage::RunDocumentGraph);

		// if self.initial_dimension() as i32 + dimensions_to_add < 1 {
		// 	self.initial_mouse_start = Some(input.mouse.position);
		// 	self.gizmo_type = self.gizmo_type.opposite_gizmo_type();
		// 	self.initial_rows = 1;
		// 	self.initial_columns = 1;
		// }
	}
}

fn check_if_over_gizmo(grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, mouse_position: DVec2, viewport: DAffine2) -> Option<(u32, u32)> {
	let layer_mouse = viewport.inverse().transform_point2(mouse_position);
	match grid_type {
		GridType::Rectangular => {
			for column in 0..columns - 1 {
				for row in 0..rows - 1 {
					let p0 = DVec2::new(spacing.x * column as f64, spacing.y * row as f64);
					let p1 = DVec2::new((1 + column) as f64 * spacing.x, (1 + row) as f64 * spacing.y);
					let rect = Rect::from_points(dvec2_to_point(p0), dvec2_to_point(p1));

					if rect.contains(dvec2_to_point(layer_mouse)) {
						return Some((column, row));
					};
				}
			}
		}
		GridType::Isometric => {
			for column in 0..columns - 1 {
				for row in 0..rows - 1 {
					let p0 = isometric_point_position(row, column, spacing, angles);
					let p1 = isometric_point_position(row, column + 1, spacing, angles);
					let p2 = isometric_point_position(row + 1, column + 1, spacing, angles);
					let p4 = isometric_point_position(row + 1, column, spacing, angles);

					let triangle1 = Triangle::new(dvec2_to_point(p0), dvec2_to_point(p1), dvec2_to_point(p2));
					let triangle2 = Triangle::new(dvec2_to_point(p0), dvec2_to_point(p2), dvec2_to_point(p4));

					if triangle2.contains(dvec2_to_point(layer_mouse)) {
						return Some((column, row));
					}

					if triangle1.contains(dvec2_to_point(layer_mouse)) {
						return Some((column, row));
					}
				}
			}
		}
	}

	None
}

// #[derive(Clone, Debug, Default, PartialEq)]
// pub enum GridSpacingGizmoType {
// 	#[default]
// 	None,
// 	Top,
// 	Down,
// 	Left,
// 	Right,
// 	IsometricMiddleUp,
// 	IsometricMiddleDown,
// }

// impl GridSpacingGizmoType {
// 	pub fn get_line_points(&self, grid_type: GridType, column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, stroke: Option<f64>) -> (DVec2, DVec2) {
// 		match grid_type {
// 			GridType::Rectangular => match self {
// 				Self::Top => get_rectangular_top_points(column_index, row_index, spacing, stroke),
// 				Self::Right => get_rectangular_right_points(column_index, row_index, spacing, stroke),
// 				Self::Down => get_rectangular_down_points(column_index, row_index, spacing, stroke),
// 				Self::Left => get_rectangular_left_points(column_index, row_index, spacing, stroke),
// 				Self::None => panic!("RowColumnGizmoType::None does not have line points"),
// 			},
// 			GridType::Isometric => match self {
// 				Self::Top => get_isometric_top_points(column_index, row_index, angles, spacing, stroke),
// 				Self::Right => get_isometric_right_points(column_index, row_index, angles, spacing, stroke),
// 				Self::Down => get_isometric_down_points(column_index, row_index, angles, spacing, stroke),
// 				Self::Left => get_isometric_left_points(column_index, row_index, angles, spacing, stroke),
// 				Self::None => panic!("RowColumnGizmoType::None does not have line points"),
// 			},
// 		}
// 	}

// 	pub fn get_closest_line(mouse_position: DVec2, grid_type: GridType, column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, viewport: DAffine2, stroke_width: Option<f64>) -> Self {
// 		let mut gizmo_type = GridSpacingGizmoType::Top;
// 		let mut closest_distance = gizmo_type
// 			.line(grid_type, column_index, row_index, angles, spacing, viewport, stroke_width)
// 			.nearest(dvec2_to_point(mouse_position), 1e-6)
// 			.distance_sq;

// 		for t in Self::all() {
// 			if matches!(t, GridSpacingGizmoType::Top) {
// 				continue;
// 			}
// 			let line = t.line(grid_type, column_index, row_index, angles, spacing, viewport, stroke_width);
// 			let nearest = line.nearest(dvec2_to_point(mouse_position), 1e-6);
// 			if nearest.distance_sq < closest_distance {
// 				gizmo_type = t;
// 				closest_distance = nearest.distance_sq;
// 			}
// 		}
// 		gizmo_type
// 	}

// 	pub fn line(&self, grid_type: GridType, column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, viewport: DAffine2, stroke_width: Option<f64>) -> Line {
// 		let (p0, p1) = self.get_line_points(grid_type, column_index, row_index, angles, spacing, stroke_width);
// 		// let gap = 2. * stroke_width.unwrap_or(3.) * (p1 - p0).perp().normalize();

// 		convert_to_gizmo_line(viewport.transform_point2(p0), viewport.transform_point2(p1))
// 	}

// 	fn opposite_gizmo_type(&self) -> Self {
// 		return match self {
// 			Self::Top => Self::Down,
// 			Self::Right => Self::Left,
// 			Self::Down => Self::Top,
// 			Self::Left => Self::Right,
// 			Self::None => panic!("RowColumnGizmoType::None does not have opposite"),
// 		};
// 	}

// 	fn new_spacing(&self, delta: f64, spacing: DVec2) -> DVec2 {
// 		match self {
// 			GridSpacingGizmoType::Top | GridSpacingGizmoType::Down => DVec2::new(spacing.x, spacing.y + delta),
// 			GridSpacingGizmoType::Right | GridSpacingGizmoType::Left => DVec2::new(spacing.x + delta, spacing.y),
// 			GridSpacingGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
// 		}
// 	}

// 	fn direction(&self, grid_type: GridType, column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, viewport: DAffine2) -> DVec2 {
// 		match grid_type {
// 			GridType::Rectangular => match self {
// 				GridSpacingGizmoType::Top => viewport.transform_vector2(DVec2::Y),
// 				GridSpacingGizmoType::Down => -viewport.transform_vector2(DVec2::Y),
// 				GridSpacingGizmoType::Right => calculate_rectangle_side_direction(spacing, viewport),
// 				GridSpacingGizmoType::Left => -calculate_rectangle_side_direction(spacing, viewport),
// 				GridSpacingGizmoType::None => panic!("RowColumnGizmoType::None does not have a line"),
// 			},
// 			GridType::Isometric => match &self {
// 				GridSpacingGizmoType::Top | GridSpacingGizmoType::Down | GridSpacingGizmoType::Left | GridSpacingGizmoType::Right => {
// 					let (p1, p2) = self.get_line_points(grid_type, column_index, row_index, angles, spacing, None);
// 					(p1 - p2).perp().normalize()
// 				}
// 				GridSpacingGizmoType::None => panic!("RowColumnGizmoType::None does not have a line"),
// 			},
// 		}
// 	}

// 	fn mouse_icon(&self) -> MouseCursorIcon {
// 		match self {
// 			GridSpacingGizmoType::Top | GridSpacingGizmoType::Down => MouseCursorIcon::NSResize,
// 			GridSpacingGizmoType::Right | GridSpacingGizmoType::Left => MouseCursorIcon::EWResize,
// 			GridSpacingGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
// 		}
// 	}

// 	pub fn all() -> [Self; 4] {
// 		[Self::Top, Self::Right, Self::Down, Self::Left]
// 	}
// }

fn get_rectangular_top_points(column_index: u32, row_index: u32, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();
	let p0 = DVec2::new(column_index as f64 * spacing.x, row_index as f64 * spacing.y) + DVec2::new(stroke_width, stroke_width);
	let p1 = p0 + DVec2::new(spacing.x - 2. * stroke_width, 0.);

	(p0, p1)
}

fn get_rectangular_right_points(column_index: u32, row_index: u32, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();
	let p0 = DVec2::new((1 + column_index) as f64 * spacing.x, row_index as f64 * spacing.y) + DVec2::new(-stroke_width, stroke_width);
	let p1 = p0 + DVec2::new(0., spacing.y - 2. * stroke_width);

	(p0, p1)
}

fn get_rectangular_down_points(column_index: u32, row_index: u32, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();

	let p0 = DVec2::new(column_index as f64 * spacing.x, (1 + row_index) as f64 * spacing.y) + DVec2::new(stroke_width, -stroke_width);
	let p1 = p0 + DVec2::new(spacing.x - 2. * stroke_width, 0.);

	(p0, p1)
}

fn get_rectangular_left_points(column_index: u32, row_index: u32, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();

	let p0 = DVec2::new(column_index as f64 * spacing.x, row_index as f64 * spacing.y) + DVec2::new(stroke_width, stroke_width);
	let p1 = p0 + DVec2::new(0., spacing.y - 2. * stroke_width);

	(p0, p1)
}

fn isometric_point_position(row: u32, col: u32, spacing: DVec2, angles: DVec2) -> DVec2 {
	let (angle_a, angle_b) = angles.into();
	let tan_a = angle_a.to_radians().tan();
	let tan_b = angle_b.to_radians().tan();

	let spacing = DVec2::new(spacing.y / (tan_a + tan_b), spacing.y);

	let a_angles_eaten = col.div_ceil(2) as f64;
	let b_angles_eaten = (col / 2) as f64;
	let offset_y_fraction = b_angles_eaten * tan_b - a_angles_eaten * tan_a;

	DVec2::new(spacing.x * col as f64, spacing.y * row as f64 + offset_y_fraction * spacing.x)
}

fn get_isometric_top_points(column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();

	let x0 = isometric_point_position(row_index, column_index, spacing, angles);
	let x1 = isometric_point_position(row_index, column_index + 1, spacing, angles);
	let direction = (x1 - x0).normalize();

	// in the direction of edge
	let padding = (x1 - x0).length() * 0.1 * direction;
	// perpendicular to the edge
	let push_out = calculate_gap_vector(direction, stroke_width);

	(x0 + push_out + padding, x1 + push_out - padding)
}

fn get_isometric_right_points(column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();

	let x0 = isometric_point_position(row_index, column_index + 1, spacing, angles);
	let x1 = isometric_point_position(row_index + 1, column_index + 1, spacing, angles);
	let direction = (x1 - x0).normalize();

	// in the direction of edge
	let padding = (x1 - x0).length() * 0.1 * direction;
	// perpendicular to the edge
	let push_out = calculate_gap_vector(direction, stroke_width);

	(x0 + push_out + padding, x1 + push_out - padding)
}

fn get_isometric_down_points(column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();

	let x0 = isometric_point_position(row_index + 1, column_index, spacing, angles);
	let x1 = isometric_point_position(row_index + 1, column_index + 1, spacing, angles);
	let direction = (x1 - x0).normalize();

	// in the direction of edge
	let padding = (x1 - x0).length() * 0.1 * direction;
	// perpendicular to the edge
	let push_out = calculate_gap_vector(direction, stroke_width);

	(x0 - push_out + padding, x1 - push_out - padding)
}

fn get_isometric_left_points(column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();

	let x0 = isometric_point_position(row_index, column_index, spacing, angles);
	let x1 = isometric_point_position(row_index + 1, column_index, spacing, angles);
	let direction = (x1 - x0).normalize();

	// in the direction of edge
	let padding = (x1 - x0).length() * 0.1 * direction;
	// perpendicular to the edge
	let push_out = calculate_gap_vector(direction, stroke_width);

	(x0 - push_out + padding, x1 - push_out - padding)
}

fn get_isometric_middle_up_points(column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();

	let (x0, x1) = if column_index % 2 == 0 {
		(
			isometric_point_position(row_index, column_index, spacing, angles),
			isometric_point_position(row_index + 1, column_index + 1, spacing, angles),
		)
	} else {
		// ref point is changed
		(
			isometric_point_position(row_index + 1, column_index, spacing, angles),
			isometric_point_position(row_index, column_index + 1, spacing, angles),
		)
	};
	let direction = (x1 - x0).normalize();

	// in the direction of edge
	let padding = (x1 - x0).length() * 0.1 * direction;
	// perpendicular to the edge
	let push_out = calculate_gap_vector(direction, stroke_width);

	(x0 - push_out + padding, x1 - push_out - padding)
}

fn get_isometric_middle_down_points(column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, stroke_width: Option<f64>) -> (DVec2, DVec2) {
	let stroke_width = stroke_width.unwrap_or_default();

	let (x0, x1) = if column_index % 2 == 0 {
		(
			isometric_point_position(row_index, column_index, spacing, angles),
			isometric_point_position(row_index + 1, column_index + 1, spacing, angles),
		)
	} else {
		// ref point is changed
		(
			isometric_point_position(row_index + 1, column_index, spacing, angles),
			isometric_point_position(row_index, column_index + 1, spacing, angles),
		)
	};
	let direction = (x1 - x0).normalize();
	// in the direction of edge
	let padding = (x1 - x0).length() * 0.1 * direction;
	// perpendicular to the edge
	let push_out = calculate_gap_vector(direction, stroke_width);

	(x0 + push_out + padding, x1 + push_out - padding)
}

fn calculate_gap_vector(direction: DVec2, stroke_width: f64) -> DVec2 {
	let perp = direction.perp().normalize();
	(stroke_width + 0.5) * perp
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RectangularGizmoType {
	#[default]
	Top,
	Right,
	Down,
	Left,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum IsometricGizmoType {
	#[default]
	Top,
	Right,
	Down,
	Left,
	IsometricMiddleUp,
	IsometricMiddleDown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GridSpacingGizmoType {
	Rect(RectangularGizmoType),
	Iso(IsometricGizmoType),
}

pub fn get_line_points_for_rect(gizmo: RectangularGizmoType, column_index: u32, row_index: u32, spacing: DVec2, stroke: Option<f64>) -> (DVec2, DVec2) {
	match gizmo {
		RectangularGizmoType::Top => get_rectangular_top_points(column_index, row_index, spacing, stroke),
		RectangularGizmoType::Right => get_rectangular_right_points(column_index, row_index, spacing, stroke),
		RectangularGizmoType::Down => get_rectangular_down_points(column_index, row_index, spacing, stroke),
		RectangularGizmoType::Left => get_rectangular_left_points(column_index, row_index, spacing, stroke),
	}
}

pub fn get_line_points_for_iso(gizmo: IsometricGizmoType, column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, stroke: Option<f64>) -> (DVec2, DVec2) {
	match gizmo {
		IsometricGizmoType::Top => get_isometric_top_points(column_index, row_index, angles, spacing, stroke),
		IsometricGizmoType::Right => get_isometric_right_points(column_index, row_index, angles, spacing, stroke),
		IsometricGizmoType::Down => get_isometric_down_points(column_index, row_index, angles, spacing, stroke),
		IsometricGizmoType::Left => get_isometric_left_points(column_index, row_index, angles, spacing, stroke),
		IsometricGizmoType::IsometricMiddleUp => get_isometric_middle_up_points(column_index, row_index, angles, spacing, stroke),
		IsometricGizmoType::IsometricMiddleDown => get_isometric_middle_down_points(column_index, row_index, angles, spacing, stroke),
	}
}

// Builds a Line after viewport transform
pub fn gizmo_line_from_points(p0: DVec2, p1: DVec2, viewport: DAffine2) -> Line {
	convert_to_gizmo_line(viewport.transform_point2(p0), viewport.transform_point2(p1))
}

// pub fn gizmo_opposite_rect(g: RectangularGizmoType) -> RectangularGizmoType {
// 	match g {
// 		RectangularGizmoType::Top => RectangularGizmoType::Down,
// 		RectangularGizmoType::Down => RectangularGizmoType::Top,
// 		RectangularGizmoType::Right => RectangularGizmoType::Left,
// 		RectangularGizmoType::Left => RectangularGizmoType::Right,
// 	}
// }

// pub fn gizmo_opposite_iso(g: IsometricGizmoType) -> IsometricGizmoType {
// 	match g {
// 		IsometricGizmoType::Top => IsometricGizmoType::Down,
// 		IsometricGizmoType::Down => IsometricGizmoType::Top,
// 		IsometricGizmoType::Right => IsometricGizmoType::Left,
// 		IsometricGizmoType::Left => IsometricGizmoType::Right,
// 		IsometricGizmoType::IsometricMiddleUp => IsometricGizmoType::IsometricMiddleDown,
// 		IsometricGizmoType::IsometricMiddleDown => IsometricGizmoType::IsometricMiddleUp,
// 	}
// }

pub fn gizmo_new_spacing_rect(g: RectangularGizmoType, delta: f64, spacing: DVec2) -> DVec2 {
	match g {
		RectangularGizmoType::Top | RectangularGizmoType::Down => DVec2::new(spacing.x, spacing.y + delta),
		RectangularGizmoType::Right | RectangularGizmoType::Left => DVec2::new(spacing.x + delta, spacing.y),
	}
}

pub fn gizmo_new_spacing_iso(g: IsometricGizmoType, delta: f64, spacing: DVec2) -> DVec2 {
	match g {
		IsometricGizmoType::Top | IsometricGizmoType::Down => DVec2::new(spacing.x, spacing.y + delta),
		IsometricGizmoType::Right | IsometricGizmoType::Left => DVec2::new(spacing.x + delta, spacing.y),
		IsometricGizmoType::IsometricMiddleUp | IsometricGizmoType::IsometricMiddleDown => {
			// Adjust both? depends on your desired behavior
			DVec2::new(spacing.x + delta, spacing.y + delta)
		}
	}
}

pub fn gizmo_direction_rect(g: RectangularGizmoType, spacing: DVec2, viewport: DAffine2) -> DVec2 {
	match g {
		RectangularGizmoType::Top => viewport.transform_vector2(DVec2::Y),
		RectangularGizmoType::Down => -viewport.transform_vector2(DVec2::Y),
		RectangularGizmoType::Right => viewport.transform_vector2(DVec2::X),
		RectangularGizmoType::Left => -viewport.transform_vector2(-DVec2::X),
	}
}

pub fn gizmo_direction_iso(g: IsometricGizmoType, column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2) -> DVec2 {
	let (p1, p2) = get_line_points_for_iso(g, column_index, row_index, angles, spacing, None);
	(p1 - p2).perp().normalize()
}

pub fn gizmo_mouse_icon_rect(g: RectangularGizmoType) -> MouseCursorIcon {
	match g {
		RectangularGizmoType::Top | RectangularGizmoType::Down => MouseCursorIcon::NSResize,
		RectangularGizmoType::Right | RectangularGizmoType::Left => MouseCursorIcon::EWResize,
	}
}

pub fn gizmo_mouse_icon_iso(g: IsometricGizmoType) -> MouseCursorIcon {
	match g {
		IsometricGizmoType::Top | IsometricGizmoType::Down | IsometricGizmoType::IsometricMiddleUp | IsometricGizmoType::IsometricMiddleDown => MouseCursorIcon::NSResize,
		IsometricGizmoType::Right | IsometricGizmoType::Left => MouseCursorIcon::EWResize,
	}
}

impl RectangularGizmoType {
	pub fn all() -> [Self; 4] {
		[Self::Top, Self::Right, Self::Down, Self::Left]
	}
}

impl IsometricGizmoType {
	pub fn all() -> [Self; 6] {
		[Self::Top, Self::Right, Self::Down, Self::Left, Self::IsometricMiddleUp, Self::IsometricMiddleDown]
	}
}

impl GridSpacingGizmoType {
	pub fn line(&self, grid_type: GridType, column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, viewport: DAffine2, stroke_width: Option<f64>) -> Line {
		match self {
			GridSpacingGizmoType::Rect(g) => {
				let (p0, p1) = get_line_points_for_rect(*g, column_index, row_index, spacing, stroke_width);
				gizmo_line_from_points(p0, p1, viewport)
			}
			GridSpacingGizmoType::Iso(g) => {
				let (p0, p1) = get_line_points_for_iso(*g, column_index, row_index, angles, spacing, stroke_width);
				gizmo_line_from_points(p0, p1, viewport)
			}
		}
	}

	pub fn get_closest_line(grid_type: GridType, mouse_position: DVec2, column_index: u32, row_index: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2, stroke_width: Option<f64>) -> Self {
		match grid_type {
			GridType::Rectangular => Self::Rect(closest_line_rect(mouse_position, column_index, row_index, spacing, viewport, stroke_width)),
			GridType::Isometric => Self::Iso(closest_line_iso(mouse_position, column_index, row_index, angles, spacing, viewport, stroke_width)),
		}
	}

	pub fn direction(&self, column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, viewport: DAffine2) -> DVec2 {
		match &self {
			GridSpacingGizmoType::Rect(g) => gizmo_direction_rect(*g, spacing, viewport),
			GridSpacingGizmoType::Iso(g) => gizmo_direction_iso(*g, column_index, row_index, angles, spacing),
		}
	}

	fn new_spacing(&self, delta: f64, spacing: DVec2) -> DVec2 {
		match &self {
			GridSpacingGizmoType::Rect(g) => gizmo_new_spacing_rect(*g, delta, spacing),
			GridSpacingGizmoType::Iso(g) => gizmo_new_spacing_iso(*g, delta, spacing),
		}
	}

	fn mouse_icon(&self) -> MouseCursorIcon {
		match self {
			GridSpacingGizmoType::Rect(g) => gizmo_mouse_icon_rect(*g),
			GridSpacingGizmoType::Iso(g) => gizmo_mouse_icon_iso(*g),
		}
	}

	pub fn transform_grid(&self, spacing_delta: DVec2, direction: DVec2, column_index: u32, row_index: u32) -> DAffine2 {
		match self {
			GridSpacingGizmoType::Rect(gizmo_type) => match gizmo_type {
				RectangularGizmoType::Right => {
					if column_index == 0 {
						DAffine2::IDENTITY
					} else {
						DAffine2::from_translation(-spacing_delta * direction * column_index as f64)
					}
				}
				RectangularGizmoType::Down => {
					if row_index == 0 {
						DAffine2::IDENTITY
					} else {
						DAffine2::from_translation(-spacing_delta * direction * row_index as f64)
					}
				}
				RectangularGizmoType::Left => {
					if column_index == 0 {
						DAffine2::from_translation(spacing_delta * direction)
					} else {
						DAffine2::from_translation(spacing_delta * direction * (column_index + 1) as f64)
					}
				}
				RectangularGizmoType::Top => {
					if row_index == 0 {
						DAffine2::from_translation(spacing_delta * direction)
					} else {
						DAffine2::from_translation(spacing_delta * direction * (row_index + 1) as f64)
					}
				}
			},

			GridSpacingGizmoType::Iso(_) => {
				// Placeholder: no transformation for now
				DAffine2::IDENTITY
			}
		}
	}
}

fn closest_line_generic<T>(mouse_position: DVec2, viewport: DAffine2, all_variants: &[T], get_line_points: impl Fn(T) -> (DVec2, DVec2)) -> T
where
	T: Copy + PartialEq,
{
	let mut gizmo_type = all_variants[0];
	let mut closest_distance = {
		let (p0, p1) = get_line_points(gizmo_type);
		gizmo_line_from_points(p0, p1, viewport).nearest(dvec2_to_point(mouse_position), 1e-6).distance_sq
	};

	for &t in all_variants.iter().skip(1) {
		let (p0, p1) = get_line_points(t);
		let nearest = gizmo_line_from_points(p0, p1, viewport).nearest(dvec2_to_point(mouse_position), 1e-6);
		if nearest.distance_sq < closest_distance {
			gizmo_type = t;
			closest_distance = nearest.distance_sq;
		}
	}
	gizmo_type
}

pub fn closest_line_rect(mouse_position: DVec2, column_index: u32, row_index: u32, spacing: DVec2, viewport: DAffine2, stroke_width: Option<f64>) -> RectangularGizmoType {
	closest_line_generic(mouse_position, viewport, &RectangularGizmoType::all(), |t| {
		get_line_points_for_rect(t, column_index, row_index, spacing, stroke_width)
	})
}

pub fn closest_line_iso(mouse_position: DVec2, column_index: u32, row_index: u32, angles: DVec2, spacing: DVec2, viewport: DAffine2, stroke_width: Option<f64>) -> IsometricGizmoType {
	closest_line_generic(mouse_position, viewport, &IsometricGizmoType::all(), |t| {
		get_line_points_for_iso(t, column_index, row_index, angles, spacing, stroke_width)
	})
}
