use super::intersection::bezpath_intersections;
use super::poisson_disk::poisson_disk_sample;
use super::util::segment_tangent;
use crate::vector::algorithms::offset_subpath::MAX_ABSOLUTE_DIFFERENCE;
use crate::vector::misc::{PointSpacingType, dvec2_to_point, point_to_dvec2};
use glam::{DMat2, DVec2};
use kurbo::{BezPath, CubicBez, DEFAULT_ACCURACY, Line, ParamCurve, ParamCurveDeriv, PathEl, PathSeg, Point, QuadBez, Rect, Shape};
use std::f64::consts::{FRAC_PI_2, PI};

/// Splits the [`BezPath`] at segment index at `t` value which lie in the range of [0, 1].
/// Returns [`None`] if the given [`BezPath`] has no segments or `t` is within f64::EPSILON of 0 or 1.
pub fn split_bezpath_at_segment(bezpath: &BezPath, segment_index: usize, t: f64) -> Option<(BezPath, BezPath)> {
	if t <= f64::EPSILON || (1. - t) <= f64::EPSILON || bezpath.segments().count() == 0 {
		return None;
	}

	// Get the segment which lies at the split.
	let segment = bezpath.get_seg(segment_index + 1).unwrap();

	// Divide the segment.
	let first_segment = segment.subsegment(0.0..t);
	let second_segment = segment.subsegment(t..1.);

	let mut first_bezpath = BezPath::new();
	let mut second_bezpath = BezPath::new();

	// Append the segments up to the subdividing segment from original bezpath to first bezpath.
	for segment in bezpath.segments().take(segment_index) {
		if first_bezpath.elements().is_empty() {
			first_bezpath.move_to(segment.start());
		}
		first_bezpath.push(segment.as_path_el());
	}

	// Append the first segment of the subdivided segment.
	if first_bezpath.elements().is_empty() {
		first_bezpath.move_to(first_segment.start());
	}
	first_bezpath.push(first_segment.as_path_el());

	// Append the second segment of the subdivided segment in the second bezpath.
	if second_bezpath.elements().is_empty() {
		second_bezpath.move_to(second_segment.start());
	}
	second_bezpath.push(second_segment.as_path_el());

	// Append the segments after the subdividing segment from original bezpath to second bezpath.
	for segment in bezpath.segments().skip(segment_index + 1) {
		if second_bezpath.elements().is_empty() {
			second_bezpath.move_to(segment.start());
		}
		second_bezpath.push(segment.as_path_el());
	}

	Some((first_bezpath, second_bezpath))
}

/// Splits the [`BezPath`] at a `t` value which lies in the range of [0, 1].
/// Returns [`None`] if the given [`BezPath`] has no segments.
pub fn split_bezpath(bezpath: &BezPath, t_value: TValue) -> Option<(BezPath, BezPath)> {
	if bezpath.segments().count() == 0 {
		return None;
	}

	// Get the segment which lies at the split.
	let (segment_index, t) = eval_bezpath(bezpath, t_value, None);
	split_bezpath_at_segment(bezpath, segment_index, t)
}

pub fn evaluate_bezpath(bezpath: &BezPath, t_value: TValue, segments_length: Option<&[f64]>) -> Point {
	let (segment_index, t) = eval_bezpath(bezpath, t_value, segments_length);
	bezpath.get_seg(segment_index + 1).unwrap().eval(t)
}

pub fn tangent_on_bezpath(bezpath: &BezPath, t_value: TValue, segments_length: Option<&[f64]>) -> Point {
	let (segment_index, t) = eval_bezpath(bezpath, t_value, segments_length);
	let segment = bezpath.get_seg(segment_index + 1).unwrap();

	match segment {
		PathSeg::Line(line) => line.deriv().eval(t),
		PathSeg::Quad(quad_bez) => quad_bez.deriv().eval(t),
		PathSeg::Cubic(cubic_bez) => cubic_bez.deriv().eval(t),
	}
}

