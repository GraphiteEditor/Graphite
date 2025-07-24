use crate::vector::misc::point_to_dvec2;
use kurbo::{BezPath, Join, ParamCurve, PathEl, PathSeg};

use super::{
	bezpath_algorithms::{clip_simple_bezpaths, miter_line_join, round_line_join},
	util::segment_tangent,
};

/// Value to control smoothness and mathematical accuracy to offset a cubic Bezier.
const CUBIC_REGULARIZATION_ACCURACY: f64 = 0.5;
/// Accuracy of fitting offset curve to Bezier paths.
const CUBIC_TO_BEZPATH_ACCURACY: f64 = 1e-3;
/// Constant used to determine if `f64`s are equivalent.
pub const MAX_ABSOLUTE_DIFFERENCE: f64 = 1e-3;

// TODO: Replace the implementation to use only Kurbo API.
/// Reduces the segments of the subpath into simple subcurves, then offset each subcurve a set `distance` away.
/// The intersections of segments of the subpath are joined using the method specified by the `join` argument.
pub fn offset_bezpath(bezpath: &BezPath, distance: f64, join: Join, miter_limit: Option<f64>) -> BezPath {
	// An offset at a distance 0 from the curve is simply the same curve.
	// An offset of a single point is not defined.
	if distance == 0. || bezpath.get_seg(1).is_none() {
		info!("not enougn segments");
		return bezpath.clone();
	}

	let mut bezpaths = bezpath
			.segments()
			.map(|bezier| bezier.to_cubic())
			.map(|cubic_bez| {
				let cubic_offset = kurbo::offset::CubicOffset::new_regularized(cubic_bez, distance, CUBIC_REGULARIZATION_ACCURACY);
				let offset_bezpath = kurbo::fit_to_bezpath(&cubic_offset, CUBIC_TO_BEZPATH_ACCURACY);
				offset_bezpath
			})
			.filter(|bezpath| bezpath.get_seg(1).is_some()) // In some cases the reduced and scaled bézier is marked by is_point (so the subpath is empty).
			.collect::<Vec<BezPath>>();

	let mut drop_common_point = vec![true; bezpaths.len()];

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

		// Calculate the angle formed between two consecutive Subpaths
		// NOTE: [BezPath] segments are one-indexed.
		let out_tangent = segment_tangent(bezpath.get_seg(i + 1).unwrap(), 1.);
		let in_tangent = segment_tangent(bezpath.get_seg(j + 1).unwrap(), 0.);
		let angle = out_tangent.angle_to(in_tangent);

		// The angle is concave. The Subpath overlap and must be clipped
		let mut apply_join = true;
		if (angle > 0. && distance > 0.) || (angle < 0. && distance < 0.) {
			// If the distance is large enough, there may still be no intersections. Also, if the angle is close enough to zero,
			// subpath intersections may find no intersections. In this case, the points are likely close enough that we can approximate
			// the points as being on top of one another.
			if let Some((clipped_subpath1, clipped_subpath2)) = clip_simple_bezpaths(bezpath1, bezpath2) {
				bezpaths[i] = clipped_subpath1;
				bezpaths[j] = clipped_subpath2;
				apply_join = false;
			}
		}
		// The angle is convex. The Subpath must be joined using the specified join type
		if apply_join {
			drop_common_point[j] = false;
			match join {
				Join::Bevel => {}
				Join::Miter => {
					let element = miter_line_join(&bezpaths[i], &bezpaths[j], miter_limit);
					if let Some(element) = element {
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
		let out_tangent = segment_tangent(bezpath.segments().last().unwrap(), 1.);
		let in_tangent = segment_tangent(bezpath.segments().next().unwrap(), 0.);
		let angle = out_tangent.angle_to(in_tangent);

		let mut apply_join = true;
		if (angle > 0. && distance > 0.) || (angle < 0. && distance < 0.) {
			if let Some((clipped_subpath1, clipped_subpath2)) = clip_simple_bezpaths(&bezpaths[bezpaths.len() - 1], &bezpaths[0]) {
				// Merge the clipped subpaths
				let last_index = bezpaths.len() - 1;
				bezpaths[last_index] = clipped_subpath1;
				bezpaths[0] = clipped_subpath2;
				apply_join = false;
			}
		}
		if apply_join {
			drop_common_point[0] = false;
			match join {
				Join::Bevel => {}
				Join::Miter => {
					let last_subpath_index = bezpaths.len() - 1;
					let element = miter_line_join(&bezpaths[last_subpath_index], &bezpaths[0], miter_limit);
					if let Some(element) = element {
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

	// Merge the subpaths. Drop points which overlap with one another.
	let segments = bezpaths.iter().flat_map(|bezpath| bezpath.segments().collect::<Vec<PathSeg>>()).collect::<Vec<PathSeg>>();
	let mut offset_bezpath = BezPath::from_path_segments(segments.into_iter());

	if is_bezpath_closed {
		offset_bezpath.close_path();
	}
	offset_bezpath
}
