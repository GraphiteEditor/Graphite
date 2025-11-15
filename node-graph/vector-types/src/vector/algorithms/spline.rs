use glam::DVec2;

/// Solve for the first handle of an open spline. (The opposite handle can be found by mirroring the result about the anchor.)
pub fn solve_spline_first_handle_open(points: &[DVec2]) -> Vec<DVec2> {
	let len_points = points.len();
	if len_points == 0 {
		return Vec::new();
	}
	if len_points == 1 {
		return vec![points[0]];
	}

	// Matrix coefficients a, b and c (see https://mathworld.wolfram.com/CubicSpline.html).
	// Because the `a` coefficients are all 1, they need not be stored.
	// This algorithm does a variation of the above algorithm.
	// Instead of using the traditional cubic (a + bt + ct^2 + dt^3), we use the bezier cubic.

	let mut b = vec![DVec2::new(4., 4.); len_points];
	b[0] = DVec2::new(2., 2.);
	b[len_points - 1] = DVec2::new(2., 2.);

	let mut c = vec![DVec2::new(1., 1.); len_points];

	// 'd' is the the second point in a cubic bezier, which is what we solve for
	let mut d = vec![DVec2::ZERO; len_points];

	d[0] = DVec2::new(2. * points[1].x + points[0].x, 2. * points[1].y + points[0].y);
	d[len_points - 1] = DVec2::new(3. * points[len_points - 1].x, 3. * points[len_points - 1].y);
	for idx in 1..(len_points - 1) {
		d[idx] = DVec2::new(4. * points[idx].x + 2. * points[idx + 1].x, 4. * points[idx].y + 2. * points[idx + 1].y);
	}

	// Solve with Thomas algorithm (see https://en.wikipedia.org/wiki/Tridiagonal_matrix_algorithm)
	// Now we do row operations to eliminate `a` coefficients.
	c[0] /= -b[0];
	d[0] /= -b[0];
	#[allow(clippy::assign_op_pattern)]
	for i in 1..len_points {
		b[i] += c[i - 1];
		// For some reason this `+=` version makes the borrow checker mad:
		// d[i] += d[i-1]
		d[i] = d[i] + d[i - 1];
		c[i] /= -b[i];
		d[i] /= -b[i];
	}

	// At this point b[i] == -a[i + 1] and a[i] == 0.
	// Now we do row operations to eliminate 'c' coefficients and solve.
	d[len_points - 1] *= -1.;
	#[allow(clippy::assign_op_pattern)]
	for i in (0..len_points - 1).rev() {
		d[i] = d[i] - (c[i] * d[i + 1]);
		d[i] *= -1.; // d[i] /= b[i]
	}

	d
}

