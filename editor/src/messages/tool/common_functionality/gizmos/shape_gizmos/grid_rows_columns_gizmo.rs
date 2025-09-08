use crate::consts::GRID_ROW_COLUMN_GIZMO_OFFSET;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage};
use crate::messages::prelude::{GraphOperationMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_grid_parameters;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::NodeInputDecleration;
use graphene_std::vector::misc::{GridType, dvec2_to_point, get_line_endpoints};
use kurbo::{Line, ParamCurveNearest, Rect};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RowColumnGizmoState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default)]
pub struct RowColumnGizmo {
	pub layer: Option<LayerNodeIdentifier>,
	pub gizmo_type: RowColumnGizmoType,
	initial_rows: u32,
	initial_columns: u32,
	spacing: DVec2,
	initial_mouse_start: Option<DVec2>,
	gizmo_state: RowColumnGizmoState,
}

impl RowColumnGizmo {
	pub fn cleanup(&mut self) {
		self.layer = None;
		self.gizmo_state = RowColumnGizmoState::Inactive;
		self.initial_mouse_start = None;
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
			RowColumnGizmoType::Top | RowColumnGizmoType::Bottom => self.initial_rows,
			RowColumnGizmoType::Left | RowColumnGizmoType::Right => self.initial_columns,
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler) {
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
			self.initial_mouse_start = None;
			self.update_state(RowColumnGizmoState::Hover);
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, layer: Option<LayerNodeIdentifier>, _shape_editor: &mut &mut ShapeState, _mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		let Some(layer) = layer.or(self.layer) else { return };
		let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(layer, document) else {
			return;
		};
		let viewport = document.metadata().transform_to_viewport(layer);

		if !matches!(self.gizmo_state, RowColumnGizmoState::Inactive) {
			let line = self.gizmo_type.line(grid_type, columns, rows, spacing, angles, viewport);
			let (p0, p1) = get_line_endpoints(line);
			overlay_context.dashed_line(p0, p1, None, None, Some(5.), Some(5.), Some(0.5));
		}
	}

	pub fn update(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else { return };
		let viewport = document.metadata().transform_to_viewport(layer);

		let Some((grid_type, _, columns, rows, angles)) = extract_grid_parameters(layer, document) else {
			return;
		};
		let direction = self.gizmo_type.direction(viewport);
		let delta_vector = input.mouse.position - self.initial_mouse_start.unwrap_or(drag_start);

		let projection = delta_vector.project_onto(self.gizmo_type.direction(viewport));
		let delta = viewport.inverse().transform_vector2(projection).length() * delta_vector.dot(direction).signum();

		if delta.abs() < 1e-6 {
			return;
		}

		let dimensions_to_add = (delta / (self.gizmo_type.spacing(self.spacing, grid_type, angles))).floor() as i32;
		let new_dimension = (self.initial_dimension() as i32 + dimensions_to_add).max(1) as u32;

		let Some(node_id) = graph_modification_utils::get_grid_id(layer, &document.network_interface) else {
			return;
		};

		let dimensions_delta = new_dimension as i32 - self.gizmo_type.initial_dimension(rows, columns) as i32;
		let transform = self.transform_grid(dimensions_delta, self.spacing, grid_type, angles, viewport);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, self.gizmo_type.index()),
			input: NodeInput::value(TaggedValue::U32((self.initial_dimension() as i32 + dimensions_to_add).max(1) as u32), false),
		});

		responses.add(GraphOperationMessage::TransformChange {
			layer,
			transform,
			transform_in: TransformIn::Viewport,
			skip_rerender: false,
		});

		responses.add(NodeGraphMessage::RunDocumentGraph);

		if self.initial_dimension() as i32 + dimensions_to_add < 1 {
			self.initial_mouse_start = Some(input.mouse.position);
			self.gizmo_type = self.gizmo_type.opposite_gizmo_type();
			self.initial_rows = 1;
			self.initial_columns = 1;
		}
	}

	fn transform_grid(&self, dimensions_delta: i32, spacing: DVec2, grid_type: GridType, angles: DVec2, viewport: DAffine2) -> DAffine2 {
		match &self.gizmo_type {
			RowColumnGizmoType::Top => {
				let move_up_by = self.gizmo_type.direction(viewport) * dimensions_delta as f64 * spacing.y;
				DAffine2::from_translation(move_up_by)
			}
			RowColumnGizmoType::Left => {
				let move_left_by = self.gizmo_type.direction(viewport) * dimensions_delta as f64 * self.gizmo_type.spacing(spacing, grid_type, angles);
				DAffine2::from_translation(move_left_by)
			}
			RowColumnGizmoType::Bottom | RowColumnGizmoType::Right | RowColumnGizmoType::None => DAffine2::IDENTITY,
		}
	}
}