pub fn sample_polyline_on_bezpath(
	bezpath: BezPath,
	point_spacing_type: PointSpacingType,
	amount: f64,
	start_offset: f64,
	stop_offset: f64,
	adaptive_spacing: bool,
	segments_length: &[f64],
) -> Option<BezPath> {
	let mut sample_bezpath = BezPath::new();

	let was_closed = matches!(bezpath.elements().last(), Some(PathEl::ClosePath));

	// Calculate the total length of the collected segments.
	let total_length: f64 = segments_length.iter().sum();

	// Adjust the usable length by subtracting start and stop offsets.
	let mut used_length = total_length - start_offset - stop_offset;

	// Sanity check that the usable length is positive.
	if used_length <= 0. {
		return None;
	}

	const SAFETY_MAX_COUNT: f64 = 10_000. - 1.;

	// Determine the number of points to generate along the path.
	let sample_count = match point_spacing_type {
		PointSpacingType::Separation => {
			let spacing = amount.min(used_length - f64::EPSILON);

			if adaptive_spacing {
				// Calculate point count to evenly distribute points while covering the entire path.
				// With adaptive spacing, we widen or narrow the points as necessary to ensure the last point is always at the end of the path.
				(used_length / spacing).round().min(SAFETY_MAX_COUNT)
			} else {
				// Calculate point count based on exact spacing, which may not cover the entire path.
				// Without adaptive spacing, we just evenly space the points at the exact specified spacing, usually falling short before the end of the path.
				let count = (used_length / spacing + f64::EPSILON).floor().min(SAFETY_MAX_COUNT);
				if count != SAFETY_MAX_COUNT {
					used_length -= used_length % spacing;
				}
				count
			}
		}
		PointSpacingType::Quantity => (amount - 1.).floor().clamp(1., SAFETY_MAX_COUNT),
	};

	// Skip if there are no points to generate.
	if sample_count < 1. {
		return None;
	}

	// Decide how many loop-iterations: if closed, skip the last duplicate point
	let sample_count_usize = sample_count as usize;
	let max_i = if was_closed { sample_count_usize } else { sample_count_usize + 1 };

	// Generate points along the path based on calculated intervals.
	let mut length_up_to_previous_segment = 0.;
	let mut next_segment_index = 0;

	for count in 0..max_i {
		let fraction = count as f64 / sample_count;
		let length_up_to_next_sample_point = fraction * used_length + start_offset;
		let mut next_length = length_up_to_next_sample_point - length_up_to_previous_segment;
		let mut next_segment_length = segments_length[next_segment_index];

		// Keep moving to the next segment while the length up to the next sample point is greater than the length up to the current segment.
		while next_length > next_segment_length {
			if next_segment_index == segments_length.len() - 1 {
				break;
			}
			length_up_to_previous_segment += next_segment_length;
			next_length = length_up_to_next_sample_point - length_up_to_previous_segment;
			next_segment_index += 1;
			next_segment_length = segments_length[next_segment_index];
		}

		let t = (next_length / next_segment_length).clamp(0., 1.);

		let segment = bezpath.get_seg(next_segment_index + 1).unwrap();
		let t = eval_pathseg_euclidean(segment, t, DEFAULT_ACCURACY);
		let point = segment.eval(t);

		if sample_bezpath.elements().is_empty() {
			sample_bezpath.move_to(point)
		} else {
			sample_bezpath.line_to(point)
		}
	}

	if was_closed {
		sample_bezpath.close_path();
	}

	Some(sample_bezpath)
}

#[derive(Debug, Clone, Copy)]
pub enum TValue {
	Parametric(f64),
	Euclidean(f64),
}

/// Return the subsegment for the given [TValue] range. Returns None if parametric value of `t1` is greater than `t2`.
pub fn trim_pathseg(segment: PathSeg, t1: TValue, t2: TValue) -> Option<PathSeg> {
	let t1 = eval_pathseg(segment, t1);
	let t2 = eval_pathseg(segment, t2);

	if t1 > t2 { None } else { Some(segment.subsegment(t1..t2)) }
}

pub fn eval_pathseg(segment: PathSeg, t_value: TValue) -> f64 {
	match t_value {
		TValue::Parametric(t) => t,
		TValue::Euclidean(t) => eval_pathseg_euclidean(segment, t, DEFAULT_ACCURACY),
	}
}

