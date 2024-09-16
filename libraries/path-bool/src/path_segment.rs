// Copyright 2024 Adam Platkevič <rflashster@gmail.com>
//
// SPDX-License-Identifier: MIT

use glam::{DMat2, DMat3, DVec2};
use std::f64::consts::{PI, TAU};

use crate::aabb::{bounding_box_around_point, expand_bounding_box, extend_bounding_box, merge_bounding_boxes, AaBb};
use crate::math::{deg2rad, lerp, vector_angle};
use crate::vector::{vectors_equal, Vector};
use crate::EPS;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathSegment {
	Line(Vector, Vector),
	Cubic(Vector, Vector, Vector, Vector),
	Quadratic(Vector, Vector, Vector),
	Arc(Vector, f64, f64, f64, bool, bool, Vector),
}

impl PathSegment {
	pub fn start_angle(&self) -> f64 {
		let angle = match *self {
			PathSegment::Line(start, end) => (end - start).angle_to(DVec2::X),
			PathSegment::Cubic(start, control1, control2, _) => {
				let diff = control1 - start;
				if vectors_equal(diff, DVec2::ZERO, EPS.point) {
					// if this diff were empty too, the segments would have been convertet to a line
					(control2 - start).angle_to(DVec2::X)
				} else {
					diff.angle_to(DVec2::X)
				}
			}
			// Apply same logic as for cubic bezier
			PathSegment::Quadratic(start, control, _) => (control - start).to_angle(),
			PathSegment::Arc(..) => arc_segment_to_cubics(self, 0.001)[0].start_angle(),
		};
		use std::f64::consts::TAU;
		(angle + TAU) % TAU
	}

	pub fn start_curvature(&self) -> f64 {
		match *self {
			PathSegment::Line(_, _) => 0.0,
			PathSegment::Cubic(start, control1, control2, _) => {
				let a = control1 - start;
				let a = 3. * a;
				let b = start - 2.0 * control1 + control2;
				let b = 6. * b;
				let numerator = a.x * b.y - a.y * b.x;
				let denominator = a.length_squared() * a.length();
				// dbg!(a, b, numerator, denominator);
				if denominator == 0.0 {
					0.0
				} else {
					numerator / denominator
				}
			}
			PathSegment::Quadratic(start, control, end) => {
				// first derivatiave
				let a = 2. * (control - start);
				// second derivatiave
				let b = 2. * (start - 2.0 * control + end);
				let numerator = a.x * b.y - a.y * b.x;
				let denominator = a.length_squared() * a.length();
				if denominator == 0.0 {
					0.0
				} else {
					numerator / denominator
				}
			}
			PathSegment::Arc(..) => arc_segment_to_cubics(self, 0.001)[0].start_curvature(),
		}
	}
	pub fn to_cubic(&self) -> [DVec2; 4] {
		match *self {
			PathSegment::Line(start, end) => [start, start, end, end],
			PathSegment::Cubic(s, c1, c2, e) => [s, c1, c2, e],
			PathSegment::Quadratic(start, control, end) => {
				// C0 = Q0
				// C1 = Q0 + (2/3) (Q1 - Q0)
				// C2 = Q2 + (2/3) (Q1 - Q2)
				// C3 = Q2
				let d1 = control - start;
				let d2 = control - end;
				[start, start + (2. / 3.) * d1, end + (2. / 3.) * d2, end]
			}
			PathSegment::Arc(..) => unimplemented!(),
		}
	}
}

pub struct PathArcSegmentCenterParametrization {
	center: Vector,
	theta1: f64,
	delta_theta: f64,
	rx: f64,
	ry: f64,
	phi: f64,
}

pub fn get_start_point(seg: &PathSegment) -> Vector {
	match seg {
		PathSegment::Line(start, _) => *start,
		PathSegment::Cubic(start, _, _, _) => *start,
		PathSegment::Quadratic(start, _, _) => *start,
		PathSegment::Arc(start, _, _, _, _, _, _) => *start,
	}
}

pub fn get_end_point(seg: &PathSegment) -> Vector {
	match seg {
		PathSegment::Line(_, end) => *end,
		PathSegment::Cubic(_, _, _, end) => *end,
		PathSegment::Quadratic(_, _, end) => *end,
		PathSegment::Arc(_, _, _, _, _, _, end) => *end,
	}
}

pub fn reverse_path_segment(seg: &PathSegment) -> PathSegment {
	match *seg {
		PathSegment::Line(start, end) => PathSegment::Line(end, start),
		PathSegment::Cubic(p1, p2, p3, p4) => PathSegment::Cubic(p4, p3, p2, p1),
		PathSegment::Quadratic(p1, p2, p3) => PathSegment::Quadratic(p3, p2, p1),
		PathSegment::Arc(start, rx, ry, phi, fa, fs, end) => PathSegment::Arc(end, rx, ry, phi, fa, !fs, start),
	}
}