fn check_if_over_gizmo(grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, mouse_position: DVec2, viewport: DAffine2) -> Option<RowColumnGizmoType> {
	let mouse_point = dvec2_to_point(mouse_position);
	let accuracy = 1e-6;
	let threshold = 32.;

	for gizmo_type in RowColumnGizmoType::all() {
		let line = gizmo_type.line(grid_type, columns, rows, spacing, angles, viewport);
		let rect = gizmo_type.rect(grid_type, columns, rows, spacing, angles, viewport);

		if rect.contains(mouse_point) || line.nearest(mouse_point, accuracy).distance_sq < threshold {
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
	let end_isometric_offset = if columns % 2 == 0 { DVec2::ZERO } else { DVec2::new(0., isometric_spacing.y) };

	(top_left + offset - isometric_offset, top_right - offset - end_isometric_offset)
}

fn calculate_isometric_bottom_line_points(columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
	let bottom_left = calculate_isometric_point(0, rows - 1, angles, spacing);
	let bottom_right = calculate_isometric_point(columns - 1, rows - 1, angles, spacing);

	let offset = if columns == 1 || rows == 1 { DVec2::ZERO } else { DVec2::new(spacing.x * 0.5, 0.) };
	let isometric_offset = if columns % 2 == 0 {
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

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RowColumnGizmoType {
	#[default]
	None,
	Top,
	Bottom,
	Left,
	Right,
}

impl RowColumnGizmoType {
	pub fn get_line_points(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2) -> (DVec2, DVec2) {
		match grid_type {
			GridType::Rectangular => match self {
				Self::Top => get_rectangle_top_line_points(columns, rows, spacing),
				Self::Right => get_rectangle_right_line_points(columns, rows, spacing),
				Self::Bottom => get_rectangle_bottom_line_points(columns, rows, spacing),
				Self::Left => get_rectangle_left_line_points(columns, rows, spacing),
				Self::None => panic!("RowColumnGizmoType::None does not have line points"),
			},
			GridType::Isometric => match self {
				Self::Top => calculate_isometric_top_line_points(columns, rows, spacing, angles),
				Self::Right => calculate_isometric_right_line_points(columns, rows, spacing, angles),
				Self::Bottom => calculate_isometric_bottom_line_points(columns, rows, spacing, angles),
				Self::Left => calculate_isometric_left_line_points(columns, rows, spacing, angles),
				Self::None => panic!("RowColumnGizmoType::None does not have line points"),
			},
		}
	}

	fn line(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2) -> Line {
		let (p0, p1) = self.get_line_points(grid_type, columns, rows, spacing, angles);
		let direction = self.direction(viewport);
		let gap = GRID_ROW_COLUMN_GIZMO_OFFSET * viewport.inverse().transform_vector2(direction).normalize();

		convert_to_gizmo_line(viewport.transform_point2(p0 + gap), viewport.transform_point2(p1 + gap))
	}

	fn rect(&self, grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2) -> Rect {
		let (p0, p1) = self.get_line_points(grid_type, columns, rows, spacing, angles);
		let direction = self.direction(viewport);
		let gap = GRID_ROW_COLUMN_GIZMO_OFFSET * direction.normalize();

		let (x0, x1) = match self {
			Self::Top | Self::Left => (viewport.transform_point2(p0 + gap), viewport.transform_point2(p1)),
			Self::Bottom | Self::Right => (viewport.transform_point2(p0), viewport.transform_point2(p1 + gap)),
			Self::None => panic!("RowColumnGizmoType::None does not have opposite"),
		};

		Rect::new(x0.x, x0.y, x1.x, x1.y)
	}

	fn opposite_gizmo_type(&self) -> Self {
		match self {
			Self::Top => Self::Bottom,
			Self::Right => Self::Left,
			Self::Bottom => Self::Top,
			Self::Left => Self::Right,
			Self::None => panic!("RowColumnGizmoType::None does not have opposite"),
		}
	}

	pub fn direction(&self, viewport: DAffine2) -> DVec2 {
		match self {
			RowColumnGizmoType::Top => viewport.transform_vector2(-DVec2::Y),
			RowColumnGizmoType::Bottom => viewport.transform_vector2(DVec2::Y),
			RowColumnGizmoType::Right => viewport.transform_vector2(DVec2::X),
			RowColumnGizmoType::Left => viewport.transform_vector2(-DVec2::X),
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a line"),
		}
	}

	fn initial_dimension(&self, rows: u32, columns: u32) -> u32 {
		match self {
			RowColumnGizmoType::Top | RowColumnGizmoType::Bottom => rows,
			RowColumnGizmoType::Left | RowColumnGizmoType::Right => columns,
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	fn spacing(&self, spacing: DVec2, grid_type: GridType, angles: DVec2) -> f64 {
		match self {
			RowColumnGizmoType::Top | RowColumnGizmoType::Bottom => spacing.y,
			RowColumnGizmoType::Left | RowColumnGizmoType::Right => {
				if grid_type == GridType::Rectangular {
					spacing.x
				} else {
					spacing.y / (angles.x.to_radians().tan() + angles.y.to_radians().tan())
				}
			}
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	fn index(&self) -> usize {
		use graphene_std::vector::generator_nodes::grid::*;

		match self {
			RowColumnGizmoType::Top | RowColumnGizmoType::Bottom => RowsInput::INDEX,
			RowColumnGizmoType::Left | RowColumnGizmoType::Right => ColumnsInput::INDEX,
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	pub fn mouse_icon(&self) -> MouseCursorIcon {
		match self {
			RowColumnGizmoType::Top | RowColumnGizmoType::Bottom => MouseCursorIcon::NSResize,
			RowColumnGizmoType::Left | RowColumnGizmoType::Right => MouseCursorIcon::EWResize,
			RowColumnGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	pub fn all() -> [Self; 4] {
		[Self::Top, Self::Right, Self::Bottom, Self::Left]
	}
}
