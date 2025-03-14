//! Defines the `PathSegment` enum and related functionality for representing and
//! manipulating path segments in 2D space.
//!
//! This module provides implementations for various types of path segments including
//! lines, cubic and quadratic Bézier curves, and elliptical arcs. It also includes
//! utility functions for operations such as bounding box calculation, segment splitting,
//! and arc-to-cubic conversion.
//!
//! The implementations in this module closely follow the SVG path specification,
//! making it suitable for use in vector graphics applications.

use crate::EPS;
use crate::aabb::{Aabb, bounding_box_around_point, expand_bounding_box, extend_bounding_box, merge_bounding_boxes};
use crate::math::{lerp, vector_angle};
use glam::{DMat2, DMat3, DVec2};
use std::f64::consts::{PI, TAU};

/// Represents a segment of a path in a 2D space, based on the SVG path specification.
///
/// This enum closely follows the path segment types defined in the SVG 2 specification.
/// For more details, see: <https://www.w3.org/TR/SVG2/paths.html>
///
/// Each variant of this enum corresponds to a different type of path segment:
/// - Line: A straight line between two points.
/// - Cubic: A cubic Bézier curve.
/// - Quadratic: A quadratic Bézier curve.
/// - Arc: An elliptical arc.
///
/// # Examples
///
/// Creating a line segment:
/// ```
/// use path_bool::PathSegment;
/// use glam::DVec2;
///
/// let line = PathSegment::Line(DVec2::new(0., 0.), DVec2::new(1., 1.));
/// ```
///
/// Creating a cubic Bézier curve:
/// ```
/// use path_bool::PathSegment;
/// use glam::DVec2;
///
/// let cubic = PathSegment::Cubic(
///     DVec2::new(0., 0.),
///     DVec2::new(1., 0.),
///     DVec2::new(1., 1.),
///     DVec2::new(2., 1.)
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathSegment {
	/// A line segment from the first point to the second.
	/// Corresponds to the SVG "L" command.
	Line(DVec2, DVec2),

	/// A cubic Bézier curve with start point, two control points, and end point.
	/// Corresponds to the SVG "C" command.
	Cubic(DVec2, DVec2, DVec2, DVec2),

	/// A quadratic Bézier curve with start point, control point, and end point.
	/// Corresponds to the SVG "Q" command.
	Quadratic(DVec2, DVec2, DVec2),

	/// An elliptical arc.
	/// Corresponds to the SVG "A" command.
	///
	/// Parameters:
	/// - Start point
	/// - X-axis radius
	/// - Y-axis radius
	/// - X-axis rotation (in radians)
	/// - Large arc flag (true if the arc should be greater than or equal to 180 degrees)
	/// - Sweep flag (true if the arc should be drawn in a "positive-angle" direction)
	/// - End point
	Arc(DVec2, f64, f64, f64, bool, bool, DVec2),
}

impl PathSegment {
	/// Calculates the angle of the tangent at the start point of the segment.
	///
	/// This method computes the angle (in radians) of the tangent vector at the
	/// beginning of the path segment. The angle is measured clockwise
	/// from the positive x-axis.
	///
	/// # Returns
	///
	/// A float representing the angle in radians, normalized to the range [0, 2π).
	///
	/// # Examples
	///
	/// ```
	/// use path_bool::PathSegment;
	/// use glam::DVec2;
	/// use std::f64::consts::{TAU, FRAC_PI_4};
	///
	/// let line = PathSegment::Line(DVec2::new(0., 0.), DVec2::new(1., 1.));
	/// assert_eq!(line.start_angle(), TAU  - (FRAC_PI_4));
	/// ```
	pub fn start_angle(&self) -> f64 {
		let angle = match *self {
			PathSegment::Line(start, end) => (end - start).angle_to(DVec2::X),
			PathSegment::Cubic(start, control1, control2, _) => {
				let diff = control1 - start;
				if diff.abs_diff_eq(DVec2::ZERO, EPS.point) {
					// if this diff were empty too, the segments would have been converted to a line
					(control2 - start).angle_to(DVec2::X)
				} else {
					diff.angle_to(DVec2::X)
				}
			}
			// Apply same logic as for cubic bezier
			PathSegment::Quadratic(start, control, _) => (control - start).to_angle(),
			PathSegment::Arc(..) => self.arc_segment_to_cubics(0.001)[0].start_angle(),
		};
		use std::f64::consts::TAU;
		(angle + TAU) % TAU
	}