/// Solve for the first handle of a closed spline. (The opposite handle can be found by mirroring the result about the anchor.)
/// If called with fewer than 3 points, this function will return an empty result.
pub fn solve_spline_first_handle_closed(points: &[DVec2]) -> Vec<DVec2> {
	let len_points = points.len();
	if len_points < 3 {
		return Vec::new();
	}

	// Matrix coefficients `a`, `b` and `c` (see https://mathworld.wolfram.com/CubicSpline.html).
	// We don't really need to allocate them but it keeps the maths understandable.
	let a = vec![DVec2::ONE; len_points];
	let b = vec![DVec2::splat(4.); len_points];
	let c = vec![DVec2::ONE; len_points];

	let mut cmod = vec![DVec2::ZERO; len_points];
	let mut u = vec![DVec2::ZERO; len_points];

	// `x` is initially the output of the matrix multiplication, but is converted to the second value.
	let mut x = vec![DVec2::ZERO; len_points];

	for (i, point) in x.iter_mut().enumerate() {
		let previous_i = i.checked_sub(1).unwrap_or(len_points - 1);
		let next_i = (i + 1) % len_points;
		*point = 3. * (points[next_i] - points[previous_i]);
	}

	// Solve using https://en.wikipedia.org/wiki/Tridiagonal_matrix_algorithm#Variants (the variant using periodic boundary conditions).
	// This code below is based on the reference C language implementation provided in that section of the article.
	let alpha = a[0];
	let beta = c[len_points - 1];

	// Arbitrary, but chosen such that division by zero is avoided.
	let gamma = -b[0];

	cmod[0] = alpha / (b[0] - gamma);
	u[0] = gamma / (b[0] - gamma);
	x[0] /= b[0] - gamma;

	// Handle from from `1` to `len_points - 2` (inclusive).
	for ix in 1..=(len_points - 2) {
		let m = 1.0 / (b[ix] - a[ix] * cmod[ix - 1]);
		cmod[ix] = c[ix] * m;
		u[ix] = (0.0 - a[ix] * u[ix - 1]) * m;
		x[ix] = (x[ix] - a[ix] * x[ix - 1]) * m;
	}

	// Handle `len_points - 1`.
	let m = 1.0 / (b[len_points - 1] - alpha * beta / gamma - beta * cmod[len_points - 2]);
	u[len_points - 1] = (alpha - a[len_points - 1] * u[len_points - 2]) * m;
	x[len_points - 1] = (x[len_points - 1] - a[len_points - 1] * x[len_points - 2]) * m;

	// Loop from `len_points - 2` to `0` (inclusive).
	for ix in (0..=(len_points - 2)).rev() {
		u[ix] = u[ix] - cmod[ix] * u[ix + 1];
		x[ix] = x[ix] - cmod[ix] * x[ix + 1];
	}

	let fact = (x[0] + x[len_points - 1] * beta / gamma) / (1.0 + u[0] + u[len_points - 1] * beta / gamma);

	for ix in 0..(len_points) {
		x[ix] -= fact * u[ix];
	}

	let mut real = vec![DVec2::ZERO; len_points];
	for i in 0..len_points {
		let previous = i.checked_sub(1).unwrap_or(len_points - 1);
		let next = (i + 1) % len_points;
		real[i] = x[previous] * a[next] + x[i] * b[i] + x[next] * c[i];
	}

	// The matrix is now solved.

	// Since we have computed the derivative, work back to find the start handle.
	for i in 0..len_points {
		x[i] = (x[i] / 3.) + points[i];
	}

	x
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn closed_spline() {
		use crate::vector::misc::{dvec2_to_point, point_to_dvec2};
		use kurbo::{BezPath, ParamCurve, ParamCurveDeriv};

		// These points are just chosen arbitrary
		let points = [DVec2::new(0., 0.), DVec2::new(0., 0.), DVec2::new(6., 5.), DVec2::new(7., 9.), DVec2::new(2., 3.)];

		// List of first handle or second point in a cubic bezier curve.
		let first_handles = solve_spline_first_handle_closed(&points);

		// Construct the Subpath
		let mut bezpath = BezPath::new();
		bezpath.move_to(dvec2_to_point(points[0]));

		for i in 0..first_handles.len() {
			let next_i = i + 1;
			let next_i = if next_i == first_handles.len() { 0 } else { next_i };

			// First handle or second point of a cubic Bezier curve.
			let p1 = dvec2_to_point(first_handles[i]);
			// Second handle or third point of a cubic Bezier curve.
			let p2 = dvec2_to_point(2. * points[next_i] - first_handles[next_i]);
			// Endpoint or fourth point of a cubic Bezier curve.
			let p3 = dvec2_to_point(points[next_i]);

			bezpath.curve_to(p1, p2, p3);
		}

		// For each pair of bézier curves, ensure that the second derivative is continuous
		for (bézier_a, bézier_b) in bezpath.segments().zip(bezpath.segments().skip(1).chain(bezpath.segments().take(1))) {
			let derivative2_end_a = point_to_dvec2(bézier_a.to_cubic().deriv().eval(1.));
			let derivative2_start_b = point_to_dvec2(bézier_b.to_cubic().deriv().eval(0.));

			assert!(
				derivative2_end_a.abs_diff_eq(derivative2_start_b, 1e-10),
				"second derivative at the end of a {derivative2_end_a} is equal to the second derivative at the start of b {derivative2_start_b}"
			);
		}
	}
}
