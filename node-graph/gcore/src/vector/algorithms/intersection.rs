use kurbo::{BezPath, DEFAULT_ACCURACY, ParamCurve, PathSeg, Shape};

/// Minimum allowable separation between adjacent `t` values when calculating curve intersections
pub const MIN_SEPARATION_VALUE: f64 = 5. * 1e-3;

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

fn segment_intersections_inner(segment1: PathSeg, min_t1: f64, max_t1: f64, segment2: PathSeg, min_t2: f64, max_t2: f64, accuracy: f64, intersections: &mut Vec<(f64, f64)>) {
	let bbox1 = segment1.bounding_box();
	let bbox2 = segment2.bounding_box();

	let mid_t1 = (min_t1 + max_t1) / 2.;
	let mid_t2 = (min_t2 + max_t2) / 2.;

	// Check if the bounding boxes overlap
	if bbox1.overlaps(bbox2) {
		// If bounding boxes are within the error threshold (i.e. are small enough), we have found an intersection
		if bbox1.width() < accuracy && bbox1.height() < accuracy && bbox2.width() < accuracy && bbox2.height() < accuracy {
			// Use the middle t value, append the corresponding `t` value.
			intersections.push((mid_t1, mid_t2));
			return;
		}

		// Split curves in half and repeat with the combinations of the two halves of each curve
		let (seg11, seg12) = segment1.subdivide();
		let (seg21, seg22) = segment2.subdivide();

		segment_intersections_inner(seg11, min_t1, mid_t1, seg21, min_t2, mid_t2, accuracy, intersections);
		segment_intersections_inner(seg11, min_t1, mid_t1, seg22, mid_t2, max_t2, accuracy, intersections);
		segment_intersections_inner(seg12, mid_t1, max_t1, seg21, min_t2, mid_t2, accuracy, intersections);
		segment_intersections_inner(seg12, mid_t1, max_t1, seg22, mid_t2, max_t2, accuracy, intersections);
	}
}

// TODO: Use an `impl Iterator` return type instead of a `Vec`
/// Returns a list of filtered parametric `t` values that correspond to intersection points between the current bezier curve and the provided one
/// such that the difference between adjacent `t` values in sorted order is greater than some minimum separation value. If the difference
/// between 2 adjacent `t` values is less than the minimum difference, the filtering takes the larger `t` value and discards the smaller `t` value.
/// The returned `t` values are with respect to the current bezier, not the provided parameter.
/// If the provided curve is linear, then zero intersection points will be returned along colinear segments.
/// - `error` - For intersections where the provided bezier is non-linear, `error` defines the threshold for bounding boxes to be considered an intersection point.
/// - `minimum_separation` - The minimum difference between adjacent `t` values in sorted order
pub fn filtered_segment_intersections(segment1: PathSeg, segment2: PathSeg, accuracy: Option<f64>, minimum_separation: Option<f64>) -> Vec<f64> {
	// TODO: Consider using the `intersections_between_vectors_of_curves` helper function here
	// Otherwise, use bounding box to determine intersections
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
/// Returns a list of pairs of filtered parametric `t` values that correspond to intersection points between the current bezier curve and the provided one
/// such that the difference between adjacent `t` values in sorted order is greater than some minimum separation value. If the difference
/// between 2 adjacent `t` values is less than the minimum difference, the filtering takes the larger `t` value and discards the smaller `t` value.
/// The first value in pair is with respect to the current bezier and the second value in pair is with respect to the provided parameter.
/// If the provided curve is linear, then zero intersection points will be returned along colinear segments.
/// - `error` - For intersections where the provided bezier is non-linear, `error` defines the threshold for bounding boxes to be considered an intersection point.
/// - `minimum_separation` - The minimum difference between adjacent `t` values in sorted order
pub fn filtered_all_segment_intersections(segment1: PathSeg, segment2: PathSeg, accuracy: Option<f64>, minimum_separation: Option<f64>) -> Vec<(f64, f64)> {
	// TODO: Consider using the `intersections_between_vectors_of_curves` helper function here
	// Otherwise, use bounding box to determine intersections
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

fn bezpath_intersections(bezpath1: &BezPath, bezpath2: &BezPath) -> Vec<f64> {
	let intersections = Vec::new();
	intersections
}
