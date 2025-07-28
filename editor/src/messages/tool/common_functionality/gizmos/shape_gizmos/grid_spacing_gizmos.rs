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
use kurbo::{Line, ParamCurveNearest, Rect};
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
		if let Some((col, row)) = check_if_over_gizmo(grid_type, columns, rows, spacing, mouse_position, viewport) {
			self.layer = Some(layer);
			self.column_index = col;
			self.row_index = row;
			self.initial_spacing = spacing;
			self.update_state(GridSpacingGizmoState::Hover);
			let closest_gizmo = GridSpacingGizmoType::get_closest_line(mouse_position, col, row, spacing, viewport, stroke_width);
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
			GridSpacingGizmoState::Hover | GridSpacingGizmoState::Dragging => {
				if let Some(gizmo_type) = &self.gizmo_type {
					let line = gizmo_type.line(self.column_index, self.row_index, spacing, viewport, stroke_width);
					let (p0, p1) = get_line_endpoints(line);
					overlay_context.dashed_line(p0, p1, None, None, Some(5.), Some(5.), Some(0.5));

					if matches!(self.gizmo_state, GridSpacingGizmoState::Hover) {
						let line = gizmo_type.opposite_gizmo_type().line(self.column_index, self.row_index, spacing, viewport, stroke_width);
						let (p0, p1) = get_line_endpoints(line);
						overlay_context.dashed_line(p0, p1, None, None, Some(5.), Some(5.), Some(0.5));
					}
				}
			}
		}
	}

	pub fn update(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else { return };
		let viewport = document.metadata().transform_to_viewport(layer);

		let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(layer, document) else {
			return;
		};

		let Some(gizmo_type) = &self.gizmo_type else { return };
		let direction = gizmo_type.direction(spacing, viewport);
		let delta_vector = input.mouse.position - drag_start;

		let delta = delta_vector.dot(direction);

		let Some(node_id) = graph_modification_utils::get_grid_id(layer, &document.network_interface) else {
			return;
		};

		let new_spacing = gizmo_type.new_spacing(delta, self.initial_spacing);
		let delta_spacing = new_spacing - spacing;

		// let transform = self.transform_grid(dimensions_delta, grid_type, viewport_spacing, angles, viewport);

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, GRID_SPACING_INDEX),
			input: NodeInput::value(TaggedValue::DVec2(new_spacing), false),
		});

		// responses.add(GraphOperationMessage::TransformChange {
		// 	layer,
		// 	transform: DAffine2::from_translation(-delta_spacing * direction),
		// 	transform_in: TransformIn::Viewport,
		// 	skip_rerender: false,
		// });

		responses.add(NodeGraphMessage::RunDocumentGraph);

		// if self.initial_dimension() as i32 + dimensions_to_add < 1 {
		// 	self.initial_mouse_start = Some(input.mouse.position);
		// 	self.gizmo_type = self.gizmo_type.opposite_gizmo_type();
		// 	self.initial_rows = 1;
		// 	self.initial_columns = 1;
		// }
	}

	fn transform_grid(&self, dimensions_delta: i32, grid_type: GridType, spacing: DVec2, angles: DVec2, viewport: DAffine2) {}
}

