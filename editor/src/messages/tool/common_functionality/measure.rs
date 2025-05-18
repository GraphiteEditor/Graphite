use crate::consts::COLOR_OVERLAY_BLUE;
use crate::messages::portfolio::document::overlays::utility_types::{OverlayContext, Pivot};
use crate::messages::tool::tool_messages::tool_prelude::*;
use graphene_std::renderer::Rect;

/// Draws a dashed line between two points transformed by the given affine transformation.
fn draw_dashed_line(line_start: DVec2, line_end: DVec2, transform: DAffine2, overlay_context: &mut OverlayContext) {
	let min_viewport = transform.transform_point2(line_start);
	let max_viewport = transform.transform_point2(line_end);

	overlay_context.dashed_line(min_viewport, max_viewport, None, None, Some(2.), Some(2.), Some(0.5));
}

/// Draws a solid line with a length annotation between two points transformed by the given affine transformations.
fn draw_line_with_length(line_start: DVec2, line_end: DVec2, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext, label_alignment: LabelAlignment) {
	let transform_to_document = document_to_viewport.inverse() * transform;
	let min_viewport = transform.transform_point2(line_start);
	let max_viewport = transform.transform_point2(line_end);

	overlay_context.line(min_viewport, max_viewport, None, None);

	// Remove trailing zeros from the formatted string
	let length = format!("{:.2}", transform_to_document.transform_vector2(line_end - line_start).length())
		.trim_end_matches('0')
		.trim_end_matches('.')
		.to_string();

	const TOLERANCE: f64 = 0.01;
	if transform_to_document.transform_vector2(line_end - line_start).length() >= TOLERANCE {
		const TEXT_PADDING: f64 = 5.;
		// Calculate midpoint of the line
		let midpoint = (min_viewport + max_viewport) / 2.;

		// Adjust text position based on line orientation and flags
		// Determine text position based on line orientation and flags
		let (pivot_x, pivot_y) = match (label_alignment.is_vertical_line, label_alignment.text_on_left, label_alignment.text_on_top) {
			(true, true, _) => (Pivot::End, Pivot::Middle),     // Vertical line, text on the left
			(true, false, _) => (Pivot::Start, Pivot::Middle),  // Vertical line, text on the right
			(false, _, true) => (Pivot::Middle, Pivot::End),    // Horizontal line, text on top
			(false, _, false) => (Pivot::Middle, Pivot::Start), // Horizontal line, text on bottom
		};
		overlay_context.text(&length, COLOR_OVERLAY_BLUE, None, DAffine2::from_translation(midpoint), TEXT_PADDING, [pivot_x, pivot_y]);
	}
}

/// Draws a dashed outline around a rectangle to visualize the AABB
fn draw_dashed_rect_outline(rect: Rect, transform: DAffine2, overlay_context: &mut OverlayContext) {
	let min = rect.min();
	let max = rect.max();

	// Create the four corners of the rectangle
	let top_left = transform.transform_point2(DVec2::new(min.x, min.y));
	let top_right = transform.transform_point2(DVec2::new(max.x, min.y));
	let bottom_right = transform.transform_point2(DVec2::new(max.x, max.y));
	let bottom_left = transform.transform_point2(DVec2::new(min.x, max.y));

	// Draw the four sides as dashed lines
	draw_dashed_line(top_left, top_right, transform, overlay_context);
	draw_dashed_line(top_right, bottom_right, transform, overlay_context);
	draw_dashed_line(bottom_right, bottom_left, transform, overlay_context);
	draw_dashed_line(bottom_left, top_left, transform, overlay_context);
}

/// Checks if the selected bounds overlap with the hovered bounds on the Y-axis.
fn does_overlap_y(selected_bounds: Rect, hovered_bounds: Rect) -> bool {
	selected_bounds.min().x <= hovered_bounds.max().x && selected_bounds.max().x >= hovered_bounds.min().x
}

