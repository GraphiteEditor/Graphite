/// Comparison functions used for tests in the bezier module
#[cfg(test)]
use super::{CircleArc, Subpath};
#[cfg(test)]
use crate::utils::f64_compare;

use crate::consts::MAX_ABSOLUTE_DIFFERENCE;

use glam::DVec2;

// Compare two f64s with some maximum absolute difference to account for floating point errors
#[cfg(test)]
pub fn compare_f64s(f1: f64, f2: f64) -> bool {
	f64_compare(f1, f2, MAX_ABSOLUTE_DIFFERENCE)
}

/// Compare points by allowing some maximum absolute difference to account for floating point errors
pub fn compare_points(p1: DVec2, p2: DVec2) -> bool {
	p1.abs_diff_eq(p2, MAX_ABSOLUTE_DIFFERENCE)
}

/// Compare vectors of points by allowing some maximum absolute difference to account for floating point errors
#[cfg(test)]
pub fn compare_vec_of_points(a: Vec<DVec2>, b: Vec<DVec2>, max_absolute_difference: f64) -> bool {
	a.len() == b.len() && a.into_iter().zip(b).all(|(p1, p2)| p1.abs_diff_eq(p2, max_absolute_difference))
}

/// Compare circle arcs by allowing some maximum absolute difference between values to account for floating point errors
#[cfg(test)]
pub fn compare_arcs(arc1: CircleArc, arc2: CircleArc) -> bool {
	compare_points(arc1.center, arc2.center)
		&& f64_compare(arc1.radius, arc1.radius, MAX_ABSOLUTE_DIFFERENCE)
		&& f64_compare(arc1.start_angle, arc2.start_angle, MAX_ABSOLUTE_DIFFERENCE)
		&& f64_compare(arc1.end_angle, arc2.end_angle, MAX_ABSOLUTE_DIFFERENCE)
}

/// Compare Subpath by verifying that their bezier segments match.
/// In this way, matching quadratic segments where the handles are on opposite manipulator groups will be considered equal.
#[cfg(test)]
pub fn compare_subpaths<ManipulatorGroupId: crate::Identifier>(subpath1: &Subpath<ManipulatorGroupId>, subpath2: &Subpath<ManipulatorGroupId>) -> bool {
	subpath1.len() == subpath2.len() && subpath1.closed() == subpath2.closed() && subpath1.iter().eq(subpath2.iter())
}
