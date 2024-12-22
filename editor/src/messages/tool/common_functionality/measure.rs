use crate::consts::COLOR_OVERLAY_BLUE;
use crate::messages::portfolio::document::overlays::utility_types::{OverlayContext, Pivot};
use crate::messages::tool::tool_messages::tool_prelude::*;

use graphene_std::renderer::Rect;

fn draw_dashed_line(line_start: DVec2, line_end: DVec2, transform: DAffine2, overlay_context: &mut OverlayContext) {
	let min_viewport = transform.transform_point2(line_start);
	let max_viewport = transform.transform_point2(line_end);

	overlay_context.dashed_line(min_viewport, max_viewport, None, Some(2.), Some(3.));
}

fn draw_line_with_length(line_start: DVec2, line_end: DVec2, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let transform_to_document = document_to_viewport.inverse() * transform;
	let min_viewport = transform.transform_point2(line_start);
	let max_viewport = transform.transform_point2(line_end);

	overlay_context.line(min_viewport, max_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(line_end - line_start).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_viewport + max_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
}

fn does_overlap_y(selected_bounds: Rect, hovered_bounds: Rect) -> bool {
	selected_bounds.min().x < hovered_bounds.max().x && selected_bounds.max().x > hovered_bounds.min().x
}
fn does_overlap_x(selected_bounds: Rect, hovered_bounds: Rect) -> bool {
	selected_bounds.min().y < hovered_bounds.max().y && selected_bounds.max().y > hovered_bounds.min().y
}

/// Draws measurements between `selected_bounds` and `hovered_bounds` when both X and Y axes are involved.
fn draw_zero_axis_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// Determine if the selected object is on the right or left of the hovered object.
	let selected_on_right = selected_bounds.min().x > hovered_bounds.max().x;
	// Determine if the selected object is on the bottom or top of the hovered object.
	let selected_on_bottom = selected_bounds.min().y > hovered_bounds.max().y;

	// Horizontal lines
	let selected_y = if selected_on_bottom { selected_bounds.min().y } else { selected_bounds.max().y };
	let hovered_y = if selected_on_bottom { hovered_bounds.max().y } else { hovered_bounds.min().y };
	let selected_x = if selected_on_right { selected_bounds.min().x } else { selected_bounds.max().x };
	let hovered_x = if selected_on_right { hovered_bounds.max().x } else { hovered_bounds.min().x };

	let line_start = DVec2::new(selected_x, selected_y);
	let line_end = DVec2::new(hovered_x, selected_y);
	draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context);

	let line_start = DVec2::new(selected_x, hovered_y);
	let line_end = DVec2::new(hovered_x, hovered_y);
	draw_dashed_line(line_start, line_end, transform, overlay_context);

	let line_start = DVec2::new(selected_x, selected_y);
	let line_end = DVec2::new(selected_x, hovered_y);
	draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context);

	let line_start = DVec2::new(hovered_x, selected_y);
	let line_end = DVec2::new(hovered_x, hovered_y);

	draw_dashed_line(line_start, line_end, transform, overlay_context);
}

