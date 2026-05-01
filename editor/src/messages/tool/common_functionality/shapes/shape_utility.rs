use super::ShapeToolData;
use super::line_shape::LineEnd;
use crate::consts::{ARC_SWEEP_GIZMO_RADIUS, ARC_SWEEP_GIZMO_TEXT_HEIGHT, BOUNDS_SELECT_THRESHOLD};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::transformation_cage::BoundingBoxManager;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::*;
use glam::{DAffine2, DMat2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::NodeInputDecleration;
use graphene_std::subpath::{self, Subpath};
use graphene_std::vector::click_target::ClickTargetType;
use graphene_std::vector::misc::{ArcType, GridType, SpiralType, dvec2_to_point};
use kurbo::{BezPath, PathEl, Shape};
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum ShapeType {
	#[default]
	Polygon = 0,
	Star,
	Circle,
	Arc,
	Spiral,
	Grid,
	Arrow,
	Line,      // KEEP THIS AT THE END
	Rectangle, // KEEP THIS AT THE END
	Ellipse,   // KEEP THIS AT THE END
}

impl ShapeType {
	pub fn name(&self) -> String {
		(match self {
			Self::Polygon => "Polygon",
			Self::Star => "Star",
			Self::Circle => "Circle",
			Self::Arc => "Arc",
			Self::Spiral => "Spiral",
			Self::Grid => "Grid",
			Self::Arrow => "Arrow",
			Self::Line => "Line",
			Self::Rectangle => "Rectangle",
			Self::Ellipse => "Ellipse",
		})
		.into()
	}

	pub fn tooltip_label(&self) -> String {
		(match self {
			Self::Line => "Line Tool",
			Self::Rectangle => "Rectangle Tool",
			Self::Ellipse => "Ellipse Tool",
			_ => "",
		})
		.into()
	}

	pub fn tooltip_description(&self) -> String {
		(match self {
			// TODO: Add descriptions to all the shape tools
			_ => "",
		})
		.into()
	}

	pub fn icon_name(&self) -> String {
		(match self {
			Self::Line => "VectorLineTool",
			Self::Rectangle => "VectorRectangleTool",
			Self::Ellipse => "VectorEllipseTool",
			_ => "",
		})
		.into()
	}

	pub fn tool_type(&self) -> ToolType {
		match self {
			Self::Line => ToolType::Line,
			Self::Rectangle => ToolType::Rectangle,
			Self::Ellipse => ToolType::Ellipse,
			_ => ToolType::Shape,
		}
	}
}

pub type ShapeToolModifierKey = [Key; 3];

/// The `ShapeGizmoHandler` trait defines the interactive behavior and overlay logic for shape-specific tools in the editor.
/// A gizmo is a visual handle or control point used to manipulate a shape's properties (e.g., number of sides, radius, angle).
pub trait ShapeGizmoHandler {
	/// Called every frame to update the gizmo's interaction state based on the mouse position and selection.
	///
	/// This includes detecting hover states and preparing interaction flags or visual feedback (e.g., highlighting a hovered handle).
	fn handle_state(&mut self, selected_shape_layers: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>);

	/// Called when a mouse click occurs over the canvas and a gizmo handle is hovered.
	///
	/// Used to initiate drag interactions or toggle states on the handle, depending on the tool.
	/// For example, a hovered "number of points" handle might enter a "Dragging" state.
	fn handle_click(&mut self);

	/// Called during a drag interaction to update the shape's parameters in real time.
	///
	/// For example, a handle might calculate the distance from the drag start to determine a new radius or update the number of points.
	fn handle_update(&mut self, drag_start: DVec2, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>);

	/// Draws the static or hover-dependent overlays associated with the gizmo.
	///
	/// These overlays include visual indicators like shape outlines, control points, and hover highlights.
	fn overlays(
		&self,
		document: &DocumentMessageHandler,
		selected_shape_layers: Option<LayerNodeIdentifier>,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	);

	/// Draws overlays specifically during a drag operation.
	///
	/// Used to give real-time visual feedback based on drag progress, such as showing the updated shape preview or snapping guides.
	fn dragging_overlays(
		&self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		shape_editor: &mut &mut ShapeState,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	);

	/// Returns `true` if any handle or control point in the gizmo is currently being hovered.
	fn is_any_gizmo_hovered(&self) -> bool;

	/// Resets or clears any internal state maintained by the gizmo when it is no longer active.
	///
	/// For example, dragging states or hover flags should be cleared to avoid visual glitches when switching tools or shapes.
	fn cleanup(&mut self);

	fn mouse_cursor_icon(&self) -> Option<MouseCursorIcon>;
}

/// Check if the mouse clicked on either endpoint of a line-like shape (Line or Arrow).
pub fn clicked_on_shape_endpoints(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, shape_tool_data: &mut ShapeToolData) -> bool {
	let line_like_shape_nodes = [
		DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::line::IDENTIFIER),
		DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::arrow::IDENTIFIER),
	];

	let node_graph_layer = NodeGraphLayer::new(layer, &document.network_interface);
	let endpoint = line_like_shape_nodes.iter().find_map(|id| {
		let node_inputs = node_graph_layer.find_node_inputs(id)?;
		let &TaggedValue::DVec2(endpoint) = node_inputs[1].as_value()? else { return None };
		Some(endpoint)
	});
	let Some(endpoint) = endpoint else { return false };

	let local_start = DVec2::ZERO;
	let local_end = endpoint;

	let transform = document.metadata().transform_to_viewport(layer);
	let mouse_pos = input.mouse.position;
	let [start, end] = [local_start, local_end].map(|point| transform.transform_point2(point));

	let start_click = (mouse_pos - start).length_squared() < BOUNDS_SELECT_THRESHOLD.powi(2);
	let end_click = (mouse_pos - end).length_squared() < BOUNDS_SELECT_THRESHOLD.powi(2);
	let endpoint_click = start_click || end_click;

	if endpoint_click {
		shape_tool_data.line_data.dragging_endpoint = Some(if end_click { LineEnd::End } else { LineEnd::Start });
		let anchor_local = if end_click { local_start } else { local_end };
		shape_tool_data.data.drag_start = document.metadata().transform_to_document(layer).transform_point2(anchor_local);
		shape_tool_data.line_data.editing_layer = Some(layer);
	}

	endpoint_click
}

