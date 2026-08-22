//! # Gizmo behaviors
//!
//! The shape-specific half of the generic gizmo system, and the only place node geometry is allowed to
//! leak into it.
//!
//! The [generic gizmos](super::generic_gizmos) own everything that is the same for every shape: the
//! hover/drag state machine, hit-testing, the handle overlay, and writing the node input. A handful of
//! behaviors are irreducibly shape-specific, though — a star's snap radii are a function of its side
//! count and its *other* radius, and no amount of registry data expresses that. Those live here as plain
//! functions, referenced from the [registry](super::gizmo_registry) table.

use crate::consts::{GIZMO_HIDE_THRESHOLD, NUMBER_OF_POINTS_DIAL_SPOKE_EXTENSION, NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH, POINT_RADIUS_HANDLE_SEGMENT_THRESHOLD};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::tool::common_functionality::gizmos::gizmo_registry::{GizmoBehavior, GizmoContext, GizmoState};
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::shapes::shape_utility::{
	draw_snapping_ticks, extract_polygon_parameters, extract_star_parameters, inside_polygon, inside_star, polygon_outline, polygon_vertex_position, star_outline, star_vertex_position,
};
use glam::{DAffine2, DVec2};
use graph_craft::document::value::TaggedValue;
use graphene_std::ParameterRef;
use graphene_std::vector::generator_nodes::star;
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_4, PI, SQRT_2, TAU};

/// The star's sides dial: previews the shape it is about to change.
pub const STAR_SIDES: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(star_sides_overlay),
	coupled_writes: None,
	handle_positions: None,
};

/// Either of the star's radius handles: snaps to the radii where the star's points line up, and previews
/// the outline while being dragged.
pub const STAR_RADIUS: GizmoBehavior = GizmoBehavior {
	snap_targets: Some(star_snap_radii),
	overlay: Some(star_radius_overlay),
	coupled_writes: None,
	handle_positions: Some(star_radius_handles),
};

/// The polygon's sides dial, the counterpart to [`STAR_SIDES`].
pub const POLYGON_SIDES: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(polygon_sides_overlay),
	coupled_writes: None,
	handle_positions: None,
};

/// The radii at which dragging one of a star's radius handles makes its points line up: the value where
/// the tips sit at 90°, the mirrored case where the handle overtakes the other radius, and then every
/// radius that puts a vertex collinear with one of its neighbors.
///
/// All of them are derived from the side count and the radius that is *not* being dragged, which is why
/// this cannot be a registry constant.
fn star_snap_radii(context: &GizmoContext) -> Vec<f64> {
	let mut snap_radii = Vec::new();

	let Some(parameters) = NodeGraphLayer::new(context.layer, &context.document.network_interface).find_node_parameters(star::IDENTIFIER) else {
		return snap_radii;
	};

	let (Some(&TaggedValue::F64(radius_1)), Some(&TaggedValue::F64(radius_2))) = (parameters.value(star::Radius1Input), parameters.value(star::Radius2Input)) else {
		return snap_radii;
	};
	let Some(&TaggedValue::U32(sides)) = parameters.value(star::SidesInput) else {
		return snap_radii;
	};

	let other_radius = if context.parameter == ParameterRef::from(star::Radius2Input) { radius_1 } else { radius_2 };

	// With one radius negative and the other positive the star is inside out, and none of the alignments
	// below describe a shape the user can see, so there is nothing worth snapping to.
	if (radius_1.signum() * radius_2.signum()).is_sign_negative() {
		return snap_radii;
	}

	let sign = if radius_1.is_sign_negative() && radius_2.is_sign_negative() { -1. } else { 1. };

	// The radius that puts the star's points at 90°, and the same alignment reached from the other side.
	let angle = (FRAC_PI_4 * 3. - PI / (sides as f64)).sin();
	snap_radii.push((other_radius.abs() * sign / angle) * FRAC_1_SQRT_2);
	snap_radii.push(other_radius.abs() * sign * angle * SQRT_2);

	// Each radius that makes a vertex collinear with one of its neighbors, walking outward.
	for i in 1..sides {
		let sides = sides as f64;
		let i = i as f64;
		let denominator = 2. * ((PI * (i - 1.)) / sides).cos() * ((PI * i) / sides).sin();
		let factor = ((2. * PI * i) / sides).sin() / denominator;

		if factor < 0. {
			break;
		}
		if other_radius.abs() * factor > 1e-6 {
			snap_radii.push(other_radius.abs() * sign * factor);
		}
		snap_radii.push((other_radius.abs() * sign) / factor);
	}

	snap_radii
}

/// A star's radius is grabbable at every vertex that radius controls: `radius_1` at the outer points,
/// `radius_2` at the inner ones. Whichever the user takes hold of, the drag runs out along that point.
fn star_radius_handles(context: &GizmoContext, value: f64) -> Vec<DVec2> {
	let Some((sides, _, _)) = extract_star_parameters(Some(context.layer), context.document) else {
		return Vec::new();
	};

	(star_first_vertex(context)..2 * sides)
		.step_by(2)
		.map(|vertex| {
			let angle = ((vertex as f64) * PI) / (sides as f64);
			DVec2::new(value * angle.sin(), -value * angle.cos())
		})
		.collect()
}

/// The vertex a star's radius parameter starts at: outer points are even, inner points odd.
fn star_first_vertex(context: &GizmoContext) -> u32 {
	if context.parameter == ParameterRef::from(star::Radius2Input) { 1 } else { 0 }
}