pub fn arc_segment_to_center(seg: &PathSegment) -> Option<PathArcSegmentCenterParametrization> {
	if let PathSegment::Arc(xy1, rx, ry, phi, fa, fs, xy2) = *seg {
		if rx == 0.0 || ry == 0.0 {
			return None;
		}

		let rotation_matrix = DMat2::from_angle(-deg2rad(phi));
		let xy1_prime = rotation_matrix * (xy1 - xy2) * 0.5;

		let mut rx2 = rx * rx;
		let mut ry2 = ry * ry;
		let x1_prime2 = xy1_prime.x * xy1_prime.x;
		let y1_prime2 = xy1_prime.y * xy1_prime.y;

		let mut rx = rx.abs();
		let mut ry = ry.abs();
		let lambda = x1_prime2 / rx2 + y1_prime2 / ry2 + 1e-12;
		if lambda > 1.0 {
			let lambda_sqrt = lambda.sqrt();
			rx *= lambda_sqrt;
			ry *= lambda_sqrt;
			let lambda_abs = lambda.abs();
			rx2 *= lambda_abs;
			ry2 *= lambda_abs;
		}

		let sign = if fa == fs { -1.0 } else { 1.0 };
		let multiplier = ((rx2 * ry2 - rx2 * y1_prime2 - ry2 * x1_prime2) / (rx2 * y1_prime2 + ry2 * x1_prime2)).sqrt();
		let cx_prime = sign * multiplier * ((rx * xy1_prime.y) / ry);
		let cy_prime = sign * multiplier * ((-ry * xy1_prime.x) / rx);

		let cxy = rotation_matrix.transpose() * DVec2::new(cx_prime, cy_prime) + (xy1 + xy2) * 0.5;

		let vec1 = DVec2::new((xy1_prime.x - cx_prime) / rx, (xy1_prime.y - cy_prime) / ry);
		let theta1 = vector_angle(DVec2::new(1.0, 0.0), vec1);
		let mut delta_theta = vector_angle(vec1, DVec2::new((-xy1_prime.x - cx_prime) / rx, (-xy1_prime.y - cy_prime) / ry));

		if !fs && delta_theta > 0.0 {
			delta_theta -= TAU;
		} else if fs && delta_theta < 0.0 {
			delta_theta += TAU;
		}

		Some(PathArcSegmentCenterParametrization {
			center: cxy,
			theta1,
			delta_theta,
			rx,
			ry,
			phi,
		})
	} else {
		None
	}
}

pub fn arc_segment_from_center(params: &PathArcSegmentCenterParametrization, start: Option<Vector>, end: Option<Vector>) -> PathSegment {
	let rotation_matrix = DMat2::from_angle(params.phi);

	let mut xy1 = rotation_matrix * DVec2::new(params.rx * params.theta1.cos(), params.ry * params.theta1.sin()) + params.center;

	let mut xy2 = rotation_matrix * DVec2::new(params.rx * (params.theta1 + params.delta_theta).cos(), params.ry * (params.theta1 + params.delta_theta).sin()) + params.center;

	let fa = params.delta_theta.abs() > PI;
	let fs = params.delta_theta > 0.0;
	xy1 = start.unwrap_or(xy1);
	xy2 = end.unwrap_or(xy2);

	PathSegment::Arc(xy1, params.rx, params.ry, params.phi, fa, fs, xy2)
}

pub fn sample_path_segment_at(seg: &PathSegment, t: f64) -> Vector {
	match *seg {
		PathSegment::Line(start, end) => start.lerp(end, t),
		PathSegment::Cubic(p1, p2, p3, p4) => {
			let p01 = p1.lerp(p2, t);
			let p12 = p2.lerp(p3, t);
			let p23 = p3.lerp(p4, t);
			let p012 = p01.lerp(p12, t);
			let p123 = p12.lerp(p23, t);
			p012.lerp(p123, t)
		}
		PathSegment::Quadratic(p1, p2, p3) => {
			let p01 = p1.lerp(p2, t);
			let p12 = p2.lerp(p3, t);
			p01.lerp(p12, t)
		}
		PathSegment::Arc(start, rx, ry, phi, _, _, end) => {
			if let Some(center_param) = arc_segment_to_center(seg) {
				let theta = center_param.theta1 + t * center_param.delta_theta;
				let p = DVec2::new(rx * theta.cos(), ry * theta.sin());
				let rotation_matrix = DMat2::from_angle(phi);
				rotation_matrix * p + center_param.center
			} else {
				start.lerp(end, t)
			}
		}
	}
}

