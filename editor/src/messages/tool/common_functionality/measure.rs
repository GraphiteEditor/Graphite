use crate::consts::{COLOR_OVERLAY_BLUE, COLOR_OVERLAY_GREEN};
use crate::messages::portfolio::document::overlays::utility_types::{OverlayContext, Pivot};
use crate::messages::tool::tool_messages::tool_prelude::*;

use graphene_std::renderer::Rect;

fn draw_zero_axis_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// Calculate the center points and transform for length calculations
	let selected_center = selected_bounds.center();
	let hovered_center = hovered_bounds.center();
	let transform_to_document = document_to_viewport.inverse() * transform;

	// Determine if the selected object is on the right or left of the hovered object
	let selected_on_right = selected_bounds.min().x > hovered_bounds.max().x;
	let selected_on_bottom = selected_bounds.min().y > hovered_bounds.max().y;

	// Horizontal line from the left or right edge of selected to hovered's center Y
	let selected_horizontal_x = if selected_on_right {
		selected_bounds.min().x // Left edge of selected if it's on the right
	} else {
		selected_bounds.max().x // Right edge of selected if it's on the left
	};
	let min_viewport_x_selected = transform.transform_point2(DVec2::new(selected_horizontal_x, selected_center.y));
	let max_viewport_x_selected = transform.transform_point2(DVec2::new(hovered_center.x, selected_center.y));
	overlay_context.line(min_viewport_x_selected, max_viewport_x_selected, None);
	let length_x_selected = format!("{:.2}", transform_to_document.transform_vector2(DVec2::X * (hovered_center.x - selected_horizontal_x)).length());
	overlay_context.text(
		&length_x_selected,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_viewport_x_selected + max_viewport_x_selected) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);

	// Horizontal line from the left or right edge of hovered to selected's center Y
	let hovered_horizontal_x = if selected_on_right {
		hovered_bounds.max().x // Right edge of hovered if selected is on the right
	} else {
		hovered_bounds.min().x // Left edge of hovered if selected is on the left
	};
	let min_viewport_x_hovered = transform.transform_point2(DVec2::new(hovered_horizontal_x, hovered_center.y));
	let max_viewport_x_hovered = transform.transform_point2(DVec2::new(selected_center.x, hovered_center.y));
	overlay_context.line(min_viewport_x_hovered, max_viewport_x_hovered, None);
	let length_x_hovered = format!("{:.2}", transform_to_document.transform_vector2(DVec2::X * (selected_center.x - hovered_horizontal_x)).length());
	overlay_context.text(
		&length_x_hovered,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_viewport_x_hovered + max_viewport_x_hovered) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);

	// Vertical line from the top or bottom edge of selected to hovered's center X
	let selected_vertical_y = if selected_on_bottom {
		selected_bounds.min().y // Top edge if selected is below hovered
	} else {
		selected_bounds.max().y // Bottom edge if selected is above hovered
	};
	let min_viewport_y_selected = transform.transform_point2(DVec2::new(selected_center.x, selected_vertical_y));
	let max_viewport_y_selected = transform.transform_point2(DVec2::new(selected_center.x, hovered_center.y));
	overlay_context.line(min_viewport_y_selected, max_viewport_y_selected, None);
	let length_y_selected = format!("{:.2}", transform_to_document.transform_vector2(DVec2::Y * (hovered_center.y - selected_vertical_y)).length());
	overlay_context.text(
		&length_y_selected,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_viewport_y_selected + max_viewport_y_selected) / 2.),
		5.,
		[Pivot::Start, Pivot::Middle],
	);

	// Vertical line from the top or bottom edge of hovered to selected's center X
	let hovered_vertical_y = if selected_on_bottom {
		hovered_bounds.max().y // Bottom edge if selected is below hovered
	} else {
		hovered_bounds.min().y // Top edge if selected is above hovered
	};
	let min_viewport_y_hovered = transform.transform_point2(DVec2::new(hovered_center.x, hovered_vertical_y));
	let max_viewport_y_hovered = transform.transform_point2(DVec2::new(hovered_center.x, selected_center.y));
	overlay_context.line(min_viewport_y_hovered, max_viewport_y_hovered, None);
	let length_y_hovered = format!("{:.2}", transform_to_document.transform_vector2(DVec2::Y * (selected_center.y - hovered_vertical_y)).length());
	overlay_context.text(
		&length_y_hovered,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_viewport_y_hovered + max_viewport_y_hovered) / 2.),
		5.,
		[Pivot::Start, Pivot::Middle],
	);
}
fn draw_single_axis_zero_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let transform_to_document = document_to_viewport.inverse() * transform;

	// Check for overlaps on both X and Y axes
	let overlap_y = selected_bounds.min().x < hovered_bounds.max().x && selected_bounds.max().x > hovered_bounds.min().x;
	let overlap_x = selected_bounds.min().y < hovered_bounds.max().y && selected_bounds.max().y > hovered_bounds.min().y;

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
		let min_viewport = transform.transform_point2(line_start);
		let max_viewport = transform.transform_point2(line_end);
		overlay_context.dashed_line(min_viewport, max_viewport, Some(COLOR_OVERLAY_GREEN), Some(2.0));
		let length = format!("{:.2}", transform_to_document.transform_vector2(line_end - line_start).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_viewport + max_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);

		// Vertical line showing distance between the intersecting parts
		let line_start = DVec2::new(vertical_line_start_x, selected_facing_edge);
		let line_end = DVec2::new(vertical_line_start_x, hovered_facing_edge);
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

		let dashed_line_start = DVec2::new(dashed_vertical_line_start_x, selected_facing_edge);
		let dashed_line_end = DVec2::new(dashed_vertical_line_start_x, hovered_facing_edge);
		let min_viewport = transform.transform_point2(dashed_line_start);
		let max_viewport = transform.transform_point2(dashed_line_end);

		overlay_context.dashed_line(min_viewport, max_viewport, None, Some(2.0));
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
		let min_viewport = transform.transform_point2(line_start);
		let max_viewport = transform.transform_point2(line_end);
		overlay_context.dashed_line(min_viewport, max_viewport, Some(COLOR_OVERLAY_GREEN), Some(2.0));
		let length = format!("{:.2}", transform_to_document.transform_vector2(line_end - line_start).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_viewport + max_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);

		// Horizontal line showing distance between the intersecting parts
		let line_start = DVec2::new(selected_facing_edge, horizontal_line_start_y);
		let line_end = DVec2::new(hovered_facing_edge, horizontal_line_start_y);
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

		let dashed_line_start = DVec2::new(selected_facing_edge, dashed_horizontal_line_start_y);
		let dashed_line_end = DVec2::new(hovered_facing_edge, dashed_horizontal_line_start_y);
		let min_viewport = transform.transform_point2(dashed_line_start);
		let max_viewport = transform.transform_point2(dashed_line_end);

		overlay_context.dashed_line(min_viewport, max_viewport, None, Some(2.0));
	}
}
fn draw_single_axis_one_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let selected_center = selected_bounds.center();
	let hovered_center = hovered_bounds.center();
	let transform_to_document = document_to_viewport.inverse() * transform;

	// Check for overlaps on both X and Y axes
	let overlap_y = selected_bounds.min().x < hovered_bounds.max().x && selected_bounds.max().x > hovered_bounds.min().x;
	let overlap_x = selected_bounds.min().y < hovered_bounds.max().y && selected_bounds.max().y > hovered_bounds.min().y;

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
}
fn draw_two_axis_one_one_crossing(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	//  pairs of equivalent cases across the two diagonals
	// intersection at top left and bottom right are equivalent
	// intersection at top right and bottom left are equivalent
	let transform_to_document = document_to_viewport.inverse() * transform;

	// handling intersections at top left and bottom right by default
	let mut top_y_bound = f64::min(selected_bounds.min().y, hovered_bounds.min().y);
	let mut bottom_y_bound: f64 = f64::max(selected_bounds.max().y, hovered_bounds.max().y);
	let mut top_x_bound = f64::max(selected_bounds.max().x, hovered_bounds.max().x);
	let mut bottom_x_bound = f64::min(selected_bounds.min().x, hovered_bounds.min().x);
	if (hovered_bounds.center().x > selected_bounds.center().x && hovered_bounds.center().y < selected_bounds.center().y)
		|| (hovered_bounds.center().x < selected_bounds.center().x && hovered_bounds.center().y > selected_bounds.center().y)
	{
		// switch horizontal lines y values for top right and bottom left intersections
		std::mem::swap(&mut top_y_bound, &mut bottom_y_bound);
		std::mem::swap(&mut top_x_bound, &mut bottom_x_bound);
	}

	// horizontal lines
	let top_x_start = DVec2::new(f64::min(selected_bounds.max().x, hovered_bounds.max().x), top_y_bound);
	let top_x_end = DVec2::new(f64::max(selected_bounds.max().x, hovered_bounds.max().x), top_y_bound);
	let min_top_x_viewport = transform.transform_point2(top_x_start);
	let max_top_x_viewport = transform.transform_point2(top_x_end);
	overlay_context.line(min_top_x_viewport, max_top_x_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(top_x_end - top_x_start).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_top_x_viewport + max_top_x_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);

	let bottom_x_start = DVec2::new(f64::min(selected_bounds.min().x, hovered_bounds.min().x), bottom_y_bound);
	let bottom_x_end = DVec2::new(f64::max(selected_bounds.min().x, hovered_bounds.min().x), bottom_y_bound);
	let min_bottom_x_viewport = transform.transform_point2(bottom_x_start);
	let max_bottom_x_viewport = transform.transform_point2(bottom_x_end);
	overlay_context.line(min_bottom_x_viewport, max_bottom_x_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(bottom_x_end - bottom_x_start).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_bottom_x_viewport + max_bottom_x_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);

	//  vertical lines
	let top_y_start = DVec2::new(top_x_bound, f64::min(selected_bounds.min().y, hovered_bounds.min().y));
	let top_y_end = DVec2::new(top_x_bound, f64::max(selected_bounds.min().y, hovered_bounds.min().y));
	let min_top_y_viewport = transform.transform_point2(top_y_start);
	let max_top_y_viewport = transform.transform_point2(top_y_end);
	overlay_context.line(min_top_y_viewport, max_top_y_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(top_y_end - top_y_start).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_top_y_viewport + max_top_y_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);

	let bottom_y_start = DVec2::new(bottom_x_bound, f64::min(selected_bounds.max().y, hovered_bounds.max().y));
	let bottom_y_end = DVec2::new(bottom_x_bound, f64::max(selected_bounds.max().y, hovered_bounds.max().y));
	let min_bottom_y_viewport = transform.transform_point2(bottom_y_start);
	let max_bottom_y_viewport = transform.transform_point2(bottom_y_end);
	overlay_context.line(min_bottom_y_viewport, max_bottom_y_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(bottom_y_end - bottom_y_start).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_bottom_y_viewport + max_bottom_y_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
}
fn draw_two_axis_one_one_two_zero_crossing(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// selected has 2 edge lines crossing hovered

	let transform_to_document = document_to_viewport.inverse() * transform;
}
fn handle_two_axis_overlap(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) -> () {
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
		((1, 1), (2, 0)) => draw_two_axis_one_one_two_zero_crossing(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		((1, 1), (0, 2)) => draw_two_axis_one_one_two_zero_crossing(hovered_bounds, selected_bounds, transform, document_to_viewport, overlay_context),
		((2, 0), (1, 1)) => draw_two_axis_one_one_two_zero_crossing(selected_bounds, hovered_bounds, transform, document_to_viewport, overlay_context),
		((0, 2), (1, 1)) => draw_two_axis_one_one_two_zero_crossing(hovered_bounds, selected_bounds, transform, document_to_viewport, overlay_context),
		((2, 0), (0, 2)) => (),
		((2, 0), (2, 0)) => (),
		_ => (),
	}
}

pub fn overlay(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	// check axis overlaps
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

	// let transform_to_document = document_to_viewport.inverse() * transform;
	// if selected_bounds.intersects(hovered_bounds) {
	// 	// TODO: Figure out what to do here
	// 	return;
	// }

	// // Always do horizontal then vertical from the selected
	// let turn_x = selected_bounds.center().x.clamp(hovered_bounds.min().x, hovered_bounds.max().x);
	// let turn_y = hovered_bounds.center().y.clamp(selected_bounds.min().y, selected_bounds.max().y);

	// let selected_x = turn_x.clamp(selected_bounds.min().x, selected_bounds.max().x);
	// let hovered_y = turn_y.clamp(hovered_bounds.min().y, hovered_bounds.max().y);

	// if turn_x != selected_x {
	// 	let min_viewport = transform.transform_point2(DVec2::new(turn_x.min(selected_x), turn_y));
	// 	let max_viewport = transform.transform_point2(DVec2::new(turn_x.max(selected_x), turn_y));
	// 	overlay_context.line(min_viewport, max_viewport, None);
	// 	let length = format!("{:.2}", transform_to_document.transform_vector2(DVec2::X * (turn_x - selected_x)).length());
	// 	let direction = -(min_viewport - max_viewport).normalize_or_zero();
	// 	let transform = DAffine2::from_translation((min_viewport + max_viewport) / 2.) * DAffine2::from_angle(-direction.angle_to(DVec2::X));
	// 	overlay_context.text(&length, COLOR_OVERLAY_BLUE, None, transform, 5., [Pivot::Middle, Pivot::Start]);
	// }
	// if turn_y != hovered_y {
	// 	let min_viewport = transform.transform_point2(DVec2::new(turn_x, turn_y.min(hovered_y)));
	// 	let max_viewport = transform.transform_point2(DVec2::new(turn_x, turn_y.max(hovered_y)));
	// 	overlay_context.line(min_viewport, max_viewport, None);
	// 	let length = format!("{:.2}", transform_to_document.transform_vector2(DVec2::Y * (turn_y - hovered_y)).length());
	// 	let direction = (min_viewport - max_viewport).normalize_or_zero().perp();
	// 	let transform = DAffine2::from_translation((min_viewport + max_viewport) / 2.) * DAffine2::from_angle(-direction.angle_to(DVec2::X));
	// 	overlay_context.text(&length, COLOR_OVERLAY_BLUE, None, transform, 5., [Pivot::Start, Pivot::Middle]);
	// }
}