/// Center, Lock Ratio, Lock Angle, Snap Angle, Increase/Decrease Side
pub fn update_radius_sign(end: DVec2, start: DVec2, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	let sign_num = if end[1] > start[1] { 1. } else { -1. };
	let new_layer = NodeGraphLayer::new(layer, &document.network_interface);

	if new_layer
		.find_input(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER), 1)
		.unwrap_or(&TaggedValue::U32(0))
		.to_u32()
		% 2 == 1
	{
		let Some(polygon_node_id) = new_layer.upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::regular_polygon::IDENTIFIER)) else {
			return;
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(polygon_node_id, 2),
			input: NodeInput::value(TaggedValue::F64(sign_num * 0.5), false),
		});
		return;
	}

	if new_layer
		.find_input(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::star::IDENTIFIER), 1)
		.unwrap_or(&TaggedValue::U32(0))
		.to_u32()
		% 2 == 1
	{
		let Some(star_node_id) = new_layer.upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::star::IDENTIFIER)) else {
			return;
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(star_node_id, 2),
			input: NodeInput::value(TaggedValue::F64(sign_num * 0.5), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(star_node_id, 3),
			input: NodeInput::value(TaggedValue::F64(sign_num * 0.25), false),
		});
	}
}

pub fn transform_cage_overlays(document: &DocumentMessageHandler, tool_data: &mut ShapeToolData, overlay_context: &mut OverlayContext) {
	let mut transform = document
		.network_interface
		.selected_nodes()
		.selected_visible_and_unlocked_layers(&document.network_interface)
		.find(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
		.map(|layer| document.metadata().transform_to_viewport_with_first_transform_node_if_group(layer, &document.network_interface))
		.unwrap_or_default();

	// Check if the matrix is not invertible
	let mut transform_tampered = false;
	if transform.matrix2.determinant() == 0. {
		transform.matrix2 += DMat2::IDENTITY * 1e-4; // TODO: Is this the cleanest way to handle this?
		transform_tampered = true;
	}

	let bounds = document
		.network_interface
		.selected_nodes()
		.selected_visible_and_unlocked_layers(&document.network_interface)
		.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
		.filter_map(|layer| {
			document
				.metadata()
				.bounding_box_with_transform(layer, transform.inverse() * document.metadata().transform_to_viewport(layer))
		})
		.reduce(graphene_std::renderer::Quad::combine_bounds);

	if let Some(bounds) = bounds {
		let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());

		bounding_box_manager.bounds = bounds;
		bounding_box_manager.transform = transform;
		bounding_box_manager.transform_tampered = transform_tampered;
		bounding_box_manager.render_overlays(overlay_context, true);
	} else {
		tool_data.bounding_box_manager.take();
	}
}

