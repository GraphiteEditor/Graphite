use crate::consts::{MAX_ABSOLUTE_DIFFERENCE, STRICT_MAX_ABSOLUTE_DIFFERENCE};

use glam::{BVec2, DVec2};
use std::f64::consts::PI;

/// Helper to perform the computation of a and c, where b is the provided point on the curve.
/// Given the correct power of `t` and `(1-t)`, the computation is the same for quadratic and cubic cases.
/// Relevant derivation and the definitions of a, b, and c can be found in [the projection identity section](https://pomax.github.io/bezierinfo/#abc) of Pomax's bezier curve primer.
fn compute_abc_through_points(start_point: DVec2, point_on_curve: DVec2, end_point: DVec2, t_to_nth_power: f64, nth_power_of_one_minus_t: f64) -> [DVec2; 3] {
	let point_c_ratio = nth_power_of_one_minus_t / (t_to_nth_power + nth_power_of_one_minus_t);
	let c = point_c_ratio * start_point + (1. - point_c_ratio) * end_point;
	let ab_bc_ratio = (t_to_nth_power + nth_power_of_one_minus_t - 1.).abs() / (t_to_nth_power + nth_power_of_one_minus_t);
	let a = point_on_curve + (point_on_curve - c) / ab_bc_ratio;
	[a, point_on_curve, c]
}

/// Compute `a`, `b`, and `c` for a quadratic curve that fits the start, end and point on curve at `t`.
/// The definition for the `a`, `b`, `c` points are defined in [the projection identity section](https://pomax.github.io/bezierinfo/#abc) of Pomax's bezier curve primer.
pub fn compute_abc_for_quadratic_through_points(start_point: DVec2, point_on_curve: DVec2, end_point: DVec2, t: f64) -> [DVec2; 3] {
	let t_squared = t * t;
	let one_minus_t = 1. - t;
	let squared_one_minus_t = one_minus_t * one_minus_t;
	compute_abc_through_points(start_point, point_on_curve, end_point, t_squared, squared_one_minus_t)
}

/// Compute `a`, `b`, and `c` for a cubic curve that fits the start, end and point on curve at `t`.
/// The definition for the `a`, `b`, `c` points are defined in [the projection identity section](https://pomax.github.io/bezierinfo/#abc) of Pomax's bezier curve primer.
pub fn compute_abc_for_cubic_through_points(start_point: DVec2, point_on_curve: DVec2, end_point: DVec2, t: f64) -> [DVec2; 3] {
	let t_cubed = t * t * t;
	let one_minus_t = 1. - t;
	let cubed_one_minus_t = one_minus_t * one_minus_t * one_minus_t;

	compute_abc_through_points(start_point, point_on_curve, end_point, t_cubed, cubed_one_minus_t)
}

/// Return the index and the value of the closest point in the LUT compared to the provided point.
pub fn get_closest_point_in_lut(lut: &[DVec2], point: DVec2) -> (i32, f64) {
	lut.iter()
		.enumerate()
		.map(|(i, p)| (i as i32, point.distance_squared(*p)))
		.min_by(|x, y| (&(x.1)).partial_cmp(&(y.1)).unwrap())
		.unwrap()
}

// TODO: Use an `Option` return type instead of a `Vec`
/// Find the roots of the linear equation `ax + b`.
pub fn solve_linear(a: f64, b: f64) -> Vec<f64> {
	let mut roots = Vec::new();
	// There exist roots when `a` is not 0
	if a.abs() > MAX_ABSOLUTE_DIFFERENCE {
		roots.push(-b / a);
	}
	roots
}

// TODO: Use an `impl Iterator` return type instead of a `Vec`
/// Find the roots of the linear equation `ax^2 + bx + c`.
/// Precompute the `discriminant` (`b^2 - 4ac`) and `two_times_a` arguments prior to calling this function for efficiency purposes.
pub fn solve_quadratic(discriminant: f64, two_times_a: f64, b: f64, c: f64) -> Vec<f64> {
	let mut roots = Vec::new();
	if two_times_a != 0. {
		if discriminant > 0. {
			let root_discriminant = discriminant.sqrt();
			roots.push((-b + root_discriminant) / (two_times_a));
			roots.push((-b - root_discriminant) / (two_times_a));
		} else if discriminant == 0. {
			roots.push(-b / (two_times_a));
		}
	} else {
		roots = solve_linear(b, c);
	}
	roots
}

/// Compute the cube root of a number.
fn cube_root(f: f64) -> f64 {
	if f < 0. {
		-(-f).powf(1. / 3.)
	} else {
		f.powf(1. / 3.)
	}
}

