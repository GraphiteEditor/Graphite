use crate::consts::COLOR_OVERLAY_BLUE;
use crate::messages::portfolio::document::overlays::utility_types::{OverlayContext, Pivot};
use crate::messages::tool::tool_messages::tool_prelude::*;

use graphene_std::renderer::Rect;

fn draw_zero_axis_crossings(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let transform_to_document = document_to_viewport.inverse() * transform;

	// Determine if the selected object is on the right or left of the hovered object
	let selected_on_right = selected_bounds.min().x > hovered_bounds.max().x;
	let selected_on_bottom = selected_bounds.min().y > hovered_bounds.max().y;

	// Horizontal lines

	let selected_y = if selected_on_bottom { selected_bounds.min().y } else { selected_bounds.max().y };
	let hovered_y = if selected_on_bottom { hovered_bounds.max().y } else { hovered_bounds.min().y };
	let selected_x = if selected_on_right { selected_bounds.min().x } else { selected_bounds.max().x };
	let hovered_x = if selected_on_right { hovered_bounds.max().x } else { hovered_bounds.min().x };

	let line_start = DVec2::new(selected_x, selected_y);
	let line_end = DVec2::new(hovered_x, selected_y);
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

	let line_start = DVec2::new(selected_x, hovered_y);
	let line_end = DVec2::new(hovered_x, hovered_y);
	let min_viewport = transform.transform_point2(line_start);
	let max_viewport = transform.transform_point2(line_end);

	overlay_context.dashed_line_with_pattern(min_viewport, max_viewport, None, 2.0, 2.0);

	let min_viewport_y_selected = transform.transform_point2(DVec2::new(selected_x, selected_y));
	let max_viewport_y_selected = transform.transform_point2(DVec2::new(selected_x, hovered_y));
	overlay_context.line(min_viewport_y_selected, max_viewport_y_selected, None);
	let length_y_selected = format!("{:.2}", transform_to_document.transform_vector2(DVec2::Y * (hovered_y - selected_y)).length());
	overlay_context.text(
		&length_y_selected,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_viewport_y_selected + max_viewport_y_selected) / 2.),
		5.,
		[Pivot::Start, Pivot::Middle],
	);

	let min_viewport_y_hovered = transform.transform_point2(DVec2::new(hovered_x, selected_y));
	let max_viewport_y_hovered = transform.transform_point2(DVec2::new(hovered_x, hovered_y));
	overlay_context.dashed_line_with_pattern(min_viewport_y_hovered, max_viewport_y_hovered, None, 2.0, 2.0);
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

		overlay_context.dashed_line_with_pattern(min_viewport, max_viewport, None, 2.0, 2.0);
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

		overlay_context.dashed_line_with_pattern(min_viewport, max_viewport, None, 2.0, 2.0);
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
fn draw_two_axis_one_one_two_zero_crossing(
	selected_bounds: Rect,
	hovered_bounds: Rect,
	transform: DAffine2,
	document_to_viewport: DAffine2,
	overlay_context: &mut OverlayContext,
	two_vertical_edge_intersect: bool,
) {
	// selected has 2 edge lines crossing hovered
	let transform_to_document = document_to_viewport.inverse() * transform;
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
		//  vertical lines
		let y_start_left = DVec2::new(hovered_bounds.min().x, f64::min(selected_bound_edge, hovered_bound_edge));
		let y_end_left = DVec2::new(hovered_bounds.min().x, f64::max(selected_bound_edge, hovered_bound_edge));
		let min_y_viewport = transform.transform_point2(y_start_left);
		let max_y_viewport = transform.transform_point2(y_end_left);
		overlay_context.line(min_y_viewport, max_y_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_left - y_start_left).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);
		let y_start_right = DVec2::new(hovered_bounds.max().x, f64::min(selected_bound_edge, hovered_bound_edge));
		let y_end_right = DVec2::new(hovered_bounds.max().x, f64::max(selected_bound_edge, hovered_bound_edge));
		let min_y_viewport = transform.transform_point2(y_start_right);
		let max_y_viewport = transform.transform_point2(y_end_right);
		overlay_context.line(min_y_viewport, max_y_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_right - y_start_right).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);

		// horizontal lines
		let horizontal_line_y_bound = if selected_bounds.center().y >= hovered_bounds.center().y {
			f64::max(selected_bound_edge, hovered_bound_edge)
		} else {
			f64::min(selected_bound_edge, hovered_bound_edge)
		};

		// horizontal lines
		let x_start_left = DVec2::new(hovered_bounds.min().x, horizontal_line_y_bound);
		let x_end_left = DVec2::new(selected_bounds.min().x, horizontal_line_y_bound);
		let min_x_viewport = transform.transform_point2(x_start_left);
		let max_x_viewport = transform.transform_point2(x_end_left);
		overlay_context.line(min_x_viewport, max_x_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_left - x_start_left).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);

		let x_start_right = DVec2::new(hovered_bounds.max().x, horizontal_line_y_bound);
		let x_end_right = DVec2::new(selected_bounds.max().x, horizontal_line_y_bound);
		let min_x_viewport = transform.transform_point2(x_start_right);
		let max_x_viewport = transform.transform_point2(x_end_right);
		overlay_context.line(min_x_viewport, max_x_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_right - x_start_right).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);
	} else {
		// horizontal intersections
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
		//  vertical lines
		// outermost x position
		let vertical_line_x = if selected_bounds.center().x >= hovered_bounds.center().x {
			f64::max(selected_bound_edge, hovered_bound_edge)
		} else {
			f64::min(selected_bound_edge, hovered_bound_edge)
		};

		let y_start_up = DVec2::new(vertical_line_x, selected_bounds.min().y);
		let y_end_up = DVec2::new(vertical_line_x, hovered_bounds.min().y);
		let min_y_viewport = transform.transform_point2(y_start_up);
		let max_y_viewport = transform.transform_point2(y_end_up);
		overlay_context.line(min_y_viewport, max_y_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_up - y_start_up).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);
		let y_start_down = DVec2::new(vertical_line_x, selected_bounds.max().y);
		let y_end_down = DVec2::new(vertical_line_x, hovered_bounds.max().y);
		let min_y_viewport = transform.transform_point2(y_start_down);
		let max_y_viewport = transform.transform_point2(y_end_down);
		overlay_context.line(min_y_viewport, max_y_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_down - y_start_down).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);

		// horizontal lines
		let horizontal_line_inner_x = if selected_bounds.center().x >= hovered_bounds.center().x {
			f64::min(selected_bound_edge, hovered_bound_edge)
		} else {
			f64::max(selected_bound_edge, hovered_bound_edge)
		};
		let x_start_up = DVec2::new(vertical_line_x, f64::min(selected_bounds.min().y, hovered_bounds.min().y));
		let x_end_up = DVec2::new(horizontal_line_inner_x, f64::min(selected_bounds.min().y, hovered_bounds.min().y));
		let min_x_viewport = transform.transform_point2(x_start_up);
		let max_x_viewport = transform.transform_point2(x_end_up);
		overlay_context.line(min_x_viewport, max_x_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_up - x_start_up).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);
		let x_start_down = DVec2::new(vertical_line_x, f64::max(selected_bounds.max().y, hovered_bounds.max().y));
		let x_end_down = DVec2::new(horizontal_line_inner_x, f64::max(selected_bounds.max().y, hovered_bounds.max().y));
		let min_x_viewport = transform.transform_point2(x_start_down);
		let max_x_viewport = transform.transform_point2(x_end_down);
		overlay_context.line(min_x_viewport, max_x_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_down - x_start_down).length());
		overlay_context.text(
			&length,
			COLOR_OVERLAY_BLUE,
			None,
			DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
			5.,
			[Pivot::Middle, Pivot::Start],
		);
	}
}
fn draw_two_axis_two_zero_zero_two(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let transform_to_document = document_to_viewport.inverse() * transform;
	//  vertical lines
	let y_start_left_top = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let y_end_left_top = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::max(hovered_bounds.min().y, selected_bounds.min().y));
	let min_y_viewport = transform.transform_point2(y_start_left_top);
	let max_y_viewport = transform.transform_point2(y_end_left_top);
	overlay_context.line(min_y_viewport, max_y_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_left_top - y_start_left_top).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
	let y_start_left_bottom = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::min(hovered_bounds.max().y, selected_bounds.max().y));
	let y_end_left_bottom = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	let min_y_viewport = transform.transform_point2(y_start_left_bottom);
	let max_y_viewport = transform.transform_point2(y_end_left_bottom);
	overlay_context.line(min_y_viewport, max_y_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_left_bottom - y_start_left_bottom).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
	let y_start_right_top = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let y_end_right_top = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::max(hovered_bounds.min().y, selected_bounds.min().y));
	let min_y_viewport = transform.transform_point2(y_start_right_top);
	let max_y_viewport = transform.transform_point2(y_end_right_top);
	overlay_context.line(min_y_viewport, max_y_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_right_top - y_start_right_top).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
	let y_start_right_bottom = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::min(hovered_bounds.max().y, selected_bounds.max().y));
	let y_end_right_bottom = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	let min_y_viewport = transform.transform_point2(y_start_right_bottom);
	let max_y_viewport = transform.transform_point2(y_end_right_bottom);
	overlay_context.line(min_y_viewport, max_y_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_right_bottom - y_start_right_bottom).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
	// horizontal lines
	let x_start_left_top = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let x_end_left_top = DVec2::new(f64::max(hovered_bounds.min().x, selected_bounds.min().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let min_x_viewport = transform.transform_point2(x_start_left_top);
	let max_x_viewport = transform.transform_point2(x_end_left_top);
	overlay_context.line(min_x_viewport, max_x_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_left_top - x_start_left_top).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
	let x_start_right_top = DVec2::new(f64::min(hovered_bounds.max().x, selected_bounds.max().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let x_end_right_top = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let min_x_viewport = transform.transform_point2(x_start_right_top);
	let max_x_viewport = transform.transform_point2(x_end_right_top);
	overlay_context.line(min_x_viewport, max_x_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_right_top - x_start_right_top).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
	let x_start_left_bottom = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	let x_end_left_bottom = DVec2::new(f64::max(hovered_bounds.min().x, selected_bounds.min().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	let min_x_viewport = transform.transform_point2(x_start_left_bottom);
	let max_x_viewport = transform.transform_point2(x_end_left_bottom);
	overlay_context.line(min_x_viewport, max_x_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_left_bottom - x_start_left_bottom).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
	let x_start_right_bottom = DVec2::new(f64::min(hovered_bounds.max().x, selected_bounds.max().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	let x_end_right_bottom = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	let min_x_viewport = transform.transform_point2(x_start_right_bottom);
	let max_x_viewport = transform.transform_point2(x_end_right_bottom);
	overlay_context.line(min_x_viewport, max_x_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_right_bottom - x_start_right_bottom).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
}
fn draw_two_axis_two_zero_two_zero(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let transform_to_document = document_to_viewport.inverse() * transform;
	// horizontal lines
	let x_start_left = DVec2::new(f64::max(hovered_bounds.min().x, selected_bounds.min().x), selected_bounds.center().y);
	let x_end_left = DVec2::new(f64::min(hovered_bounds.min().x, selected_bounds.min().x), selected_bounds.center().y);
	let min_x_viewport = transform.transform_point2(x_start_left);
	let max_x_viewport = transform.transform_point2(x_end_left);
	overlay_context.line(min_x_viewport, max_x_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_left - x_start_left).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
	let x_start_right = DVec2::new(f64::min(hovered_bounds.max().x, selected_bounds.max().x), selected_bounds.center().y);
	let x_end_right = DVec2::new(f64::max(hovered_bounds.max().x, selected_bounds.max().x), selected_bounds.center().y);
	let min_x_viewport = transform.transform_point2(x_start_right);
	let max_x_viewport = transform.transform_point2(x_end_right);
	overlay_context.line(min_x_viewport, max_x_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(x_end_right - x_start_right).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_x_viewport + max_x_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);

	//vertical lines
	let y_start_top = DVec2::new(selected_bounds.center().x, f64::max(hovered_bounds.min().y, selected_bounds.min().y));
	let y_end_top = DVec2::new(selected_bounds.center().x, f64::min(hovered_bounds.min().y, selected_bounds.min().y));
	let min_y_viewport = transform.transform_point2(y_start_top);
	let max_y_viewport = transform.transform_point2(y_end_top);
	overlay_context.line(min_y_viewport, max_y_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_top - y_start_top).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
	let y_start_bottom = DVec2::new(selected_bounds.center().x, f64::min(hovered_bounds.max().y, selected_bounds.max().y));
	let y_end_bottom = DVec2::new(selected_bounds.center().x, f64::max(hovered_bounds.max().y, selected_bounds.max().y));
	let min_y_viewport = transform.transform_point2(y_start_bottom);
	let max_y_viewport = transform.transform_point2(y_end_bottom);
	overlay_context.line(min_y_viewport, max_y_viewport, None);
	let length = format!("{:.2}", transform_to_document.transform_vector2(y_end_bottom - y_start_bottom).length());
	overlay_context.text(
		&length,
		COLOR_OVERLAY_BLUE,
		None,
		DAffine2::from_translation((min_y_viewport + max_y_viewport) / 2.),
		5.,
		[Pivot::Middle, Pivot::Start],
	);
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
	// TODO: for all cases apply object rotation to bounds before drawing lines

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
}
