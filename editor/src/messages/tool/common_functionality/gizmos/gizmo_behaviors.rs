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

use crate::consts::{ARC_SNAP_THRESHOLD, COLOR_OVERLAY_RED};
use crate::consts::{GIZMO_HIDE_THRESHOLD, NUMBER_OF_POINTS_DIAL_SPOKE_EXTENSION, NUMBER_OF_POINTS_DIAL_SPOKE_LENGTH, POINT_RADIUS_HANDLE_SEGMENT_THRESHOLD};
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::overlays::utility_functions::text_width;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::tool::common_functionality::gizmos::gizmo_registry::{DragInput, DragWrites, GizmoBehavior, GizmoContext, GizmoState};
use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, get_stroke_width};
use crate::messages::tool::common_functionality::shapes::grid_shape::RowColumnGizmoType;
use crate::messages::tool::common_functionality::shapes::shape_utility::{
	arc_end_points, arc_end_points_ignore_layer, arc_outline, calculate_arc_text_transform, draw_snapping_ticks, extract_arc_parameters, extract_circle_radius, extract_grid_parameters,
	extract_polygon_parameters, extract_spiral_parameters, extract_star_parameters, format_rounded, inside_polygon, inside_star, polygon_outline, polygon_vertex_position, star_outline,
	star_vertex_position,
};
use crate::messages::tool::common_functionality::shapes::spiral_shape::calculate_spiral_endpoints;
use glam::{DAffine2, DVec2};
use graph_craft::document::value::TaggedValue;
use graphene_std::NodeParameter;
use graphene_std::ParameterRef;
use graphene_std::vector::algorithms::shapes::{calculate_growth_factor, spiral_point};
use graphene_std::vector::generator_nodes::star;
use graphene_std::vector::misc::{GridType, SpiralType, dvec2_to_point, get_line_endpoints};
use kurbo::ParamCurveNearest;
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_4, PI, SQRT_2, TAU};

/// The star's sides dial: previews the shape it is about to change.
pub const STAR_SIDES: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(star_sides_overlay),
	coupled_writes: None,
	handle_positions: None,
	hover_distances: None,
	drag: None,
	angle_deadzone: 0.,
	draws_own_handle: false,
};

/// Either of the star's radius handles: snaps to the radii where the star's points line up, and previews
/// the outline while being dragged.
pub const STAR_RADIUS: GizmoBehavior = GizmoBehavior {
	snap_targets: Some(star_snap_radii),
	overlay: Some(star_radius_overlay),
	coupled_writes: None,
	handle_positions: Some(star_radius_handles),
	hover_distances: None,
	drag: None,
	angle_deadzone: 0.,
	draws_own_handle: false,
};

/// A circular radius, for the circle and the arc. Grabbed anywhere on the circumference rather than at one
/// point on it, which is how the hand-written handler worked and what the shape invites.
pub const CIRCULAR_RADIUS: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(circular_radius_overlay),
	coupled_writes: None,
	handle_positions: None,
	hover_distances: Some(circular_radius_distances),
	drag: None,
	angle_deadzone: 0.,
	draws_own_handle: true,
};

/// The grid's row count, grabbed along its top or bottom edge.
pub const GRID_ROWS: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(grid_edge_overlay),
	coupled_writes: None,
	handle_positions: None,
	hover_distances: Some(grid_row_distances),
	drag: Some(grid_edge_drag),
	angle_deadzone: 0.,
	draws_own_handle: true,
};

/// The grid's column count, grabbed along its left or right edge.
pub const GRID_COLUMNS: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(grid_edge_overlay),
	coupled_writes: None,
	handle_positions: None,
	hover_distances: Some(grid_column_distances),
	drag: Some(grid_edge_drag),
	angle_deadzone: 0.,
	draws_own_handle: true,
};

/// The arc's sweep, grabbable at either end of the curve. Dragging either endpoint reshapes the arc; the
/// start endpoint carries the whole arc round with it, the end endpoint only opens or closes the sweep.
pub const ARC_SWEEP: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(arc_sweep_overlay),
	coupled_writes: None,
	handle_positions: Some(arc_sweep_handles),
	hover_distances: None,
	drag: Some(arc_sweep_drag),
	angle_deadzone: 0.,
	draws_own_handle: false,
};