pub fn anchor_overlays(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	for layer in document.network_interface.selected_nodes().selected_layers(document.metadata()) {
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
		let transform = document.metadata().transform_to_viewport(layer);

		overlay_context.outline_vector(&vector, transform);

		for (_, &position) in vector.point_domain.ids().iter().zip(vector.point_domain.positions()) {
			overlay_context.manipulator_anchor(transform.transform_point2(position), false, None);
		}
	}
}

/// Extract the node input values of Star.
/// Returns an option of (sides, radius1, radius2).
pub fn extract_star_parameters(layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler) -> Option<(u32, f64, f64)> {
	let node_inputs = NodeGraphLayer::new(layer?, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::star::IDENTIFIER))?;

	let (Some(&TaggedValue::U32(sides)), Some(&TaggedValue::F64(radius_1)), Some(&TaggedValue::F64(radius_2))) =
		(node_inputs.get(1)?.as_value(), node_inputs.get(2)?.as_value(), node_inputs.get(3)?.as_value())
	else {
		return None;
	};

	Some((sides, radius_1, radius_2))
}

/// Extract the node input values of Polygon.
/// Returns an option of (sides, radius).
pub fn extract_polygon_parameters(layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler) -> Option<(u32, f64)> {
	let node_inputs =
		NodeGraphLayer::new(layer?, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER))?;

	let (Some(&TaggedValue::U32(n)), Some(&TaggedValue::F64(radius))) = (node_inputs.get(1)?.as_value(), node_inputs.get(2)?.as_value()) else {
		return None;
	};

	Some((n, radius))
}

/// Extract the node input values of an arc.
/// Returns an option of (radius, start angle, sweep angle, arc type).
pub fn extract_arc_parameters(layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler) -> Option<(f64, f64, f64, ArcType)> {
	let node_inputs = NodeGraphLayer::new(layer?, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::arc::IDENTIFIER))?;

	let (Some(&TaggedValue::F64(radius)), Some(&TaggedValue::F64(start_angle)), Some(&TaggedValue::F64(sweep_angle)), Some(&TaggedValue::ArcType(arc_type))) = (
		node_inputs.get(1)?.as_value(),
		node_inputs.get(2)?.as_value(),
		node_inputs.get(3)?.as_value(),
		node_inputs.get(4)?.as_value(),
	) else {
		return None;
	};

	Some((radius, start_angle, sweep_angle, arc_type))
}

/// Extract the node input values of spiral.
/// Returns an option of (spiral type, start angle, inner radius, outer radius, turns, angle resolution).
pub fn extract_spiral_parameters(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Option<(SpiralType, f64, f64, f64, f64, f64)> {
	use graphene_std::vector::generator_nodes::spiral::*;

	let node_inputs = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::spiral::IDENTIFIER))?;

	let (
		Some(&TaggedValue::SpiralType(spiral_type)),
		Some(&TaggedValue::F64(start_angle)),
		Some(&TaggedValue::F64(inner_radius)),
		Some(&TaggedValue::F64(outer_radius)),
		Some(&TaggedValue::F64(turns)),
		Some(&TaggedValue::F64(angle_resolution)),
	) = (
		node_inputs.get(SpiralTypeInput::INDEX)?.as_value(),
		node_inputs.get(StartAngleInput::INDEX)?.as_value(),
		node_inputs.get(InnerRadiusInput::INDEX)?.as_value(),
		node_inputs.get(OuterRadiusInput::INDEX)?.as_value(),
		node_inputs.get(TurnsInput::INDEX)?.as_value(),
		node_inputs.get(AngularResolutionInput::INDEX)?.as_value(),
	)
	else {
		return None;
	};

	Some((spiral_type, start_angle, inner_radius, outer_radius, turns, angle_resolution))
}