/// At rest, mark every vertex this radius controls so the handles are discoverable. Once one is engaged,
/// swap to the ray it is being pulled along, the outline of the shape being reshaped, and ticks at each
/// radius the drag will snap to.
fn star_radius_overlay(context: &GizmoContext, overlay_context: &mut OverlayContext) {
	let Some((sides, radius_1, radius_2)) = extract_star_parameters(Some(context.layer), context.document) else {
		return;
	};
	let viewport = context.document.metadata().transform_to_viewport(context.layer);
	let center = viewport.transform_point2(DVec2::ZERO);
	let first_vertex = star_first_vertex(context);

	if context.state == GizmoState::Inactive {
		for vertex in (first_vertex..2 * sides).step_by(2) {
			let point = star_vertex_position(viewport, vertex as i32, sides, radius_1, radius_2);

			// Once the star is this small on screen the handles crowd its center and cannot be told apart.
			if point.distance(center) < GIZMO_HIDE_THRESHOLD {
				return;
			}
			overlay_context.manipulator_handle(point, false, None);
		}
		return;
	}

	let vertex = first_vertex as i32 + 2 * context.handle_index as i32;
	let point = star_vertex_position(viewport, vertex, sides, radius_1, radius_2);
	let Some(direction) = (point - center).try_normalize() else { return };

	// Extend the ray across the viewport: the radius keeps growing past the edge of the shape, and the line
	// is what makes the direction of the drag readable.
	overlay_context.line(center, center + direction * overlay_context.viewport.size().into_dvec2().length(), None, None);
	star_outline(Some(context.layer), context.document, overlay_context);

	// The snap radii are only meaningful while both radii share a sign; see `star_snap_radii`.
	if (radius_1.signum() * radius_2.signum()).is_sign_positive() {
		let angle = ((vertex as f64) * PI) / (sides as f64);
		draw_snapping_ticks(&star_snap_radii(context), direction, viewport, angle, overlay_context);
	}
}

fn star_sides_overlay(context: &GizmoContext, overlay_context: &mut OverlayContext) {
	let Some((sides, radius_1, radius_2)) = extract_star_parameters(Some(context.layer), context.document) else {
		return;
	};
	let radius = radius_1.max(radius_2);
	let viewport = context.document.metadata().transform_to_viewport(context.layer);

	if context.state == GizmoState::Inactive {
		// At rest the spokes are only a hint that the dial is there, so they appear once the cursor is
		// inside the star, and stand down near an editable segment where they would compete with the path
		// editor's own overlays.
		if over_editable_segment(context) {
			return;
		}
		let center = viewport.transform_point2(DVec2::ZERO);
		let outermost = star_vertex_position(viewport, 0, sides, radius_1, radius_2);
		if !inside_star(viewport, sides, radius_1, radius_2, context.mouse_position) || outermost.distance(center) <= GIZMO_HIDE_THRESHOLD {
			return;
		}
	} else {
		star_outline(Some(context.layer), context.document, overlay_context);
	}

	draw_spokes(viewport, sides, radius, context.state, overlay_context);
}

fn polygon_sides_overlay(context: &GizmoContext, overlay_context: &mut OverlayContext) {
	let Some((sides, radius)) = extract_polygon_parameters(Some(context.layer), context.document) else {
		return;
	};
	let viewport = context.document.metadata().transform_to_viewport(context.layer);

	if context.state == GizmoState::Inactive {
		if over_editable_segment(context) {
			return;
		}
		let center = viewport.transform_point2(DVec2::ZERO);
		let outermost = polygon_vertex_position(viewport, 0, sides, radius);
		if !inside_polygon(viewport, sides, radius, context.mouse_position) || outermost.distance(center) <= GIZMO_HIDE_THRESHOLD {
			return;
		}
	} else {
		polygon_outline(Some(context.layer), context.document, overlay_context);
	}

	draw_spokes(viewport, sides, radius, context.state, overlay_context);
}

/// True when the cursor is close enough to one of this layer's segments that the path editor owns it.
fn over_editable_segment(context: &GizmoContext) -> bool {
	let Some(shape_editor) = context.shape_editor else { return false };

	shape_editor
		.upper_closest_segment(&context.document.network_interface, context.mouse_position, POINT_RADIUS_HANDLE_SEGMENT_THRESHOLD)
		.is_some_and(|segment| segment.layer() == context.layer)
}

/// One short line per side, radiating from the center. They lengthen once the dial is engaged, which is
/// what makes the count being edited legible while dragging.
fn draw_spokes(viewport: DAffine2, sides: u32, radius: f64, state: GizmoState, overlay_context: &mut OverlayContext) {
	let center = viewport.transform_point2(DVec2::ZERO);
	let length = match state {
		GizmoState::Inactive => NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH,
		_ => NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH * NUMBER_OF_POINTS_DIAL_SPOKE_EXTENSION,
	};

	for i in 0..sides {
		let angle = ((i as f64) * TAU) / (sides as f64);
		let point = viewport.transform_point2(DVec2::new(radius * angle.sin(), -radius * angle.cos()));

		let Some(direction) = (point - center).try_normalize() else { continue };

		// Once the shape is this small on screen the spokes are longer than the shape itself, which reads
		// as noise rather than as a control.
		if point.distance(center) < GIZMO_HIDE_THRESHOLD {
			return;
		}

		overlay_context.line(center, center + direction * length, None, None);
	}
}
