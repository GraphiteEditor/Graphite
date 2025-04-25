use kurbo::{BezPath, ParamCurve, Point, Shape};

pub fn position_on_bezpath(bezpath: &BezPath, t: f64, euclidian: bool) -> Point {
	if euclidian {
		let (segment_index, t) = t_value_to_parametric(&bezpath, BezPathTValue::GlobalEuclidean(t));
		let segment = bezpath.get_seg(segment_index + 1).unwrap();
		eval_pathseg_euclidian(segment, t, POSITION_ACCURACY)
	} else {
		let (segment_index, t) = t_value_to_parametric(&bezpath, BezPathTValue::GlobalParametric(t));
		let segment = bezpath.get_seg(segment_index + 1).unwrap();
		segment.eval(t)
	}
}

/// Accuracy to find the position on [kurbo::Bezpath].
const POSITION_ACCURACY: f64 = 1e-3;
/// Accuracy to find the length of the [kurbo::PathSeg].
const PERIMETER_ACCURACY: f64 = 1e-3;

/// Finds the point on the given path segment i.e fractional distance along the segment's total length.
/// It uses a binary search to find the value `t` such that the ratio `length_upto_t / total_length` approximates the input `distance`.
fn eval_pathseg_euclidian(path: kurbo::PathSeg, distance: f64, accuracy: f64) -> kurbo::Point {
	let mut low_t = 0.;
	let mut hight_t = 1.;
	let mut mid_t = 0.5;

	let total_length = path.perimeter(accuracy);

	if !total_length.is_finite() || total_length <= f64::EPSILON {
		return path.start();
	}

	let distance = distance.clamp(0., 1.);

	while hight_t - low_t > accuracy {
		let current_length = path.subsegment(0.0..mid_t).perimeter(accuracy);
		let current_distance = current_length / total_length;

		if current_distance > distance {
			hight_t = mid_t;
		} else {
			low_t = mid_t;
		}
		mid_t = (hight_t + low_t) / 2.;
	}

	path.eval(mid_t)
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
	(bezpath.segments().count() - 2, 1.)
}

enum BezPathTValue {
	GlobalEuclidean(f64),
	GlobalParametric(f64),
}

/// Convert a [BezPathTValue] to a parametric `(segment_index, t)` tuple.
/// - Asserts that `t` values contained within the `SubpathTValue` argument lie in the range [0, 1].
fn t_value_to_parametric(bezpath: &kurbo::BezPath, t: BezPathTValue) -> (usize, f64) {
	let segment_len = bezpath.segments().count();
	assert!(segment_len >= 1);

	match t {
		BezPathTValue::GlobalEuclidean(t) => {
			let lengths = bezpath.segments().map(|bezier| bezier.perimeter(PERIMETER_ACCURACY)).collect::<Vec<f64>>();
			let total_length: f64 = lengths.iter().sum();
			global_euclidean_to_local_euclidean(&bezpath, t, lengths.as_slice(), total_length)
		}
		BezPathTValue::GlobalParametric(global_t) => {
			assert!((0.0..=1.).contains(&global_t));

			if global_t == 1. {
				return (segment_len - 1, 1.);
			}

			let scaled_t = global_t * segment_len as f64;
			let segment_index = scaled_t.floor() as usize;
			let t = scaled_t - segment_index as f64;

			(segment_index, t)
		}
	}
}
