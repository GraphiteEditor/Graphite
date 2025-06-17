use bezier_rs::{Bezier, BezierHandles, Join, Subpath, TValue};
use graphene_vector::PointId;

/// Value to control smoothness and mathematical accuracy to offset a cubic Bezier.
const CUBIC_REGULARIZATION_ACCURACY: f64 = 0.5;
/// Accuracy of fitting offset curve to Bezier paths.
const CUBIC_TO_BEZPATH_ACCURACY: f64 = 1e-3;
/// Constant used to determine if `f64`s are equivalent.
pub const MAX_ABSOLUTE_DIFFERENCE: f64 = 1e-3;

fn segment_to_bezier(seg: kurbo::PathSeg) -> Bezier {
	match seg {
		kurbo::PathSeg::Line(line) => Bezier::from_linear_coordinates(line.p0.x, line.p0.y, line.p1.x, line.p1.y),
		kurbo::PathSeg::Quad(quad_bez) => Bezier::from_quadratic_coordinates(quad_bez.p0.x, quad_bez.p0.y, quad_bez.p1.x, quad_bez.p1.y, quad_bez.p1.x, quad_bez.p1.y),
		kurbo::PathSeg::Cubic(cubic_bez) => Bezier::from_cubic_coordinates(
			cubic_bez.p0.x,
			cubic_bez.p0.y,
			cubic_bez.p1.x,
			cubic_bez.p1.y,
			cubic_bez.p2.x,
			cubic_bez.p2.y,
			cubic_bez.p3.x,
			cubic_bez.p3.y,
		),
	}
}