/// Checks if the selected bounds overlap with the hovered bounds on the X-axis.
fn does_overlap_x(selected_bounds: Rect, hovered_bounds: Rect) -> bool {
	selected_bounds.min().y <= hovered_bounds.max().y && selected_bounds.max().y >= hovered_bounds.min().y
}

/// Draws measurements when both X and Y axes are involved in the overlap between selected and hovered bounds.
fn draw_zero_axis_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let (selected_min, selected_max) = (selected_bounds.min(), selected_bounds.max());
	let (hovered_min, hovered_max) = (hovered_bounds.min(), hovered_bounds.max());

	let selected_on_right = selected_min.x > hovered_max.x;
	let selected_on_bottom = selected_min.y > hovered_max.y;

	let selected_y = if selected_on_bottom { selected_min.y } else { selected_max.y };
	let hovered_y = if selected_on_bottom { hovered_max.y } else { hovered_min.y };
	let selected_x = if selected_on_right { selected_min.x } else { selected_max.x };
	let hovered_x = if selected_on_right { hovered_max.x } else { hovered_min.x };

	// Draw horizontal solid line with length
	let line_start = DVec2::new(selected_x, selected_y);
	let line_end = DVec2::new(hovered_x, selected_y);
	let label_alignment = LabelAlignment::new(false, false, !selected_on_bottom);
	draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context, label_alignment);

	// Draw horizontal dashed line
	let line_start = DVec2::new(selected_x, hovered_y);
	let line_end = DVec2::new(hovered_x, hovered_y);
	draw_dashed_line(line_start, line_end, transform, overlay_context);

	// Draw vertical solid line with length
	let line_start = DVec2::new(selected_x, selected_y);
	let line_end = DVec2::new(selected_x, hovered_y);
	let label_alignment = LabelAlignment::new(true, !selected_on_right, false);
	draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context, label_alignment);

	// Draw vertical dashed line
	let line_start = DVec2::new(hovered_x, selected_y);
	let line_end = DVec2::new(hovered_x, hovered_y);
	draw_dashed_line(line_start, line_end, transform, overlay_context);
}

