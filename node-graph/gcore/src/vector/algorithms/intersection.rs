use super::contants::MIN_SEPARATION_VALUE;
use kurbo::{BezPath, DEFAULT_ACCURACY, ParamCurve, PathSeg, Shape};

/// Calculates the intersection points the bezpath has with a given segment and returns a list of `(usize, f64)` tuples,
/// where the `usize` represents the index of the segment in the bezpath, and the `f64` represents the `t`-value local to
/// that segment where the intersection occurred.
///
/// `minimum_separation` is the minimum difference that two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
pub fn bezpath_and_segment_intersections(bezpath: &BezPath, segment: PathSeg, accuracy: Option<f64>, minimum_separation: Option<f64>) -> Vec<(usize, f64)> {
	bezpath
		.segments()
		.enumerate()
		.flat_map(|(index, this_segment)| {
			filtered_segment_intersections(this_segment, segment, accuracy, minimum_separation)
				.into_iter()
				.map(|t| (index, t))
				.collect::<Vec<(usize, f64)>>()
		})
		.collect()
}

/// Calculates the intersection points the bezpath has with another given bezpath and returns a list of parametric `t`-values.
pub fn bezpath_intersections(bezpath1: &BezPath, bezpath2: &BezPath, accuracy: Option<f64>, minimum_separation: Option<f64>) -> Vec<(usize, f64)> {
	let mut intersection_t_values: Vec<(usize, f64)> = bezpath2
		.segments()
		.flat_map(|bezier| bezpath_and_segment_intersections(bezpath1, bezier, accuracy, minimum_separation))
		.collect();

	intersection_t_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
	intersection_t_values
}

/// Calculates the intersection points the segment has with another given segment and returns a list of parametric `t`-values with given accuracy.
pub fn segment_intersections(segment1: PathSeg, segment2: PathSeg, accuracy: Option<f64>) -> Vec<(f64, f64)> {
	let accuracy = accuracy.unwrap_or(DEFAULT_ACCURACY);

	match (segment1, segment2) {
		(PathSeg::Line(line), segment2) => segment2.intersect_line(line).iter().map(|i| (i.line_t, i.segment_t)).collect(),
		(segment1, PathSeg::Line(line)) => segment1.intersect_line(line).iter().map(|i| (i.segment_t, i.line_t)).collect(),
		(segment1, segment2) => {
			let mut intersections = Vec::new();
			segment_intersections_inner(segment1, 0., 1., segment2, 0., 1., accuracy, &mut intersections);
			intersections
		}
	}
}

/// Implements [https://pomax.github.io/bezierinfo/#curveintersection] to find intersection between two Bezier segments
/// by splitting the segment recursively until the size of the subsegment's bounding box is smaller than the accuracy.
#[allow(clippy::too_many_arguments)]
fn segment_intersections_inner(segment1: PathSeg, min_t1: f64, max_t1: f64, segment2: PathSeg, min_t2: f64, max_t2: f64, accuracy: f64, intersections: &mut Vec<(f64, f64)>) {
	let bbox1 = segment1.bounding_box();
	let bbox2 = segment2.bounding_box();

	let mid_t1 = (min_t1 + max_t1) / 2.;
	let mid_t2 = (min_t2 + max_t2) / 2.;

	// Check if the bounding boxes overlap
	if bbox1.overlaps(bbox2) {
		// If bounding boxes overlap and they are small enough, we have found an intersection
		if bbox1.width() < accuracy && bbox1.height() < accuracy && bbox2.width() < accuracy && bbox2.height() < accuracy {
			// Use the middle `t` value, append the corresponding `t` value
			intersections.push((mid_t1, mid_t2));
			return;
		}

		// Split curves in half
		let (seg11, seg12) = segment1.subdivide();
		let (seg21, seg22) = segment2.subdivide();

		// Repeat checking the intersection with the combinations of the two halves of each curve
		segment_intersections_inner(seg11, min_t1, mid_t1, seg21, min_t2, mid_t2, accuracy, intersections);
		segment_intersections_inner(seg11, min_t1, mid_t1, seg22, mid_t2, max_t2, accuracy, intersections);
		segment_intersections_inner(seg12, mid_t1, max_t1, seg21, min_t2, mid_t2, accuracy, intersections);
		segment_intersections_inner(seg12, mid_t1, max_t1, seg22, mid_t2, max_t2, accuracy, intersections);
	}
}

