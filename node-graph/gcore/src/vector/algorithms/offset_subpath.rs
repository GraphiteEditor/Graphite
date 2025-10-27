use super::bezpath_algorithms::{clip_simple_bezpaths, miter_line_join, round_line_join};
use crate::vector::misc::point_to_dvec2;
use kurbo::{BezPath, Join, ParamCurve, PathEl, PathSeg};

/// Value to control smoothness and mathematical accuracy to offset a cubic Bezier.
const CUBIC_REGULARIZATION_ACCURACY: f64 = 0.5;
/// Accuracy of fitting offset curve to Bezier paths.
const CUBIC_TO_BEZPATH_ACCURACY: f64 = 1e-3;
/// Constant used to determine if `f64`s are equivalent.
pub const MAX_ABSOLUTE_DIFFERENCE: f64 = 1e-7;
/// Squared version to avoid sqrt in distance checks.
const MAX_ABSOLUTE_DIFFERENCE_SQUARED: f64 = MAX_ABSOLUTE_DIFFERENCE * MAX_ABSOLUTE_DIFFERENCE;
const MAX_FITTED_SEGMENTS: usize = 10000;

/// Reduces the segments of the bezpath into simple subcurves, then offset each subcurve a set `distance` away.
/// The intersections of segments of the subpath are joined using the method specified by the `join` argument.
pub fn offset_bezpath(bezpath: &BezPath, distance: f64, join: Join, miter_limit: Option<f64>) -> BezPath {
	// An offset at a distance 0 from the curve is simply the same curve.
	// An offset of a single point is not defined.
	if distance == 0. || bezpath.get_seg(1).is_none() {
		return bezpath.clone();
	}

	let mut bezpaths = bezpath
		.segments()
		.map(|bezier| bezier.to_cubic())
		.filter_map(|cubic_bez| {
			// Skip degenerate curves where all control points are at the same location.
			// Offsetting a point is undefined and causes infinite recursion in fit_to_bezpath.
			let start = cubic_bez.p0;
			let is_degenerate = start.distance_squared(cubic_bez.p1) < MAX_ABSOLUTE_DIFFERENCE_SQUARED
				&& start.distance_squared(cubic_bez.p2) < MAX_ABSOLUTE_DIFFERENCE_SQUARED
				&& start.distance_squared(cubic_bez.p3) < MAX_ABSOLUTE_DIFFERENCE_SQUARED;

			if is_degenerate {
				return None;
			}

			let cubic_offset = kurbo::offset::CubicOffset::new_regularized(cubic_bez, distance, CUBIC_REGULARIZATION_ACCURACY);

			let fitted = kurbo::fit_to_bezpath(&cubic_offset, CUBIC_TO_BEZPATH_ACCURACY);

			if fitted.segments().count() > MAX_FITTED_SEGMENTS {
				None
			} else {
				fitted.get_seg(1).is_some().then_some(fitted)
			}
		})
		.collect::<Vec<BezPath>>();

	// Clip or join consecutive Subpaths
	for i in 0..bezpaths.len() - 1 {
		let j = i + 1;
		let bezpath1 = &bezpaths[i];
		let bezpath2 = &bezpaths[j];

		let last_segment_end = point_to_dvec2(bezpath1.segments().last().unwrap().end());
		let first_segment_start = point_to_dvec2(bezpath2.segments().next().unwrap().start());

		// If the anchors are approximately equal, there is no need to clip / join the segments
		if last_segment_end.abs_diff_eq(first_segment_start, MAX_ABSOLUTE_DIFFERENCE) {
			continue;
		}

		// The angle is concave. The Subpath overlap and must be clipped
		let mut apply_join = true;

		if let Some((clipped_subpath1, clipped_subpath2)) = clip_simple_bezpaths(bezpath1, bezpath2) {
			bezpaths[i] = clipped_subpath1;
			bezpaths[j] = clipped_subpath2;
			apply_join = false;
		}
		// The angle is convex. The Subpath must be joined using the specified join type
		if apply_join {
			match join {
				Join::Bevel => {
					let element = PathEl::LineTo(bezpaths[j].segments().next().unwrap().start());
					bezpaths[i].push(element);
				}
				Join::Miter => {
					let element = miter_line_join(&bezpaths[i], &bezpaths[j], miter_limit);
					if let Some(element) = element {
						bezpaths[i].push(element[0]);
						bezpaths[i].push(element[1]);
					} else {
						let element = PathEl::LineTo(bezpaths[j].segments().next().unwrap().start());
						bezpaths[i].push(element);
					}
				}
				Join::Round => {
					let center = point_to_dvec2(bezpath.get_seg(i + 1).unwrap().end());
					let elements = round_line_join(&bezpaths[i], &bezpaths[j], center);
					bezpaths[i].push(elements[0]);
					bezpaths[i].push(elements[1]);
				}
			}
		}
	}

	// Clip any overlap in the last segment
	let is_bezpath_closed = bezpath.elements().last().is_some_and(|element| *element == PathEl::ClosePath);
	if is_bezpath_closed {
		let mut apply_join = true;
		if let Some((clipped_subpath1, clipped_subpath2)) = clip_simple_bezpaths(&bezpaths[bezpaths.len() - 1], &bezpaths[0]) {
			// Merge the clipped subpaths
			let last_index = bezpaths.len() - 1;
			bezpaths[last_index] = clipped_subpath1;
			bezpaths[0] = clipped_subpath2;
			apply_join = false;
		}

		if apply_join {
			match join {
				Join::Bevel => {
					let last_subpath_index = bezpaths.len() - 1;
					let element = PathEl::LineTo(bezpaths[0].segments().next().unwrap().start());
					bezpaths[last_subpath_index].push(element);
				}
				Join::Miter => {
					let last_subpath_index = bezpaths.len() - 1;
					let element = miter_line_join(&bezpaths[last_subpath_index], &bezpaths[0], miter_limit);
					if let Some(element) = element {
						bezpaths[last_subpath_index].push(element[0]);
						bezpaths[last_subpath_index].push(element[1]);
					} else {
						let element = PathEl::LineTo(bezpaths[0].segments().next().unwrap().start());
						bezpaths[last_subpath_index].push(element);
					}
				}
				Join::Round => {
					let last_subpath_index = bezpaths.len() - 1;
					let center = point_to_dvec2(bezpath.get_seg(1).unwrap().start());
					let elements = round_line_join(&bezpaths[last_subpath_index], &bezpaths[0], center);
					bezpaths[last_subpath_index].push(elements[0]);
					bezpaths[last_subpath_index].push(elements[1]);
				}
			}
		}
	}

	// Merge the bezpaths and its segments. Drop points which overlap with one another.
	let segments = bezpaths.iter().flat_map(|bezpath| bezpath.segments().collect::<Vec<PathSeg>>()).collect::<Vec<PathSeg>>();
	let mut offset_bezpath = segments.iter().fold(BezPath::new(), |mut acc, segment| {
		if acc.elements().is_empty() {
			acc.move_to(segment.start());
		}
		acc.push(segment.as_path_el());
		acc
	});

	if is_bezpath_closed {
		offset_bezpath.close_path();
	}

	offset_bezpath
}