/// Draws measurements between `selected_bounds` and `hovered_bounds` when only one axis is involved at a time.
fn draw_single_axis_zero_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// Check for overlaps on both X and Y axes
	let overlap_y = does_overlap_y(selected_bounds, hovered_bounds);
	let overlap_x = does_overlap_x(selected_bounds, hovered_bounds);

	if overlap_y {
		let selected_facing_edge = if hovered_bounds.max().y < selected_bounds.min().y {
			selected_bounds.min().y
		} else {
			selected_bounds.max().y
		};
		let hovered_facing_edge = if hovered_bounds.max().y < selected_bounds.min().y {
			hovered_bounds.max().y
		} else {
			hovered_bounds.min().y
		};
		let vertical_line_start_x = if hovered_bounds.max().x > selected_bounds.max().x {
			selected_bounds.max().x
		} else {
			selected_bounds.min().x
		};
		let dashed_vertical_line_start_x = if hovered_bounds.max().x > selected_bounds.max().x {
			hovered_bounds.min().x
		} else {
			hovered_bounds.max().x
		};

		let line_start = DVec2::new(f64::min(hovered_bounds.max().x, selected_bounds.max().x), selected_facing_edge);
		let line_end = DVec2::new(f64::max(hovered_bounds.min().x, selected_bounds.min().x), selected_facing_edge);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context);

		// Vertical line showing distance between the intersecting parts
		let line_start = DVec2::new(vertical_line_start_x, selected_facing_edge);
		let line_end = DVec2::new(vertical_line_start_x, hovered_facing_edge);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context);

		let dashed_line_start = DVec2::new(dashed_vertical_line_start_x, selected_facing_edge);
		let dashed_line_end = DVec2::new(dashed_vertical_line_start_x, hovered_facing_edge);
		draw_dashed_line(dashed_line_start, dashed_line_end, transform, overlay_context);
	} else if overlap_x {
		let selected_facing_edge = if hovered_bounds.max().x < selected_bounds.min().x {
			selected_bounds.min().x
		} else {
			selected_bounds.max().x
		};
		let hovered_facing_edge = if hovered_bounds.max().x < selected_bounds.min().x {
			hovered_bounds.max().x
		} else {
			hovered_bounds.min().x
		};
		let horizontal_line_start_y = if hovered_bounds.max().y > selected_bounds.max().y {
			selected_bounds.max().y
		} else {
			selected_bounds.min().y
		};
		let dashed_horizontal_line_start_y = if hovered_bounds.max().y > selected_bounds.max().y {
			hovered_bounds.min().y
		} else {
			hovered_bounds.max().y
		};

		let line_start = DVec2::new(selected_facing_edge, f64::min(hovered_bounds.max().y, selected_bounds.max().y));
		let line_end = DVec2::new(selected_facing_edge, f64::max(hovered_bounds.min().y, selected_bounds.min().y));
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context);

		// Horizontal line showing distance between the intersecting parts
		let line_start = DVec2::new(selected_facing_edge, horizontal_line_start_y);
		let line_end = DVec2::new(hovered_facing_edge, horizontal_line_start_y);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context);

		let dashed_line_start = DVec2::new(selected_facing_edge, dashed_horizontal_line_start_y);
		let dashed_line_end = DVec2::new(hovered_facing_edge, dashed_horizontal_line_start_y);
		draw_dashed_line(dashed_line_start, dashed_line_end, transform, overlay_context);
	}
}

/// Draws measurements when only one axis is involved at a time for overlaps between two bounding boxes.
fn draw_single_axis_one_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let selected_center = selected_bounds.center();
	let hovered_center = hovered_bounds.center();

	// Check for overlaps on both the X and Y axes
	let overlap_y = does_overlap_y(selected_bounds, hovered_bounds);
	let overlap_x = does_overlap_x(selected_bounds, hovered_bounds);

	if overlap_y {
		let selected_facing_edge = if hovered_bounds.max().y < selected_bounds.min().y {
			selected_bounds.min().y
		} else {
			selected_bounds.max().y
		};
		let hovered_facing_edge = if hovered_bounds.max().y < selected_bounds.min().y {
			hovered_bounds.max().y
		} else {
			hovered_bounds.min().y
		};
		let vertical_line_start = if selected_center.x < hovered_bounds.max().x && selected_center.x > hovered_bounds.min().x {
			selected_center.x
		} else {
			hovered_center.x
		};

		let line_start = DVec2::new(vertical_line_start, selected_facing_edge);
		let line_end = DVec2::new(vertical_line_start, hovered_facing_edge);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context);
	} else if overlap_x {
		let selected_facing_edge = if hovered_bounds.max().x < selected_bounds.min().x {
			selected_bounds.min().x
		} else {
			selected_bounds.max().x
		};
		let hovered_facing_edge = if hovered_bounds.max().x < selected_bounds.min().x {
			hovered_bounds.max().x
		} else {
			hovered_bounds.min().x
		};
		let horizontal_line_start_y = if selected_center.y < hovered_bounds.max().y && selected_center.y > hovered_bounds.min().y {
			selected_center.y
		} else {
			hovered_center.y
		};

		let line_start = DVec2::new(selected_facing_edge, horizontal_line_start_y);
		let line_end = DVec2::new(hovered_facing_edge, horizontal_line_start_y);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context);
	}
}