	/// Computes the curvature at the start point of the segment.
	///
	/// The curvature is a measure of how sharply a curve bends. A straight line
	/// has a curvature of 0, while a tight curve has a higher curvature value.
	///
	/// # Returns
	///
	/// A float representing the curvature. Positive values indicate a left
	/// curve, while negative values indicate a right curve.
	///
	/// # Examples
	///
	/// ```
	/// use path_bool::PathSegment;
	/// use glam::DVec2;
	///
	/// let line = PathSegment::Line(DVec2::new(0., 0.), DVec2::new(1., 1.));
	/// assert_eq!(line.start_curvature(), 0.);
	///
	/// let curve = PathSegment::Cubic(
	///     DVec2::new(0., 0.),
	///     DVec2::new(0., 1.),
	///     DVec2::new(1., 1.),
	///     DVec2::new(1., 0.)
	/// );
	/// assert!(curve.start_curvature() < 0.);
	/// ```
	pub fn start_curvature(&self) -> f64 {
		match *self {
			PathSegment::Line(_, _) => 0.,
			PathSegment::Cubic(start, control1, control2, _) => {
				let a = control1 - start;
				let a = 3. * a;
				let b = start - 2. * control1 + control2;
				let b = 6. * b;
				let numerator = a.x * b.y - a.y * b.x;
				let denominator = a.length_squared() * a.length();
				if denominator == 0. { 0. } else { numerator / denominator }
			}
			PathSegment::Quadratic(start, control, end) => {
				// First derivative
				let a = 2. * (control - start);
				// Second derivative
				let b = 2. * (start - 2. * control + end);
				let numerator = a.x * b.y - a.y * b.x;
				let denominator = a.length_squared() * a.length();
				if denominator == 0. { 0. } else { numerator / denominator }
			}
			PathSegment::Arc(..) => self.arc_segment_to_cubics(0.001)[0].start_curvature(),
		}
	}
	/// Converts the segment to a cubic Bézier curve representation.
	///
	/// This method provides a uniform representation of all segment types as
	/// cubic Bézier curves. For segments that are not naturally cubic Bézier
	/// curves (like lines or quadratic Bézier curves), an equivalent cubic
	/// Bézier representation is computed.
	///
	/// # Returns
	///
	/// An array of four `DVec2` points representing the cubic Bézier curve:
	/// [start point, first control point, second control point, end point]
	///
	/// # Examples
	///
	/// ```
	/// use path_bool::PathSegment;
	/// use glam::DVec2;
	///
	/// let line = PathSegment::Line(DVec2::new(0., 0.), DVec2::new(1., 1.));
	/// let cubic = line.to_cubic();
	/// assert_eq!(cubic[0], DVec2::new(0., 0.));
	/// assert_eq!(cubic[3], DVec2::new(1., 1.));
	/// ```
	///
	/// # Panics
	///
	/// This method is not implemented for `PathSegment::Arc`. Attempting to call
	/// `to_cubic()` on an `Arc` segment will result in a panic.
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

	#[must_use]
	/// Retrieves the start point of a path segment.
	pub fn start(&self) -> DVec2 {
		match self {
			PathSegment::Line(start, _) => *start,
			PathSegment::Cubic(start, _, _, _) => *start,
			PathSegment::Quadratic(start, _, _) => *start,
			PathSegment::Arc(start, _, _, _, _, _, _) => *start,
		}
	}

	#[must_use]
	/// Retrieves the end point of a path segment.
	pub fn end(&self) -> DVec2 {
		match self {
			PathSegment::Line(_, end) => *end,
			PathSegment::Cubic(_, _, _, end) => *end,
			PathSegment::Quadratic(_, _, end) => *end,
			PathSegment::Arc(_, _, _, _, _, _, end) => *end,
		}
	}