/// Finds the t value of point on the given path segment i.e fractional distance along the segment's total length.
/// It uses a binary search to find the value `t` such that the ratio `length_up_to_t / total_length` approximates the input `distance`.
pub fn eval_pathseg_euclidean(segment: PathSeg, distance: f64, accuracy: f64) -> f64 {
	let mut low_t = 0.;
	let mut mid_t = 0.5;
	let mut high_t = 1.;

	let total_length = segment.perimeter(accuracy);

	if !total_length.is_finite() || total_length <= f64::EPSILON {
		return 0.;
	}

	let distance = distance.clamp(0., 1.);

	while high_t - low_t > accuracy {
		let current_length = segment.subsegment(0.0..mid_t).perimeter(accuracy);
		let current_distance = current_length / total_length;

		if current_distance > distance {
			high_t = mid_t;
		} else {
			low_t = mid_t;
		}
		mid_t = (high_t + low_t) / 2.;
	}

	mid_t
}

/// Converts from a bezpath (composed of multiple segments) to a point along a certain segment represented.
/// The returned tuple represents the segment index and the `t` value along that segment.
/// Both the input global `t` value and the output `t` value are in euclidean space, meaning there is a constant rate of change along the arc length.
fn eval_bazpath_to_euclidean(bezpath: &BezPath, global_t: f64, lengths: &[f64], total_length: f64) -> (usize, f64) {
	let mut accumulator = 0.;
	for (index, length) in lengths.iter().enumerate() {
		let length_ratio = length / total_length;
		if (index == 0 || accumulator <= global_t) && global_t <= accumulator + length_ratio {
			return (index, ((global_t - accumulator) / length_ratio).clamp(0., 1.));
		}
		accumulator += length_ratio;
	}
	(bezpath.segments().count() - 1, 1.)
}

/// Convert a [TValue] to a parametric `(segment_index, t)` tuple.
/// - Asserts that `t` values contained within the `TValue` argument lie in the range [0, 1].
fn eval_bezpath(bezpath: &BezPath, t: TValue, precomputed_segments_length: Option<&[f64]>) -> (usize, f64) {
	let segment_count = bezpath.segments().count();
	assert!(segment_count >= 1);

	match t {
		TValue::Euclidean(t) => {
			let computed_segments_length;

			let segments_length = if let Some(segments_length) = precomputed_segments_length {
				segments_length
			} else {
				computed_segments_length = bezpath.segments().map(|segment| segment.perimeter(DEFAULT_ACCURACY)).collect::<Vec<f64>>();
				computed_segments_length.as_slice()
			};

			let total_length = segments_length.iter().sum();

			let (segment_index, t) = eval_bazpath_to_euclidean(bezpath, t, segments_length, total_length);
			let segment = bezpath.get_seg(segment_index + 1).unwrap();
			(segment_index, eval_pathseg_euclidean(segment, t, DEFAULT_ACCURACY))
		}
		TValue::Parametric(t) => {
			assert!((0.0..=1.).contains(&t));

			if t == 1. {
				return (segment_count - 1, 1.);
			}

			let scaled_t = t * segment_count as f64;
			let segment_index = scaled_t.floor() as usize;
			let t = scaled_t - segment_index as f64;

			(segment_index, t)
		}
	}
}