/// The spiral's winding control. Grabbable at either end of the curve; dragging winds or unwinds it.
pub const SPIRAL_TURNS: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(spiral_turns_overlay),
	coupled_writes: None,
	handle_positions: Some(spiral_turns_handles),
	hover_distances: None,
	drag: Some(spiral_turns_drag),
	angle_deadzone: 0.5,
	draws_own_handle: false,
};

/// The polygon's radius, grabbable at any of its corners.
pub const POLYGON_RADIUS: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(polygon_radius_overlay),
	coupled_writes: None,
	handle_positions: Some(polygon_radius_handles),
	hover_distances: None,
	drag: None,
	angle_deadzone: 0.,
	draws_own_handle: false,
};

/// The polygon's sides dial, the counterpart to [`STAR_SIDES`].
pub const POLYGON_SIDES: GizmoBehavior = GizmoBehavior {
	snap_targets: None,
	overlay: Some(polygon_sides_overlay),
	coupled_writes: None,
	handle_positions: None,
	hover_distances: None,
	drag: None,
	angle_deadzone: 0.,
	draws_own_handle: false,
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

/// The spiral winds the opposite way round from the shared accumulator's sense of positive rotation.
fn spiral_swept_angle(drag: &DragInput) -> f64 {
	-drag.total_angle
}

/// Read one of the node's inputs as it stood when the drag began.
fn initial_f64(drag: &DragInput, index: usize) -> Option<f64> {
	match drag.initial_parameters.get(index)? {
		Some(TaggedValue::F64(value)) => Some(*value),
		_ => None,
	}
}

/// The spiral is grabbable at both ends of the curve: the inner end where it starts winding and the outer
/// end where it stops.
fn spiral_turns_handles(context: &GizmoContext, _value: f64) -> Vec<DVec2> {
	let Some((spiral_type, start_angle, inner_radius, outer_radius, turns, _)) = extract_spiral_parameters(context.layer, context.document) else {
		return Vec::new();
	};
	let growth_factor = calculate_growth_factor(inner_radius, turns, outer_radius, spiral_type);
	let start_angle = start_angle.to_radians();

	vec![
		spiral_point(start_angle, inner_radius, growth_factor, spiral_type),
		spiral_point(turns * TAU + start_angle, inner_radius, growth_factor, spiral_type),
	]
}

/// Winding the spiral by dragging either end.
///
/// Turns alone would change the spiral's tightness as it grows, so the outer radius moves with it by
/// whatever keeps the growth factor the drag started with. Taking hold of the inner end winds the other
/// way and carries the start angle along, so the end the user is *not* holding stays put.
fn spiral_turns_drag(context: &GizmoContext, drag: &mut DragInput) -> DragWrites {
	use graphene_std::vector::generator_nodes::spiral::*;

	let (Some(initial_turns), Some(initial_outer_radius), Some(initial_inner_radius), Some(initial_start_angle)) = (
		initial_f64(drag, TurnsInput::INDEX),
		initial_f64(drag, OuterRadiusInput::INDEX),
		initial_f64(drag, InnerRadiusInput::INDEX),
		initial_f64(drag, StartAngleInput::INDEX),
	) else {
		return DragWrites::default();
	};
	let Some((spiral_type, ..)) = extract_spiral_parameters(context.layer, context.document) else {
		return DragWrites::default();
	};

	let growth_factor = calculate_growth_factor(initial_inner_radius, initial_turns, initial_outer_radius, spiral_type);
	let turns_delta = spiral_swept_angle(drag) / 360.;

	let outer_radius_change = match spiral_type {
		SpiralType::Archimedean => turns_delta * growth_factor * TAU,
		SpiralType::Logarithmic => initial_outer_radius * ((growth_factor * TAU * turns_delta).exp() - 1.),
	};
	if !outer_radius_change.is_finite() {
		return DragWrites::default();
	}

	// Handle 0 is the inner end of the curve; dragging it winds the spiral in the opposite direction.
	let dragging_inner_end = context.handle_index == 0;
	let sign = if dragging_inner_end { -1. } else { 1. };

	// A spiral needs at least half a turn to read as one, and a non-positive outer radius has no curve.
	let mut writes = vec![
		(TurnsInput.into(), TaggedValue::F64((initial_turns + turns_delta * sign).max(0.5))),
		(OuterRadiusInput.into(), TaggedValue::F64((initial_outer_radius + outer_radius_change * sign).max(0.1))),
	];
	if dragging_inner_end {
		writes.push((StartAngleInput.into(), TaggedValue::F64(initial_start_angle + spiral_swept_angle(drag))));
	}

	DragWrites::inputs(writes)
}

/// Mark both ends of the spiral at rest, and the end being held once one is grabbed.
fn spiral_turns_overlay(context: &GizmoContext, overlay_context: &mut OverlayContext) {
	let viewport = context.document.metadata().transform_to_viewport(context.layer);

	if context.state == GizmoState::Inactive {
		for theta in [0., TAU] {
			if let Some(endpoint) = calculate_spiral_endpoints(context.layer, context.document, viewport, theta) {
				overlay_context.manipulator_handle(endpoint, false, None);
			}
		}
		return;
	}

	let theta = if context.handle_index == 0 { 0. } else { TAU };
	if let Some(endpoint) = calculate_spiral_endpoints(context.layer, context.document, viewport, theta) {
		overlay_context.manipulator_handle(endpoint, true, Some(COLOR_OVERLAY_RED));
	}
}

/// Overwrite one of the drag's remembered starting values, for a gesture that has to re-anchor itself.
fn set_initial(drag: &mut DragInput, index: usize, value: f64) {
	if let Some(slot) = drag.initial_parameters.get_mut(index) {
		*slot = Some(TaggedValue::F64(value));
	}
}

/// The sweep is grabbable at both ends of the arc.
fn arc_sweep_handles(context: &GizmoContext, _value: f64) -> Vec<DVec2> {
	let Some((radius, start_angle, sweep_angle, _)) = extract_arc_parameters(Some(context.layer), context.document) else {
		return Vec::new();
	};
	let Some((start, end)) = arc_end_points_ignore_layer(radius, start_angle, sweep_angle, None) else {
		return Vec::new();
	};

	vec![start, end]
}

/// The angles a sweep settles onto: every eighth of a turn, from closed to fully round.
fn arc_snap_angles() -> Vec<f64> {
	(0..=8).map(|i| (i as f64 * FRAC_PI_4).to_degrees()).collect()
}

/// How far the sweep must move to land on the nearest snap angle, or `None` if none is close enough.
fn arc_snap_delta(sweep_angle: f64, dragging_start: bool) -> Option<f64> {
	arc_snap_angles().into_iter().find(|angle| (angle - sweep_angle).abs() <= ARC_SNAP_THRESHOLD).map(|angle| {
		let delta = angle - sweep_angle;
		// Dragging the start endpoint moves the sweep the opposite way from the cursor.
		if dragging_start { -delta } else { delta }
	})
}

/// Reshape the arc by dragging one of its endpoints.
///
/// The sweep is held to a single turn and never runs backwards, and the start angle is kept inside
/// [-180°, 180°]. Both limits are reached by *continuing* a drag rather than ending it, so rather than
/// stopping at the limit the gesture re-anchors: dragging the start endpoint past a full sweep hands over
/// to the end endpoint and carries on from there, which is why the baseline is rewritten as it goes.
fn arc_sweep_drag(context: &GizmoContext, drag: &mut DragInput) -> DragWrites {
	use graphene_std::vector::generator_nodes::arc::*;

	let Some((_, current_start_angle, current_sweep_angle, _)) = extract_arc_parameters(Some(context.layer), context.document) else {
		return DragWrites::default();
	};
	let (Some(initial_start_angle), Some(initial_sweep_angle)) = (initial_f64(drag, StartAngleInput::INDEX), initial_f64(drag, SweepAngleInput::INDEX)) else {
		return DragWrites::default();
	};

	let angle_delta = drag.angle_delta;
	let angle = drag.total_angle;
	let dragging_start = drag.handle_index == 0;

	let write = |start: f64, sweep: f64| DragWrites::inputs(vec![(StartAngleInput.into(), TaggedValue::F64(start)), (SweepAngleInput.into(), TaggedValue::F64(sweep))]);

	if dragging_start {
		// The start endpoint drags the whole arc round, so the sweep closes by as much as the start opens.
		let sign = -angle.signum();
		let new_start_angle = initial_start_angle + angle;
		let new_sweep_angle = initial_sweep_angle + angle.abs() * sign;

		if new_sweep_angle > 360. {
			// Sweep closed all the way round: hand over to the end endpoint and continue from a full turn.
			let wrapped = new_sweep_angle % 360.;
			drag.total_angle = -wrapped;
			drag.handle_index = 1;
			set_initial(drag, SweepAngleInput::INDEX, 360.);
			set_initial(drag, StartAngleInput::INDEX, current_start_angle);

			return write(current_start_angle, 360. - wrapped);
		}
		if new_sweep_angle < 0. {
			// Sweep closed to nothing: hand over to the end endpoint and reopen from there.
			let rest_angle = angle_delta + new_sweep_angle;
			drag.total_angle = new_sweep_angle.abs();
			drag.handle_index = 1;
			set_initial(drag, SweepAngleInput::INDEX, 0.);
			set_initial(drag, StartAngleInput::INDEX, current_start_angle + rest_angle);

			return write(current_start_angle + rest_angle, new_sweep_angle.abs());
		}
		if new_start_angle > 180. {
			// Start angle ran off the top of its range: jump it to the bottom and shrink the sweep to match.
			let overflow = new_start_angle % 180.;
			let rest_angle = angle_delta - overflow;
			drag.total_angle = rest_angle;
			set_initial(drag, StartAngleInput::INDEX, -180.);
			set_initial(drag, SweepAngleInput::INDEX, current_sweep_angle - rest_angle);

			return write(-180. + overflow, current_sweep_angle - rest_angle - overflow);
		}
		if new_start_angle < -180. {
			// Same in the other direction: the start wraps to the top and the sweep grows to match.
			let underflow = new_start_angle % 180.;
			let rest_angle = angle_delta - underflow;
			drag.total_angle = underflow;
			set_initial(drag, StartAngleInput::INDEX, 180.);
			set_initial(drag, SweepAngleInput::INDEX, current_sweep_angle + rest_angle.abs());

			return write(180. + underflow, current_sweep_angle + rest_angle.abs() + underflow.abs());
		}

		let mut total = angle;
		if let Some(snapped) = arc_snap_delta(initial_sweep_angle + angle.abs() * sign, true) {
			total += snapped;
		}

		return write(initial_start_angle + total, initial_sweep_angle + total.abs() * sign);
	}

	// The end endpoint only opens or closes the sweep; the start stays put.
	let new_sweep_angle = initial_sweep_angle + angle;

	if new_sweep_angle < 0. {
		// Closed past nothing: hand back to the start endpoint, which reopens it the other way.
		let delta = angle_delta - current_sweep_angle;
		let sign = -delta.signum();
		drag.total_angle = delta;
		drag.handle_index = 0;
		set_initial(drag, SweepAngleInput::INDEX, 0.);

		return write(initial_start_angle + delta, delta.abs() * sign);
	}
	if new_sweep_angle > 360. {
		// Opened past a full turn: hand back to the start endpoint from a full sweep.
		let delta = angle_delta - (360. - new_sweep_angle);
		let sign = -delta.signum();
		drag.total_angle = delta;
		drag.handle_index = 0;
		set_initial(drag, SweepAngleInput::INDEX, 360.);

		return write(initial_start_angle + angle_delta, 360. + angle_delta.abs() * sign);
	}

	let mut total = angle;
	if let Some(snapped) = arc_snap_delta(initial_sweep_angle + angle, false) {
		total += snapped;
	}

	write(initial_start_angle, initial_sweep_angle + total)
}

/// Mark both endpoints at rest, highlight the one under the cursor, and while dragging show the sweep being
/// described: the arc between where the endpoint started and where it is now, labelled with its angle.
fn arc_sweep_overlay(context: &GizmoContext, overlay_context: &mut OverlayContext) {
	let Some((current_start, current_end)) = arc_end_points(Some(context.layer), context.document) else {
		return;
	};

	if context.state == GizmoState::Inactive {
		overlay_context.manipulator_handle(current_start, false, None);
		overlay_context.manipulator_handle(current_end, false, None);
		return;
	}

	let dragging_start = context.handle_index == 0;
	let (point, other_point) = if dragging_start { (current_start, current_end) } else { (current_end, current_start) };

	// The outline shows the whole arc responding, not just the endpoint being held.
	arc_outline(Some(context.layer), context.document, overlay_context);

	if context.state == GizmoState::Hover {
		overlay_context.manipulator_handle(point, true, None);
		overlay_context.manipulator_handle(other_point, false, None);
		return;
	}

	let viewport = context.document.metadata().transform_to_viewport(context.layer);
	let center = viewport.transform_point2(DVec2::ZERO);

	overlay_context.manipulator_handle(other_point, false, None);
	overlay_context.dashed_line(other_point, center, None, None, Some(5.), Some(5.), Some(0.5));

	// The sweep readout runs from the endpoint the user is not holding to the one they are.
	let tilt_offset = context.document.document_ptz.unmodified_tilt();
	let initial_vector = other_point - center;
	let final_vector = point - center;
	let offset_angle = initial_vector.to_angle() + tilt_offset;
	let angle = initial_vector.angle_to(final_vector).to_degrees();
	let display_angle = viewport.inverse().transform_point2(point).angle_to(viewport.inverse().transform_point2(other_point)).to_degrees();

	let text = format!("{}°", format_rounded(display_angle, 2));
	const FONT_SIZE: f64 = 12.;
	let transform = calculate_arc_text_transform(angle, offset_angle, center, text_width(&text, FONT_SIZE) / 2.);

	overlay_context.arc_sweep_angle(offset_angle, angle, point, point.distance(center), center, &text, transform);
}

/// Squared viewport distance within which an edge counts as grabbed, matching the hand-written gizmo.
const GRID_EDGE_THRESHOLD_SQUARED: f64 = 32.;

/// The two edges that control a grid's rows, in the order their handle indices refer to.
const GRID_ROW_EDGES: [RowColumnGizmoType; 2] = [RowColumnGizmoType::Top, RowColumnGizmoType::Bottom];
/// The two edges that control a grid's columns.
const GRID_COLUMN_EDGES: [RowColumnGizmoType; 2] = [RowColumnGizmoType::Left, RowColumnGizmoType::Right];

fn grid_row_distances(context: &GizmoContext) -> Vec<Option<f64>> {
	grid_edge_distances(context, GRID_ROW_EDGES)
}

fn grid_column_distances(context: &GizmoContext) -> Vec<Option<f64>> {
	grid_edge_distances(context, GRID_COLUMN_EDGES)
}

/// A grid's dimensions are grabbed anywhere along an edge, not at a point on it, so proximity is measured to
/// the edge line -- or to nothing at all, if the cursor is inside the band the edge occupies.
fn grid_edge_distances(context: &GizmoContext, edges: [RowColumnGizmoType; 2]) -> Vec<Option<f64>> {
	let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(context.layer, context.document) else {
		return vec![None, None];
	};
	let viewport = context.document.metadata().transform_to_viewport(context.layer);
	let mouse_point = dvec2_to_point(context.mouse_position);

	edges
		.into_iter()
		.map(|edge| {
			if edge.rect(grid_type, columns, rows, spacing, angles, viewport).contains(mouse_point) {
				return Some(0.);
			}
			let distance_squared = edge.line(grid_type, columns, rows, spacing, angles, viewport).nearest(mouse_point, 1e-6).distance_sq;

			(distance_squared < GRID_EDGE_THRESHOLD_SQUARED).then(|| distance_squared.sqrt())
		})
		.collect()
}

fn grid_edges(context: &GizmoContext) -> [RowColumnGizmoType; 2] {
	use graphene_std::vector::generator_nodes::grid;

	if context.parameter == ParameterRef::from(grid::ColumnsInput) {
		GRID_COLUMN_EDGES
	} else {
		GRID_ROW_EDGES
	}
}

/// Add or remove rows and columns by dragging an edge.
///
/// The grid also has to move as it resizes. Dragging the top edge upward adds rows, but the node builds its
/// grid downward from the origin, so without a matching translation the new rows would appear at the bottom
/// and the edge would slide out from under the cursor.
///
/// Dragging an edge past the last row or column does not stop at one: the grid turns inside out and the
/// opposite edge takes over, which is why the gesture re-anchors rather than clamping.
fn grid_edge_drag(context: &GizmoContext, drag: &mut DragInput) -> DragWrites {
	let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(context.layer, context.document) else {
		return DragWrites::default();
	};
	let viewport = context.document.metadata().transform_to_viewport(context.layer);
	let edge = grid_edges(context)[drag.handle_index.min(1)];

	let direction = edge.direction(viewport);
	let delta_vector = drag.mouse_position - drag.drag_start;
	let projection = delta_vector.project_onto(direction);
	let delta = viewport.inverse().transform_vector2(projection).length() * delta_vector.dot(direction).signum();

	if delta.abs() < 1e-6 {
		return DragWrites::default();
	}

	let initial_dimension = match initial_u32(drag, edge.parameter().input_index) {
		Some(dimension) => dimension as i32,
		None => return DragWrites::default(),
	};
	let dimensions_to_add = (delta / edge.spacing(spacing, grid_type, angles)).floor() as i32;
	let new_dimension = (initial_dimension + dimensions_to_add).max(1) as u32;
	let dimensions_delta = new_dimension as i32 - edge.initial_dimension(rows, columns) as i32;

	let mut writes = DragWrites {
		inputs: vec![(edge.parameter(), TaggedValue::U32(new_dimension))],
		transform: Some(grid_edge_transform(edge, dimensions_delta, spacing, grid_type, angles, viewport)),
	};

	// Dragged past the last row or column: flip to the opposite edge and start counting again from one.
	if initial_dimension + dimensions_to_add < 1 {
		drag.drag_start = drag.mouse_position;
		drag.handle_index = 1 - drag.handle_index.min(1);
		set_initial_u32(drag, edge.parameter().input_index, 1);
		writes.inputs = vec![(edge.parameter(), TaggedValue::U32(1))];
	}

	writes
}

/// Only the top and left edges move the layer: the grid is built rightward and downward from its origin, so
/// growing from the other two edges already puts the new cells where the cursor is.
fn grid_edge_transform(edge: RowColumnGizmoType, dimensions_delta: i32, spacing: DVec2, grid_type: GridType, angles: DVec2, viewport: DAffine2) -> DAffine2 {
	match edge {
		RowColumnGizmoType::Top => DAffine2::from_translation(edge.direction(viewport) * dimensions_delta as f64 * spacing.y),
		RowColumnGizmoType::Left => DAffine2::from_translation(edge.direction(viewport) * dimensions_delta as f64 * edge.spacing(spacing, grid_type, angles)),
		_ => DAffine2::IDENTITY,
	}
}

/// Mark the edge in play with a dashed line along it.
fn grid_edge_overlay(context: &GizmoContext, overlay_context: &mut OverlayContext) {
	if context.state == GizmoState::Inactive {
		return;
	}
	let Some((grid_type, spacing, columns, rows, angles)) = extract_grid_parameters(context.layer, context.document) else {
		return;
	};
	let viewport = context.document.metadata().transform_to_viewport(context.layer);
	let edge = grid_edges(context)[context.handle_index.min(1)];

	let (p0, p1) = get_line_endpoints(edge.line(grid_type, columns, rows, spacing, angles, viewport));
	overlay_context.dashed_line(p0, p1, None, None, Some(5.), Some(5.), Some(0.5));
}

fn initial_u32(drag: &DragInput, index: usize) -> Option<u32> {
	match drag.initial_parameters.get(index)? {
		Some(TaggedValue::U32(value)) => Some(*value),
		_ => None,
	}
}

fn set_initial_u32(drag: &mut DragInput, index: usize, value: u32) {
	if let Some(slot) = drag.initial_parameters.get_mut(index) {
		*slot = Some(TaggedValue::U32(value));
	}
}

/// A point on a circle of the given radius, at `theta` measured counterclockwise from +X.
fn circle_point(theta: f64, radius: f64) -> DVec2 {
	DVec2::new(radius * theta.cos(), -radius * theta.sin())
}

/// Half the width of the band around the circumference that counts as grabbing it. It widens with the
/// stroke, so a thick outline is still grabbable at its edge, and narrows for a circle that is small on
/// screen so the band cannot swallow the whole shape.
fn circular_grab_spacing(viewport: DAffine2, radius: f64, center: DVec2, stroke_width: f64) -> f64 {
	const SMALL_ON_SCREEN: f64 = 15.;

	let x_extent = viewport.transform_point2(circle_point(0., radius)).distance(center);
	let y_extent = viewport.transform_point2(circle_point(FRAC_PI_2, radius)).distance(center);
	let smallest = x_extent.min(y_extent);

	stroke_width + if smallest < SMALL_ON_SCREEN { 10. * (smallest / SMALL_ON_SCREEN) } else { 10. }
}

/// The radius this gizmo edits, whichever of the two shapes owns it.
fn circular_radius(context: &GizmoContext) -> Option<f64> {
	extract_circle_radius(context.layer, context.document).or_else(|| extract_arc_parameters(Some(context.layer), context.document).map(|(radius, ..)| radius))
}

/// How far the cursor is from the circumference, or `None` when it is not on it. Reporting the radial
/// distance rather than a flat "yes" lets an arc's endpoints still win the cursor where they overlap.
fn circular_radius_distances(context: &GizmoContext) -> Vec<Option<f64>> {
	let Some(radius) = circular_radius(context) else { return vec![None] };
	let radius = radius.abs();
	let viewport = context.document.metadata().transform_to_viewport(context.layer);
	let center = viewport.transform_point2(DVec2::ZERO);

	let angle = viewport.inverse().transform_point2(context.mouse_position).angle_to(DVec2::X);
	let on_circumference = viewport.transform_point2(circle_point(angle, radius));

	// Too small on screen to aim at.
	if on_circumference.distance(center) < GIZMO_HIDE_THRESHOLD {
		return vec![None];
	}

	let stroke_width = get_stroke_width(context.layer, &context.document.network_interface).unwrap_or(0.);
	let spacing = circular_grab_spacing(viewport, radius, center, stroke_width);
	let deviation = (context.mouse_position.distance(center) - on_circumference.distance(center)).abs();

	vec![(deviation <= spacing).then_some(deviation)]
}

/// The band itself, drawn as a pair of dashed ellipses once the radius is in play.
fn circular_radius_overlay(context: &GizmoContext, overlay_context: &mut OverlayContext) {
	if context.state == GizmoState::Inactive {
		return;
	}
	let Some(radius) = circular_radius(context) else { return };
	let viewport = context.document.metadata().transform_to_viewport(context.layer);
	let center = viewport.transform_point2(DVec2::ZERO);

	let x_point = viewport.transform_point2(circle_point(0., radius));
	let y_point = viewport.transform_point2(circle_point(FRAC_PI_2, radius));

	let Some(stroke_width) = get_stroke_width(context.layer, &context.document.network_interface) else {
		overlay_context.dashed_ellipse(
			center,
			x_point.distance(center),
			y_point.distance(center),
			None,
			None,
			None,
			None,
			None,
			None,
			Some(4.),
			Some(4.),
			Some(0.5),
		);
		return;
	};

	let spacing = circular_grab_spacing(viewport, radius, center, stroke_width);
	let direction_x = viewport.transform_vector2(DVec2::X);
	let direction_y = viewport.transform_vector2(-DVec2::Y);

	for sign in [-1., 1.] {
		let x_radius = (x_point + direction_x * spacing * sign).distance(center);
		let y_radius = (y_point + direction_y * spacing * sign).distance(center);
		overlay_context.dashed_ellipse(center, x_radius, y_radius, None, None, None, None, None, None, Some(4.), Some(4.), Some(0.5));
	}
}

/// A regular polygon's radius reaches every corner equally, so every corner is a grab point -- the same
/// arrangement as the star's, with one vertex per side rather than alternating between two radii.
fn polygon_radius_handles(context: &GizmoContext, value: f64) -> Vec<DVec2> {
	let Some((sides, _)) = extract_polygon_parameters(Some(context.layer), context.document) else {
		return Vec::new();
	};

	(0..sides)
		.map(|vertex| {
			let angle = ((vertex as f64) * TAU) / (sides as f64);
			DVec2::new(value * angle.sin(), -value * angle.cos())
		})
		.collect()
}

/// Mark every corner at rest, and show the outline being resized once one is held.
fn polygon_radius_overlay(context: &GizmoContext, overlay_context: &mut OverlayContext) {
	let Some((sides, radius)) = extract_polygon_parameters(Some(context.layer), context.document) else {
		return;
	};
	let viewport = context.document.metadata().transform_to_viewport(context.layer);
	let center = viewport.transform_point2(DVec2::ZERO);

	if context.state == GizmoState::Inactive {
		for vertex in 0..sides {
			let point = polygon_vertex_position(viewport, vertex as i32, sides, radius);

			// Once the polygon is this small the corners crowd its centre and cannot be told apart.
			if point.distance(center) < GIZMO_HIDE_THRESHOLD {
				return;
			}
			overlay_context.manipulator_handle(point, false, None);
		}
		return;
	}

	polygon_outline(Some(context.layer), context.document, overlay_context);
}