pub fn arc_segment_to_cubics(seg: &PathSegment, max_delta_theta: f64) -> Vec<PathSegment> {
	if let PathSegment::Arc(start, rx, ry, phi, _, _, end) = *seg {
		if let Some(center_param) = arc_segment_to_center(seg) {
			let count = ((center_param.delta_theta.abs() / max_delta_theta).ceil() as usize).max(1);

			let from_unit = DMat3::from_translation(center_param.center) * DMat3::from_angle(deg2rad(phi)) * DMat3::from_scale(DVec2::new(rx, ry));

			let theta = center_param.delta_theta / count as f64;
			let k = (4.0 / 3.0) * (theta / 4.0).tan();
			let sin_theta = theta.sin();
			let cos_theta = theta.cos();

			(0..count)
				.map(|i| {
					let start = DVec2::new(1.0, 0.0);
					let control1 = DVec2::new(1.0, k);
					let control2 = DVec2::new(cos_theta + k * sin_theta, sin_theta - k * cos_theta);
					let end = DVec2::new(cos_theta, sin_theta);

					let matrix = DMat3::from_angle(center_param.theta1 + i as f64 * theta) * from_unit;
					let start = (matrix * start.extend(1.0)).truncate();
					let control1 = (matrix * control1.extend(1.0)).truncate();
					let control2 = (matrix * control2.extend(1.0)).truncate();
					let end = (matrix * end.extend(1.0)).truncate();

					PathSegment::Cubic(start, control1, control2, end)
				})
				.collect()
		} else {
			vec![PathSegment::Line(start, end)]
		}
	} else {
		vec![*seg]
	}
}

fn eval_cubic_1d(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
	let p01 = lerp(p0, p1, t);
	let p12 = lerp(p1, p2, t);
	let p23 = lerp(p2, p3, t);
	let p012 = lerp(p01, p12, t);
	let p123 = lerp(p12, p23, t);
	lerp(p012, p123, t)
}

fn cubic_bounding_interval(p0: f64, p1: f64, p2: f64, p3: f64) -> (f64, f64) {
	let mut min = p0.min(p3);
	let mut max = p0.max(p3);

	let a = 3.0 * (-p0 + 3.0 * p1 - 3.0 * p2 + p3);
	let b = 6.0 * (p0 - 2.0 * p1 + p2);
	let c = 3.0 * (p1 - p0);
	let d = b * b - 4.0 * a * c;

	if d < 0.0 || a == 0.0 {
		// TODO: if a=0, solve linear
		return (min, max);
	}

	let sqrt_d = d.sqrt();

	let t0 = (-b - sqrt_d) / (2.0 * a);
	if 0.0 < t0 && t0 < 1.0 {
		let x0 = eval_cubic_1d(p0, p1, p2, p3, t0);
		min = min.min(x0);
		max = max.max(x0);
	}

	let t1 = (-b + sqrt_d) / (2.0 * a);
	if 0.0 < t1 && t1 < 1.0 {
		let x1 = eval_cubic_1d(p0, p1, p2, p3, t1);
		min = min.min(x1);
		max = max.max(x1);
	}

	(min, max)
}

fn eval_quadratic_1d(p0: f64, p1: f64, p2: f64, t: f64) -> f64 {
	let p01 = lerp(p0, p1, t);
	let p12 = lerp(p1, p2, t);
	lerp(p01, p12, t)
}

fn quadratic_bounding_interval(p0: f64, p1: f64, p2: f64) -> (f64, f64) {
	let mut min = p0.min(p2);
	let mut max = p0.max(p2);

	let denominator = p0 - 2.0 * p1 + p2;

	if denominator == 0.0 {
		return (min, max);
	}

	let t = (p0 - p1) / denominator;
	if (0.0..=1.0).contains(&t) {
		let x = eval_quadratic_1d(p0, p1, p2, t);
		min = min.min(x);
		max = max.max(x);
	}

	(min, max)
}

fn in_interval(x: f64, x0: f64, x1: f64) -> bool {
	let mapped = (x - x0) / (x1 - x0);
	(0.0..=1.0).contains(&mapped)
}