// TODO: Use an `impl Iterator` return type instead of a `Vec`
/// Returns a list of filtered parametric `t` values that correspond to intersection points between the current bezier segment and the provided one
/// such that the difference between adjacent `t` values in sorted order is greater than some minimum separation value. If the difference
/// between 2 adjacent `t` values is less than the minimum difference, the filtering takes the larger `t` value and discards the smaller `t` value.
/// The returned `t` values are with respect to the current bezier segment, not the provided parameter.
/// If the provided segment is linear, then zero intersection points will be returned along colinear segments.
///
/// `accuracy` defines, for intersections where the provided bezier segment is non-linear, the maximum size of the bounding boxes to be considered an intersection point.
///
/// `minimum_separation` is the minimum difference between adjacent `t` values in sorted order.
pub fn filtered_segment_intersections(segment1: PathSeg, segment2: PathSeg, accuracy: Option<f64>, minimum_separation: Option<f64>) -> Vec<f64> {
	let mut intersection_t_values = segment_intersections(segment1, segment2, accuracy);
	intersection_t_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

	intersection_t_values.iter().map(|x| x.0).fold(Vec::new(), |mut accumulator, t| {
		if !accumulator.is_empty() && (accumulator.last().unwrap() - t).abs() < minimum_separation.unwrap_or(MIN_SEPARATION_VALUE) {
			accumulator.pop();
		}
		accumulator.push(t);
		accumulator
	})
}

// TODO: Use an `impl Iterator` return type instead of a `Vec`
/// Returns a list of pairs of filtered parametric `t` values that correspond to intersection points between the current bezier curve and the provided
/// one such that the difference between adjacent `t` values in sorted order is greater than some minimum separation value. If the difference between
/// two adjacent `t` values is less than the minimum difference, the filtering takes the larger `t` value and discards the smaller `t` value.
/// The first value in pair is with respect to the current bezier and the second value in pair is with respect to the provided parameter.
/// If the provided curve is linear, then zero intersection points will be returned along colinear segments.
///
/// `error`, for intersections where the provided bezier is non-linear, defines the threshold for bounding boxes to be considered an intersection point.
///
/// `minimum_separation` is the minimum difference between adjacent `t` values in sorted order
pub fn filtered_all_segment_intersections(segment1: PathSeg, segment2: PathSeg, accuracy: Option<f64>, minimum_separation: Option<f64>) -> Vec<(f64, f64)> {
	let mut intersection_t_values = segment_intersections(segment1, segment2, accuracy);
	intersection_t_values.sort_by(|a, b| (a.0 + a.1).partial_cmp(&(b.0 + b.1)).unwrap());

	intersection_t_values.iter().fold(Vec::new(), |mut accumulator, t| {
		if !accumulator.is_empty()
			&& (accumulator.last().unwrap().0 - t.0).abs() < minimum_separation.unwrap_or(MIN_SEPARATION_VALUE)
			&& (accumulator.last().unwrap().1 - t.1).abs() < minimum_separation.unwrap_or(MIN_SEPARATION_VALUE)
		{
			accumulator.pop();
		}
		accumulator.push(*t);
		accumulator
	})
}

#[cfg(test)]
mod tests {
	use super::{bezpath_and_segment_intersections, filtered_segment_intersections};
	use crate::vector::algorithms::{
		contants::MAX_ABSOLUTE_DIFFERENCE,
		util::{compare_points, compare_vec_of_points, dvec2_compare},
	};

	use kurbo::{BezPath, CubicBez, Line, ParamCurve, PathEl, PathSeg, Point, QuadBez};

	#[test]
	fn test_intersect_line_segment_quadratic() {
		let p1 = Point::new(30., 50.);
		let p2 = Point::new(140., 30.);
		let p3 = Point::new(160., 170.);

		// Intersection at edge of curve
		let bezier = PathSeg::Quad(QuadBez::new(p1, p2, p3));
		let line1 = PathSeg::Line(Line::new(Point::new(20., 50.), Point::new(40., 50.)));
		let intersections1 = filtered_segment_intersections(bezier, line1, None, None);
		assert!(intersections1.len() == 1);
		assert!(compare_points(bezier.eval(intersections1[0]), p1));

		// Intersection in the middle of curve
		let line2 = PathSeg::Line(Line::new(Point::new(150., 150.), Point::new(30., 30.)));
		let intersections2 = filtered_segment_intersections(bezier, line2, None, None);
		assert!(compare_points(bezier.eval(intersections2[0]), Point::new(47.77355, 47.77354)));
	}