fn check_if_over_gizmo(grid_type: GridType, columns: u32, rows: u32, spacing: DVec2, mouse_position: DVec2, viewport: DAffine2) -> Option<(u32, u32)> {
	let layer_mouse = viewport.inverse().transform_point2(mouse_position);
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

	None
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum GridSpacingGizmoType {
	#[default]
	None,
	Top,
	Down,
	Left,
	Right,
}

impl GridSpacingGizmoType {
	pub fn get_line_points(&self, column_index: u32, row_index: u32, spacing: DVec2, stroke: Option<f64>) -> (DVec2, DVec2) {
		match self {
			Self::Top => get_rectangular_top_points(column_index, row_index, spacing, stroke),
			Self::Right => get_rectangular_right_points(column_index, row_index, spacing, stroke),
			Self::Down => get_rectangular_down_points(column_index, row_index, spacing, stroke),
			Self::Left => get_rectangular_left_points(column_index, row_index, spacing, stroke),
			Self::None => panic!("RowColumnGizmoType::None does not have line points"),
		}
	}

	pub fn get_closest_line(mouse_position: DVec2, column_index: u32, row_index: u32, spacing: DVec2, viewport: DAffine2, stroke_width: Option<f64>) -> Self {
		let mut gizmo_type = GridSpacingGizmoType::Top;
		let mut closest_distance = gizmo_type
			.line(column_index, row_index, spacing, viewport, stroke_width)
			.nearest(dvec2_to_point(mouse_position), 1e-6)
			.distance_sq;

		for t in Self::all() {
			if matches!(t, GridSpacingGizmoType::Top) {
				continue;
			}
			let line = t.line(column_index, row_index, spacing, viewport, stroke_width);
			let nearest = line.nearest(dvec2_to_point(mouse_position), 1e-6);
			if nearest.distance_sq < closest_distance {
				gizmo_type = t;
				closest_distance = nearest.distance_sq;
			}
		}
		gizmo_type
	}

	pub fn line(&self, column_index: u32, row_index: u32, spacing: DVec2, viewport: DAffine2, stroke_width: Option<f64>) -> Line {
		let (p0, p1) = self.get_line_points(column_index, row_index, spacing, stroke_width);
		let gap = -5. * self.direction(spacing, viewport);

		convert_to_gizmo_line(viewport.transform_point2(p0) + gap, viewport.transform_point2(p1) + gap)
	}

	fn opposite(&self, grid_type: GridType, column_index: u32, row_index: u32, spacing: DVec2, angles: DVec2, viewport: DAffine2, stroke: Option<f64>) -> Line {
		let opposite_gizmo_type = self.opposite_gizmo_type();
		opposite_gizmo_type.line(column_index, row_index, spacing, viewport, stroke)
	}

	fn opposite_gizmo_type(&self) -> Self {
		return match self {
			Self::Top => Self::Down,
			Self::Right => Self::Left,
			Self::Down => Self::Top,
			Self::Left => Self::Right,
			Self::None => panic!("RowColumnGizmoType::None does not have opposite"),
		};
	}

	fn new_spacing(&self, delta: f64, spacing: DVec2) -> DVec2 {
		match self {
			GridSpacingGizmoType::Top | GridSpacingGizmoType::Down => DVec2::new(spacing.x, spacing.y + delta),
			GridSpacingGizmoType::Right | GridSpacingGizmoType::Left => DVec2::new(spacing.x + delta, spacing.y),
			GridSpacingGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	fn direction(&self, spacing: DVec2, viewport: DAffine2) -> DVec2 {
		match self {
			GridSpacingGizmoType::Top => calculate_rectangle_top_direction(spacing, viewport),
			GridSpacingGizmoType::Down => -calculate_rectangle_top_direction(spacing, viewport),
			GridSpacingGizmoType::Right => calculate_rectangle_side_direction(spacing, viewport),
			GridSpacingGizmoType::Left => -calculate_rectangle_side_direction(spacing, viewport),
			GridSpacingGizmoType::None => panic!("RowColumnGizmoType::None does not have a line"),
		}
	}

	fn mouse_icon(&self) -> MouseCursorIcon {
		match self {
			GridSpacingGizmoType::Top | GridSpacingGizmoType::Down => MouseCursorIcon::NSResize,
			GridSpacingGizmoType::Right | GridSpacingGizmoType::Left => MouseCursorIcon::EWResize,
			GridSpacingGizmoType::None => panic!("RowColumnGizmoType::None does not have a mouse_icon"),
		}
	}

	pub fn all() -> [Self; 4] {
		[Self::Top, Self::Right, Self::Down, Self::Left]
	}
}

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
