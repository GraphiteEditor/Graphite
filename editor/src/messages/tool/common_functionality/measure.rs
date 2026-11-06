use crate::consts::COLOR_OVERLAY_BLUE;
use crate::messages::portfolio::document::overlays::utility_types::{OverlayContext, Pivot};
use crate::messages::tool::tool_messages::tool_prelude::*;
use graphene_std::renderer::Rect;

/// Lengths shorter than this, in document units, are drawn without a label.
const LABEL_MIN_LENGTH: f64 = 0.01;
/// How far a length label sits from its line, in viewport pixels.
const LABEL_PADDING: f64 = 5.;

/// The side of a measured line that its length label goes on, in document space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
	Left,
	Right,
	Top,
	Bottom,
}

impl Side {
	/// The same side with the X and Y axes swapped.
	fn transposed(self) -> Self {
		match self {
			Self::Left => Self::Top,
			Self::Top => Self::Left,
			Self::Right => Self::Bottom,
			Self::Bottom => Self::Right,
		}
	}

	/// The side across the line.
	fn opposite(self) -> Self {
		match self {
			Self::Left => Self::Right,
			Self::Right => Self::Left,
			Self::Top => Self::Bottom,
			Self::Bottom => Self::Top,
		}
	}

	/// Which way the side lies from its line, in document space where Y points down.
	fn direction(self) -> DVec2 {
		match self {
			Self::Left => -DVec2::X,
			Self::Right => DVec2::X,
			Self::Top => -DVec2::Y,
			Self::Bottom => DVec2::Y,
		}
	}
}

/// One line of a measurement, in document space.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mark {
	/// A solid line labeled with its length.
	Measured { from: DVec2, to: DVec2, side: Side },
	/// A dashed line.
	Guide { from: DVec2, to: DVec2 },
}

/// Collects the lines of a measurement. With the axes swapped, a routine written for one axis also serves the other.
#[derive(Default)]
struct Marks {
	collected: Vec<Mark>,
	swapped: bool,
}

impl Marks {
	fn point(&self, [x, y]: [f64; 2]) -> DVec2 {
		if self.swapped { DVec2::new(y, x) } else { DVec2::new(x, y) }
	}

	fn measured(&mut self, from: [f64; 2], to: [f64; 2], side: Side) {
		// Edges the boxes share have no distance between them to show
		if from == to {
			return;
		}

		let side = if self.swapped { side.transposed() } else { side };
		let mark = Mark::Measured {
			from: self.point(from),
			to: self.point(to),
			side,
		};
		self.collected.push(mark);
	}

	fn guide(&mut self, from: [f64; 2], to: [f64; 2]) {
		let mark = Mark::Guide {
			from: self.point(from),
			to: self.point(to),
		};
		self.collected.push(mark);
	}

	/// Runs a routine on two boxes with their X and Y axes swapped, swapping its lines back.
	fn with_axes_swapped(&mut self, first: Rect, second: Rect, routine: fn(&mut Self, Rect, Rect)) {
		let swap = |rect: Rect| Rect(rect.0.map(|corner| DVec2::new(corner.y, corner.x)));

		self.swapped = true;
		routine(self, swap(first), swap(second));
		self.swapped = false;
	}
}

/// The boxes overlap in neither axis, so they sit diagonally apart.
fn apart_diagonally(marks: &mut Marks, selected: Rect, hovered: Rect) {
	let (selected_min, selected_max) = (selected.min(), selected.max());
	let (hovered_min, hovered_max) = (hovered.min(), hovered.max());

	let selected_on_right = selected_min.x > hovered_max.x;
	let selected_on_bottom = selected_min.y > hovered_max.y;
	let selected_x = if selected_on_right { selected_min.x } else { selected_max.x };
	let hovered_x = if selected_on_right { hovered_max.x } else { hovered_min.x };
	let selected_y = if selected_on_bottom { selected_min.y } else { selected_max.y };
	let hovered_y = if selected_on_bottom { hovered_max.y } else { hovered_min.y };

	marks.measured([selected_x, selected_y], [hovered_x, selected_y], if selected_on_bottom { Side::Bottom } else { Side::Top });
	marks.guide([selected_x, hovered_y], [hovered_x, hovered_y]);
	marks.measured([selected_x, selected_y], [selected_x, hovered_y], if selected_on_right { Side::Right } else { Side::Left });
	marks.guide([hovered_x, selected_y], [hovered_x, hovered_y]);
}