/// Draws measurements for partial overlaps where lines cross on both the X and Y axes, handling two diagonal cases.
fn draw_two_axis_one_one_crossing(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// Handle intersections at top-left & bottom-right, or top-right & bottom-left
	let mut top_y_bound = f64::min(selected_bounds.min().y, hovered_bounds.min().y);
	let mut bottom_y_bound = f64::max(selected_bounds.max().y, hovered_bounds.max().y);
	let mut top_x_bound = f64::max(selected_bounds.max().x, hovered_bounds.max().x);
	let mut bottom_x_bound = f64::min(selected_bounds.min().x, hovered_bounds.min().x);
	if (hovered_bounds.center().x > selected_bounds.center().x && hovered_bounds.center().y < selected_bounds.center().y)
		|| (hovered_bounds.center().x < selected_bounds.center().x && hovered_bounds.center().y > selected_bounds.center().y)
	{
		// Switch horizontal lines Y values for top-right and bottom-left intersection
		std::mem::swap(&mut top_y_bound, &mut bottom_y_bound);
		std::mem::swap(&mut top_x_bound, &mut bottom_x_bound);
	}

	// Horizontal lines
	let top_x_start = DVec2::new(f64::min(selected_bounds.max().x, hovered_bounds.max().x), top_y_bound);
	let top_x_end = DVec2::new(f64::max(selected_bounds.max().x, hovered_bounds.max().x), top_y_bound);
	draw_line_with_length(top_x_start, top_x_end, transform, document_to_viewport, overlay_context);

	let bottom_x_start = DVec2::new(f64::min(selected_bounds.min().x, hovered_bounds.min().x), bottom_y_bound);
	let bottom_x_end = DVec2::new(f64::max(selected_bounds.min().x, hovered_bounds.min().x), bottom_y_bound);
	draw_line_with_length(bottom_x_start, bottom_x_end, transform, document_to_viewport, overlay_context);

	// Vertical lines
	let top_y_start = DVec2::new(top_x_bound, f64::min(selected_bounds.min().y, hovered_bounds.min().y));
	let top_y_end = DVec2::new(top_x_bound, f64::max(selected_bounds.min().y, hovered_bounds.min().y));
	draw_line_with_length(top_y_start, top_y_end, transform, document_to_viewport, overlay_context);

	let bottom_y_start = DVec2::new(bottom_x_bound, f64::min(selected_bounds.max().y, hovered_bounds.max().y));
	let bottom_y_end = DVec2::new(bottom_x_bound, f64::max(selected_bounds.max().y, hovered_bounds.max().y));
	draw_line_with_length(bottom_y_start, bottom_y_end, transform, document_to_viewport, overlay_context);
}