	#[must_use]
	/// Reverses the direction of the path segment.
	///
	/// This method creates a new `PathSegment` that represents the same geometric shape
	/// but in the opposite direction.
	///
	/// # Examples
	///
	/// ```
	/// use path_bool::PathSegment;
	/// use glam::DVec2;
	///
	/// let line = PathSegment::Line(DVec2::new(0., 0.), DVec2::new(1., 1.));
	/// let reversed = line.reverse();
	/// assert_eq!(reversed.start(), DVec2::new(1., 1.));
	/// assert_eq!(reversed.end(), DVec2::new(0., 0.));
	/// ```
	pub fn reverse(&self) -> PathSegment {
		match *self {
			PathSegment::Line(start, end) => PathSegment::Line(end, start),
			PathSegment::Cubic(p1, p2, p3, p4) => PathSegment::Cubic(p4, p3, p2, p1),
			PathSegment::Quadratic(p1, p2, p3) => PathSegment::Quadratic(p3, p2, p1),
			PathSegment::Arc(start, rx, ry, phi, fa, fs, end) => PathSegment::Arc(end, rx, ry, phi, fa, !fs, start),
		}
	}

	#[must_use]
	/// Converts an arc segment to its center parameterization.
	///
	/// This method is only meaningful for `Arc` segments. For other segment types,
	/// it returns `None`.
	///
	/// # Returns
	///
	/// An `Option` containing `PathArcSegmentCenterParametrization` if the segment
	/// is an `Arc`, or `None` otherwise.
	pub fn arc_segment_to_center(&self) -> Option<PathArcSegmentCenterParametrization> {
		if let PathSegment::Arc(xy1, rx, ry, phi, fa, fs, xy2) = *self {
			if rx == 0. || ry == 0. {
				return None;
			}

			let rotation_matrix = DMat2::from_angle(-phi.to_radians());
			let xy1_prime = rotation_matrix * (xy1 - xy2) * 0.5;

			let mut rx2 = rx * rx;
			let mut ry2 = ry * ry;
			let x1_prime2 = xy1_prime.x * xy1_prime.x;
			let y1_prime2 = xy1_prime.y * xy1_prime.y;

			let mut rx = rx.abs();
			let mut ry = ry.abs();
			let lambda = x1_prime2 / rx2 + y1_prime2 / ry2 + 1e-12;
			if lambda > 1. {
				let lambda_sqrt = lambda.sqrt();
				rx *= lambda_sqrt;
				ry *= lambda_sqrt;
				let lambda_abs = lambda.abs();
				rx2 *= lambda_abs;
				ry2 *= lambda_abs;
			}

			let sign = if fa == fs { -1. } else { 1. };
			let multiplier = ((rx2 * ry2 - rx2 * y1_prime2 - ry2 * x1_prime2) / (rx2 * y1_prime2 + ry2 * x1_prime2)).sqrt();
			let cx_prime = sign * multiplier * ((rx * xy1_prime.y) / ry);
			let cy_prime = sign * multiplier * ((-ry * xy1_prime.x) / rx);

			let cxy = rotation_matrix.transpose() * DVec2::new(cx_prime, cy_prime) + (xy1 + xy2) * 0.5;

			let vec1 = DVec2::new((xy1_prime.x - cx_prime) / rx, (xy1_prime.y - cy_prime) / ry);
			let theta1 = vector_angle(DVec2::new(1., 0.), vec1);
			let mut delta_theta = vector_angle(vec1, DVec2::new((-xy1_prime.x - cx_prime) / rx, (-xy1_prime.y - cy_prime) / ry));

			if !fs && delta_theta > 0. {
				delta_theta -= TAU;
			} else if fs && delta_theta < 0. {
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

	#[must_use]
	/// Samples a point on the path segment at a given parameter value.
	///
	/// # Arguments
	///
	/// * `t` - A value between 0. and 1. representing the position along the segment.
	///
	/// # Examples
	///
	/// ```
	/// use path_bool::PathSegment;
	/// use glam::DVec2;
	///
	/// let line = PathSegment::Line(DVec2::new(0., 0.), DVec2::new(2., 2.));
	/// assert_eq!(line.sample_at(0.5), DVec2::new(1., 1.));
	/// ```
	pub fn sample_at(&self, t: f64) -> DVec2 {
		match *self {
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
				if let Some(center_param) = self.arc_segment_to_center() {
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

	#[must_use]
	/// Approximates an arc segment with a series of cubic Bézier curves.
	///
	/// This method is primarily used for `Arc` segments, converting them into
	/// a series of cubic Bézier curves for easier rendering or manipulation.
	/// For non-`Arc` segments, it returns a vector containing only the original segment.
	///
	/// # Arguments
	///
	/// * `max_delta_theta` - The maximum angle (in radians) that each cubic Bézier
	///   curve approximation should span.
	///
	/// # Returns
	///
	/// A vector of `PathSegment::Cubic` approximating the original segment.
	pub fn arc_segment_to_cubics(&self, max_delta_theta: f64) -> Vec<PathSegment> {
		if let PathSegment::Arc(start, rx, ry, phi, _, _, end) = *self {
			if let Some(center_param) = self.arc_segment_to_center() {
				let count = ((center_param.delta_theta.abs() / max_delta_theta).ceil() as usize).max(1);

				let from_unit = DMat3::from_translation(center_param.center) * DMat3::from_angle(phi.to_radians()) * DMat3::from_scale(DVec2::new(rx, ry));

				let theta = center_param.delta_theta / count as f64;
				let k = (4. / 3.) * (theta / 4.).tan();
				let sin_theta = theta.sin();
				let cos_theta = theta.cos();

				(0..count)
					.map(|i| {
						let start = DVec2::new(1., 0.);
						let control1 = DVec2::new(1., k);
						let control2 = DVec2::new(cos_theta + k * sin_theta, sin_theta - k * cos_theta);
						let end = DVec2::new(cos_theta, sin_theta);

						let matrix = DMat3::from_angle(center_param.theta1 + i as f64 * theta) * from_unit;
						let start = (matrix * start.extend(1.)).truncate();
						let control1 = (matrix * control1.extend(1.)).truncate();
						let control2 = (matrix * control2.extend(1.)).truncate();
						let end = (matrix * end.extend(1.)).truncate();

						PathSegment::Cubic(start, control1, control2, end)
					})
					.collect()
			} else {
				vec![PathSegment::Line(start, end)]
			}
		} else {
			vec![*self]
		}
	}
}

/// Represents the center parameterization of an elliptical arc.
///
/// This struct is used internally to perform calculations on arc segments.
pub struct PathArcSegmentCenterParametrization {
	center: DVec2,
	theta1: f64,
	delta_theta: f64,
	rx: f64,
	ry: f64,
	phi: f64,
}

/// Converts the center parameterization back to an arc segment.
///
/// # Arguments
///
/// * `start` - Optional start point of the arc. If `None`, the start point is calculated.
/// * `end` - Optional end point of the arc. If `None`, the end point is calculated.
///
/// # Returns
///
/// A `PathSegment::Arc` representing the arc described by this parameterization.
impl PathArcSegmentCenterParametrization {
	#[must_use]
	pub fn arc_segment_from_center(&self, start: Option<DVec2>, end: Option<DVec2>) -> PathSegment {
		let rotation_matrix = DMat2::from_angle(self.phi);

		let mut xy1 = rotation_matrix * DVec2::new(self.rx * self.theta1.cos(), self.ry * self.theta1.sin()) + self.center;

		let mut xy2 = rotation_matrix * DVec2::new(self.rx * (self.theta1 + self.delta_theta).cos(), self.ry * (self.theta1 + self.delta_theta).sin()) + self.center;

		let fa = self.delta_theta.abs() > PI;
		let fs = self.delta_theta > 0.;
		xy1 = start.unwrap_or(xy1);
		xy2 = end.unwrap_or(xy2);

		PathSegment::Arc(xy1, self.rx, self.ry, self.phi, fa, fs, xy2)
	}
}

/// Evaluates a 1D cubic Bézier curve at a given parameter value.
///
/// # Arguments
///
/// * `p0`, `p1`, `p2`, `p3` - Control points of the cubic Bézier curve.
/// * `t` - Parameter value between 0 and 1.
///
/// # Returns
///
/// The value of the Bézier curve at parameter `t`.
fn eval_cubic_1d(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
	let p01 = lerp(p0, p1, t);
	let p12 = lerp(p1, p2, t);
	let p23 = lerp(p2, p3, t);
	let p012 = lerp(p01, p12, t);
	let p123 = lerp(p12, p23, t);
	lerp(p012, p123, t)
}

/// Computes the bounding interval of a 1D cubic Bézier curve.
///
/// This function finds the minimum and maximum values of a cubic Bézier curve
/// over the interval [0, 1].
///
/// # Arguments
///
/// * `p0`, `p1`, `p2`, `p3` - Control points of the cubic Bézier curve.
///
/// # Returns
///
/// A tuple `(min, max)` representing the bounding interval.
fn cubic_bounding_interval(p0: f64, p1: f64, p2: f64, p3: f64) -> (f64, f64) {
	let mut min = p0.min(p3);
	let mut max = p0.max(p3);

	let a = 3. * (-p0 + 3. * p1 - 3. * p2 + p3);
	let b = 6. * (p0 - 2. * p1 + p2);
	let c = 3. * (p1 - p0);
	let d = b * b - 4. * a * c;

	if d < 0. || a == 0. {
		// TODO: if a=0, solve linear
		return (min, max);
	}

	let sqrt_d = d.sqrt();

	let t0 = (-b - sqrt_d) / (2. * a);
	if 0. < t0 && t0 < 1. {
		let x0 = eval_cubic_1d(p0, p1, p2, p3, t0);
		min = min.min(x0);
		max = max.max(x0);
	}

	let t1 = (-b + sqrt_d) / (2. * a);
	if 0. < t1 && t1 < 1. {
		let x1 = eval_cubic_1d(p0, p1, p2, p3, t1);
		min = min.min(x1);
		max = max.max(x1);
	}

	(min, max)
}

/// Evaluates a 1D quadratic Bézier curve at a given parameter value.
///
/// # Arguments
///
/// * `p0`, `p1`, `p2` - Control points of the quadratic Bézier curve.
/// * `t` - Parameter value between 0 and 1.
///
/// # Returns
///
/// The value of the Bézier curve at parameter `t`.
fn eval_quadratic_1d(p0: f64, p1: f64, p2: f64, t: f64) -> f64 {
	let p01 = lerp(p0, p1, t);
	let p12 = lerp(p1, p2, t);
	lerp(p01, p12, t)
}

/// Computes the bounding interval of a 1D quadratic Bézier curve.
///
/// This function finds the minimum and maximum values of a quadratic Bézier curve
/// over the interval [0, 1].
///
/// # Arguments
///
/// * `p0`, `p1`, `p2` - Control points of the quadratic Bézier curve.
///
/// # Returns
///
/// A tuple `(min, max)` representing the bounding interval.
fn quadratic_bounding_interval(p0: f64, p1: f64, p2: f64) -> (f64, f64) {
	let mut min = p0.min(p2);
	let mut max = p0.max(p2);

	let denominator = p0 - 2. * p1 + p2;

	if denominator == 0. {
		return (min, max);
	}

	let t = (p0 - p1) / denominator;
	if (0.0..=1.).contains(&t) {
		let x = eval_quadratic_1d(p0, p1, p2, t);
		min = min.min(x);
		max = max.max(x);
	}

	(min, max)
}

fn in_interval(x: f64, x0: f64, x1: f64) -> bool {
	(x0..=x1).contains(&x)
}

impl PathSegment {
	/// Computes the bounding box of the path segment.
	///
	/// # Returns
	///
	/// An [`Aabb`] representing the axis-aligned bounding box of the segment.
	pub(crate) fn bounding_box(&self) -> Aabb {
		match *self {
			PathSegment::Line(start, end) => Aabb {
				top: start.y.min(end.y),
				right: start.x.max(end.x),
				bottom: start.y.max(end.y),
				left: start.x.min(end.x),
			},
			PathSegment::Cubic(p1, p2, p3, p4) => {
				let (left, right) = cubic_bounding_interval(p1.x, p2.x, p3.x, p4.x);
				let (top, bottom) = cubic_bounding_interval(p1.y, p2.y, p3.y, p4.y);
				Aabb { top, right, bottom, left }
			}
			PathSegment::Quadratic(p1, p2, p3) => {
				let (left, right) = quadratic_bounding_interval(p1.x, p2.x, p3.x);
				let (top, bottom) = quadratic_bounding_interval(p1.y, p2.y, p3.y);
				Aabb { top, right, bottom, left }
			}
			PathSegment::Arc(start, rx, ry, phi, _, _, end) => {
				if let Some(center_param) = self.arc_segment_to_center() {
					let theta2 = center_param.theta1 + center_param.delta_theta;
					let mut bounding_box = extend_bounding_box(Some(bounding_box_around_point(start, 0.)), end);

					if phi == 0. || rx == ry {
						// TODO: Fix the fact that the following gives false positives, resulting in larger boxes
						if in_interval(-PI, center_param.theta1, theta2) || in_interval(PI, center_param.theta1, theta2) {
							bounding_box = extend_bounding_box(Some(bounding_box), DVec2::new(center_param.center.x - rx, center_param.center.y));
						}
						if in_interval(-PI / 2., center_param.theta1, theta2) || in_interval(3. * PI / 2., center_param.theta1, theta2) {
							bounding_box = extend_bounding_box(Some(bounding_box), DVec2::new(center_param.center.x, center_param.center.y - ry));
						}
						if in_interval(0., center_param.theta1, theta2) || in_interval(2. * PI, center_param.theta1, theta2) {
							bounding_box = extend_bounding_box(Some(bounding_box), DVec2::new(center_param.center.x + rx, center_param.center.y));
						}
						if in_interval(PI / 2., center_param.theta1, theta2) || in_interval(5. * PI / 2., center_param.theta1, theta2) {
							bounding_box = extend_bounding_box(Some(bounding_box), DVec2::new(center_param.center.x, center_param.center.y + ry));
						}
						expand_bounding_box(&bounding_box, 1e-11) // TODO: Get rid of expansion
					} else {
						// TODO: Don't convert to cubics
						let cubics = self.arc_segment_to_cubics(PI / 16.);
						let mut bounding_box = None;
						for cubic_seg in cubics {
							bounding_box = Some(merge_bounding_boxes(bounding_box, &cubic_seg.bounding_box()));
						}
						bounding_box.unwrap_or_else(|| bounding_box_around_point(start, 0.))
					}
				} else {
					extend_bounding_box(Some(bounding_box_around_point(start, 0.)), end)
				}
			}
		}
	}

	/// Splits the path segment at a given parameter value.
	///
	/// # Arguments
	///
	/// * `t` - A value between 0. and 1. representing the split point along the segment.
	///
	/// # Returns
	///
	/// A tuple of two `PathSegment`s representing the parts before and after the split point.
	///
	/// # Examples
	///
	/// ```
	/// use path_bool::PathSegment;
	/// use glam::DVec2;
	///
	/// let line = PathSegment::Line(DVec2::new(0., 0.), DVec2::new(2., 2.));
	/// let (first_half, second_half) = line.split_at(0.5);
	/// assert_eq!(first_half.end(), DVec2::new(1., 1.));
	/// assert_eq!(second_half.start(), DVec2::new(1., 1.));
	/// ```
	pub fn split_at(&self, t: f64) -> (PathSegment, PathSegment) {
		match *self {
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
				if let Some(center_param) = self.arc_segment_to_center() {
					let mid_delta_theta = center_param.delta_theta * t;
					let seg1 = PathArcSegmentCenterParametrization {
						delta_theta: mid_delta_theta,
						..center_param
					}
					.arc_segment_from_center(Some(start), None);
					let seg2 = PathArcSegmentCenterParametrization {
						theta1: center_param.theta1 + mid_delta_theta,
						delta_theta: center_param.delta_theta - mid_delta_theta,
						..center_param
					}
					.arc_segment_from_center(None, Some(end));
					(seg1, seg2)
				} else {
					// https://svgwg.org/svg2-draft/implnote.html#ArcCorrectionOutOfRangeRadii
					let p = start.lerp(end, t);
					(PathSegment::Line(start, p), PathSegment::Line(p, end))
				}
			}
		}
	}
}