// TODO: Use an `impl Iterator` return type instead of a `Vec`
/// Solve a cubic of the form `x^3 + px + q`, derivation from: <https://trans4mind.com/personal_development/mathematics/polynomials/cubicAlgebra.htm>.
pub fn solve_reformatted_cubic(discriminant: f64, a: f64, p: f64, q: f64) -> Vec<f64> {
	let mut roots = Vec::new();
	if p.abs() <= STRICT_MAX_ABSOLUTE_DIFFERENCE {
		// Handle when p is approximately 0
		roots.push(cube_root(-q));
	} else if q.abs() <= STRICT_MAX_ABSOLUTE_DIFFERENCE {
		// Handle when q is approximately 0
		if p < 0. {
			roots.push((-p).powf(1. / 2.));
		}
	} else if discriminant.abs() <= STRICT_MAX_ABSOLUTE_DIFFERENCE {
		// When discriminant is 0 (check for approximation because of floating point errors), all roots are real, and 2 are repeated
		let q_divided_by_2 = q / 2.;
		let a_divided_by_3 = a / 3.;

		roots.push(2. * cube_root(-q_divided_by_2) - a_divided_by_3);
		roots.push(cube_root(q_divided_by_2) - a_divided_by_3);
	} else if discriminant > 0. {
		// When discriminant > 0, there is one real and two imaginary roots
		let q_divided_by_2 = q / 2.;
		let square_root_discriminant = discriminant.powf(1. / 2.);

		roots.push(cube_root(-q_divided_by_2 + square_root_discriminant) - cube_root(q_divided_by_2 + square_root_discriminant) - a / 3.);
	} else {
		// Otherwise, discriminant < 0 and there are three real roots
		let p_divided_by_3 = p / 3.;
		let a_divided_by_3 = a / 3.;
		let cube_root_r = (-p_divided_by_3).powf(1. / 2.);
		let phi = (-q / (2. * cube_root_r.powi(3))).acos();

		let two_times_cube_root_r = 2. * cube_root_r;
		roots.push(two_times_cube_root_r * (phi / 3.).cos() - a_divided_by_3);
		roots.push(two_times_cube_root_r * ((phi + 2. * PI) / 3.).cos() - a_divided_by_3);
		roots.push(two_times_cube_root_r * ((phi + 4. * PI) / 3.).cos() - a_divided_by_3);
	}
	roots
}

// TODO: Use an `impl Iterator` return type instead of a `Vec`
/// Solve a cubic of the form `ax^3 + bx^2 + ct + d`.
pub fn solve_cubic(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
	if a.abs() <= STRICT_MAX_ABSOLUTE_DIFFERENCE {
		if b.abs() <= STRICT_MAX_ABSOLUTE_DIFFERENCE {
			// If both a and b are approximately 0, treat as a linear problem
			solve_linear(c, d)
		} else {
			// If a is approximately 0, treat as a quadratic problem
			let discriminant = c * c - 4. * b * d;
			solve_quadratic(discriminant, 2. * b, c, d)
		}
	} else {
		let new_a = b / a;
		let new_b = c / a;
		let new_c = d / a;

		// Refactor cubic to be of the form: a(t^3 + pt + q), derivation from: https://trans4mind.com/personal_development/mathematics/polynomials/cubicAlgebra.htm
		let p = (3. * new_b - new_a * new_a) / 3.;
		let q = (2. * new_a.powi(3) - 9. * new_a * new_b + 27. * new_c) / 27.;
		let discriminant = (p / 3.).powi(3) + (q / 2.).powi(2);
		solve_reformatted_cubic(discriminant, new_a, p, q)
	}
}

/// Compare two `f64` numbers with a provided max absolute value difference.
pub fn f64_compare(f1: f64, f2: f64, max_abs_diff: f64) -> bool {
	(f1 - f2).abs() < max_abs_diff
}

/// Determine if an `f64` number is within a given range by using a max absolute value difference comparison.
pub fn f64_approximately_in_range(value: f64, min: f64, max: f64, max_abs_diff: f64) -> bool {
	(min..=max).contains(&value) || f64_compare(value, min, max_abs_diff) || f64_compare(value, max, max_abs_diff)
}

/// Compare the two values in a `DVec2` independently with a provided max absolute value difference.
pub fn dvec2_compare(dv1: DVec2, dv2: DVec2, max_abs_diff: f64) -> BVec2 {
	BVec2::new((dv1.x - dv2.x).abs() < max_abs_diff, (dv1.y - dv2.y).abs() < max_abs_diff)
}

/// Determine if the values in a `DVec2` are within a given range independently by using a max absolute value difference comparison.
pub fn dvec2_approximately_in_range(point: DVec2, min: DVec2, max: DVec2, max_abs_diff: f64) -> BVec2 {
	(point.cmpge(min) & point.cmple(max)) | dvec2_compare(point, min, max_abs_diff) | dvec2_compare(point, max, max_abs_diff)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;

	/// Compare vectors of `f64`s with a provided max absolute value difference.
	fn f64_compare_vector(vec1: Vec<f64>, vec2: Vec<f64>, max_abs_diff: f64) -> bool {
		vec1.len() == vec2.len() && vec1.into_iter().zip(vec2.into_iter()).all(|(a, b)| f64_compare(a, b, max_abs_diff))
	}

	#[test]
	fn test_solve_linear() {
		// Line that is on the x-axis
		assert!(solve_linear(0., 0.).is_empty());
		// Line that is parallel to but not on the x-axis
		assert!(solve_linear(0., 1.).is_empty());
		// Line with a non-zero slope
		assert!(solve_linear(2., -8.) == vec![4.]);
	}

	#[test]
	fn test_solve_cubic() {
		// discriminant == 0
		let roots1 = solve_cubic(1., 0., 0., 0.);
		assert!(roots1 == vec![0.]);

		let roots2 = solve_cubic(1., 3., 0., -4.);
		assert!(roots2 == vec![1., -2.]);

		// p == 0
		let roots3 = solve_cubic(1., 0., 0., -1.);
		assert!(roots3 == vec![1.]);

		// discriminant > 0
		let roots4 = solve_cubic(1., 3., 0., 2.);
		assert!(f64_compare_vector(roots4, vec![-3.196], MAX_ABSOLUTE_DIFFERENCE));

		// discriminant < 0
		let roots5 = solve_cubic(1., 3., 0., -1.);
		assert!(f64_compare_vector(roots5, vec![0.532, -2.879, -0.653], MAX_ABSOLUTE_DIFFERENCE));

		// quadratic
		let roots6 = solve_cubic(0., 3., 0., -3.);
		assert!(roots6 == vec![1., -1.]);

		// linear
		let roots7 = solve_cubic(0., 0., 1., -1.);
		assert!(roots7 == vec![1.]);
	}
}