	#[test]
	fn test_intersect_curve_cubic_edge_case() {
		// M34 107 C40 40 120 120 102 29

		let p1 = Point::new(34., 107.);
		let p2 = Point::new(40., 40.);
		let p3 = Point::new(120., 120.);
		let p4 = Point::new(102., 29.);
		let cubic_segment = PathSeg::Cubic(CubicBez::new(p1, p2, p3, p4));

		let linear_segment = PathSeg::Line(Line::new(Point::new(150., 150.), Point::new(20., 20.)));
		let intersections = filtered_segment_intersections(cubic_segment, linear_segment, None, None);

		assert_eq!(intersections.len(), 1);
	}

	#[test]
	fn test_intersect_curve() {
		let p0 = Point::new(30., 30.);
		let p1 = Point::new(60., 140.);
		let p2 = Point::new(150., 30.);
		let p3 = Point::new(160., 160.);

		let cubic_segment = PathSeg::Cubic(CubicBez::new(p0, p1, p2, p3));

		let p0 = Point::new(175., 140.);
		let p1 = Point::new(20., 20.);
		let p2 = Point::new(120., 20.);

		let quadratic_segment = PathSeg::Quad(QuadBez::new(p0, p1, p2));

		let intersections1 = filtered_segment_intersections(cubic_segment, quadratic_segment, None, None);
		let intersections2 = filtered_segment_intersections(quadratic_segment, cubic_segment, None, None);

		let intersections1_points: Vec<Point> = intersections1.iter().map(|&t| cubic_segment.eval(t)).collect();
		let intersections2_points: Vec<Point> = intersections2.iter().map(|&t| quadratic_segment.eval(t)).rev().collect();

		assert!(compare_vec_of_points(intersections1_points, intersections2_points, 2.));
	}

	#[test]
	fn intersection_linear_multiple_subpath_curves_test_one() {
		// M 35 125 C 40 40 120 120 43 43 Q 175 90 145 150 Q 70 185 35 125 Z

		let cubic_start = Point::new(35., 125.);
		let cubic_handle_1 = Point::new(40., 40.);
		let cubic_handle_2 = Point::new(120., 120.);
		let cubic_end = Point::new(43., 43.);

		let quadratic_1_handle = Point::new(175., 90.);
		let quadratic_end = Point::new(145., 150.);

		let quadratic_2_handle = Point::new(70., 185.);

		let cubic_segment = PathSeg::Cubic(CubicBez::new(cubic_start, cubic_handle_1, cubic_handle_2, cubic_end));
		let quadratic_segment = PathSeg::Quad(QuadBez::new(cubic_end, quadratic_1_handle, quadratic_end));

		let bezpath = BezPath::from_vec(vec![
			PathEl::MoveTo(cubic_start),
			PathEl::CurveTo(cubic_handle_1, cubic_handle_2, cubic_end),
			PathEl::QuadTo(quadratic_1_handle, quadratic_end),
			PathEl::QuadTo(quadratic_2_handle, cubic_start),
			PathEl::ClosePath,
		]);

		let linear_segment = PathSeg::Line(Line::new(Point::new(150., 150.), Point::new(20., 20.)));

		let cubic_intersections = filtered_segment_intersections(cubic_segment, linear_segment, None, None);
		let quadratic_1_intersections = filtered_segment_intersections(quadratic_segment, linear_segment, None, None);
		let bezpath_intersections = bezpath_and_segment_intersections(&bezpath, linear_segment, None, None);

		assert!(
			dvec2_compare(
				cubic_segment.eval(cubic_intersections[0]),
				bezpath.segments().nth(bezpath_intersections[0].0).unwrap().eval(bezpath_intersections[0].1),
				MAX_ABSOLUTE_DIFFERENCE
			)
			.all()
		);

		assert!(
			dvec2_compare(
				quadratic_segment.eval(quadratic_1_intersections[0]),
				bezpath.segments().nth(bezpath_intersections[1].0).unwrap().eval(bezpath_intersections[1].1),
				MAX_ABSOLUTE_DIFFERENCE
			)
			.all()
		);

		assert!(
			dvec2_compare(
				quadratic_segment.eval(quadratic_1_intersections[1]),
				bezpath.segments().nth(bezpath_intersections[2].0).unwrap().eval(bezpath_intersections[2].1),
				MAX_ABSOLUTE_DIFFERENCE
			)
			.all()
		);
	}