/// The boxes overlap along X and are apart along Y, with neither center within the other's span: the overlap, the gap at one end of it, and a guide at its other end.
fn apart_vertically_offset(marks: &mut Marks, selected: Rect, hovered: Rect) {
	let (selected_min, selected_max) = (selected.min(), selected.max());
	let (hovered_min, hovered_max) = (hovered.min(), hovered.max());

	let hovered_above = hovered_max.y < selected_min.y;
	let selected_edge = if hovered_above { selected_min.y } else { selected_max.y };
	let hovered_edge = if hovered_above { hovered_max.y } else { hovered_min.y };
	let hovered_reaches_right = hovered_max.x > selected_max.x;
	let gap_x = if hovered_reaches_right { selected_max.x } else { selected_min.x };
	let guide_x = if hovered_reaches_right { hovered_min.x } else { hovered_max.x };

	let selected_on_bottom = selected.center().y > hovered.center().y;
	let selected_on_right = selected.center().x > hovered.center().x;
	let overlap_side = if selected_on_bottom { Side::Top } else { Side::Bottom };
	let gap_side = if selected_on_right { Side::Right } else { Side::Left };

	marks.measured([hovered_max.x.min(selected_max.x), selected_edge], [hovered_min.x.max(selected_min.x), selected_edge], overlap_side);
	marks.measured([gap_x, selected_edge], [gap_x, hovered_edge], gap_side);
	marks.guide([guide_x, selected_edge], [guide_x, hovered_edge]);
}

/// The boxes overlap along X and are apart along Y, with a center within the other's span: just the gap, through a center.
fn apart_vertically_aligned(marks: &mut Marks, selected: Rect, hovered: Rect) {
	let (selected_min, selected_max) = (selected.min(), selected.max());
	let (hovered_min, hovered_max) = (hovered.min(), hovered.max());

	let hovered_above = hovered_max.y < selected_min.y;
	let selected_edge = if hovered_above { selected_min.y } else { selected_max.y };
	let hovered_edge = if hovered_above { hovered_max.y } else { hovered_min.y };
	let selected_center = selected.center().x;
	let x = if (hovered_min.x..=hovered_max.x).contains(&selected_center) {
		selected_center
	} else {
		hovered.center().x
	};

	marks.measured([x, selected_edge], [x, hovered_edge], Side::Left);
}

/// The boxes overlap at a corner: the offsets between their matching edges, drawn along the outside of both.
fn overlapping_corners(marks: &mut Marks, selected: Rect, hovered: Rect) {
	let (selected_min, selected_max) = (selected.min(), selected.max());
	let (hovered_min, hovered_max) = (hovered.min(), hovered.max());

	let mut top_y = selected_min.y.min(hovered_min.y);
	let mut bottom_y = selected_max.y.max(hovered_max.y);
	let mut right_x = selected_max.x.max(hovered_max.x);
	let mut left_x = selected_min.x.min(hovered_min.x);

	// When one box is up and to the right of the other, the lines and their labels go along the other pair of outer sides
	let (selected_center, hovered_center) = (selected.center(), hovered.center());
	let other_sides = (hovered_center.x > selected_center.x && hovered_center.y < selected_center.y) || (hovered_center.x < selected_center.x && hovered_center.y > selected_center.y);
	if other_sides {
		std::mem::swap(&mut top_y, &mut bottom_y);
		std::mem::swap(&mut right_x, &mut left_x);
	}
	let outward = |side: Side| if other_sides { side.opposite() } else { side };

	let (right_edges_start, right_edges_end) = ordered(selected_max.x, hovered_max.x);
	let (left_edges_start, left_edges_end) = ordered(selected_min.x, hovered_min.x);
	let (top_edges_start, top_edges_end) = ordered(selected_min.y, hovered_min.y);
	let (bottom_edges_start, bottom_edges_end) = ordered(selected_max.y, hovered_max.y);

	marks.measured([right_edges_start, top_y], [right_edges_end, top_y], outward(Side::Top));
	marks.measured([left_edges_start, bottom_y], [left_edges_end, bottom_y], outward(Side::Bottom));
	marks.measured([right_x, top_edges_start], [right_x, top_edges_end], outward(Side::Right));
	marks.measured([left_x, bottom_edges_start], [left_x, bottom_edges_end], outward(Side::Left));
}

