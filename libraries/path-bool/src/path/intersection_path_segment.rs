use crate::aabb::{Aabb, bounding_box_max_extent, bounding_boxes_overlap};
use crate::epsilons::Epsilons;
use crate::line_segment::{line_segment_intersection, line_segments_intersect};
use crate::line_segment_aabb::line_segment_aabb_intersect;
use crate::math::lerp;
use crate::path_segment::PathSegment;
use glam::DVec2;
use lyon_geom::{CubicBezierSegment, Point};

/// Convert PathSegment::Cubic to lyon_geom::CubicBezierSegment
fn path_segment_cubic_to_lyon(start: DVec2, ctrl1: DVec2, ctrl2: DVec2, end: DVec2) -> CubicBezierSegment<f64> {
	CubicBezierSegment {
		from: Point::new(start.x, start.y),
		ctrl1: Point::new(ctrl1.x, ctrl1.y),
		ctrl2: Point::new(ctrl2.x, ctrl2.y),
		to: Point::new(end.x, end.y),
	}
}

#[derive(Clone)]
struct IntersectionSegment {
	seg: PathSegment,
	start_param: f64,
	end_param: f64,
	bounding_box: Aabb,
}

#[inline(never)]
fn subdivide_intersection_segment(int_seg: &IntersectionSegment) -> [IntersectionSegment; 2] {
	let (seg0, seg1) = int_seg.seg.split_at(0.5);
	let mid_param = (int_seg.start_param + int_seg.end_param) / 2.;
	[
		IntersectionSegment {
			seg: seg0,
			start_param: int_seg.start_param,
			end_param: mid_param,
			bounding_box: seg0.approx_bounding_box(),
		},
		IntersectionSegment {
			seg: seg1,
			start_param: mid_param,
			end_param: int_seg.end_param,
			bounding_box: seg1.approx_bounding_box(),
		},
	]
}

#[inline(never)]
fn path_segment_to_line_segment(seg: &PathSegment) -> [DVec2; 2] {
	match seg {
		PathSegment::Line(start, end) => [*start, *end],
		PathSegment::Cubic(start, _, _, end) => [*start, *end],
		PathSegment::Quadratic(start, _, end) => [*start, *end],
		PathSegment::Arc(start, _, _, _, _, _, end) => [*start, *end],
	}
}

#[inline(never)]
fn intersection_segments_overlap(seg0: &IntersectionSegment, seg1: &IntersectionSegment) -> bool {
	match (&seg0.seg, &seg1.seg) {
		(PathSegment::Line(start0, end0), PathSegment::Line(start1, end1)) => {
			line_segments_intersect([*start0, *end0], [*start1, *end1], 1e-6) // TODO: configurable
		}
		(PathSegment::Line(start, end), _) => line_segment_aabb_intersect([*start, *end], &seg1.bounding_box),
		(_, PathSegment::Line(start, end)) => line_segment_aabb_intersect([*start, *end], &seg0.bounding_box),
		_ => bounding_boxes_overlap(&seg0.bounding_box, &seg1.bounding_box),
	}
}

#[inline(never)]
pub fn segments_equal(seg0: &PathSegment, seg1: &PathSegment, point_epsilon: f64) -> bool {
	match (*seg0, *seg1) {
		(PathSegment::Line(start0, end0), PathSegment::Line(start1, end1)) => start0.abs_diff_eq(start1, point_epsilon) && end0.abs_diff_eq(end1, point_epsilon),
		(PathSegment::Cubic(p00, p01, p02, p03), PathSegment::Cubic(p10, p11, p12, p13)) => {
			let start_and_end_equal = p00.abs_diff_eq(p10, point_epsilon) && p03.abs_diff_eq(p13, point_epsilon);

			let parameter_equal = p01.abs_diff_eq(p11, point_epsilon) && p02.abs_diff_eq(p12, point_epsilon);
			let direction1 = seg0.sample_at(0.1);
			let direction2 = seg1.sample_at(0.1);
			let angles_equal = (direction1 - p00).angle_to(direction2 - p00).abs() < point_epsilon * 4.;

			start_and_end_equal && (parameter_equal || angles_equal)
		}
		(PathSegment::Quadratic(p00, p01, p02), PathSegment::Quadratic(p10, p11, p12)) => {
			p00.abs_diff_eq(p10, point_epsilon) && p01.abs_diff_eq(p11, point_epsilon) && p02.abs_diff_eq(p12, point_epsilon)
		}
		(PathSegment::Arc(p00, rx0, ry0, angle0, large_arc0, sweep0, p01), PathSegment::Arc(p10, rx1, ry1, angle1, large_arc1, sweep1, p11)) => {
			p00.abs_diff_eq(p10, point_epsilon) &&
			(rx0 - rx1).abs() < point_epsilon &&
			(ry0 - ry1).abs() < point_epsilon &&
			(angle0 - angle1).abs() < point_epsilon && // TODO: Phi can be anything if rx = ry. Also, handle rotations by Pi/2.
			large_arc0 == large_arc1 &&
			sweep0 == sweep1 &&
			p01.abs_diff_eq(p11, point_epsilon)
		}
		_ => false,
	}
}