	#[test]
	fn intersection_linear_multiple_subpath_curves_test_two() {
		// M34 107 C40 40 120 120 102 29 Q175 90 129 171 Q70 185 34 107 Z
		// M150 150 L 20 20

		let cubic_start = Point::new(34., 107.);
		let cubic_handle_1 = Point::new(40., 40.);
		let cubic_handle_2 = Point::new(120., 120.);
		let cubic_end = Point::new(102., 29.);

		let quadratic_1_handle = Point::new(175., 90.);
		let quadratic_end = Point::new(129., 171.);

		let quadratic_2_handle = Point::new(70., 185.);

		let cubic_segment = PathSeg::Cubic(CubicBez::new(cubic_start, cubic_handle_1, cubic_handle_2, cubic_end));
		let quadratic_segment = PathSeg::Quad(QuadBez::new(cubic_end, quadratic_1_handle, quadratic_end));

		let bezpath = BezPath::from_vec(vec![
			PathEl::MoveTo(cubic_start),
			PathEl::CurveTo(cubic_handle_1, cubic_handle_2, cubic_end),
			PathEl::QuadTo(quadratic_1_handle, quadratic_end),
			PathEl::QuadTo(quadratic_2_handle, cubic_start),
			PathEl::ClosePath,
		]);

		let line = PathSeg::Line(Line::new(Point::new(150., 150.), Point::new(20., 20.)));

		let cubic_intersections = filtered_segment_intersections(cubic_segment, line, None, None);
		let quadratic_1_intersections = filtered_segment_intersections(quadratic_segment, line, None, None);
		let bezpath_intersections = bezpath_and_segment_intersections(&bezpath, line, None, None);

		assert!(
			dvec2_compare(
				cubic_segment.eval(cubic_intersections[0]),
				bezpath.segments().nth(bezpath_intersections[0].0).unwrap().eval(bezpath_intersections[0].1),
				MAX_ABSOLUTE_DIFFERENCE
			)
			.all()
		);

		assert!(
			dvec2_compare(
				quadratic_segment.eval(quadratic_1_intersections[0]),
				bezpath.segments().nth(bezpath_intersections[1].0).unwrap().eval(bezpath_intersections[1].1),
				MAX_ABSOLUTE_DIFFERENCE
			)
			.all()
		);
	}

	#[test]
	fn intersection_linear_multiple_subpath_curves_test_three() {
		// M35 125 C40 40 120 120 44 44 Q175 90 145 150 Q70 185 35 125 Z

		let cubic_start = Point::new(35., 125.);
		let cubic_handle_1 = Point::new(40., 40.);
		let cubic_handle_2 = Point::new(120., 120.);
		let cubic_end = Point::new(44., 44.);

		let quadratic_1_handle = Point::new(175., 90.);
		let quadratic_end = Point::new(145., 150.);

		let quadratic_2_handle = Point::new(70., 185.);

		let cubic_segment = PathSeg::Cubic(CubicBez::new(cubic_start, cubic_handle_1, cubic_handle_2, cubic_end));
		let quadratic_segment = PathSeg::Quad(QuadBez::new(cubic_end, quadratic_1_handle, quadratic_end));

		let bezpath = BezPath::from_vec(vec![
			PathEl::MoveTo(cubic_start),
			PathEl::CurveTo(cubic_handle_1, cubic_handle_2, cubic_end),
			PathEl::QuadTo(quadratic_1_handle, quadratic_end),
			PathEl::QuadTo(quadratic_2_handle, cubic_start),
			PathEl::ClosePath,
		]);

		let line = PathSeg::Line(Line::new(Point::new(150., 150.), Point::new(20., 20.)));

		let cubic_intersections = filtered_segment_intersections(cubic_segment, line, None, None);
		let quadratic_1_intersections = filtered_segment_intersections(quadratic_segment, line, None, None);
		let bezpath_intersections = bezpath_and_segment_intersections(&bezpath, line, None, None);

		assert!(
			dvec2_compare(
				cubic_segment.eval(cubic_intersections[0]),
				bezpath.segments().nth(bezpath_intersections[0].0).unwrap().eval(bezpath_intersections[0].1),
				MAX_ABSOLUTE_DIFFERENCE
			)
			.all()
		);

		assert!(
			dvec2_compare(
				quadratic_segment.eval(quadratic_1_intersections[0]),
				bezpath.segments().nth(bezpath_intersections[1].0).unwrap().eval(bezpath_intersections[1].1),
				MAX_ABSOLUTE_DIFFERENCE
			)
			.all()
		);

		assert!(
			dvec2_compare(
				quadratic_segment.eval(quadratic_1_intersections[1]),
				bezpath.segments().nth(bezpath_intersections[2].0).unwrap().eval(bezpath_intersections[2].1),
				MAX_ABSOLUTE_DIFFERENCE
			)
			.all()
		);
	}
}
