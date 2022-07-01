use glam::DVec2;
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

/// Compute a, b, and c for a quadratic curve that fits the start, end and point on curve at `t`.
/// The definition for the a, b, c points are defined in [the projection identity section](https://pomax.github.io/bezierinfo/#abc) of Pomax's bezier curve primer.
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

pub fn get_closest_point_in_lut(lut: &[DVec2], point: DVec2) -> (i32, f64) {
	lut.iter()
		.enumerate()
		.map(|(i, p)| (i as i32, point.distance(*p)))
		.min_by(|x, y| (&(x.1)).partial_cmp(&(y.1)).unwrap())
		.unwrap()
}

/// Find the roots of the linear equation `ax + b`
pub fn solve_linear(a: f64, b: f64) -> Vec<f64> {
	let mut roots = Vec::new();
	if a != 0. {
		roots.push(-b / a);
	}
	roots
}

/// Find the roots of the linear equation `ax^2 + bx + c`
/// Precompute the `discriminant` (`b^2 - 4ac`) and `two_times_a` arguments prior to calling this function for efficiency purposes
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

/// Solve a cubic of the form t^3 + pt + q, derivation from: https://trans4mind.com/personal_development/mathematics/polynomials/cubicAlgebra.htm
pub fn solve_cubic(discriminant: f64, a: f64, p: f64, q: f64) -> Vec<f64> {
	let mut roots = Vec::new();
	if p == 0. {
		roots.push((-q).powf(1. / 3.));
	} else if q == 0. {
		if p < 0. {
			roots.push((-p).powf(1. / 2.));
		}
	} else if discriminant == 0. {
		let q_divided_by_2 = q / 2.;
		// all roots are real, and 2 are repeated
		roots.push(2. * (-q_divided_by_2).powf(1. / 3.) - a / 3.);
		roots.push((q_divided_by_2).powf(1. / 3.) - a / 3.);
	} else if discriminant > 0. {
		// one real and two imaginary roots
		let q_divided_by_2 = q / 2.;
		roots.push((-q_divided_by_2 + discriminant).powf(1. / 3.) - (q_divided_by_2 + discriminant).powf(1. / 3.) - a / 3.);
	} else {
		let q_divided_by_2 = q / 2.;
		let p_divided_by_3 = p / 3.;
		let a_divided_by_3 = a / 3.;
		let r = (-p_divided_by_3).powf(1. / 2.);
		let phi = (-q / (2. * (-p_divided_by_3).powf(1. / 3.))).acos();

		let two_times_r = 2. * r;

		// three real roots
		roots.push(two_times_r * (phi / 3.).cos() - a_divided_by_3);
		roots.push(two_times_r * ((phi + 2. * PI) / 3.).cos() - a_divided_by_3);
		roots.push(two_times_r * ((phi + 4. * PI) / 3.).cos() - a_divided_by_3);
	}
	roots
}

#[cfg(test)]
mod tests {
	use std::f32::consts::PI;

	// use crate::Bezier;
	use glam::DVec2;

	#[test]
	fn angle() {
		let line: [DVec2; 2] = [DVec2::new(20., 20.), DVec2::new(10., 0.)];
		let slope = line[1] - line[0];
		println!("slope {}", slope);
		let angle_between = DVec2::new(1., 0.).angle_between(slope) * 180. / (PI as f64);
		let slope_angle = (slope.y / slope.x).atan().to_degrees();
		let slope_angle_2 = (slope.y).atan2(slope.x).to_degrees();
		println!("{} vs {} vs {}", angle_between, slope_angle, slope_angle_2);
		// assert!(compare_f64(angle_between, slope_angle));
	}
}