/// Box `inner` lies within box `outer` along X and crosses one of its horizontal edges: the offsets from its sides to `outer`'s, along its edge beyond `outer`, and how far that edge reaches past `outer`'s.
fn straddling_an_edge(marks: &mut Marks, inner: Rect, outer: Rect) {
	let (inner_min, inner_max) = (inner.min(), inner.max());
	let (outer_min, outer_max) = (outer.min(), outer.max());

	let inner_lower = inner.center().y >= outer.center().y;
	let inner_edge = if inner_lower { inner_max.y } else { inner_min.y };
	let outer_edge = if inner_lower { outer_max.y } else { outer_min.y };
	let (upper_edge, lower_edge) = ordered(inner_edge, outer_edge);
	let (y, outward) = if inner_lower { (lower_edge, Side::Bottom) } else { (upper_edge, Side::Top) };

	marks.measured([outer_min.x, upper_edge], [outer_min.x, lower_edge], Side::Left);
	marks.measured([outer_max.x, upper_edge], [outer_max.x, lower_edge], Side::Right);
	marks.measured([outer_min.x, y], [inner_min.x, y], outward);
	marks.measured([outer_max.x, y], [inner_max.x, y], outward);
}

/// Each box is the longer one along a different axis, so they cross: the offsets between all four pairs of matching edges, at the outer corners.
fn crossing(marks: &mut Marks, selected: Rect, hovered: Rect) {
	let (selected_min, selected_max) = (selected.min(), selected.max());
	let (hovered_min, hovered_max) = (hovered.min(), hovered.max());

	let (left_x, right_x) = (hovered_min.x.min(selected_min.x), hovered_max.x.max(selected_max.x));
	let (top_y, bottom_y) = (hovered_min.y.min(selected_min.y), hovered_max.y.max(selected_max.y));
	let (top_edges_start, top_edges_end) = ordered(hovered_min.y, selected_min.y);
	let (bottom_edges_start, bottom_edges_end) = ordered(hovered_max.y, selected_max.y);
	let (left_edges_start, left_edges_end) = ordered(hovered_min.x, selected_min.x);
	let (right_edges_start, right_edges_end) = ordered(hovered_max.x, selected_max.x);

	marks.measured([left_x, top_edges_start], [left_x, top_edges_end], Side::Left);
	marks.measured([left_x, bottom_edges_start], [left_x, bottom_edges_end], Side::Left);
	marks.measured([right_x, top_edges_start], [right_x, top_edges_end], Side::Right);
	marks.measured([right_x, bottom_edges_start], [right_x, bottom_edges_end], Side::Right);

	marks.measured([left_edges_start, top_y], [left_edges_end, top_y], Side::Top);
	marks.measured([right_edges_start, top_y], [right_edges_end, top_y], Side::Top);
	marks.measured([left_edges_start, bottom_y], [left_edges_end, bottom_y], Side::Bottom);
	marks.measured([right_edges_start, bottom_y], [right_edges_end, bottom_y], Side::Bottom);
}

/// Box `inner` lies within box `outer`: the distance from each of its sides to `outer`'s, through its center.
fn contained(marks: &mut Marks, inner: Rect, outer: Rect) {
	let (inner_min, inner_max) = (inner.min(), inner.max());
	let (outer_min, outer_max) = (outer.min(), outer.max());
	let center = inner.center();

	marks.measured([outer_min.x.max(inner_min.x), center.y], [outer_min.x.min(inner_min.x), center.y], Side::Top);
	marks.measured([outer_max.x.min(inner_max.x), center.y], [outer_max.x.max(inner_max.x), center.y], Side::Top);
	marks.measured([center.x, outer_min.y.max(inner_min.y)], [center.x, outer_min.y.min(inner_min.y)], Side::Right);
	marks.measured([center.x, outer_max.y.min(inner_max.y)], [center.x, outer_max.y.max(inner_max.y)], Side::Right);
}

