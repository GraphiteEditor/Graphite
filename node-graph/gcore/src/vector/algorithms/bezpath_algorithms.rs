/// Accuracy to find the position on [kurbo::Bezpath].
const POSITION_ACCURACY: f64 = 1e-5;
/// Accuracy to find the length of the [kurbo::PathSeg].
pub const PERIMETER_ACCURACY: f64 = 1e-5;

use kurbo::{BezPath, ParamCurve, ParamCurveDeriv, PathSeg, Point, Shape};

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

		// Keep moving to the next segment while the length up to the next sample point is less or equals to the length up to the segment.
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
fn bezpath_t_value_to_parametric(bezpath: &kurbo::BezPath, t: BezPathTValue, segments_length: Option<&[f64]>) -> (usize, f64) {
	let segment_count = bezpath.segments().count();
	assert!(segment_count >= 1);

	match t {
		BezPathTValue::GlobalEuclidean(t) => {
			let lengths = segments_length
				.map(|segments_length| segments_length.to_vec())
				.unwrap_or(bezpath.segments().map(|segment| segment.perimeter(PERIMETER_ACCURACY)).collect());

			let total_length = lengths.iter().sum();

			global_euclidean_to_local_euclidean(bezpath, t, lengths.as_slice(), total_length)
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
