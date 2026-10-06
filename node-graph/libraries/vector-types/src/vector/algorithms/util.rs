use glam::DVec2;
use kurbo::{ParamCurve, ParamCurveDeriv, PathSeg};

pub fn pathseg_tangent(segment: PathSeg, t: f64) -> DVec2 {
	// NOTE: .deriv() method gives inaccurate result when it is 1.
	let t = if t == 1. { 1. - f64::EPSILON } else { t };

	let tangent = match segment {
		PathSeg::Line(line) => line.deriv().eval(t),
		PathSeg::Quad(quad_bez) => quad_bez.deriv().eval(t),
		PathSeg::Cubic(cubic_bez) => cubic_bez.deriv().eval(t),
	};

	DVec2::new(tangent.x, tangent.y)
}

/// The direction a segment runs as it leaves (or enters) its endpoint, as a unit vector or zero when the segment is a point.
///
/// A handle left sitting on its anchor has a zero derivative, which carries no direction, so the first control point
/// that actually differs from the endpoint is what an endpoint marker should face.
pub fn pathseg_endpoint_tangent(segment: PathSeg, at_start: bool) -> DVec2 {
	fn direction_from(mut points: impl Iterator<Item = kurbo::Point>, flip: bool) -> DVec2 {
		let Some(endpoint) = points.next() else { return DVec2::ZERO };
		for control in points {
			let direction = DVec2::new(control.x - endpoint.x, control.y - endpoint.y);
			if direction.length_squared() > f64::EPSILON {
				// An endpoint marker faces the way the path travels, which points back toward the endpoint from inside
				return if flip { -direction } else { direction }.normalize_or_zero();
			}
		}
		DVec2::ZERO
	}

	match segment {
		PathSeg::Line(line) if at_start => direction_from([line.p0, line.p1].into_iter(), false),
		PathSeg::Line(line) => direction_from([line.p1, line.p0].into_iter(), true),
		PathSeg::Quad(quad) if at_start => direction_from([quad.p0, quad.p1, quad.p2].into_iter(), false),
		PathSeg::Quad(quad) => direction_from([quad.p2, quad.p1, quad.p0].into_iter(), true),
		PathSeg::Cubic(cubic) if at_start => direction_from([cubic.p0, cubic.p1, cubic.p2, cubic.p3].into_iter(), false),
		PathSeg::Cubic(cubic) => direction_from([cubic.p3, cubic.p2, cubic.p1, cubic.p0].into_iter(), true),
	}
}

/// Compare points by allowing some maximum absolute difference to account for floating point errors
#[cfg(test)]
pub(crate) fn compare_points(p1: kurbo::Point, p2: kurbo::Point) -> bool {
	let (p1, p2) = (crate::vector::misc::point_to_dvec2(p1), crate::vector::misc::point_to_dvec2(p2));
	p1.abs_diff_eq(p2, super::consts::MAX_ABSOLUTE_DIFFERENCE)
}

/// Compare vectors of points by allowing some maximum absolute difference to account for floating point errors
#[cfg(test)]
pub(crate) fn compare_vec_of_points(a: Vec<kurbo::Point>, b: Vec<kurbo::Point>, max_absolute_difference: f64) -> bool {
	a.len() == b.len()
		&& a.into_iter()
			.zip(b)
			.map(|(p1, p2)| (crate::vector::misc::point_to_dvec2(p1), crate::vector::misc::point_to_dvec2(p2)))
			.all(|(p1, p2)| p1.abs_diff_eq(p2, max_absolute_difference))
}

/// Compare the two values in a `DVec2` independently with a provided max absolute value difference.
#[cfg(test)]
pub(crate) fn dvec2_compare(a: kurbo::Point, b: kurbo::Point, max_abs_diff: f64) -> glam::BVec2 {
	glam::BVec2::new((a.x - b.x).abs() < max_abs_diff, (a.y - b.y).abs() < max_abs_diff)
}
