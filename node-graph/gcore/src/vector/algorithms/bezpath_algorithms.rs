use super::poisson_disk::poisson_disk_sample;
use crate::vector::misc::dvec2_to_point;
use glam::DVec2;
use kurbo::{Affine, BezPath, Line, ParamCurve, ParamCurveDeriv, PathSeg, Point, Rect, Shape};

/// Accuracy to find the position on [kurbo::Bezpath].
const POSITION_ACCURACY: f64 = 1e-5;
/// Accuracy to find the length of the [kurbo::PathSeg].
pub const PERIMETER_ACCURACY: f64 = 1e-5;

pub fn position_on_bezpath(bezpath: &BezPath, t: f64, euclidian: bool, segments_length: Option<&[f64]>) -> Point {
	let (segment_index, t) = t_value_to_parametric(bezpath, t, euclidian, segments_length);
	bezpath.get_seg(segment_index + 1).unwrap().eval(t)
}

pub fn tangent_on_bezpath(bezpath: &BezPath, t: f64, euclidian: bool, segments_length: Option<&[f64]>) -> Point {
	let (segment_index, t) = t_value_to_parametric(bezpath, t, euclidian, segments_length);
	let segment = bezpath.get_seg(segment_index + 1).unwrap();
	match segment {
		PathSeg::Line(line) => line.deriv().eval(t),
		PathSeg::Quad(quad_bez) => quad_bez.deriv().eval(t),
		PathSeg::Cubic(cubic_bez) => cubic_bez.deriv().eval(t),
	}
}

pub fn sample_points_on_bezpath(bezpath: BezPath, spacing: f64, start_offset: f64, stop_offset: f64, adaptive_spacing: bool, segments_length: &[f64]) -> Option<BezPath> {
	let mut sample_bezpath = BezPath::new();

	// Calculate the total length of the collected segments.
	let total_length: f64 = segments_length.iter().sum();

	// Adjust the usable length by subtracting start and stop offsets.
	let mut used_length = total_length - start_offset - stop_offset;

	if used_length <= 0. {
		return None;
	}

	// Determine the number of points to generate along the path.
	let sample_count = if adaptive_spacing {
		// Calculate point count to evenly distribute points while covering the entire path.
		// With adaptive spacing, we widen or narrow the points as necessary to ensure the last point is always at the end of the path.
		(used_length / spacing).round()
	} else {
		// Calculate point count based on exact spacing, which may not cover the entire path.

		// Without adaptive spacing, we just evenly space the points at the exact specified spacing, usually falling short before the end of the path.
		let count = (used_length / spacing + f64::EPSILON).floor();
		used_length -= used_length % spacing;
		count
	};

	// Skip if there are no points to generate.
	if sample_count < 1. {
		return None;
	}

	// Generate points along the path based on calculated intervals.
	let mut length_up_to_previous_segment = 0.;
	let mut next_segment_index = 0;

	for count in 0..=sample_count as usize {
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
		let t = eval_pathseg_euclidean(segment, t, POSITION_ACCURACY);
		let point = segment.eval(t);

		if sample_bezpath.elements().is_empty() {
			sample_bezpath.move_to(point)
		} else {
			sample_bezpath.line_to(point)
		}
	}

	Some(sample_bezpath)
}

pub fn t_value_to_parametric(bezpath: &BezPath, t: f64, euclidian: bool, segments_length: Option<&[f64]>) -> (usize, f64) {
	if euclidian {
		let (segment_index, t) = bezpath_t_value_to_parametric(bezpath, BezPathTValue::GlobalEuclidean(t), segments_length);
		let segment = bezpath.get_seg(segment_index + 1).unwrap();
		return (segment_index, eval_pathseg_euclidean(segment, t, POSITION_ACCURACY));
	}
	bezpath_t_value_to_parametric(bezpath, BezPathTValue::GlobalParametric(t), segments_length)
}