/// Calculate the viewport positions of arc endpoints
pub fn arc_end_points(layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler) -> Option<(DVec2, DVec2)> {
	let (radius, start_angle, sweep_angle, _) = extract_arc_parameters(Some(layer?), document)?;

	let viewport = document.metadata().transform_to_viewport(layer?);

	arc_end_points_ignore_layer(radius, start_angle, sweep_angle, Some(viewport))
}

pub fn arc_end_points_ignore_layer(radius: f64, start_angle: f64, sweep_angle: f64, viewport: Option<DAffine2>) -> Option<(DVec2, DVec2)> {
	let end_angle = start_angle.to_radians() + sweep_angle.to_radians();

	let start_point = radius * DVec2::from_angle(start_angle.to_radians());
	let end_point = radius * DVec2::from_angle(end_angle);

	if let Some(transform) = viewport {
		return Some((transform.transform_point2(start_point), transform.transform_point2(end_point)));
	}

	Some((start_point, end_point))
}

/// Calculate the viewport position of a star vertex given its index
/// Extract the node input values of Circle.
/// Returns an option of (radius).
pub fn extract_circle_radius(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Option<f64> {
	let node_inputs = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::circle::IDENTIFIER))?;

	let Some(&TaggedValue::F64(radius)) = node_inputs.get(1)?.as_value() else {
		return None;
	};

	Some(radius)
}

/// Calculate the viewport position of as a star vertex given its index
pub fn star_vertex_position(viewport: DAffine2, vertex_index: i32, n: u32, radius1: f64, radius2: f64) -> DVec2 {
	let angle = ((vertex_index as f64) * PI) / (n as f64);
	let radius = if vertex_index % 2 == 0 { radius1 } else { radius2 };

	viewport.transform_point2(DVec2 {
		x: radius * angle.sin(),
		y: -radius * angle.cos(),
	})
}

/// Calculate the viewport position of a polygon vertex given its index
pub fn polygon_vertex_position(viewport: DAffine2, vertex_index: i32, n: u32, radius: f64) -> DVec2 {
	let angle = ((vertex_index as f64) * TAU) / (n as f64);

	viewport.transform_point2(DVec2 {
		x: radius * angle.sin(),
		y: -radius * angle.cos(),
	})
}

/// Outlines the geometric shape made by star-node
pub fn star_outline(layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	let Some(layer) = layer else { return };
	let Some((sides, radius1, radius2)) = extract_star_parameters(Some(layer), document) else {
		return;
	};

	let viewport = document.metadata().transform_to_viewport(layer);

	let points = sides as u64;
	let diameter: f64 = radius1 * 2.;
	let inner_diameter = radius2 * 2.;

	let subpath: Vec<ClickTargetType> = vec![ClickTargetType::Subpath(Subpath::new_star_polygon(DVec2::splat(-diameter), points, diameter, inner_diameter))];

	overlay_context.outline(subpath.iter(), viewport, None);
}

/// Outlines the geometric shape made by polygon-node
pub fn polygon_outline(layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	let Some(layer) = layer else { return };
	let Some((sides, radius)) = extract_polygon_parameters(Some(layer), document) else {
		return;
	};

	let viewport = document.metadata().transform_to_viewport(layer);

	let points = sides as u64;
	let radius: f64 = radius * 2.;

	let subpath: Vec<ClickTargetType> = vec![ClickTargetType::Subpath(Subpath::new_regular_polygon(DVec2::splat(-radius), points, radius))];

	overlay_context.outline(subpath.iter(), viewport, None);
}

/// Outlines the geometric shape made by an Arc node
pub fn arc_outline(layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	let Some(layer) = layer else { return };

	let Some((radius, start_angle, sweep_angle, arc_type)) = extract_arc_parameters(Some(layer), document) else {
		return;
	};

	let subpath: Vec<ClickTargetType> = vec![ClickTargetType::Subpath(Subpath::new_arc(
		radius,
		start_angle / 360. * std::f64::consts::TAU,
		sweep_angle / 360. * std::f64::consts::TAU,
		match arc_type {
			ArcType::Open => subpath::ArcType::Open,
			ArcType::Closed => subpath::ArcType::Closed,
			ArcType::PieSlice => subpath::ArcType::PieSlice,
		},
	))];
	let viewport = document.metadata().transform_to_viewport(layer);

	overlay_context.outline(subpath.iter(), viewport, None);
}