/// Two values from the smaller to the larger.
fn ordered(a: f64, b: f64) -> (f64, f64) {
	(a.min(b), a.max(b))
}

/// How many of `a`'s two edges along an axis lie within `b`'s span along it, counting edges that meet.
fn edges_within(a_min: f64, a_max: f64, b_min: f64, b_max: f64) -> u8 {
	let within = |edge: f64| (b_min..=b_max).contains(&edge);
	within(a_min) as u8 + within(a_max) as u8
}

/// Counts a span sharing one edge with the span it lies within as lying strictly within it.
fn within_despite_a_shared_edge(crossings: (u8, u8)) -> (u8, u8) {
	match crossings {
		(2, 1) => (2, 0),
		(1, 2) => (0, 2),
		crossings => crossings,
	}
}

/// The lines that measure the hovered box against the selected one, chosen by how the two boxes overlap.
fn measurement_marks(selected: Rect, hovered: Rect) -> Vec<Mark> {
	let (selected_min, selected_max) = (selected.min(), selected.max());
	let (hovered_min, hovered_max) = (hovered.min(), hovered.max());
	let (selected_center, hovered_center) = (selected.center(), hovered.center());
	let mut marks = Marks::default();

	// Whether the boxes' spans overlap along each axis, counting spans that meet
	let overlap_along_x = selected_min.x <= hovered_max.x && selected_max.x >= hovered_min.x;
	let overlap_along_y = selected_min.y <= hovered_max.y && selected_max.y >= hovered_min.y;

	// Whether either box's center lies within the other's span along each axis
	let center_within_x = (hovered_min.x..=hovered_max.x).contains(&selected_center.x) || (selected_min.x..=selected_max.x).contains(&hovered_center.x);
	let center_within_y = (hovered_min.y..=hovered_max.y).contains(&selected_center.y) || (selected_min.y..=selected_max.y).contains(&hovered_center.y);

	match (overlap_along_x, overlap_along_y) {
		(false, false) => apart_diagonally(&mut marks, selected, hovered),
		(true, false) if center_within_x => apart_vertically_aligned(&mut marks, selected, hovered),
		(true, false) => apart_vertically_offset(&mut marks, selected, hovered),
		(false, true) if center_within_y => marks.with_axes_swapped(selected, hovered, apart_vertically_aligned),
		(false, true) => marks.with_axes_swapped(selected, hovered, apart_vertically_offset),
		(true, true) => {
			let crossings_along_y = within_despite_a_shared_edge((
				edges_within(selected_min.y, selected_max.y, hovered_min.y, hovered_max.y),
				edges_within(hovered_min.y, hovered_max.y, selected_min.y, selected_max.y),
			));
			let crossings_along_x = within_despite_a_shared_edge((
				edges_within(selected_min.x, selected_max.x, hovered_min.x, hovered_max.x),
				edges_within(hovered_min.x, hovered_max.x, selected_min.x, selected_max.x),
			));

			// Identical spans along one axis take on the arrangement along the other, and identical boxes draw nothing
			let (crossings_along_y, crossings_along_x) = match (crossings_along_y, crossings_along_x) {
				((2, 2), other) | (other, (2, 2)) => (other, other),
				pair => pair,
			};

			match (crossings_along_y, crossings_along_x) {
				((1, 1), (1, 1)) => overlapping_corners(&mut marks, selected, hovered),
				((1, 1), (2, 0)) => straddling_an_edge(&mut marks, selected, hovered),
				((1, 1), (0, 2)) => straddling_an_edge(&mut marks, hovered, selected),
				((2, 0), (1, 1)) => marks.with_axes_swapped(selected, hovered, straddling_an_edge),
				((0, 2), (1, 1)) => marks.with_axes_swapped(hovered, selected, straddling_an_edge),
				((2, 0), (0, 2)) | ((0, 2), (2, 0)) => crossing(&mut marks, selected, hovered),
				((2, 0), (2, 0)) => contained(&mut marks, selected, hovered),
				((0, 2), (0, 2)) => contained(&mut marks, hovered, selected),
				_ => {}
			}
		}
	}

	marks.collected
}