/// Draws measurements when only one axis is involved in the overlap between selected and hovered bounds.
fn draw_single_axis_zero_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let (selected_min, selected_max) = (selected_bounds.min(), selected_bounds.max());
	let (hovered_min, hovered_max) = (hovered_bounds.min(), hovered_bounds.max());

	let overlap_y = does_overlap_y(selected_bounds, hovered_bounds) || does_overlap_y(hovered_bounds, selected_bounds);
	let overlap_x = does_overlap_x(selected_bounds, hovered_bounds) || does_overlap_x(hovered_bounds, selected_bounds);

	let selected_on_bottom = selected_bounds.center().y > hovered_bounds.center().y;
	let selected_on_right = selected_bounds.center().x > hovered_bounds.center().x;
	if overlap_y {
		let selected_facing_edge = if hovered_max.y < selected_min.y { selected_min.y } else { selected_max.y };
		let hovered_facing_edge = if hovered_max.y < selected_min.y { hovered_max.y } else { hovered_min.y };
		let vertical_line_start_x = if hovered_max.x > selected_max.x { selected_max.x } else { selected_min.x };
		let dashed_vertical_line_start_x = if hovered_max.x > selected_max.x { hovered_min.x } else { hovered_max.x };

		// Draw horizontal solid line with length
		let line_start = DVec2::new(f64::min(hovered_max.x, selected_max.x), selected_facing_edge);
		let line_end = DVec2::new(f64::max(hovered_min.x, selected_min.x), selected_facing_edge);
		let label_alignment = LabelAlignment::new(false, false, selected_on_bottom);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context, label_alignment);

		// Draw vertical solid line with length
		let line_start = DVec2::new(vertical_line_start_x, selected_facing_edge);
		let line_end = DVec2::new(vertical_line_start_x, hovered_facing_edge);
		let label_alignment = LabelAlignment::new(true, !selected_on_right, false);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context, label_alignment);

		// Draw vertical dashed line
		let dashed_line_start = DVec2::new(dashed_vertical_line_start_x, selected_facing_edge);
		let dashed_line_end = DVec2::new(dashed_vertical_line_start_x, hovered_facing_edge);
		draw_dashed_line(dashed_line_start, dashed_line_end, transform, overlay_context);
	} else if overlap_x {
		let selected_facing_edge = if hovered_max.x < selected_min.x { selected_min.x } else { selected_max.x };
		let hovered_facing_edge = if hovered_max.x < selected_min.x { hovered_max.x } else { hovered_min.x };
		let horizontal_line_start_y = if hovered_max.y > selected_max.y { selected_max.y } else { selected_min.y };
		let dashed_horizontal_line_start_y = if hovered_max.y > selected_max.y { hovered_min.y } else { hovered_max.y };

		// Draw vertical solid line with length
		let line_start = DVec2::new(selected_facing_edge, f64::min(hovered_max.y, selected_max.y));
		let line_end = DVec2::new(selected_facing_edge, f64::max(hovered_min.y, selected_min.y));
		let label_alignment = LabelAlignment::new(true, selected_on_right, false);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context, label_alignment);

		// Draw horizontal solid line with length
		let line_start = DVec2::new(selected_facing_edge, horizontal_line_start_y);
		let line_end = DVec2::new(hovered_facing_edge, horizontal_line_start_y);
		let label_alignment = LabelAlignment::new(false, false, !selected_on_bottom);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context, label_alignment);

		// Draw horizontal dashed line
		let dashed_line_start = DVec2::new(selected_facing_edge, dashed_horizontal_line_start_y);
		let dashed_line_end = DVec2::new(hovered_facing_edge, dashed_horizontal_line_start_y);
		draw_dashed_line(dashed_line_start, dashed_line_end, transform, overlay_context);
	}
}

/// Draws measurements when only one axis is involved and there is one crossing between selected and hovered bounds.
fn draw_single_axis_one_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let (selected_min, selected_max) = (selected_bounds.min(), selected_bounds.max());
	let (hovered_min, hovered_max) = (hovered_bounds.min(), hovered_bounds.max());

	let selected_center = selected_bounds.center();
	let hovered_center = hovered_bounds.center();

	let overlap_y = does_overlap_y(selected_bounds, hovered_bounds) || does_overlap_y(hovered_bounds, selected_bounds);
	let overlap_x = does_overlap_x(selected_bounds, hovered_bounds) || does_overlap_x(hovered_bounds, selected_bounds);

	if overlap_y {
		let selected_facing_edge = if hovered_max.y < selected_min.y { selected_min.y } else { selected_max.y };
		let hovered_facing_edge = if hovered_max.y < selected_min.y { hovered_max.y } else { hovered_min.y };
		let vertical_line_start = if selected_center.x < hovered_max.x && selected_center.x > hovered_min.x {
			selected_center.x
		} else {
			hovered_center.x
		};

		// Draw vertical solid line with length
		let line_start = DVec2::new(vertical_line_start, selected_facing_edge);
		let line_end = DVec2::new(vertical_line_start, hovered_facing_edge);
		let label_alignment = LabelAlignment::new(true, true, false);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context, label_alignment);
	} else if overlap_x {
		let selected_facing_edge = if hovered_max.x < selected_min.x { selected_min.x } else { selected_max.x };
		let hovered_facing_edge = if hovered_max.x < selected_min.x { hovered_max.x } else { hovered_min.x };
		let horizontal_line_start_y = if selected_center.y < hovered_max.y && selected_center.y > hovered_min.y {
			selected_center.y
		} else {
			hovered_center.y
		};

		// Draw horizontal solid line with length
		let line_start = DVec2::new(selected_facing_edge, horizontal_line_start_y);
		let line_end = DVec2::new(hovered_facing_edge, horizontal_line_start_y);
		let label_alignment = LabelAlignment::new(false, false, true);
		draw_line_with_length(line_start, line_end, transform, document_to_viewport, overlay_context, label_alignment);
	}
}

