// Copyright 2024 Adam Platkevič <rflashster@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::vector::Vector;

pub type LineSegment = [Vector; 2];

const COLLINEAR_EPS: f64 = f64::EPSILON * 64.0;

pub fn line_segment_intersection([p1, p2]: LineSegment, [p3, p4]: LineSegment, eps: f64) -> Option<(f64, f64)> {
	// https://en.wikipedia.org/wiki/Intersection_(geometry)#Two_line_segments

	let a1 = p2.x - p1.x;
	let b1 = p3.x - p4.x;
	let c1 = p3.x - p1.x;
	let a2 = p2.y - p1.y;
	let b2 = p3.y - p4.y;
	let c2 = p3.y - p1.y;

	let denom = a1 * b2 - a2 * b1;

	if denom.abs() < COLLINEAR_EPS {
		return None;
	}

	let s = (c1 * b2 - c2 * b1) / denom;
	let t = (a1 * c2 - a2 * c1) / denom;

	if (-eps..=1.0 + eps).contains(&s) && (-eps..=1.0 + eps).contains(&t) {
		Some((s, t))
	} else {
		None
	}
}

pub fn line_segments_intersect(seg1: LineSegment, seg2: LineSegment, eps: f64) -> bool {
	line_segment_intersection(seg1, seg2, eps).is_some()
}