/// The pivots that put a label on a side of its line as that side appears on screen, which a rotated canvas turns.
fn label_pivots(side: Side, document_to_viewport: DAffine2) -> [Pivot; 2] {
	let direction = document_to_viewport.transform_vector2(side.direction());

	match (direction.x.abs() > direction.y.abs(), direction.x < 0., direction.y < 0.) {
		(true, true, _) => [Pivot::End, Pivot::Middle],
		(true, false, _) => [Pivot::Start, Pivot::Middle],
		(false, _, true) => [Pivot::Middle, Pivot::End],
		(false, _, false) => [Pivot::Middle, Pivot::Start],
	}
}

/// Draws dashed outlines of the selected and hovered boxes, given in document space, and the lines that measure the distances between them.
pub fn overlay(selected_bounds: Rect, hovered_bounds: Rect, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let dashed_line = |overlay_context: &mut OverlayContext, from: DVec2, to: DVec2| {
		let [from, to] = [from, to].map(|point| document_to_viewport.transform_point2(point));
		overlay_context.dashed_line(from, to, None, None, Some(2.), Some(2.), Some(0.5));
	};

	for bounds in [selected_bounds, hovered_bounds] {
		let (min, max) = (bounds.min(), bounds.max());
		let corners = [min, DVec2::new(max.x, min.y), max, DVec2::new(min.x, max.y)];
		for (index, &corner) in corners.iter().enumerate() {
			dashed_line(overlay_context, corner, corners[(index + 1) % 4]);
		}
	}

	for mark in measurement_marks(selected_bounds, hovered_bounds) {
		match mark {
			Mark::Guide { from, to } => dashed_line(overlay_context, from, to),
			Mark::Measured { from, to, side } => {
				let [from_viewport, to_viewport] = [from, to].map(|point| document_to_viewport.transform_point2(point));
				overlay_context.line(from_viewport, to_viewport, None, None);

				let length = (to - from).length();
				if length >= LABEL_MIN_LENGTH {
					let label = format!("{length:.2}").trim_end_matches('0').trim_end_matches('.').to_string();
					let midpoint = DAffine2::from_translation((from_viewport + to_viewport) / 2.);
					overlay_context.text(&label, COLOR_OVERLAY_BLUE, None, midpoint, LABEL_PADDING, label_pivots(side, document_to_viewport));
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn rect(min: [f64; 2], max: [f64; 2]) -> Rect {
		Rect::from_box([DVec2::from(min), DVec2::from(max)])
	}

	fn measured(from: [f64; 2], to: [f64; 2], side: Side) -> Mark {
		Mark::Measured {
			from: DVec2::from(from),
			to: DVec2::from(to),
			side,
		}
	}

	fn guide(from: [f64; 2], to: [f64; 2]) -> Mark {
		Mark::Guide {
			from: DVec2::from(from),
			to: DVec2::from(to),
		}
	}

	#[test]
	fn boxes_apart_diagonally_are_measured_across_both_gaps() {
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([20., 30.], [40., 50.]));

		let expected = vec![
			measured([10., 10.], [20., 10.], Side::Top),
			guide([10., 30.], [20., 30.]),
			measured([10., 10.], [10., 30.], Side::Left),
			guide([20., 10.], [20., 30.]),
		];
		assert_eq!(marks, expected);
	}

	#[test]
	fn stacked_boxes_with_neither_center_over_the_other_are_measured_across_the_gap_and_overlap() {
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([8., 20.], [30., 30.]));

		let expected = vec![measured([10., 10.], [8., 10.], Side::Bottom), measured([10., 10.], [10., 20.], Side::Left), guide([8., 10.], [8., 20.])];
		assert_eq!(marks, expected);
	}

	#[test]
	fn stacked_boxes_with_a_center_over_the_other_are_measured_across_the_gap_through_that_center() {
		// The selection's center is over the hovered box
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([2., 20.], [20., 30.]));
		assert_eq!(marks, vec![measured([5., 10.], [5., 20.], Side::Left)]);

		// The selection's center is on the hovered box's edge
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([5., 20.], [30., 30.]));
		assert_eq!(marks, vec![measured([5., 10.], [5., 20.], Side::Left)]);

		// Only the hovered box's center is over the other, so the gap is measured through it instead
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([6., 20.], [8., 30.]));
		assert_eq!(marks, vec![measured([7., 10.], [7., 20.], Side::Left)]);
	}

	#[test]
	fn boxes_side_by_side_are_measured_like_stacked_ones_turned_sideways() {
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([20., 8.], [30., 30.]));
		let expected = vec![measured([10., 10.], [10., 8.], Side::Right), measured([10., 10.], [20., 10.], Side::Top), guide([10., 8.], [20., 8.])];
		assert_eq!(marks, expected);

		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([20., 6.], [30., 8.]));
		assert_eq!(marks, vec![measured([10., 7.], [20., 7.], Side::Top)]);
	}

	#[test]
	fn boxes_overlapping_at_a_corner_are_measured_along_the_outside_of_both() {
		// The hovered box down and to the right
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([6., 6.], [18., 15.]));
		let expected = vec![
			measured([10., 0.], [18., 0.], Side::Top),
			measured([0., 15.], [6., 15.], Side::Bottom),
			measured([18., 0.], [18., 6.], Side::Right),
			measured([0., 10.], [0., 15.], Side::Left),
		];
		assert_eq!(marks, expected);

		// Up and to the right, along the other pair of outer sides
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([6., -6.], [18., 3.]));
		let expected = vec![
			measured([10., 10.], [18., 10.], Side::Bottom),
			measured([0., -6.], [6., -6.], Side::Top),
			measured([0., -6.], [0., 0.], Side::Left),
			measured([18., 3.], [18., 10.], Side::Right),
		];
		assert_eq!(marks, expected);
	}

	#[test]
	fn a_box_straddling_an_edge_of_a_wider_one_is_measured_from_its_sides_and_to_that_edge() {
		// The wider box crosses the selection's top
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([-4., -6.], [14., 4.]));
		let expected = vec![
			measured([-4., 4.], [-4., 10.], Side::Left),
			measured([14., 4.], [14., 10.], Side::Right),
			measured([-4., 10.], [0., 10.], Side::Bottom),
			measured([14., 10.], [10., 10.], Side::Bottom),
		];
		assert_eq!(marks, expected);

		// It crosses the selection's bottom
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([-4., 6.], [14., 16.]));
		let expected = vec![
			measured([-4., 0.], [-4., 6.], Side::Left),
			measured([14., 0.], [14., 6.], Side::Right),
			measured([-4., 0.], [0., 0.], Side::Top),
			measured([14., 0.], [10., 0.], Side::Top),
		];
		assert_eq!(marks, expected);
	}

	#[test]
	fn crossing_boxes_measure_each_pair_of_edges_at_both_outer_sides() {
		let marks = measurement_marks(rect([0., 10.], [100., 20.]), rect([40., 0.], [60., 30.]));

		let expected = vec![
			measured([0., 0.], [0., 10.], Side::Left),
			measured([0., 20.], [0., 30.], Side::Left),
			measured([100., 0.], [100., 10.], Side::Right),
			measured([100., 20.], [100., 30.], Side::Right),
			measured([0., 0.], [40., 0.], Side::Top),
			measured([60., 0.], [100., 0.], Side::Top),
			measured([0., 30.], [40., 30.], Side::Bottom),
			measured([60., 30.], [100., 30.], Side::Bottom),
		];
		assert_eq!(marks, expected);
	}

	#[test]
	fn a_box_within_the_other_is_measured_through_its_own_center() {
		let (outer, inner) = (rect([0., 0.], [100., 100.]), rect([60., 10.], [80., 30.]));
		let expected = vec![
			measured([60., 20.], [0., 20.], Side::Top),
			measured([80., 20.], [100., 20.], Side::Top),
			measured([70., 10.], [70., 0.], Side::Right),
			measured([70., 30.], [70., 100.], Side::Right),
		];

		// Whether it is the hovered box or the selection
		assert_eq!(measurement_marks(outer, inner), expected);
		assert_eq!(measurement_marks(inner, outer), expected);
	}

	#[test]
	fn boxes_sharing_an_edge_measure_only_the_edges_that_differ() {
		// A header across the top of its artboard
		let marks = measurement_marks(rect([0., 0.], [100., 100.]), rect([0., 0.], [100., 20.]));
		assert_eq!(marks, vec![measured([50., 20.], [50., 100.], Side::Right)]);

		// Within the other at a shared corner
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([0., 0.], [20., 20.]));
		assert_eq!(marks, vec![measured([10., 5.], [20., 5.], Side::Top), measured([5., 10.], [5., 20.], Side::Right)]);

		// Just as wide and overlapping partway down
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([0., 5.], [10., 15.]));
		assert_eq!(marks, vec![measured([10., 0.], [10., 5.], Side::Right), measured([0., 10.], [0., 15.], Side::Left)]);

		// Taller and sharing the selection's top while crossing its right side
		let marks = measurement_marks(rect([0., 0.], [10., 10.]), rect([5., 0.], [15., 20.]));
		let expected = vec![
			measured([0., 0.], [5., 0.], Side::Top),
			measured([0., 20.], [5., 20.], Side::Bottom),
			measured([0., 20.], [0., 10.], Side::Left),
		];
		assert_eq!(marks, expected);
	}

	#[test]
	fn identical_boxes_draw_no_measurements() {
		let both = rect([0., 0.], [10., 10.]);
		assert!(measurement_marks(both, both).is_empty());
	}

	#[test]
	fn labels_along_the_outside_of_both_boxes_face_away_from_them() {
		let selected = rect([0., 0.], [10., 10.]);
		// Overlapping at each of the four corners
		let corners = [[6., 6.], [6., -6.], [-6., 6.], [-6., -6.]].map(|[x, y]| rect([x, y], [x + 12., y + 9.]));
		// Wider or taller than the selection, crossing each of its four sides
		let straddled = [rect([-4., -6.], [14., 4.]), rect([-4., 6.], [14., 16.]), rect([-6., -4.], [4., 14.]), rect([6., -4.], [16., 14.])];
		// Narrower or shorter than the selection, crossing each of its four sides
		let straddling = [rect([3., -4.], [7., 4.]), rect([3., 6.], [7., 14.]), rect([-4., 3.], [4., 7.]), rect([6., 3.], [14., 7.])];

		for hovered in corners.into_iter().chain(straddled).chain(straddling) {
			let center = Rect::combine_bounds(selected, hovered).center();

			for mark in measurement_marks(selected, hovered) {
				let Mark::Measured { from, to, side } = mark else { continue };
				let midpoint = (from + to) / 2.;
				assert!(side.direction().dot(midpoint - center) > 0., "{side:?} label of {from} to {to} faces inward for hovered {hovered:?}");
			}
		}
	}

	#[test]
	fn a_label_goes_on_its_side_as_seen_on_a_rotated_canvas() {
		let upright = DAffine2::from_scale(DVec2::splat(2.));
		assert!(matches!(label_pivots(Side::Left, upright), [Pivot::End, Pivot::Middle]));
		assert!(matches!(label_pivots(Side::Right, upright), [Pivot::Start, Pivot::Middle]));
		assert!(matches!(label_pivots(Side::Top, upright), [Pivot::Middle, Pivot::End]));
		assert!(matches!(label_pivots(Side::Bottom, upright), [Pivot::Middle, Pivot::Start]));

		// A quarter turn carries the left side of a vertical line up, so its label goes above the line as it now lies
		let quarter_turn = DAffine2::from_angle(std::f64::consts::FRAC_PI_2);
		assert!(matches!(label_pivots(Side::Left, quarter_turn), [Pivot::Middle, Pivot::End]));
		assert!(matches!(label_pivots(Side::Top, quarter_turn), [Pivot::Start, Pivot::Middle]));
	}
}
