use super::contants::MAX_ABSOLUTE_DIFFERENCE;
use crate::vector::misc::point_to_dvec2;

use glam::BVec2;
use kurbo::Point;

// Compare two f64s with some maximum absolute difference to account for floating point errors
#[cfg(test)]
pub fn compare_f64s(f1: f64, f2: f64) -> bool {
	(f1 - f2).abs() < MAX_ABSOLUTE_DIFFERENCE
}

/// Compare points by allowing some maximum absolute difference to account for floating point errors
pub fn compare_points(p1: Point, p2: Point) -> bool {
	let (p1, p2) = (point_to_dvec2(p1), point_to_dvec2(p2));
	p1.abs_diff_eq(p2, MAX_ABSOLUTE_DIFFERENCE)
}

/// Compare vectors of points by allowing some maximum absolute difference to account for floating point errors
#[cfg(test)]
pub fn compare_vec_of_points(a: Vec<Point>, b: Vec<Point>, max_absolute_difference: f64) -> bool {
	a.len() == b.len()
		&& a.into_iter()
			.zip(b)
			.map(|(p1, p2)| (point_to_dvec2(p1), point_to_dvec2(p2)))
			.all(|(p1, p2)| p1.abs_diff_eq(p2, max_absolute_difference))
}

/// Compare the two values in a `DVec2` independently with a provided max absolute value difference.
pub fn dvec2_compare(a: Point, b: Point, max_abs_diff: f64) -> BVec2 {
	BVec2::new((a.x - b.x).abs() < max_abs_diff, (a.y - b.y).abs() < max_abs_diff)
}