/// Randomly places points across the filled surface of this subpath (which is assumed to be closed).
/// The `separation_disk_diameter` determines the minimum distance between all points from one another.
/// Conceptually, this works by "throwing a dart" at the subpath's bounding box and keeping the dart only if:
/// - It's inside the shape
/// - It's not closer than `separation_disk_diameter` to any other point from a previous accepted dart throw
///
/// This repeats until accepted darts fill all possible areas between one another.
///
/// While the conceptual process described above asymptotically slows down and is never guaranteed to produce a maximal set in finite time,
/// this is implemented with an algorithm that produces a maximal set in O(n) time. The slowest part is actually checking if points are inside the subpath shape.
pub fn poisson_disk_points(bezpath_index: usize, bezpaths: &[(BezPath, Rect)], separation_disk_diameter: f64, rng: impl FnMut() -> f64) -> Vec<DVec2> {
	let (this_bezpath, this_bbox) = bezpaths[bezpath_index].clone();

	if this_bezpath.elements().is_empty() {
		return Vec::new();
	}

	let point_in_shape_checker = |point: DVec2| {
		// Check against all paths the point is contained in to compute the correct winding number
		let mut number = 0;

		for (i, (shape, bbox)) in bezpaths.iter().enumerate() {
			if bbox.x0 > point.x || bbox.y0 > point.y || bbox.x1 < point.x || bbox.y1 < point.y {
				continue;
			}

			let winding = shape.winding(dvec2_to_point(point));
			if winding == 0 && i == bezpath_index {
				return false;
			}
			number += winding;
		}

		// Non-zero fill rule
		number != 0
	};

	let line_intersect_shape_checker = |p0: (f64, f64), p1: (f64, f64)| {
		for segment in this_bezpath.segments() {
			if !segment.intersect_line(Line::new(p0, p1)).is_empty() {
				return true;
			}
		}

		false
	};

	let offset = DVec2::new(this_bbox.x0, this_bbox.y0);
	let width = this_bbox.width();
	let height = this_bbox.height();

	poisson_disk_sample(offset, width, height, separation_disk_diameter, point_in_shape_checker, line_intersect_shape_checker, rng)
}

/// Returns true if the Bezier curve is equivalent to a line.
///
/// **NOTE**: This is different from simply checking if the segment is [`PathSeg::Line`] or [`PathSeg::Quad`] or [`PathSeg::Cubic`]. Bezier curve can also be a line if the control points are colinear to the start and end points. Therefore if the handles exceed the start and end point, it will still be considered as a line.
pub fn is_linear(segment: &PathSeg) -> bool {
	let is_colinear = |a: Point, b: Point, c: Point| -> bool { ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() < MAX_ABSOLUTE_DIFFERENCE };

	match *segment {
		PathSeg::Line(_) => true,
		PathSeg::Quad(QuadBez { p0, p1, p2 }) => is_colinear(p0, p1, p2),
		PathSeg::Cubic(CubicBez { p0, p1, p2, p3 }) => is_colinear(p0, p1, p3) && is_colinear(p0, p2, p3),
	}
}

// TODO: If a segment curls back on itself tightly enough it could intersect again at the portion that should be trimmed. This could cause the Subpaths to be clipped
// TODO: at the incorrect location. This can be avoided by first trimming the two Subpaths at any extrema, effectively ignoring loopbacks.
/// Helper function to clip overlap of two intersecting open BezPaths. Returns an Option because intersections may not exist for certain arrangements and distances.
/// Assumes that the BezPaths represents simple Bezier segments, and clips the BezPaths at the last intersection of the first BezPath, and first intersection of the last BezPath.
pub fn clip_simple_bezpaths(bezpath1: &BezPath, bezpath2: &BezPath) -> Option<(BezPath, BezPath)> {
	// Split the first subpath at its last intersection
	let subpath_1_intersections = bezpath_intersections(bezpath1, bezpath2, None, None);
	if subpath_1_intersections.is_empty() {
		return None;
	}
	let (segment_index, t) = *subpath_1_intersections.last()?;
	let (clipped_subpath1, _) = split_bezpath_at_segment(bezpath1, segment_index, t)?;

	// Split the second subpath at its first intersection
	let subpath_2_intersections = bezpath_intersections(bezpath2, bezpath1, None, None);
	if subpath_2_intersections.is_empty() {
		return None;
	}
	let (segment_index, t) = subpath_2_intersections[0];
	let (_, clipped_subpath2) = split_bezpath_at_segment(bezpath2, segment_index, t)?;

	Some((clipped_subpath1, clipped_subpath2))
}