/// Check if the the cursor is inside the geometric star shape made by the Star node without any upstream node modifications
pub fn inside_star(viewport: DAffine2, n: u32, radius1: f64, radius2: f64, mouse_position: DVec2) -> bool {
	let mut paths = Vec::new();

	for i in 0..2 * n {
		let new_point = dvec2_to_point(star_vertex_position(viewport, i as i32, n, radius1, radius2));

		if i == 0 {
			paths.push(PathEl::MoveTo(new_point));
		} else {
			paths.push(PathEl::LineTo(new_point));
		}
	}

	paths.push(PathEl::ClosePath);

	let bez_path = BezPath::from_vec(paths);
	let (shape, bbox) = (bez_path.clone(), bez_path.bounding_box());

	if bbox.x0 > mouse_position.x || bbox.y0 > mouse_position.y || bbox.x1 < mouse_position.x || bbox.y1 < mouse_position.y {
		return false;
	}

	let winding = shape.winding(dvec2_to_point(mouse_position));

	// Non-zero fill rule
	winding != 0
}

/// Check if the the cursor is inside the geometric polygon shape made by the Polygon node without any upstream node modifications
pub fn inside_polygon(viewport: DAffine2, n: u32, radius: f64, mouse_position: DVec2) -> bool {
	let mut paths = Vec::new();

	for i in 0..n {
		let new_point = dvec2_to_point(polygon_vertex_position(viewport, i as i32, n, radius));

		if i == 0 {
			paths.push(PathEl::MoveTo(new_point));
		} else {
			paths.push(PathEl::LineTo(new_point));
		}
	}

	paths.push(PathEl::ClosePath);

	let bez_path = BezPath::from_vec(paths);
	let (shape, bbox) = (bez_path.clone(), bez_path.bounding_box());

	if bbox.x0 > mouse_position.x || bbox.y0 > mouse_position.y || bbox.x1 < mouse_position.x || bbox.y1 < mouse_position.y {
		return false;
	}

	let winding = shape.winding(dvec2_to_point(mouse_position));

	// Non-zero fill rule
	winding != 0
}

pub fn draw_snapping_ticks(snap_radii: &[f64], direction: DVec2, viewport: DAffine2, angle: f64, overlay_context: &mut OverlayContext) {
	for &snapped_radius in snap_radii {
		let Some(tick_direction) = direction.perp().try_normalize() else {
			return;
		};

		let tick_position = viewport.transform_point2(DVec2 {
			x: snapped_radius * angle.sin(),
			y: -snapped_radius * angle.cos(),
		});

		overlay_context.line(tick_position, tick_position + tick_direction * 5., None, Some(2.));
		overlay_context.line(tick_position, tick_position - tick_direction * 5., None, Some(2.));
	}
}

/// Wraps an angle (in radians) into the range [0, 2π).
pub fn wrap_to_tau(angle: f64) -> f64 {
	(angle % TAU + TAU) % TAU
}

pub fn format_rounded(value: f64, precision: usize) -> String {
	format!("{value:.precision$}").trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Gives the approximated angle to display in degrees, given an angle in degrees.
pub fn calculate_display_angle(angle: f64) -> f64 {
	if angle.is_sign_positive() {
		angle - (angle / 360.).floor() * 360.
	} else if angle.is_sign_negative() {
		angle - ((angle / 360.).floor() + 1.) * 360.
	} else {
		angle
	}
}

pub fn calculate_arc_text_transform(angle: f64, offset_angle: f64, center: DVec2, width: f64) -> DAffine2 {
	let text_angle_on_unit_circle = DVec2::from_angle((angle.to_radians() % TAU) / 2. + offset_angle);
	let text_texture_position = DVec2::new(
		(ARC_SWEEP_GIZMO_RADIUS + 4. + width) * text_angle_on_unit_circle.x,
		(ARC_SWEEP_GIZMO_RADIUS + ARC_SWEEP_GIZMO_TEXT_HEIGHT) * text_angle_on_unit_circle.y,
	);
	DAffine2::from_translation(text_texture_position + center)
}

/// Extract the node input values of Grid.
/// Returns an option of (grid_type, spacing, columns, rows, angles).
pub fn extract_grid_parameters(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Option<(GridType, DVec2, u32, u32, DVec2)> {
	use graphene_std::vector::generator_nodes::grid::*;

	let node_inputs = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::grid::IDENTIFIER))?;

	let (Some(&TaggedValue::GridType(grid_type)), Some(&TaggedValue::DVec2(spacing)), Some(&TaggedValue::U32(columns)), Some(&TaggedValue::U32(rows)), Some(&TaggedValue::DVec2(angles))) = (
		node_inputs.get(GridTypeInput::INDEX)?.as_value(),
		node_inputs.get(SpacingInput::<f64>::INDEX)?.as_value(),
		node_inputs.get(ColumnsInput::INDEX)?.as_value(),
		node_inputs.get(RowsInput::INDEX)?.as_value(),
		node_inputs.get(AnglesInput::INDEX)?.as_value(),
	) else {
		return None;
	};

	Some((grid_type, spacing, columns, rows, angles))
}