pub fn path_segment_intersection(seg0: &PathSegment, seg1: &PathSegment, endpoints: bool, eps: &Epsilons) -> Vec<[f64; 2]> {
	match (seg0, seg1) {
		(PathSegment::Line(start0, end0), PathSegment::Line(start1, end1)) => {
			if let Some(st) = line_segment_intersection([*start0, *end0], [*start1, *end1], eps.param) {
				if !endpoints && (st.0 < eps.param || st.0 > 1. - eps.param) && (st.1 < eps.param || st.1 > 1. - eps.param) {
					return vec![];
				}
				return vec![st.into()];
			}
		}
		(PathSegment::Cubic(s1, c11, c21, e1), PathSegment::Cubic(s2, c12, c22, e2)) => {
			let path1 = path_segment_cubic_to_lyon(*s1, *c11, *c21, *e1);
			let path2 = path_segment_cubic_to_lyon(*s2, *c12, *c22, *e2);

			let intersections = path1.cubic_intersections_t(&path2);
			let intersections: Vec<_> = intersections.into_iter().map(|(s, t)| [s, t]).collect();
			return intersections;
		}
		_ => (),
	};

	// Fallback for quadratics and arc segments
	// https://math.stackexchange.com/questions/20321/how-can-i-tell-when-two-cubic-b%C3%A9zier-curves-intersect

	let mut pairs = vec![(
		IntersectionSegment {
			seg: *seg0,
			start_param: 0.,
			end_param: 1.,
			bounding_box: seg0.approx_bounding_box(),
		},
		IntersectionSegment {
			seg: *seg1,
			start_param: 0.,
			end_param: 1.,
			bounding_box: seg1.approx_bounding_box(),
		},
	)];
	let mut next_pairs = Vec::new();

	let mut params = Vec::new();
	let mut subdivided0 = Vec::new();
	let mut subdivided1 = Vec::new();

	// Check if start and end points are on the other bezier curves. If so, add an intersection.

	while !pairs.is_empty() {
		next_pairs.clear();

		if pairs.len() > 256 {
			return calculate_overlap_intersections(seg0, seg1, eps);
		}

		for (seg0, seg1) in pairs.iter() {
			if segments_equal(&seg0.seg, &seg1.seg, eps.point) {
				// TODO: move this outside of this loop?
				continue; // TODO: what to do?
			}

			let is_linear0 = bounding_box_max_extent(&seg0.bounding_box) <= eps.linear || (seg0.end_param - seg0.start_param).abs() < eps.param;
			let is_linear1 = bounding_box_max_extent(&seg1.bounding_box) <= eps.linear || (seg1.end_param - seg1.start_param).abs() < eps.param;

			if is_linear0 && is_linear1 {
				let line_segment0 = path_segment_to_line_segment(&seg0.seg);
				let line_segment1 = path_segment_to_line_segment(&seg1.seg);
				if let Some(st) = line_segment_intersection(line_segment0, line_segment1, eps.param) {
					params.push([lerp(seg0.start_param, seg0.end_param, st.0), lerp(seg1.start_param, seg1.end_param, st.1)]);
				}
			} else {
				subdivided0.clear();
				subdivided1.clear();
				if is_linear0 {
					subdivided0.push(seg0.clone())
				} else {
					subdivided0.extend_from_slice(&subdivide_intersection_segment(seg0))
				};
				if is_linear1 {
					subdivided1.push(seg1.clone())
				} else {
					subdivided1.extend_from_slice(&subdivide_intersection_segment(seg1))
				};

				for seg0 in &subdivided0 {
					for seg1 in &subdivided1 {
						if intersection_segments_overlap(seg0, seg1) {
							next_pairs.push((seg0.clone(), seg1.clone()));
						}
					}
				}
			}
		}

		std::mem::swap(&mut pairs, &mut next_pairs);
	}

	params
}