/// Finds the t value of point on the given path segment i.e fractional distance along the segment's total length.
/// It uses a binary search to find the value `t` such that the ratio `length_up_to_t / total_length` approximates the input `distance`.
pub fn eval_pathseg_euclidean(path_segment: kurbo::PathSeg, distance: f64, accuracy: f64) -> f64 {
	let mut low_t = 0.;
	let mut mid_t = 0.5;
	let mut high_t = 1.;

	let total_length = path_segment.perimeter(accuracy);

	if !total_length.is_finite() || total_length <= f64::EPSILON {
		return 0.;
	}

	let distance = distance.clamp(0., 1.);

	while high_t - low_t > accuracy {
		let current_length = path_segment.subsegment(0.0..mid_t).perimeter(accuracy);
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
fn global_euclidean_to_local_euclidean(bezpath: &kurbo::BezPath, global_t: f64, lengths: &[f64], total_length: f64) -> (usize, f64) {
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

enum BezPathTValue {
	GlobalEuclidean(f64),
	GlobalParametric(f64),
}

/// Convert a [BezPathTValue] to a parametric `(segment_index, t)` tuple.
/// - Asserts that `t` values contained within the `SubpathTValue` argument lie in the range [0, 1].
fn bezpath_t_value_to_parametric(bezpath: &kurbo::BezPath, t: BezPathTValue, precomputed_segments_length: Option<&[f64]>) -> (usize, f64) {
	let segment_count = bezpath.segments().count();
	assert!(segment_count >= 1);

	match t {
		BezPathTValue::GlobalEuclidean(t) => {
			let computed_segments_length;

			let segments_length = if let Some(segments_length) = precomputed_segments_length {
				segments_length
			} else {
				computed_segments_length = bezpath.segments().map(|segment| segment.perimeter(PERIMETER_ACCURACY)).collect::<Vec<f64>>();
				computed_segments_length.as_slice()
			};

			let total_length = segments_length.iter().sum();

			global_euclidean_to_local_euclidean(bezpath, t, segments_length, total_length)
		}
		BezPathTValue::GlobalParametric(global_t) => {
			assert!((0.0..=1.).contains(&global_t));

			if global_t == 1. {
				return (segment_count - 1, 1.);
			}

			let scaled_t = global_t * segment_count as f64;
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
pub fn poisson_disk_points(bezpath: &BezPath, separation_disk_diameter: f64, rng: impl FnMut() -> f64, subpaths: &[(BezPath, Rect)], subpath_index: usize) -> Vec<DVec2> {
	if bezpath.elements().is_empty() {
		return Vec::new();
	}
	let bbox = bezpath.bounding_box();
	let (offset_x, offset_y) = (bbox.x0, bbox.y0);
	let (width, height) = (bbox.x1 - bbox.x0, bbox.y1 - bbox.y0);

	// TODO: Optimize the following code and make it more robust

	let mut shape = bezpath.clone();
	shape.close_path();
	shape.apply_affine(Affine::translate((-offset_x, -offset_y)));

	let point_in_shape_checker = |point: DVec2| {
		// Check against all paths the point is contained in to compute the correct winding number
		let mut number = 0;
		for (i, (shape, bbox)) in subpaths.iter().enumerate() {
			let point = point + DVec2::new(bbox.x0, bbox.y0);
			if bbox.x0 > point.x || bbox.y0 > point.y || bbox.x1 < point.x || bbox.y1 < point.y {
				continue;
			}
			let winding = shape.winding(dvec2_to_point(point));

			if i == subpath_index && winding == 0 {
				return false;
			}
			number += winding;
		}
		number != 0
	};

	let square_edges_intersect_shape_checker = |position: DVec2, size: f64| {
		let rect = Rect::new(position.x, position.y, position.x + size, position.y + size);
		bezpath_rectangle_intersections_exist(bezpath, rect)
	};

	let mut points = poisson_disk_sample(width, height, separation_disk_diameter, point_in_shape_checker, square_edges_intersect_shape_checker, rng);
	for point in &mut points {
		point.x += offset_x;
		point.y += offset_y;
	}
	points
}

fn bezpath_rectangle_intersections_exist(bezpath: &BezPath, rect: Rect) -> bool {
	if !bezpath.bounding_box().overlaps(rect) {
		return false;
	}

	// Top left
	let p1 = Point::new(rect.x0, rect.y0);
	// Top right
	let p2 = Point::new(rect.x1, rect.y0);
	// Bottom right
	let p3 = Point::new(rect.x1, rect.y1);
	// Bottom left
	let p4 = Point::new(rect.x0, rect.y1);

	let top_line = Line::new((p1.x, p1.y), (p2.x, p2.y));
	let right_line = Line::new((p2.x, p2.y), (p3.x, p3.y));
	let bottom_line = Line::new((p3.x, p3.y), (p4.x, p4.y));
	let left_line = Line::new((p4.x, p4.y), (p1.x, p1.y));

	for segment in bezpath.segments() {
		for line in [top_line, right_line, bottom_line, left_line] {
			if !segment.intersect_line(line).is_empty() {
				return true;
			}
		}
	}

	false
}