// TODO: Replace the implementation to use only Kurbo API.
/// Reduces the segments of the subpath into simple subcurves, then offset each subcurve a set `distance` away.
/// The intersections of segments of the subpath are joined using the method specified by the `join` argument.
pub fn offset_subpath(subpath: &Subpath<PointId>, distance: f64, join: Join) -> Subpath<PointId> {
	// An offset at a distance 0 from the curve is simply the same curve.
	// An offset of a single point is not defined.
	if distance == 0. || subpath.len() <= 1 || subpath.len_segments() < 1 {
		return subpath.clone();
	}

	let mut subpaths = subpath
			.iter()
			.filter(|bezier| !bezier.is_point())
			.map(|bezier| bezier.to_cubic())
			.map(|cubic| {
				let Bezier { start, end, handles } = cubic;
				let BezierHandles::Cubic { handle_start, handle_end } = handles else { unreachable!()};

				let cubic_bez = kurbo::CubicBez::new((start.x, start.y), (handle_start.x, handle_start.y), (handle_end.x, handle_end.y), (end.x, end.y));
				let cubic_offset = kurbo::offset::CubicOffset::new_regularized(cubic_bez, distance, CUBIC_REGULARIZATION_ACCURACY);
				let offset_bezpath = kurbo::fit_to_bezpath(&cubic_offset, CUBIC_TO_BEZPATH_ACCURACY);

				let beziers = offset_bezpath.segments().fold(Vec::new(), |mut acc, seg| {
					acc.push(segment_to_bezier(seg));
					acc
				});

				Subpath::from_beziers(&beziers, false)
			})
			.filter(|subpath| subpath.len() >= 2) // In some cases the reduced and scaled bézier is marked by is_point (so the subpath is empty).
			.collect::<Vec<Subpath<PointId>>>();

	let mut drop_common_point = vec![true; subpath.len()];

	// Clip or join consecutive Subpaths
	for i in 0..subpaths.len() - 1 {
		let j = i + 1;
		let subpath1 = &subpaths[i];
		let subpath2 = &subpaths[j];

		let last_segment = subpath1.get_segment(subpath1.len_segments() - 1).unwrap();
		let first_segment = subpath2.get_segment(0).unwrap();

		// If the anchors are approximately equal, there is no need to clip / join the segments
		if last_segment.end().abs_diff_eq(first_segment.start(), MAX_ABSOLUTE_DIFFERENCE) {
			continue;
		}

		// Calculate the angle formed between two consecutive Subpaths
		let out_tangent = subpath.get_segment(i).unwrap().tangent(TValue::Parametric(1.));
		let in_tangent = subpath.get_segment(j).unwrap().tangent(TValue::Parametric(0.));
		let angle = out_tangent.angle_to(in_tangent);

		// The angle is concave. The Subpath overlap and must be clipped
		let mut apply_join = true;
		if (angle > 0. && distance > 0.) || (angle < 0. && distance < 0.) {
			// If the distance is large enough, there may still be no intersections. Also, if the angle is close enough to zero,
			// subpath intersections may find no intersections. In this case, the points are likely close enough that we can approximate
			// the points as being on top of one another.
			if let Some((clipped_subpath1, clipped_subpath2)) = Subpath::clip_simple_subpaths(subpath1, subpath2) {
				subpaths[i] = clipped_subpath1;
				subpaths[j] = clipped_subpath2;
				apply_join = false;
			}
		}
		// The angle is convex. The Subpath must be joined using the specified join type
		if apply_join {
			drop_common_point[j] = false;
			match join {
				Join::Bevel => {}
				Join::Miter(miter_limit) => {
					let miter_manipulator_group = subpaths[i].miter_line_join(&subpaths[j], miter_limit);
					if let Some(miter_manipulator_group) = miter_manipulator_group {
						subpaths[i].manipulator_groups_mut().push(miter_manipulator_group);
					}
				}
				Join::Round => {
					let (out_handle, round_point, in_handle) = subpaths[i].round_line_join(&subpaths[j], subpath.manipulator_groups()[j].anchor);
					let last_index = subpaths[i].manipulator_groups().len() - 1;
					subpaths[i].manipulator_groups_mut()[last_index].out_handle = Some(out_handle);
					subpaths[i].manipulator_groups_mut().push(round_point);
					subpaths[j].manipulator_groups_mut()[0].in_handle = Some(in_handle);
				}
			}
		}
	}

	// Clip any overlap in the last segment
	if subpath.closed {
		let out_tangent = subpath.get_segment(subpath.len_segments() - 1).unwrap().tangent(TValue::Parametric(1.));
		let in_tangent = subpath.get_segment(0).unwrap().tangent(TValue::Parametric(0.));
		let angle = out_tangent.angle_to(in_tangent);

		let mut apply_join = true;
		if (angle > 0. && distance > 0.) || (angle < 0. && distance < 0.) {
			if let Some((clipped_subpath1, clipped_subpath2)) = Subpath::clip_simple_subpaths(&subpaths[subpaths.len() - 1], &subpaths[0]) {
				// Merge the clipped subpaths
				let last_index = subpaths.len() - 1;
				subpaths[last_index] = clipped_subpath1;
				subpaths[0] = clipped_subpath2;
				apply_join = false;
			}
		}
		if apply_join {
			drop_common_point[0] = false;
			match join {
				Join::Bevel => {}
				Join::Miter(miter_limit) => {
					let last_subpath_index = subpaths.len() - 1;
					let miter_manipulator_group = subpaths[last_subpath_index].miter_line_join(&subpaths[0], miter_limit);
					if let Some(miter_manipulator_group) = miter_manipulator_group {
						subpaths[last_subpath_index].manipulator_groups_mut().push(miter_manipulator_group);
					}
				}
				Join::Round => {
					let last_subpath_index = subpaths.len() - 1;
					let (out_handle, round_point, in_handle) = subpaths[last_subpath_index].round_line_join(&subpaths[0], subpath.manipulator_groups()[0].anchor);
					let last_index = subpaths[last_subpath_index].manipulator_groups().len() - 1;
					subpaths[last_subpath_index].manipulator_groups_mut()[last_index].out_handle = Some(out_handle);
					subpaths[last_subpath_index].manipulator_groups_mut().push(round_point);
					subpaths[0].manipulator_groups_mut()[0].in_handle = Some(in_handle);
				}
			}
		}
	}

	// Merge the subpaths. Drop points which overlap with one another.
	let mut manipulator_groups = subpaths[0].manipulator_groups().to_vec();
	for i in 1..subpaths.len() {
		if drop_common_point[i] {
			let last_group = manipulator_groups.pop().unwrap();
			let mut manipulators_copy = subpaths[i].manipulator_groups().to_vec();
			manipulators_copy[0].in_handle = last_group.in_handle;

			manipulator_groups.append(&mut manipulators_copy);
		} else {
			manipulator_groups.append(&mut subpaths[i].manipulator_groups().to_vec());
		}
	}
	if subpath.closed && drop_common_point[0] {
		let last_group = manipulator_groups.pop().unwrap();
		manipulator_groups[0].in_handle = last_group.in_handle;
	}

	Subpath::new(manipulator_groups, subpath.closed)
}
