use glam::DVec2;
use kurbo::{ParamCurve, ParamCurveDeriv, PathSeg};

pub fn segment_tangent(segment: PathSeg, t: f64) -> DVec2 {
	// NOTE: .deriv() method gives inaccurate result when it is 1.
	let t = if t == 1. { 1. - f64::EPSILON } else { t };

	let tangent = match segment {
		PathSeg::Line(line) => line.deriv().eval(t),
		PathSeg::Quad(quad_bez) => quad_bez.deriv().eval(t),
		PathSeg::Cubic(cubic_bez) => cubic_bez.deriv().eval(t),
	};

	DVec2::new(tangent.x, tangent.y)
}

// Compare two f64s with some maximum absolute difference to account for floating point errors
#[cfg(test)]
pub fn compare_f64s(f1: f64, f2: f64) -> bool {
	(f1 - f2).abs() < super::contants::MAX_ABSOLUTE_DIFFERENCE
}

/// Compare points by allowing some maximum absolute difference to account for floating point errors
#[cfg(test)]
pub fn compare_points(p1: kurbo::Point, p2: kurbo::Point) -> bool {
	let (p1, p2) = (crate::vector::misc::point_to_dvec2(p1), crate::vector::misc::point_to_dvec2(p2));
	p1.abs_diff_eq(p2, super::contants::MAX_ABSOLUTE_DIFFERENCE)
}

/// Compare vectors of points by allowing some maximum absolute difference to account for floating point errors
#[cfg(test)]
pub fn compare_vec_of_points(a: Vec<kurbo::Point>, b: Vec<kurbo::Point>, max_absolute_difference: f64) -> bool {
	a.len() == b.len()
		&& a.into_iter()
			.zip(b)
			.map(|(p1, p2)| (crate::vector::misc::point_to_dvec2(p1), crate::vector::misc::point_to_dvec2(p2)))
			.all(|(p1, p2)| p1.abs_diff_eq(p2, max_absolute_difference))
}

/// Compare the two values in a `DVec2` independently with a provided max absolute value difference.
#[cfg(test)]
pub fn dvec2_compare(a: kurbo::Point, b: kurbo::Point, max_abs_diff: f64) -> glam::BVec2 {
	glam::BVec2::new((a.x - b.x).abs() < max_abs_diff, (a.y - b.y).abs() < max_abs_diff)
}