/// Draws measurements for cases where lines cross on both X and Y axes, handling diagonal intersections.
fn draw_two_axis_one_one_crossing(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let (selected_min, selected_max) = (selected_bounds.min(), selected_bounds.max());
	let (hovered_min, hovered_max) = (hovered_bounds.min(), hovered_bounds.max());

	let mut top_y_bound = f64::min(selected_min.y, hovered_min.y);
	let mut bottom_y_bound = f64::max(selected_max.y, hovered_max.y);
	let mut top_x_bound = f64::max(selected_max.x, hovered_max.x);
	let mut bottom_x_bound = f64::min(selected_min.x, hovered_min.x);

	// Handle diagonal intersection cases by swapping bounds if necessary
	if (hovered_bounds.center().x > selected_bounds.center().x && hovered_bounds.center().y < selected_bounds.center().y)
		|| (hovered_bounds.center().x < selected_bounds.center().x && hovered_bounds.center().y > selected_bounds.center().y)
	{
		std::mem::swap(&mut top_y_bound, &mut bottom_y_bound);
		std::mem::swap(&mut top_x_bound, &mut bottom_x_bound);
	}

	// Draw horizontal solid lines with length
	let top_x_start = DVec2::new(f64::min(selected_max.x, hovered_max.x), top_y_bound);
	let top_x_end = DVec2::new(f64::max(selected_max.x, hovered_max.x), top_y_bound);
	let label_alignment = LabelAlignment::new(false, false, true);
	draw_line_with_length(top_x_start, top_x_end, transform, document_to_viewport, overlay_context, label_alignment);

	let bottom_x_start = DVec2::new(f64::min(selected_min.x, hovered_min.x), bottom_y_bound);
	let bottom_x_end = DVec2::new(f64::max(selected_min.x, hovered_min.x), bottom_y_bound);
	let label_alignment = LabelAlignment::new(false, false, false);
	draw_line_with_length(bottom_x_start, bottom_x_end, transform, document_to_viewport, overlay_context, label_alignment);

	// Draw vertical solid lines with length
	let top_y_start = DVec2::new(top_x_bound, f64::min(selected_min.y, hovered_min.y));
	let top_y_end = DVec2::new(top_x_bound, f64::max(selected_min.y, hovered_min.y));
	let label_alignment = LabelAlignment::new(true, false, false);
	draw_line_with_length(top_y_start, top_y_end, transform, document_to_viewport, overlay_context, label_alignment);

	let bottom_y_start = DVec2::new(bottom_x_bound, f64::min(selected_max.y, hovered_max.y));
	let bottom_y_end = DVec2::new(bottom_x_bound, f64::max(selected_max.y, hovered_max.y));
	let label_alignment = LabelAlignment::new(true, true, false);
	draw_line_with_length(bottom_y_start, bottom_y_end, transform, document_to_viewport, overlay_context, label_alignment);
}