fn calculate_overlap_intersections(seg0: &PathSegment, seg1: &PathSegment, eps: &Epsilons) -> Vec<[f64; 2]> {
	let start0 = seg0.start();
	let end0 = seg0.end();
	let start1 = seg1.start();
	let end1 = seg1.end();

	let mut intersections = Vec::new();

	// Check start0 against seg1
	if let Some(t1) = find_point_on_segment(seg1, start0, eps) {
		intersections.push([0., t1]);
	}

	// Check end0 against seg1
	if let Some(t1) = find_point_on_segment(seg1, end0, eps) {
		intersections.push([1., t1]);
	}

	// Check start1 against seg0
	if let Some(t0) = find_point_on_segment(seg0, start1, eps) {
		intersections.push([t0, 0.]);
	}

	// Check end1 against seg0
	if let Some(t0) = find_point_on_segment(seg0, end1, eps) {
		intersections.push([t0, 1.]);
	}

	// Remove duplicates and sort intersections
	intersections.sort_unstable_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
	intersections.dedup_by(|a, b| DVec2::from(*a).abs_diff_eq(DVec2::from(*b), eps.param));

	// Handle special cases
	if intersections.is_empty() {
		// Check if segments are identical
		if (start0.abs_diff_eq(start1, eps.point)) && end0.abs_diff_eq(end1, eps.point) {
			return vec![[0., 0.], [1., 1.]];
		}
	} else if intersections.len() > 2 {
		// Keep only the first and last intersection points
		intersections = vec![intersections[0], intersections[intersections.len() - 1]];
	}

	intersections
}

fn find_point_on_segment(seg: &PathSegment, point: DVec2, eps: &Epsilons) -> Option<f64> {
	let start = 0.;
	let end = 1.;
	let mut t = 0.5;

	for _ in 0..32 {
		// Limit iterations to prevent infinite loops
		let current_point = seg.sample_at(t);

		if current_point.abs_diff_eq(point, eps.point) {
			return Some(t);
		}

		let start_point = seg.sample_at(start);
		let end_point = seg.sample_at(end);

		let dist_start = (point - start_point).length_squared();
		let dist_end = (point - end_point).length_squared();
		let dist_current = (point - current_point).length_squared();

		if dist_current < dist_start && dist_current < dist_end {
			return Some(t);
		}

		if dist_start < dist_end {
			t = (start + t) / 2.;
		} else {
			t = (t + end) / 2.;
		}

		if (end - start) < eps.param {
			break;
		}
	}

	None
}

#[cfg(test)]
mod test {
	use super::*;
	use glam::DVec2;

	#[test]
	fn intersect_cubic_slow_first() {
		path_segment_intersection(&a(), &b(), true, &crate::EPS);
	}
	#[test]
	fn intersect_cubic_slow_second() {
		path_segment_intersection(&c(), &d(), true, &crate::EPS);
	}

	fn a() -> PathSegment {
		PathSegment::Cubic(
			DVec2::new(458.37027, 572.165771),
			DVec2::new(428.525848, 486.720093),
			DVec2::new(368.618805, 467.485992),
			DVec2::new(273., 476.),
		)
	}
	fn b() -> PathSegment {
		PathSegment::Cubic(DVec2::new(273., 476.), DVec2::new(419., 463.), DVec2::new(481.741198, 514.692273), DVec2::new(481.333333, 768.))
	}
	fn c() -> PathSegment {
		PathSegment::Cubic(DVec2::new(273., 476.), DVec2::new(107.564178, 490.730591), DVec2::new(161.737915, 383.575775), DVec2::new(0., 340.))
	}
	fn d() -> PathSegment {
		PathSegment::Cubic(DVec2::new(0., 340.), DVec2::new(161.737914, 383.575765), DVec2::new(107.564182, 490.730587), DVec2::new(273., 476.))
	}
}
