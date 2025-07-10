use super::ShapeToolData;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::transformation_cage::BoundingBoxManager;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::*;
use bezier_rs::Subpath;
use glam::{DAffine2, DMat2, DVec2};
use graph_craft::document::{InputConnector, NodeInput};
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::click_target::ClickTargetType;
use graphene_std::vector::misc::dvec2_to_point;
use kurbo::{BezPath, PathEl, Shape};
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ShapeType {
	#[default]
	Polygon = 0,
	Star = 1,
	Rectangle = 2,
	Ellipse = 3,
	Line = 4,
}

impl ShapeType {
	pub fn name(&self) -> String {
		(match self {
			Self::Polygon => "Polygon",
			Self::Star => "Star",
			Self::Rectangle => "Rectangle",
			Self::Ellipse => "Ellipse",
			Self::Line => "Line",
		})
		.into()
	}

	pub fn tooltip(&self) -> String {
		(match self {
			Self::Line => "Line Tool",
			Self::Rectangle => "Rectangle Tool",
			Self::Ellipse => "Ellipse Tool",
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

pub type ShapeToolModifierKey = [Key; 4];

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
}

/// Center, Lock Ratio, Lock Angle, Snap Angle, Increase/Decrease Side
pub fn update_radius_sign(end: DVec2, start: DVec2, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	let sign_num = if end[1] > start[1] { 1. } else { -1. };
	let new_layer = NodeGraphLayer::new(layer, &document.network_interface);

	if new_layer.find_input("Regular Polygon", 1).unwrap_or(&TaggedValue::U32(0)).to_u32() % 2 == 1 {
		let Some(polygon_node_id) = new_layer.upstream_node_id_from_name("Regular Polygon") else { return };

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(polygon_node_id, 2),
			input: NodeInput::value(TaggedValue::F64(sign_num * 0.5), false),
		});
		return;
	}

	if new_layer.find_input("Star", 1).unwrap_or(&TaggedValue::U32(0)).to_u32() % 2 == 1 {
		let Some(star_node_id) = new_layer.upstream_node_id_from_name("Star") else { return };

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
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else { continue };
		let transform = document.metadata().transform_to_viewport(layer);

		overlay_context.outline_vector(&vector_data, transform);

		for (_, &position) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
			overlay_context.manipulator_anchor(transform.transform_point2(position), false, None);
		}
	}
}

/// Extract the node input values of Star.
/// Returns an option of (sides, radius1, radius2).
pub fn extract_star_parameters(layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler) -> Option<(u32, f64, f64)> {
	let node_inputs = NodeGraphLayer::new(layer?, &document.network_interface).find_node_inputs("Star")?;

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
	let node_inputs = NodeGraphLayer::new(layer?, &document.network_interface).find_node_inputs("Regular Polygon")?;

	let (Some(&TaggedValue::U32(n)), Some(&TaggedValue::F64(radius))) = (node_inputs.get(1)?.as_value(), node_inputs.get(2)?.as_value()) else {
		return None;
	};

	Some((n, radius))
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