/// Draws measurements for partial overlaps with two vertical or horizontal edge intersections.
fn draw_two_axis_one_one_two_zero_crossing(
	selected_bounds: Rect,
	hovered_bounds: Rect,
	transform: DAffine2,
	document_to_viewport: DAffine2,
	overlay_context: &mut OverlayContext,
	two_vertical_edge_intersect: bool,
) {
	let (selected_min, selected_max) = (selected_bounds.min(), selected_bounds.max());
	let (hovered_min, hovered_max) = (hovered_bounds.min(), hovered_bounds.max());

	if two_vertical_edge_intersect {
		let selected_bound_edge = if selected_bounds.center().y >= hovered_bounds.center().y {
			selected_max.y
		} else {
			selected_min.y
		};
		let hovered_bound_edge = if selected_bounds.center().y >= hovered_bounds.center().y { hovered_max.y } else { hovered_min.y };

		// Draw vertical solid lines with length
		let y_start_left = DVec2::new(hovered_min.x, f64::min(selected_bound_edge, hovered_bound_edge));
		let y_end_left = DVec2::new(hovered_min.x, f64::max(selected_bound_edge, hovered_bound_edge));
		let label_alignment = LabelAlignment::new(true, true, false);
		draw_line_with_length(y_start_left, y_end_left, transform, document_to_viewport, overlay_context, label_alignment);

		let y_start_right = DVec2::new(hovered_max.x, f64::min(selected_bound_edge, hovered_bound_edge));
		let y_end_right = DVec2::new(hovered_max.x, f64::max(selected_bound_edge, hovered_bound_edge));
		let label_alignment = LabelAlignment::new(true, false, false);
		draw_line_with_length(y_start_right, y_end_right, transform, document_to_viewport, overlay_context, label_alignment);

		// Draw horizontal solid lines with length
		let horizontal_line_y_bound = if selected_bounds.center().y >= hovered_bounds.center().y {
			f64::max(selected_bound_edge, hovered_bound_edge)
		} else {
			f64::min(selected_bound_edge, hovered_bound_edge)
		};

		let x_start_left = DVec2::new(hovered_min.x, horizontal_line_y_bound);
		let x_end_left = DVec2::new(selected_min.x, horizontal_line_y_bound);
		let label_alignment = LabelAlignment::new(false, false, false);
		draw_line_with_length(x_start_left, x_end_left, transform, document_to_viewport, overlay_context, label_alignment);

		let x_start_right = DVec2::new(hovered_max.x, horizontal_line_y_bound);
		let x_end_right = DVec2::new(selected_max.x, horizontal_line_y_bound);
		let label_alignment = LabelAlignment::new(false, false, false);
		draw_line_with_length(x_start_right, x_end_right, transform, document_to_viewport, overlay_context, label_alignment);
	} else {
		let selected_bound_edge = if selected_bounds.center().x >= hovered_bounds.center().x {
			selected_max.x
		} else {
			selected_min.x
		};
		let hovered_bound_edge = if selected_bounds.center().x >= hovered_bounds.center().x { hovered_max.x } else { hovered_min.x };

		// Determine the outermost X position for vertical lines
		let vertical_line_x = if selected_bounds.center().x >= hovered_bounds.center().x {
			f64::max(selected_bound_edge, hovered_bound_edge)
		} else {
			f64::min(selected_bound_edge, hovered_bound_edge)
		};

		// Draw vertical solid lines with length
		let y_start_up = DVec2::new(vertical_line_x, selected_min.y);
		let y_end_up = DVec2::new(vertical_line_x, hovered_min.y);
		let label_alignment = LabelAlignment::new(true, false, false);
		draw_line_with_length(y_start_up, y_end_up, transform, document_to_viewport, overlay_context, label_alignment);

		let y_start_down = DVec2::new(vertical_line_x, selected_max.y);
		let y_end_down = DVec2::new(vertical_line_x, hovered_max.y);
		let label_alignment = LabelAlignment::new(true, false, false);
		draw_line_with_length(y_start_down, y_end_down, transform, document_to_viewport, overlay_context, label_alignment);

		// Draw horizontal solid lines with length
		let horizontal_line_inner_x = if selected_bounds.center().x >= hovered_bounds.center().x {
			f64::min(selected_bound_edge, hovered_bound_edge)
		} else {
			f64::max(selected_bound_edge, hovered_bound_edge)
		};
		let x_start_up = DVec2::new(vertical_line_x, f64::min(selected_min.y, hovered_min.y));
		let x_end_up = DVec2::new(horizontal_line_inner_x, f64::min(selected_min.y, hovered_min.y));
		let label_alignment = LabelAlignment::new(false, false, true);
		draw_line_with_length(x_start_up, x_end_up, transform, document_to_viewport, overlay_context, label_alignment);

		let x_start_down = DVec2::new(vertical_line_x, f64::max(selected_max.y, hovered_max.y));
		let x_end_down = DVec2::new(horizontal_line_inner_x, f64::max(selected_max.y, hovered_max.y));
		let label_alignment = LabelAlignment::new(false, false, false);
		draw_line_with_length(x_start_down, x_end_down, transform, document_to_viewport, overlay_context, label_alignment);
	}
}