#[cfg(test)]
mod tests {
	use super::{arc_end_points_ignore_layer, calculate_display_angle, format_rounded, inside_polygon, inside_star, polygon_vertex_position, star_vertex_position, wrap_to_tau};
	use glam::{DAffine2, DVec2};
	use std::f64::consts::{PI, TAU};

	// ── wrap_to_tau ─────────────────────────────────────────────────────────────

	#[test]
	fn wrap_zero_stays_zero() {
		assert_eq!(wrap_to_tau(0.), 0.);
	}

	#[test]
	fn wrap_pi_stays_pi() {
		assert!((wrap_to_tau(PI) - PI).abs() < 1e-10);
	}

	#[test]
	fn wrap_tau_becomes_zero() {
		assert!(wrap_to_tau(TAU).abs() < 1e-10);
	}

	#[test]
	fn wrap_beyond_tau_reduces_to_remainder() {
		// TAU + 1 wraps back to 1
		assert!((wrap_to_tau(TAU + 1.) - 1.).abs() < 1e-10);
	}

	#[test]
	fn wrap_negative_pi_becomes_pi() {
		// -π + 2π = π
		assert!((wrap_to_tau(-PI) - PI).abs() < 1e-10);
	}

	#[test]
	fn wrap_negative_small_angle_wraps_near_tau() {
		// -0.5 → TAU - 0.5
		assert!((wrap_to_tau(-0.5) - (TAU - 0.5)).abs() < 1e-10);
	}

	#[test]
	fn wrap_two_full_turns_returns_zero() {
		assert!(wrap_to_tau(2. * TAU).abs() < 1e-10);
	}

	// ── format_rounded ──────────────────────────────────────────────────────────

	#[test]
	fn format_rounded_trims_trailing_zeros_and_dot() {
		assert_eq!(format_rounded(1.0, 2), "1");
	}

	#[test]
	fn format_rounded_keeps_significant_decimal() {
		assert_eq!(format_rounded(1.5, 2), "1.5");
	}

	#[test]
	fn format_rounded_trims_trailing_zero_only() {
		assert_eq!(format_rounded(1.50, 3), "1.5");
	}

	#[test]
	fn format_rounded_zero_precision_integer() {
		assert_eq!(format_rounded(100.0, 0), "100");
	}

	#[test]
	fn format_rounded_rounds_last_digit() {
		assert_eq!(format_rounded(3.14159, 3), "3.142");
	}

	#[test]
	fn format_rounded_zero_value() {
		assert_eq!(format_rounded(0.0, 3), "0");
	}

	#[test]
	fn format_rounded_preserves_all_significant_digits() {
		assert_eq!(format_rounded(1.23, 2), "1.23");
	}

	// ── calculate_display_angle ─────────────────────────────────────────────────

	#[test]
	fn display_angle_positive_within_range_unchanged() {
		assert!((calculate_display_angle(45.) - 45.).abs() < 1e-10);
	}

	#[test]
	fn display_angle_positive_beyond_360_wraps() {
		// 400° → 40°
		assert!((calculate_display_angle(400.) - 40.).abs() < 1e-10);
	}

	#[test]
	fn display_angle_exactly_360_becomes_zero() {
		assert!(calculate_display_angle(360.).abs() < 1e-10);
	}

