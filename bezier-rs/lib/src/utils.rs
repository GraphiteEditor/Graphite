use glam::DVec2;

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
	if b != 0. {
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
