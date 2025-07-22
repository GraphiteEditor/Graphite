use kurbo::{BezPath, ParamCurve, PathSeg, Shape};

pub fn segment_intersections(segment1: PathSeg, segment2: PathSeg, accuracy: f64) -> Vec<(f64, f64)> {
	let mut intersections = Vec::new();
	segment_intersections_inner(segment1, 0., 1., segment2, 0., 1., accuracy, &mut intersections);
	intersections
}

fn segment_intersections_inner(segment1: PathSeg, min_t1: f64, max_t1: f64, segment2: PathSeg, min_t2: f64, max_t2: f64, accuracy: f64, intersections: &mut Vec<(f64, f64)>) {
	let bbox1 = segment1.bounding_box();
	let bbox2 = segment2.bounding_box();

	let mid_t1 = (min_t1 + max_t1) / 2.;
	let mid_t2 = (min_t2 + max_t2) / 2.;

	// Check if the bounding boxes overlap
	if bbox1.overlaps(bbox2) {
		// If bounding boxes are within the error threshold (i.e. are small enough), we have found an intersection
		if bbox1.width() < accuracy && bbox1.height() < accuracy {
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

fn bezpath_intersections(bezpath1: &BezPath, bezpath2: &BezPath) -> Vec<f64> {
	let intersections = Vec::new();
	intersections
}