	#[test]
	fn display_angle_720_becomes_zero() {
		assert!(calculate_display_angle(720.).abs() < 1e-10);
	}

	#[test]
	fn display_angle_negative_small_unchanged() {
		// -45 is in (−360, 0): formula returns -45
		assert!((calculate_display_angle(-45.) - (-45.)).abs() < 1e-10);
	}

	#[test]
	fn display_angle_negative_beyond_neg_360_wraps() {
		// -400° → -40°
		assert!((calculate_display_angle(-400.) - (-40.)).abs() < 1e-10);
	}

	#[test]
	fn display_angle_positive_zero_returns_zero() {
		// +0.0 is sign-positive, first branch: 0 − 0 = 0
		assert_eq!(calculate_display_angle(0.), 0.);
	}

	// ── arc_end_points_ignore_layer ─────────────────────────────────────────────

	#[test]
	fn arc_endpoints_no_viewport_zero_start_zero_sweep_at_unit_radius() {
		// start=0°, sweep=0°: both points at (1, 0)
		let (start, end) = arc_end_points_ignore_layer(1., 0., 0., None).unwrap();
		assert!((start.x - 1.).abs() < 1e-10, "start.x expected 1, got {}", start.x);
		assert!(start.y.abs() < 1e-10, "start.y expected 0, got {}", start.y);
		assert!((end.x - 1.).abs() < 1e-10, "end.x expected 1, got {}", end.x);
		assert!(end.y.abs() < 1e-10, "end.y expected 0, got {}", end.y);
	}

	#[test]
	fn arc_endpoints_no_viewport_quarter_sweep() {
		// start=0°, sweep=90°: start at (1,0), end at (0,1)
		let (start, end) = arc_end_points_ignore_layer(1., 0., 90., None).unwrap();
		assert!((start.x - 1.).abs() < 1e-10, "start.x expected 1, got {}", start.x);
		assert!(start.y.abs() < 1e-10, "start.y expected 0, got {}", start.y);
		assert!(end.x.abs() < 1e-10, "end.x expected 0, got {}", end.x);
		assert!((end.y - 1.).abs() < 1e-10, "end.y expected 1, got {}", end.y);
	}

	#[test]
	fn arc_endpoints_scales_with_radius() {
		// Radius 5 at start=0°, sweep=0°: start at (5, 0)
		let (start, _) = arc_end_points_ignore_layer(5., 0., 0., None).unwrap();
		assert!((start.x - 5.).abs() < 1e-10, "start.x expected 5, got {}", start.x);
	}

	#[test]
	fn arc_endpoints_with_identity_viewport_matches_no_viewport() {
		// Identity transform must not change coordinates
		let (start_id, end_id) = arc_end_points_ignore_layer(1., 0., 90., Some(DAffine2::IDENTITY)).unwrap();
		let (start_none, end_none) = arc_end_points_ignore_layer(1., 0., 90., None).unwrap();
		assert!((start_id - start_none).length() < 1e-10);
		assert!((end_id - end_none).length() < 1e-10);
	}

	#[test]
	fn arc_endpoints_half_circle_sweep() {
		// start=0°, sweep=180°: end lands at (-1, 0) for unit radius
		let (_, end) = arc_end_points_ignore_layer(1., 0., 180., None).unwrap();
		assert!((end.x - (-1.)).abs() < 1e-10, "end.x expected -1, got {}", end.x);
		assert!(end.y.abs() < 1e-10, "end.y expected 0, got {}", end.y);
	}

	// ── star_vertex_position ────────────────────────────────────────────────────

	#[test]
	fn star_vertex_even_index_uses_outer_radius() {
		// vertex_index=0 (even) → outer radius, angle=0 → (0, -radius1)
		let pos = star_vertex_position(DAffine2::IDENTITY, 0, 5, 10., 5.);
		assert!(pos.x.abs() < 1e-10, "x expected ~0, got {}", pos.x);
		assert!((pos.y - (-10.)).abs() < 1e-10, "y expected -10, got {}", pos.y);
	}

	#[test]
	fn star_vertex_odd_index_uses_inner_radius() {
		// vertex_index=1 (odd) → inner radius
		let pos = star_vertex_position(DAffine2::IDENTITY, 1, 5, 10., 5.);
		let angle = PI / 5.;
		assert!((pos.x - 5. * angle.sin()).abs() < 1e-10, "x mismatch, got {}", pos.x);
		assert!((pos.y - (-5. * angle.cos())).abs() < 1e-10, "y mismatch, got {}", pos.y);
	}

