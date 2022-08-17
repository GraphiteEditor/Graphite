use super::{Bezier, CircleArc};
use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
use crate::utils::f64_compare;

use glam::DVec2;

pub fn compare_f64s(f1: f64, f2: f64) -> bool {
	f64_compare(f1, f2, MAX_ABSOLUTE_DIFFERENCE)
}

/// Compare points by allowing some maximum absolute difference to account for floating point errors
pub fn compare_points(p1: DVec2, p2: DVec2) -> bool {
	p1.abs_diff_eq(p2, MAX_ABSOLUTE_DIFFERENCE)
}

/// Compare vectors of points by allowing some maximum absolute difference to account for floating point errors
pub fn compare_vec_of_points(a: Vec<DVec2>, b: Vec<DVec2>, max_absolute_difference: f64) -> bool {
	a.len() == b.len() && a.into_iter().zip(b.into_iter()).all(|(p1, p2)| p1.abs_diff_eq(p2, max_absolute_difference))
}

/// Compare vectors of beziers by allowing some maximum absolute difference between points to account for floating point errors
pub fn compare_vector_of_beziers(beziers: &[Bezier], expected_bezier_points: Vec<Vec<DVec2>>) -> bool {
	beziers
		.iter()
		.zip(expected_bezier_points.iter())
		.all(|(&a, b)| compare_vec_of_points(a.get_points().collect::<Vec<DVec2>>(), b.to_vec(), MAX_ABSOLUTE_DIFFERENCE))
}

/// Compare circle arcs by allowing some maximum absolute difference between values to account for floating point errors
pub fn compare_arcs(arc1: CircleArc, arc2: CircleArc) -> bool {
	compare_points(arc1.center, arc2.center)
		&& f64_compare(arc1.radius, arc1.radius, MAX_ABSOLUTE_DIFFERENCE)
		&& f64_compare(arc1.start_angle, arc2.start_angle, MAX_ABSOLUTE_DIFFERENCE)
		&& f64_compare(arc1.end_angle, arc2.end_angle, MAX_ABSOLUTE_DIFFERENCE)
}