/// Draws measurements for cases with two vertical and two horizontal zero crossings.
fn draw_two_axis_two_zero_zero_two(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let (selected_min, selected_max) = (selected_bounds.min(), selected_bounds.max());
	let (hovered_min, hovered_max) = (hovered_bounds.min(), hovered_bounds.max());

	// Draw vertical solid lines with length
	let y_start_left_top = DVec2::new(f64::min(hovered_min.x, selected_min.x), f64::min(hovered_min.y, selected_min.y));
	let y_end_left_top = DVec2::new(f64::min(hovered_min.x, selected_min.x), f64::max(hovered_min.y, selected_min.y));
	let label_alignment = LabelAlignment::new(true, true, false);
	draw_line_with_length(y_start_left_top, y_end_left_top, transform, document_to_viewport, overlay_context, label_alignment);
	let label_alignment = LabelAlignment::new(true, true, false);
	draw_line_with_length(y_start_left_top, y_end_left_top, transform, document_to_viewport, overlay_context, label_alignment);

	let y_start_left_bottom = DVec2::new(f64::min(hovered_min.x, selected_min.x), f64::min(hovered_max.y, selected_max.y));
	let y_end_left_bottom = DVec2::new(f64::min(hovered_min.x, selected_min.x), f64::max(hovered_max.y, selected_max.y));
	let label_alignment = LabelAlignment::new(true, true, false);
	draw_line_with_length(y_start_left_bottom, y_end_left_bottom, transform, document_to_viewport, overlay_context, label_alignment);

	let y_start_right_top = DVec2::new(f64::max(hovered_max.x, selected_max.x), f64::min(hovered_min.y, selected_min.y));
	let y_end_right_top = DVec2::new(f64::max(hovered_max.x, selected_max.x), f64::max(hovered_min.y, selected_min.y));
	let label_alignment = LabelAlignment::new(true, false, false);
	draw_line_with_length(y_start_right_top, y_end_right_top, transform, document_to_viewport, overlay_context, label_alignment);

	let y_start_right_bottom = DVec2::new(f64::max(hovered_max.x, selected_max.x), f64::min(hovered_max.y, selected_max.y));
	let y_end_right_bottom = DVec2::new(f64::max(hovered_max.x, selected_max.x), f64::max(hovered_max.y, selected_max.y));
	let label_alignment = LabelAlignment::new(true, false, false);
	draw_line_with_length(y_start_right_bottom, y_end_right_bottom, transform, document_to_viewport, overlay_context, label_alignment);

	// Draw horizontal solid lines with length
	let x_start_left_top = DVec2::new(f64::min(hovered_min.x, selected_min.x), f64::min(hovered_min.y, selected_min.y));
	let x_end_left_top = DVec2::new(f64::max(hovered_min.x, selected_min.x), f64::min(hovered_min.y, selected_min.y));
	let label_alignment = LabelAlignment::new(false, false, true);
	draw_line_with_length(x_start_left_top, x_end_left_top, transform, document_to_viewport, overlay_context, label_alignment);

	let x_start_right_top = DVec2::new(f64::min(hovered_max.x, selected_max.x), f64::min(hovered_min.y, selected_min.y));
	let x_end_right_top = DVec2::new(f64::max(hovered_max.x, selected_max.x), f64::min(hovered_min.y, selected_min.y));
	let label_alignment = LabelAlignment::new(false, false, true);
	draw_line_with_length(x_start_right_top, x_end_right_top, transform, document_to_viewport, overlay_context, label_alignment);

	let x_start_left_bottom = DVec2::new(f64::min(hovered_min.x, selected_min.x), f64::max(hovered_max.y, selected_max.y));
	let x_end_left_bottom = DVec2::new(f64::max(hovered_min.x, selected_min.x), f64::max(hovered_max.y, selected_max.y));
	let label_alignment = LabelAlignment::new(false, false, false);
	draw_line_with_length(x_start_left_bottom, x_end_left_bottom, transform, document_to_viewport, overlay_context, label_alignment);

	let x_start_right_bottom = DVec2::new(f64::min(hovered_max.x, selected_max.x), f64::max(hovered_max.y, selected_max.y));
	let x_end_right_bottom = DVec2::new(f64::max(hovered_max.x, selected_max.x), f64::max(hovered_max.y, selected_max.y));
	let label_alignment = LabelAlignment::new(false, false, false);
	draw_line_with_length(x_start_right_bottom, x_end_right_bottom, transform, document_to_viewport, overlay_context, label_alignment);
}