	#[test]
	fn star_vertex_second_outer_point() {
		// vertex_index=2 (even) → outer radius, angle = 2π/5
		let pos = star_vertex_position(DAffine2::IDENTITY, 2, 5, 10., 5.);
		let angle = 2. * PI / 5.;
		assert!((pos.x - 10. * angle.sin()).abs() < 1e-10, "x mismatch, got {}", pos.x);
		assert!((pos.y - (-10. * angle.cos())).abs() < 1e-10, "y mismatch, got {}", pos.y);
	}

	// ── polygon_vertex_position ──────────────────────────────────────────────────

	#[test]
	fn polygon_vertex_zero_index_points_up() {
		// vertex 0: angle=0 → x=0, y=−radius
		let pos = polygon_vertex_position(DAffine2::IDENTITY, 0, 4, 10.);
		assert!(pos.x.abs() < 1e-10, "x expected ~0, got {}", pos.x);
		assert!((pos.y - (-10.)).abs() < 1e-10, "y expected -10, got {}", pos.y);
	}

	#[test]
	fn polygon_vertex_first_of_square_points_right() {
		// n=4, vertex 1: angle=TAU/4=90° → x=radius, y=0
		let pos = polygon_vertex_position(DAffine2::IDENTITY, 1, 4, 10.);
		assert!((pos.x - 10.).abs() < 1e-10, "x expected 10, got {}", pos.x);
		assert!(pos.y.abs() < 1e-10, "y expected ~0, got {}", pos.y);
	}

	#[test]
	fn polygon_vertex_halfway_around_points_down() {
		// n=4, vertex 2: angle=TAU/2=180° → x=0, y=+radius
		let pos = polygon_vertex_position(DAffine2::IDENTITY, 2, 4, 10.);
		assert!(pos.x.abs() < 1e-10, "x expected ~0, got {}", pos.x);
		assert!((pos.y - 10.).abs() < 1e-10, "y expected 10, got {}", pos.y);
	}

	// ── inside_polygon ───────────────────────────────────────────────────────────

	#[test]
	fn inside_polygon_center_is_inside() {
		assert!(inside_polygon(DAffine2::IDENTITY, 6, 50., DVec2::ZERO), "Center of hexagon should be inside");
	}

	#[test]
	fn inside_polygon_far_point_is_outside() {
		assert!(!inside_polygon(DAffine2::IDENTITY, 6, 50., DVec2::new(1000., 1000.)), "Far point should be outside");
	}

	#[test]
	fn inside_polygon_point_beyond_vertex_is_outside() {
		// Hexagon radius=50, topmost vertex at (0,−50); point at (0,−60) is beyond
		assert!(!inside_polygon(DAffine2::IDENTITY, 6, 50., DVec2::new(0., -60.)), "Point beyond outer vertex should be outside");
	}

	#[test]
	fn inside_polygon_point_near_center_is_inside() {
		assert!(inside_polygon(DAffine2::IDENTITY, 6, 50., DVec2::new(10., 10.)), "Point near center should be inside hexagon");
	}

	// ── inside_star ──────────────────────────────────────────────────────────────

	#[test]
	fn inside_star_center_is_inside() {
		assert!(inside_star(DAffine2::IDENTITY, 5, 50., 25., DVec2::ZERO), "Center should be inside 5-point star");
	}

	#[test]
	fn inside_star_far_point_is_outside() {
		assert!(!inside_star(DAffine2::IDENTITY, 5, 50., 25., DVec2::new(1000., 0.)), "Far point should be outside");
	}

	#[test]
	fn inside_star_point_beyond_outer_tip_is_outside() {
		// Outermost tip at (0,−50); point at (0,−60) is outside
		assert!(!inside_star(DAffine2::IDENTITY, 5, 50., 25., DVec2::new(0., -60.)), "Point beyond outer tip should be outside");
	}

	#[test]
	fn inside_star_point_near_center_is_inside() {
		assert!(inside_star(DAffine2::IDENTITY, 5, 50., 25., DVec2::new(5., 5.)), "Point near center should be inside star");
	}
}
