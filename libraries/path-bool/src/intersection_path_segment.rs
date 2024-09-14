// Copyright 2024 Adam Platkevič <rflashster@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::aabb::{bounding_box_max_extent, bounding_boxes_overlap, AaBb};
use crate::epsilons::Epsilons;
use crate::line_segment::{line_segment_intersection, line_segments_intersect};
use crate::line_segment_aabb::line_segment_aabb_intersect;
use crate::math::lerp;
use crate::path_segment::{path_segment_bounding_box, sample_path_segment_at, split_segment_at, PathSegment};
use crate::vector::{vectors_equal, Vector};

#[derive(Clone)]
struct IntersectionSegment {
	seg: PathSegment,
	start_param: f64,
	end_param: f64,
	bounding_box: AaBb,
}

#[inline(never)]
fn subdivide_intersection_segment(int_seg: &IntersectionSegment) -> [IntersectionSegment; 2] {
	let (seg0, seg1) = split_segment_at(&int_seg.seg, 0.5);
	let mid_param = (int_seg.start_param + int_seg.end_param) / 2.0;
	[
		IntersectionSegment {
			seg: seg0,
			start_param: int_seg.start_param,
			end_param: mid_param,
			bounding_box: path_segment_bounding_box(&seg0),
		},
		IntersectionSegment {
			seg: seg1,
			start_param: mid_param,
			end_param: int_seg.end_param,
			bounding_box: path_segment_bounding_box(&seg1),
		},
	]
}

#[inline(never)]
fn path_segment_to_line_segment(seg: &PathSegment) -> [Vector; 2] {
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
		(PathSegment::Line(start0, end0), PathSegment::Line(start1, end1)) => vectors_equal(start0, start1, point_epsilon) && vectors_equal(end0, end1, point_epsilon),
		(PathSegment::Cubic(p00, p01, p02, p03), PathSegment::Cubic(p10, p11, p12, p13)) => {
			vectors_equal(p00, p10, point_epsilon) && vectors_equal(p01, p11, point_epsilon) && vectors_equal(p02, p12, point_epsilon) && vectors_equal(p03, p13, point_epsilon)
		}
		(PathSegment::Quadratic(p00, p01, p02), PathSegment::Quadratic(p10, p11, p12)) => {
			vectors_equal(p00, p10, point_epsilon) && vectors_equal(p01, p11, point_epsilon) && vectors_equal(p02, p12, point_epsilon)
		}
		(PathSegment::Arc(p00, rx0, ry0, angle0, large_arc0, sweep0, p01), PathSegment::Arc(p10, rx1, ry1, angle1, large_arc1, sweep1, p11)) => {
			vectors_equal(p00, p10, point_epsilon) &&
            (rx0 - rx1).abs() < point_epsilon &&
            (ry0 - ry1).abs() < point_epsilon &&
            (angle0 - angle1).abs() < point_epsilon && // TODO: Phi can be anything if rx = ry. Also, handle rotations by Pi/2.
            large_arc0 == large_arc1 &&
            sweep0 == sweep1 &&
            vectors_equal(p01, p11, point_epsilon)
		}
		_ => false,
	}
}

pub fn path_segment_intersection(seg0: &PathSegment, seg1: &PathSegment, endpoints: bool, eps: &Epsilons) -> Vec<[f64; 2]> {
	// dbg!(&seg0, &seg1, endpoints);
	if let (PathSegment::Line(start0, end0), PathSegment::Line(start1, end1)) = (seg0, seg1) {
		if let Some(st) = line_segment_intersection([*start0, *end0], [*start1, *end1], eps.param) {
			if !endpoints && (st.0 < eps.param || st.0 > 1.0 - eps.param) && (st.1 < eps.param || st.1 > 1.0 - eps.param) {
				return vec![];
			}
			return vec![st.into()];
		}
	}

	// https://math.stackexchange.com/questions/20321/how-can-i-tell-when-two-cubic-b%C3%A9zier-curves-intersect

	let mut pairs = vec![(
		IntersectionSegment {
			seg: *seg0,
			start_param: 0.0,
			end_param: 1.0,
			bounding_box: path_segment_bounding_box(seg0),
		},
		IntersectionSegment {
			seg: *seg1,
			start_param: 0.0,
			end_param: 1.0,
			bounding_box: path_segment_bounding_box(seg1),
		},
	)];
	let mut next_pairs = Vec::new();

	let mut params = Vec::new();
	let mut subdivided0 = Vec::new();
	let mut subdivided1 = Vec::new();

	while !pairs.is_empty() {
		next_pairs.clear();
		dbg!("checking pairs");

		for (seg0, seg1) in pairs.iter() {
			if segments_equal(&seg0.seg, &seg1.seg, eps.point) {
				// TODO: move this outside of this loop?
				continue; // TODO: what to do?
			}

			let is_linear0 = bounding_box_max_extent(&seg0.bounding_box) <= eps.linear || (seg1.end_param - seg1.start_param).abs() < eps.param;
			let is_linear1 = bounding_box_max_extent(&seg1.bounding_box) <= eps.linear || (seg1.end_param - seg1.start_param).abs() < eps.param;

			if is_linear0 && is_linear1 {
				let line_segment0 = path_segment_to_line_segment(&seg0.seg);
				let line_segment1 = path_segment_to_line_segment(&seg1.seg);
				if let Some(st) = line_segment_intersection(line_segment0, line_segment1, eps.param) {
					dbg!("pushing param");
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

	if !endpoints {
		params.retain(|[s, t]| (s > &eps.param && s < &(1.0 - eps.param)) || (t > &eps.param && t < &(1.0 - eps.param)));
	}

	params
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
			DVec2::new(273.0, 476.0),
		)
	}
	fn b() -> PathSegment {
		PathSegment::Cubic(DVec2::new(273.0, 476.0), DVec2::new(419.0, 463.0), DVec2::new(481.741198, 514.692273), DVec2::new(481.333333, 768.0))
	}
	fn c() -> PathSegment {
		PathSegment::Cubic(DVec2::new(273.0, 476.0), DVec2::new(107.564178, 490.730591), DVec2::new(161.737915, 383.575775), DVec2::new(0.0, 340.0))
	}
	fn d() -> PathSegment {
		PathSegment::Cubic(DVec2::new(0.0, 340.0), DVec2::new(161.737914, 383.575765), DVec2::new(107.564182, 490.730587), DVec2::new(273.0, 476.0))
	}
}