/// Draws measurements where selected and hovered bounds have two vertical edges crossing each other.
fn draw_two_axis_two_zero_two_zero(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let (selected_min, selected_max) = (selected_bounds.min(), selected_bounds.max());
	let (hovered_min, hovered_max) = (hovered_bounds.min(), hovered_bounds.max());

	// Draw horizontal solid lines with length
	let x_start_left = DVec2::new(f64::max(hovered_min.x, selected_min.x), selected_bounds.center().y);
	let x_end_left = DVec2::new(f64::min(hovered_min.x, selected_min.x), selected_bounds.center().y);
	let label_alignment = LabelAlignment::new(false, false, true);
	draw_line_with_length(x_start_left, x_end_left, transform, document_to_viewport, overlay_context, label_alignment);

	let x_start_right = DVec2::new(f64::min(hovered_max.x, selected_max.x), selected_bounds.center().y);
	let x_end_right = DVec2::new(f64::max(hovered_max.x, selected_max.x), selected_bounds.center().y);
	let label_alignment = LabelAlignment::new(false, false, true);
	draw_line_with_length(x_start_right, x_end_right, transform, document_to_viewport, overlay_context, label_alignment);

	// Draw vertical solid lines with length
	let y_start_top = DVec2::new(selected_bounds.center().x, f64::max(hovered_min.y, selected_min.y));
	let y_end_top = DVec2::new(selected_bounds.center().x, f64::min(hovered_min.y, selected_min.y));
	let label_alignment = LabelAlignment::new(true, false, false);
	draw_line_with_length(y_start_top, y_end_top, transform, document_to_viewport, overlay_context, label_alignment);

	let y_start_bottom = DVec2::new(selected_bounds.center().x, f64::min(hovered_max.y, selected_max.y));
	let y_end_bottom = DVec2::new(selected_bounds.center().x, f64::max(hovered_max.y, selected_max.y));
	let label_alignment = LabelAlignment::new(true, false, false);
	draw_line_with_length(y_start_bottom, y_end_bottom, transform, document_to_viewport, overlay_context, label_alignment);
}