pub fn path_segment_bounding_box(seg: &PathSegment) -> AaBb {
	match *seg {
		PathSegment::Line(start, end) => AaBb {
			top: start.y.min(end.y),
			right: start.x.max(end.x),
			bottom: start.y.max(end.y),
			left: start.x.min(end.x),
		},
		PathSegment::Cubic(p1, p2, p3, p4) => {
			let (left, right) = cubic_bounding_interval(p1.x, p2.x, p3.x, p4.x);
			let (top, bottom) = cubic_bounding_interval(p1.y, p2.y, p3.y, p4.y);
			AaBb { top, right, bottom, left }
		}
		PathSegment::Quadratic(p1, p2, p3) => {
			let (left, right) = quadratic_bounding_interval(p1.x, p2.x, p3.x);
			let (top, bottom) = quadratic_bounding_interval(p1.y, p2.y, p3.y);
			AaBb { top, right, bottom, left }
		}
		PathSegment::Arc(start, rx, ry, phi, _, _, end) => {
			if let Some(center_param) = arc_segment_to_center(seg) {
				let theta2 = center_param.theta1 + center_param.delta_theta;
				let mut bounding_box = extend_bounding_box(Some(bounding_box_around_point(start, 0.0)), end);

				if phi == 0.0 || rx == ry {
					// FIXME: the following gives false positives, resulting in larger boxes
					if in_interval(-PI, center_param.theta1, theta2) || in_interval(PI, center_param.theta1, theta2) {
						bounding_box = extend_bounding_box(Some(bounding_box), DVec2::new(center_param.center.x - rx, center_param.center.y));
					}
					if in_interval(-PI / 2.0, center_param.theta1, theta2) || in_interval(3.0 * PI / 2.0, center_param.theta1, theta2) {
						bounding_box = extend_bounding_box(Some(bounding_box), DVec2::new(center_param.center.x, center_param.center.y - ry));
					}
					if in_interval(0.0, center_param.theta1, theta2) || in_interval(2.0 * PI, center_param.theta1, theta2) {
						bounding_box = extend_bounding_box(Some(bounding_box), DVec2::new(center_param.center.x + rx, center_param.center.y));
					}
					if in_interval(PI / 2.0, center_param.theta1, theta2) || in_interval(5.0 * PI / 2.0, center_param.theta1, theta2) {
						bounding_box = extend_bounding_box(Some(bounding_box), DVec2::new(center_param.center.x, center_param.center.y + ry));
					}
					expand_bounding_box(&bounding_box, 1e-11) // TODO: get rid of expansion
				} else {
					// TODO: don't convert to cubics
					let cubics = arc_segment_to_cubics(seg, PI / 16.0);
					let mut bounding_box = None;
					for cubic_seg in cubics {
						bounding_box = Some(merge_bounding_boxes(bounding_box, &path_segment_bounding_box(&cubic_seg)));
					}
					bounding_box.unwrap_or_else(|| bounding_box_around_point(start, 0.0))
				}
			} else {
				extend_bounding_box(Some(bounding_box_around_point(start, 0.0)), end)
			}
		}
	}
}

pub fn split_segment_at(seg: &PathSegment, t: f64) -> (PathSegment, PathSegment) {
	match *seg {
		PathSegment::Line(start, end) => {
			let p = start.lerp(end, t);
			(PathSegment::Line(start, p), PathSegment::Line(p, end))
		}
		PathSegment::Cubic(p0, p1, p2, p3) => {
			let p01 = p0.lerp(p1, t);
			let p12 = p1.lerp(p2, t);
			let p23 = p2.lerp(p3, t);
			let p012 = p01.lerp(p12, t);
			let p123 = p12.lerp(p23, t);
			let p = p012.lerp(p123, t);

			(PathSegment::Cubic(p0, p01, p012, p), PathSegment::Cubic(p, p123, p23, p3))
		}
		PathSegment::Quadratic(p0, p1, p2) => {
			let p01 = p0.lerp(p1, t);
			let p12 = p1.lerp(p2, t);
			let p = p01.lerp(p12, t);

			(PathSegment::Quadratic(p0, p01, p), PathSegment::Quadratic(p, p12, p2))
		}
		PathSegment::Arc(start, _, _, _, _, _, end) => {
			if let Some(center_param) = arc_segment_to_center(seg) {
				let mid_delta_theta = center_param.delta_theta * t;
				let seg1 = arc_segment_from_center(
					&PathArcSegmentCenterParametrization {
						delta_theta: mid_delta_theta,
						..center_param
					},
					Some(start),
					None,
				);
				let seg2 = arc_segment_from_center(
					&PathArcSegmentCenterParametrization {
						theta1: center_param.theta1 + mid_delta_theta,
						delta_theta: center_param.delta_theta - mid_delta_theta,
						..center_param
					},
					None,
					Some(end),
				);
				(seg1, seg2)
			} else {
				// https://svgwg.org/svg2-draft/implnote.html#ArcCorrectionOutOfRangeRadii
				let p = start.lerp(end, t);
				(PathSegment::Line(start, p), PathSegment::Line(p, end))
			}
		}
	}
}