/// Draws measurements where the selected bounding box has two vertical edges that cross the hovered box (or vice versa).
fn draw_two_axis_one_one_two_zero_crossing(
	selected_bounds: Rect,
	hovered_bounds: Rect,
	transform: DAffine2,
	document_to_viewport: DAffine2,
	overlay_context: &mut OverlayContext,
	two_vertical_edge_intersect: bool,
) {
	if two_vertical_edge_intersect {
		let selected_bound_edge = if selected_bounds.center().y >= hovered_bounds.center().y {
			selected_bounds.max().y
		} else {
			selected_bounds.min().y
		};
		let hovered_bound_edge = if selected_bounds.center().y >= hovered_bounds.center().y {
			hovered_bounds.max().y
		} else {
			hovered_bounds.min().y
		};

		// Vertical lines
		let y_start_left = DVec2::new(hovered_bounds.min().x, f64::min(selected_bound_edge, hovered_bound_edge));
		let y_end_left = DVec2::new(hovered_bounds.min().x, f64::max(selected_bound_edge, hovered_bound_edge));
		draw_line_with_length(y_start_left, y_end_left, transform, document_to_viewport, overlay_context);

		let y_start_right = DVec2::new(hovered_bounds.max().x, f64::min(selected_bound_edge, hovered_bound_edge));
		let y_end_right = DVec2::new(hovered_bounds.max().x, f64::max(selected_bound_edge, hovered_bound_edge));
		draw_line_with_length(y_start_right, y_end_right, transform, document_to_viewport, overlay_context);

		// Horizontal lines
		let horizontal_line_y_bound = if selected_bounds.center().y >= hovered_bounds.center().y {
			f64::max(selected_bound_edge, hovered_bound_edge)
		} else {
			f64::min(selected_bound_edge, hovered_bound_edge)
		};

		let x_start_left = DVec2::new(hovered_bounds.min().x, horizontal_line_y_bound);
		let x_end_left = DVec2::new(selected_bounds.min().x, horizontal_line_y_bound);
		draw_line_with_length(x_start_left, x_end_left, transform, document_to_viewport, overlay_context);

		let x_start_right = DVec2::new(hovered_bounds.max().x, horizontal_line_y_bound);
		let x_end_right = DVec2::new(selected_bounds.max().x, horizontal_line_y_bound);
		draw_line_with_length(x_start_right, x_end_right, transform, document_to_viewport, overlay_context);
	} else {
		// Horizontal intersections
		let selected_bound_edge = if selected_bounds.center().x >= hovered_bounds.center().x {
			selected_bounds.max().x
		} else {
			selected_bounds.min().x
		};
		let hovered_bound_edge = if selected_bounds.center().x >= hovered_bounds.center().x {
			hovered_bounds.max().x
		} else {
			hovered_bounds.min().x
		};

		// Outermost X position
		let vertical_line_x = if selected_bounds.center().x >= hovered_bounds.center().x {
			f64::max(selected_bound_edge, hovered_bound_edge)
		} else {
			f64::min(selected_bound_edge, hovered_bound_edge)
		};

		let y_start_up = DVec2::new(vertical_line_x, selected_bounds.min().y);
		let y_end_up = DVec2::new(vertical_line_x, hovered_bounds.min().y);
		draw_line_with_length(y_start_up, y_end_up, transform, document_to_viewport, overlay_context);

		let y_start_down = DVec2::new(vertical_line_x, selected_bounds.max().y);
		let y_end_down = DVec2::new(vertical_line_x, hovered_bounds.max().y);
		draw_line_with_length(y_start_down, y_end_down, transform, document_to_viewport, overlay_context);

		// Horizontal lines
		let horizontal_line_inner_x = if selected_bounds.center().x >= hovered_bounds.center().x {
			f64::min(selected_bound_edge, hovered_bound_edge)
		} else {
			f64::max(selected_bound_edge, hovered_bound_edge)
		};
		let x_start_up = DVec2::new(vertical_line_x, f64::min(selected_bounds.min().y, hovered_bounds.min().y));
		let x_end_up = DVec2::new(horizontal_line_inner_x, f64::min(selected_bounds.min().y, hovered_bounds.min().y));
		draw_line_with_length(x_start_up, x_end_up, transform, document_to_viewport, overlay_context);

		let x_start_down = DVec2::new(vertical_line_x, f64::max(selected_bounds.max().y, hovered_bounds.max().y));
		let x_end_down = DVec2::new(horizontal_line_inner_x, f64::max(selected_bounds.max().y, hovered_bounds.max().y));
		draw_line_with_length(x_start_down, x_end_down, transform, document_to_viewport, overlay_context);
	}
}