/// Handles overlapping scenarios involving two axes between selected and hovered bounds.
fn handle_two_axis_overlap(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let (selected_min, selected_max) = (selected_bounds.min(), selected_bounds.max());
	let (hovered_min, hovered_max) = (hovered_bounds.min(), hovered_bounds.max());

	// Calculate edge crossings on the X-axis
	let selected_x_crosses = (selected_min.y >= hovered_min.y && selected_min.y <= hovered_max.y) as u8 + (selected_max.y >= hovered_min.y && selected_max.y <= hovered_max.y) as u8;
	let hovered_x_crosses = (hovered_min.y >= selected_min.y && hovered_min.y <= selected_max.y) as u8 + (hovered_max.y >= selected_min.y && hovered_max.y <= selected_max.y) as u8;

	// Calculate edge crossings on the Y-axis
	let selected_y_crosses = (selected_min.x >= hovered_min.x && selected_min.x <= hovered_max.x) as u8 + (selected_max.x >= hovered_min.x && selected_max.x <= hovered_max.x) as u8;
	let hovered_y_crosses = (hovered_min.x >= selected_min.x && hovered_min.x <= selected_max.x) as u8 + (hovered_max.x >= selected_min.x && hovered_max.x <= selected_max.x) as u8;

	// Determine the overlap case based on edge crossings
	match ((selected_x_crosses, hovered_x_crosses), (selected_y_crosses, hovered_y_crosses)) {
		((1, 1), (1, 1)) => draw_two_axis_one_one_crossing(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		((1, 1), (2, 0)) => draw_two_axis_one_one_two_zero_crossing(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context, true),
		((1, 1), (0, 2)) => draw_two_axis_one_one_two_zero_crossing(hovered_bounds, selected_bounds, transform, document_to_viewport, overlay_context, true),
		((2, 0), (1, 1)) => draw_two_axis_one_one_two_zero_crossing(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context, false),
		((0, 2), (1, 1)) => draw_two_axis_one_one_two_zero_crossing(hovered_bounds, selected_bounds, transform, document_to_viewport, overlay_context, false),
		((2, 0), (0, 2)) | ((0, 2), (2, 0)) => draw_two_axis_two_zero_zero_two(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		((2, 0), (2, 0)) | ((0, 2), (0, 2)) => draw_two_axis_two_zero_two_zero(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		_ => (),
	}
}

/// Overlays measurement lines between selected and hovered bounds based on their spatial relationships.
pub fn overlay(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	draw_dashed_rect_outline(selected_bounds, transform, overlay_context);
	draw_dashed_rect_outline(hovered_bounds, transform, overlay_context);
	let (selected_min, selected_max) = (selected_bounds.min(), selected_bounds.max());
	let (hovered_min, hovered_max) = (hovered_bounds.min(), hovered_bounds.max());

	// Determine axis overlaps
	let overlap_y = does_overlap_y(selected_bounds, hovered_bounds) || does_overlap_y(hovered_bounds, selected_bounds);
	let overlap_x = does_overlap_x(selected_bounds, hovered_bounds) || does_overlap_x(hovered_bounds, selected_bounds);
	let overlap_axes = match (overlap_x, overlap_y) {
		(true, true) => 2,
		(true, false) | (false, true) => 1,
		_ => 0,
	};

	// Determine centerline crossings
	let center_x_intersects =
		(selected_bounds.center().y >= hovered_min.y && selected_bounds.center().y <= hovered_max.y) || (hovered_bounds.center().y >= selected_min.y && hovered_bounds.center().y <= selected_max.y);
	let center_y_intersects =
		(selected_bounds.center().x >= hovered_min.x && selected_bounds.center().x <= hovered_max.x) || (hovered_bounds.center().x >= selected_min.x && hovered_bounds.center().x <= selected_max.x);
	let centerline_crosses = match (center_x_intersects, center_y_intersects) {
		(true, true) => 2,
		(true, false) | (false, true) => 1,
		_ => 0,
	};

	// Handle each overlap case
	match (overlap_axes, centerline_crosses) {
		(0, _) => draw_zero_axis_crossings(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		(1, 0) => draw_single_axis_zero_crossings(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		(1, 1) | (1, 2) => draw_single_axis_one_crossings(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		(2, _) => handle_two_axis_overlap(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		_ => (), // Fallback case, should not typically happen
	}
}

struct LabelAlignment {
	is_vertical_line: bool,
	text_on_left: bool,
	text_on_top: bool,
}

impl LabelAlignment {
	fn new(is_vertical_line: bool, text_on_left: bool, text_on_top: bool) -> Self {
		Self {
			is_vertical_line,
			text_on_left,
			text_on_top,
		}
	}
}
