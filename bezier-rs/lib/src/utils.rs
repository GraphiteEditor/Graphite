use crate::consts::{MAX_ABSOLUTE_DIFFERENCE, STRICT_MAX_ABSOLUTE_DIFFERENCE};

use glam::{BVec2, DMat2, DVec2};
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

/// Determine if two rectangles have any overlap. The rectangles are represented by a pair of coordinates that designate the top left and bottom right corners (in a graphical coordinate system).
pub fn do_rectangles_overlap(rectangle1: [DVec2; 2], rectangle2: [DVec2; 2]) -> bool {
	let [bottom_left1, top_right1] = rectangle1;
	let [bottom_left2, top_right2] = rectangle2;

	top_right1.x >= bottom_left2.x && top_right2.x >= bottom_left1.x && top_right2.y >= bottom_left1.y && top_right1.y >= bottom_left2.y
}

/// Returns the intersection of two lines. The lines are given by a point on the line and its slope (represented by a vector).
pub fn line_intersection(point1: DVec2, point1_slope_vector: DVec2, point2: DVec2, point2_slope_vector: DVec2) -> DVec2 {
	assert!(point1_slope_vector.normalize() != point2_slope_vector.normalize());

	// Find the intersection when the first line is vertical
	if f64_compare(point1_slope_vector.x, 0., MAX_ABSOLUTE_DIFFERENCE) {
		let m2 = point2_slope_vector.y / point2_slope_vector.x;
		let b2 = point2.y - m2 * point2.x;
		DVec2::new(point1.x, point1.x * m2 + b2)
	}
	// Find the intersection when the second line is vertical
	else if f64_compare(point2_slope_vector.x, 0., MAX_ABSOLUTE_DIFFERENCE) {
		let m1 = point1_slope_vector.y / point1_slope_vector.x;
		let b1 = point1.y - m1 * point1.x;
		DVec2::new(point2.x, point2.x * m1 + b1)
	}
	// Find the intersection where neither line is vertical
	else {
		let m1 = point1_slope_vector.y / point1_slope_vector.x;
		let b1 = point1.y - m1 * point1.x;
		let m2 = point2_slope_vector.y / point2_slope_vector.x;
		let b2 = point2.y - m2 * point2.x;
		let intersection_x = (b2 - b1) / (m1 - m2);
		DVec2::new(intersection_x, intersection_x * m1 + b1)
	}
}

/// Check if 3 points are collinear.
pub fn are_points_collinear(p1: DVec2, p2: DVec2, p3: DVec2) -> bool {
	let matrix = DMat2::from_cols(p1 - p2, p2 - p3);
	f64_compare(matrix.determinant() / 2., 0., MAX_ABSOLUTE_DIFFERENCE)
}

/// Compute the center of the circle that passes through all three provided points. The provided points cannot be collinear.
pub fn compute_circle_center_from_points(p1: DVec2, p2: DVec2, p3: DVec2) -> Option<DVec2> {
	if are_points_collinear(p1, p2, p3) {
		return None;
	}

	let midpoint_a = p1.lerp(p2, 0.5);
	let midpoint_b = p2.lerp(p3, 0.5);
	let midpoint_c = p3.lerp(p1, 0.5);

	let tangent_a = (p1 - p2).perp();
	let tangent_b = (p2 - p3).perp();
	let tangent_c = (p3 - p1).perp();

	let intersect_a_b = line_intersection(midpoint_a, tangent_a, midpoint_b, tangent_b);
	let intersect_b_c = line_intersection(midpoint_b, tangent_b, midpoint_c, tangent_c);
	let intersect_c_a = line_intersection(midpoint_c, tangent_c, midpoint_a, tangent_a);

	Some((intersect_a_b + intersect_b_c + intersect_c_a) / 3.)
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

	#[test]
	fn test_do_rectangles_overlap() {
		// Rectangles overlap
		assert!(do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(20., 20.)], [DVec2::new(10., 10.), DVec2::new(30., 20.)]));
		// Rectangles share a side
		assert!(do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(10., 10.)], [DVec2::new(10., 10.), DVec2::new(30., 30.)]));
		// Rectangle inside the other
		assert!(do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(10., 10.)], [DVec2::new(2., 2.), DVec2::new(6., 4.)]));
		// No overlap, rectangles are beside each other
		assert!(!do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(10., 10.)], [DVec2::new(20., 0.), DVec2::new(30., 10.)]));
		// No overlap, rectangles are above and below each other
		assert!(!do_rectangles_overlap([DVec2::new(0., 0.), DVec2::new(10., 10.)], [DVec2::new(0., 20.), DVec2::new(20., 30.)]));
	}

	#[test]
	fn test_find_intersection() {
		// y = 2x + 10
		// y = 5x + 4
		// intersect at (2, 14)

		let start1 = DVec2::new(0., 10.);
		let end1 = DVec2::new(0., 4.);
		let start_direction1 = DVec2::new(1., 2.);
		let end_direction1 = DVec2::new(1., 5.);
		assert!(line_intersection(start1, start_direction1, end1, end_direction1) == DVec2::new(2., 14.));

		// y = x
		// y = -x + 8
		// intersect at (4, 4)

		let start2 = DVec2::new(0., 0.);
		let end2 = DVec2::new(8., 0.);
		let start_direction2 = DVec2::new(1., 1.);
		let end_direction2 = DVec2::new(1., -1.);
		assert!(line_intersection(start2, start_direction2, end2, end_direction2) == DVec2::new(4., 4.));
	}

	#[test]
	fn test_are_points_collinear() {
		assert!(are_points_collinear(DVec2::new(2., 4.), DVec2::new(6., 8.), DVec2::new(4., 6.)));
		assert!(!are_points_collinear(DVec2::new(1., 4.), DVec2::new(6., 8.), DVec2::new(4., 6.)));
	}

	#[test]
	fn test_compute_circle_center_from_points() {
		// 3/4 of unit circle
		let center1 = compute_circle_center_from_points(DVec2::new(0., 1.), DVec2::new(-1., 0.), DVec2::new(1., 0.));
		assert_eq!(center1.unwrap(), DVec2::new(0., 0.));
		// 1/4 of unit circle
		let center2 = compute_circle_center_from_points(DVec2::new(-1., 0.), DVec2::new(0., 1.), DVec2::new(1., 0.));
		assert_eq!(center2.unwrap(), DVec2::new(0., 0.));
	}
}