/// Returns the [`PathEl`] that is needed for a miter join if it is possible.
///
/// `miter_limit` defines a limit for the ratio between the miter length and the stroke width.
/// Alternatively, this can be interpreted as limiting the angle that the miter can form.
/// When the limit is exceeded, no [`PathEl`] will be returned.
/// This value should be greater than 0. If not, the default of 4 will be used.
pub fn miter_line_join(bezpath1: &BezPath, bezpath2: &BezPath, miter_limit: Option<f64>) -> Option<[PathEl; 2]> {
	let miter_limit = match miter_limit {
		Some(miter_limit) if miter_limit > f64::EPSILON => miter_limit,
		_ => 4.,
	};
	// TODO: Besides returning None using the `?` operator, is there a more appropriate way to handle a `None` result from `get_segment`?
	let in_segment = bezpath1.segments().last()?;
	let out_segment = bezpath2.segments().next()?;

	let in_tangent = segment_tangent(in_segment, 1.);
	let out_tangent = segment_tangent(out_segment, 0.);

	if in_tangent == DVec2::ZERO || out_tangent == DVec2::ZERO {
		// Avoid panic from normalizing zero vectors
		// TODO: Besides returning None, is there a more appropriate way to handle this?
		return None;
	}

	let angle = (in_tangent * -1.).angle_to(out_tangent).abs();

	if angle.to_degrees() < miter_limit {
		return None;
	}

	let p1 = in_segment.end();
	let p2 = point_to_dvec2(p1) + in_tangent.normalize();
	let line1 = Line::new(p1, dvec2_to_point(p2));

	let p1 = out_segment.start();
	let p2 = point_to_dvec2(p1) + out_tangent.normalize();
	let line2 = Line::new(p1, dvec2_to_point(p2));

	// If we don't find the intersection point to draw the miter join, we instead default to a bevel join.
	// Otherwise, we return the element to create the join.
	let intersection = line1.crossing_point(line2)?;

	Some([PathEl::LineTo(intersection), PathEl::LineTo(out_segment.start())])
}

/// Computes the [`PathEl`] to form a circular join from `left` to `right`, along a circle around `center`.
/// By default, the angle is assumed to be 180 degrees.
pub fn compute_circular_subpath_details(left: DVec2, arc_point: DVec2, right: DVec2, center: DVec2, angle: Option<f64>) -> [PathEl; 2] {
	let center_to_arc_point = arc_point - center;

	// Based on https://pomax.github.io/bezierinfo/#circles_cubic
	let handle_offset_factor = if let Some(angle) = angle { 4. / 3. * (angle / 4.).tan() } else { 0.551784777779014 };

	let p1 = dvec2_to_point(left - (left - center).perp() * handle_offset_factor);
	let p2 = dvec2_to_point(arc_point + center_to_arc_point.perp() * handle_offset_factor);
	let p3 = dvec2_to_point(arc_point);

	let first_half = PathEl::CurveTo(p1, p2, p3);

	let p1 = dvec2_to_point(arc_point - center_to_arc_point.perp() * handle_offset_factor);
	let p2 = dvec2_to_point(right + (right - center).perp() * handle_offset_factor);
	let p3 = dvec2_to_point(right);

	let second_half = PathEl::CurveTo(p1, p2, p3);

	[first_half, second_half]
}

/// Returns two [`PathEl`] to create a round join with the provided center.
pub fn round_line_join(bezpath1: &BezPath, bezpath2: &BezPath, center: DVec2) -> [PathEl; 2] {
	let left = point_to_dvec2(bezpath1.segments().last().unwrap().end());
	let right = point_to_dvec2(bezpath2.segments().next().unwrap().start());

	let center_to_right = right - center;
	let center_to_left = left - center;

	let in_segment = bezpath1.segments().last();
	let in_tangent = in_segment.map(|in_segment| segment_tangent(in_segment, 1.));

	let mut angle = center_to_right.angle_to(center_to_left) / 2.;
	let mut arc_point = center + DMat2::from_angle(angle).mul_vec2(center_to_right);

	if in_tangent.map(|in_tangent| (arc_point - left).angle_to(in_tangent).abs()).unwrap_or_default() > FRAC_PI_2 {
		angle = angle - PI * (if angle < 0. { -1. } else { 1. });
		arc_point = center + DMat2::from_angle(angle).mul_vec2(center_to_right);
	}

	compute_circular_subpath_details(left, arc_point, right, center, Some(angle))
}