fn draw_two_axis_two_zero_zero_two(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// Vertical lines
	let y_start_left_top = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let y_end_left_top = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::max(hovered_bounds.min().y, selected_bounds.min().y));
	draw_line_with_length(y_start_left_top, y_end_left_top, transform, document_to_viewport, overlay_context);

	let y_start_left_bottom = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::min(hovered_bounds.max().y, selected_bounds.max().y));
	let y_end_left_bottom = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	draw_line_with_length(y_start_left_bottom, y_end_left_bottom, transform, document_to_viewport, overlay_context);

	let y_start_right_top = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let y_end_right_top = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::max(hovered_bounds.min().y, selected_bounds.min().y));
	draw_line_with_length(y_start_right_top, y_end_right_top, transform, document_to_viewport, overlay_context);

	let y_start_right_bottom = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::min(hovered_bounds.max().y, selected_bounds.max().y));
	let y_end_right_bottom = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	draw_line_with_length(y_start_right_bottom, y_end_right_bottom, transform, document_to_viewport, overlay_context);

	// Horizontal lines
	let x_start_left_top = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let x_end_left_top = DVec2::new(f64::max(hovered_bounds.min().x, selected_bounds.min().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	draw_line_with_length(x_start_left_top, x_end_left_top, transform, document_to_viewport, overlay_context);

	let x_start_right_top = DVec2::new(f64::min(hovered_bounds.max().x, selected_bounds.max().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let x_end_right_top = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	draw_line_with_length(x_start_right_top, x_end_right_top, transform, document_to_viewport, overlay_context);

	let x_start_left_bottom = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	let x_end_left_bottom = DVec2::new(f64::max(hovered_bounds.min().x, selected_bounds.min().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	draw_line_with_length(x_start_left_bottom, x_end_left_bottom, transform, document_to_viewport, overlay_context);

	let x_start_right_bottom = DVec2::new(f64::min(hovered_bounds.max().x, selected_bounds.max().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	let x_end_right_bottom = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	draw_line_with_length(x_start_right_bottom, x_end_right_bottom, transform, document_to_viewport, overlay_context);
}

fn draw_two_axis_two_zero_two_zero(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// Horizontal lines
	let x_start_left = DVec2::new(f64::max(hovered_bounds.min().x, selected_bounds.min().x), selected_bounds.center().y);
	let x_end_left = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), selected_bounds.center().y);
	draw_line_with_length(x_start_left, x_end_left, transform, document_to_viewport, overlay_context);

	let x_start_right = DVec2::new(f64::min(hovered_bounds.max().x, selected_bounds.max().x), selected_bounds.center().y);
	let x_end_right = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), selected_bounds.center().y);
	draw_line_with_length(x_start_right, x_end_right, transform, document_to_viewport, overlay_context);

	// Vertical lines
	let y_start_top = DVec2::new(selected_bounds.center().x, f64::max(hovered_bounds.min().y, selected_bounds.min().y));
	let y_end_top = DVec2::new(selected_bounds.center().x, f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	draw_line_with_length(y_start_top, y_end_top, transform, document_to_viewport, overlay_context);

	let y_start_bottom = DVec2::new(selected_bounds.center().x, f64::min(hovered_bounds.max().y, selected_bounds.max().y));
	let y_end_bottom = DVec2::new(selected_bounds.center().x, f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	draw_line_with_length(y_start_bottom, y_end_bottom, transform, document_to_viewport, overlay_context);
}

fn handle_two_axis_overlap(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// X-axis edge crossings
	let selected_x_crosses = (selected_bounds.min().y >= hovered_bounds.min().y && selected_bounds.min().y <= hovered_bounds.max().y) as u8
		+ (selected_bounds.max().y >= hovered_bounds.min().y && selected_bounds.max().y <= hovered_bounds.max().y) as u8;
	let hovered_x_crosses = (hovered_bounds.min().y >= selected_bounds.min().y && hovered_bounds.min().y <= selected_bounds.max().y) as u8
		+ (hovered_bounds.max().y >= selected_bounds.min().y && hovered_bounds.max().y <= selected_bounds.max().y) as u8;

	// Y-axis edge crossings
	let selected_y_crosses = (selected_bounds.min().x >= hovered_bounds.min().x && selected_bounds.min().x <= hovered_bounds.max().x) as u8
		+ (selected_bounds.max().x >= hovered_bounds.min().x && selected_bounds.max().x <= hovered_bounds.max().x) as u8;
	let hovered_y_crosses = (hovered_bounds.min().x >= selected_bounds.min().x && hovered_bounds.min().x <= selected_bounds.max().x) as u8
		+ (hovered_bounds.max().x >= selected_bounds.min().x && hovered_bounds.max().x <= selected_bounds.max().x) as u8;

	// Identify the case based on edge crossings along each axis
	match ((selected_x_crosses, hovered_x_crosses), (selected_y_crosses, hovered_y_crosses)) {
		((1, 1), (1, 1)) => draw_two_axis_one_one_crossing(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		((1, 1), (2, 0)) => draw_two_axis_one_one_two_zero_crossing(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context, true),
		((1, 1), (0, 2)) => draw_two_axis_one_one_two_zero_crossing(hovered_bounds, selected_bounds, transform, document_to_viewport, overlay_context, true),
		((2, 0), (1, 1)) => draw_two_axis_one_one_two_zero_crossing(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context, false),
		((0, 2), (1, 1)) => draw_two_axis_one_one_two_zero_crossing(hovered_bounds, selected_bounds, transform, document_to_viewport, overlay_context, false),
		((2, 0), (0, 2)) => draw_two_axis_two_zero_zero_two(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		((0, 2), (2, 0)) => draw_two_axis_two_zero_zero_two(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		((2, 0), (2, 0)) => draw_two_axis_two_zero_two_zero(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		((0, 2), (0, 2)) => draw_two_axis_two_zero_two_zero(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		_ => (),
	}
}

pub fn overlay(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// TODO: For all cases, apply object rotation to bounds before drawing lines.

	// Check axis overlaps
	let overlap_x = selected_bounds.min().x <= hovered_bounds.max().x && selected_bounds.max().x >= hovered_bounds.min().x;
	let overlap_y = selected_bounds.min().y <= hovered_bounds.max().y && selected_bounds.max().y >= hovered_bounds.min().y;
	let overlap_axes = match (overlap_x, overlap_y) {
		(true, true) => 2,
		(true, false) | (false, true) => 1,
		_ => 0,
	};

	// Check centerline crossings
	let center_x_intersects = (selected_bounds.center().y >= hovered_bounds.min().y && selected_bounds.center().y <= hovered_bounds.max().y)
		|| (hovered_bounds.center().y >= selected_bounds.min().y && hovered_bounds.center().y <= selected_bounds.max().y);
	let center_y_intersects = (selected_bounds.center().x >= hovered_bounds.min().x && selected_bounds.center().x <= hovered_bounds.max().x)
		|| (hovered_bounds.center().x >= selected_bounds.min().x && hovered_bounds.center().x <= selected_bounds.max().x);
	let centerline_crosses = match (center_x_intersects, center_y_intersects) {
		(true, true) => 2,
		(true, false) | (false, true) => 1,
		_ => 0,
	};

	// Handle each case based on overlap axes and centerline crossings
	match (overlap_axes, centerline_crosses) {
		(0, _) => draw_zero_axis_crossings(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),

		(1, 0) => draw_single_axis_zero_crossings(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		(1, 1) => draw_single_axis_one_crossings(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		(1, 2) => draw_single_axis_one_crossings(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		(2, _) => handle_two_axis_overlap(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),

		// Fallback case, should not typically happen.
		_ => (),
	}
}
