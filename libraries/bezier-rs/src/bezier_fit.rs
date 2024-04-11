/*
* Modifications and Rust port copyright (C) 2024 by 0Hypercube.
*
* Original version by lib2geom: <https://gitlab.com/inkscape/lib2geom>
*
* The entirety of this file is specially licensed under MPL 1.1 terms:
*
*  Original code published in:
*    An Algorithm for Automatically Fitting Digitized Curves
*    by Philip J. Schneider
*   "Graphics Gems", Academic Press, 1990
*
*  Authors:
*    Philip J. Schneider
*    Lauris Kaplinski <lauris@kaplinski.com>
*    Peter Moulder <pmoulder@mail.csse.monash.edu.au>
*
*  Copyright (C) 1990 Philip J. Schneider
*  Copyright (C) 2001 Lauris Kaplinski
*  Copyright (C) 2001 Ximian, Inc.
*  Copyright (C) 2003,2004 Monash University

*  Original authors listed in the version control history of the following files:
*  - https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp
*
* This file is free software; you can redistribute it and/or modify it
* either under the terms of the Mozilla Public License Version 1.1 (the
* "MPL").
*
* The contents of this file are subject to the Mozilla Public License
* Version 1.1 (the "License"); you may not use this file except in
* compliance with the License. You may obtain a copy of the License at
* https://www.mozilla.org/MPL/1.1/
*
* This software is distributed on an "AS IS" basis, WITHOUT WARRANTY
* OF ANY KIND, either express or implied. See the MPL for the specific
* language governing rights and limitations.
*/

use glam::DVec2;

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L192
fn bezier_fit_cubic_full(bezier: &mut [DVec2], data: &[DVec2], t_hat_1: DVec2, t_hat_2: DVec2, error: f64, max_beziers: usize) -> Option<usize> {
	if data.len() < 2 {
		return Some(0);
	}
	if data.len() == 2 {
		// Fit 2 points
		bezier[0] = data[0];
		bezier[3] = data[data.len() - 1];
		let dist = bezier[0].distance(bezier[3]) / 3.;
		if dist.is_finite() {
			bezier[1] = if t_hat_1 == DVec2::ZERO { (2. * bezier[0] + bezier[3]) / 3. } else { bezier[0] + dist * t_hat_1 };
			bezier[2] = if t_hat_2 == DVec2::ZERO { (bezier[0] + 2. * bezier[3]) / 3. } else { bezier[3] + dist * t_hat_2 };
		} else {
			bezier[1] = bezier[0];
			bezier[2] = bezier[3];
		}
		return Some(1);
	}

	/*  Parameterize points, and attempt to fit curve */

	let mut u = chord_length_parameterize(data);
	if u[data.len() - 1] == 0. {
		return Some(0);
	}

	generate_bezier(bezier, data, &u, t_hat_1, t_hat_2, error);
	reparameterize(data, &mut u, bezier);

	/* Find max deviation of points to fitted curve. */
	let tolerance = (error + 1e-9).sqrt();
	let (mut split_point, mut max_error_ratio) = compute_max_error_ratio(data, &u, bezier, tolerance, 0);

	if max_error_ratio.abs() <= 1. {
		return Some(1);
	}

	/* If error not too large, then try some reparameterization and iteration. */
	if (0.0..=3.).contains(&max_error_ratio) {
		let max_iterations = 4; /* max times to try iterating */
		for _ in 0..max_iterations {
			generate_bezier(bezier, data, &u, t_hat_1, t_hat_2, error);
			reparameterize(data, &mut u, bezier);
			(split_point, max_error_ratio) = compute_max_error_ratio(data, &u, bezier, tolerance, split_point);
			if (max_error_ratio.abs()) <= 1. {
				return Some(1);
			}
		}
	}
	let is_corner = max_error_ratio < 0.;

	if is_corner {
		assert!(split_point < data.len());
		if split_point == 0 {
			if t_hat_1 == DVec2::ZERO {
				/* Got spike even with unconstrained initial tangent. */
				split_point += 1;
			} else {
				return bezier_fit_cubic_full(bezier, data, DVec2::ZERO, t_hat_2, error, max_beziers);
			}
		} else if split_point == data.len() - 1 {
			if t_hat_2 == DVec2::ZERO {
				/* Got spike even with unconstrained final tangent. */
				split_point -= 1;
			} else {
				return bezier_fit_cubic_full(bezier, data, t_hat_1, DVec2::ZERO, error, max_beziers);
			}
		}
	}

	if 1 < max_beziers {
		/*
		 *  Fitting failed -- split at max error point and fit recursively
		 */
		let rec_max_beziers1 = max_beziers - 1;

		let [rec_t_hat_1, rec_t_hat_2] = if is_corner {
			if !(0 < split_point && split_point < data.len() - 1) {
				return None;
			}
			[DVec2::ZERO; 2]
		} else {
			/* Unit tangent vector at splitPoint. */
			let rec_t_hat_2 = darray_center_tangent(data, split_point, data.len());
			[-rec_t_hat_2, rec_t_hat_2]
		};
		let nsegs1 = bezier_fit_cubic_full(bezier, &data[..split_point + 1], t_hat_1, rec_t_hat_2, error, rec_max_beziers1)?; //

		assert!(nsegs1 != 0);
		let rec_max_beziers2 = max_beziers - nsegs1;
		let nsegs2 = bezier_fit_cubic_full(&mut bezier[nsegs1 * 4..], &data[split_point..], rec_t_hat_1, t_hat_2, error, rec_max_beziers2)?;
		Some(nsegs1 + nsegs2)
	} else {
		None
	}
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L538
fn reparameterize(d: &[DVec2], u: &mut [f64], bez_curve: &[DVec2]) {
	let len = u.len();
	assert!(2 <= len);

	let last = len - 1;
	assert!(bez_curve[0] == d[0]);
	assert!(bez_curve[3] == d[last]);
	assert!(u[0] == 0.);
	assert!(u[last] == 1.);
	/* Otherwise, consider including 0 and last in the below loop. */

	for i in 1..last {
		u[i] = newton_raphson_root_find(bez_curve, d[i], u[i]);
	}
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L567
fn newton_raphson_root_find(q: &[DVec2], p: DVec2, u: f64) -> f64 {
	assert!(0. <= u);
	assert!(u <= 1.);

	/* Generate control vertices for Q'. */
	let mut q1 = [DVec2::ZERO; 3];
	for i in 0..3 {
		q1[i] = 3. * (q[i + 1] - q[i]);
	}

	/* Generate control vertices for Q''. */
	let mut q2 = [DVec2::ZERO; 2];
	for i in 0..2 {
		q2[i] = 2. * (q1[i + 1] - q1[i]);
	}

	/* Compute Q(u), Q'(u) and Q''(u). */

	let q_u = crate::Bezier::from_cubic_dvec2(q[0], q[1], q[2], q[3]).evaluate(crate::TValue::Parametric(u));
	let q1_u = crate::Bezier::from_quadratic_dvec2(q1[0], q1[1], q1[2]).evaluate(crate::TValue::Parametric(u));
	let q2_u = crate::Bezier::from_linear_dvec2(q2[0], q2[1]).evaluate(crate::TValue::Parametric(u));

	/* Compute f(u)/f'(u), where f is the derivative wrt u of distsq(u) = 0.5 * the square of the
	distance from P to Q(u).  Here we're using Newton-Raphson to find a stationary point in the
	distsq(u), hopefully corresponding to a local minimum in distsq (and hence a local minimum
	distance from P to Q(u)). */
	let diff = q_u - p;
	let numerator = diff.dot(q1_u);
	let denominator = q1_u.dot(q1_u) + diff.dot(q2_u);

	let mut improved_u = if denominator > 0. {
		/* One iteration of Newton-Raphson:
		improved_u = u - f(u)/f'(u) */
		u - (numerator / denominator)
	} else {
		/* Using Newton-Raphson would move in the wrong direction (towards a local maximum rather
		than local minimum), so we move an arbitrary amount in the right direction. */
		if numerator > 0. {
			u * 0.98 - 0.001
		} else if numerator < 0. {
			/* Deliberately asymmetrical, to reduce the chance of cycling. */
			0.031 + u * 0.98
		} else {
			u
		}
	};

	if !improved_u.is_finite() {
		improved_u = u;
	} else {
		improved_u = improved_u.clamp(0., 1.);
	}

	/* Ensure that improved_u isn't actually worse. */
	{
		let diff_lensq = diff.length_squared();
		let mut proportion = 0.125;
		loop {
			let bezier_pt = crate::Bezier::from_cubic_dvec2(q[0], q[1], q[2], q[3]).evaluate(crate::TValue::Parametric(improved_u));
			if (bezier_pt - p).length_squared() > diff_lensq {
				if proportion > 1. {
					//g_warning("found proportion %g", proportion);
					improved_u = u;
					break;
				}
				improved_u = (1. - proportion) * improved_u + proportion * u;
			} else {
				break;
			}
			proportion += 0.125;
		}
	}

	improved_u
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L807
fn darray_center_tangent(d: &[DVec2], center: usize, len: usize) -> DVec2 {
	assert!(center != 0);
	assert!(center < len - 1);

	if d[center + 1] == d[center - 1] {
		/* Rotate 90 degrees in an arbitrary direction. */
		let diff = d[center] - d[center - 1];
		diff.perp()
	} else {
		d[center - 1] - d[center + 1]
	}
	.normalize()
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L387
fn estimate_lengths(bezier: &mut [DVec2], data: &[DVec2], u_prime: &[f64], t_hat_1: DVec2, t_hat_2: DVec2) {
	let len = data.len();
	let mut c = [[0.; 2]; 2]; /* Matrix C. */
	let mut x = [0.; 2]; /* Matrix X. */

	/* First and last control points of the Bezier curve are positioned exactly at the first and
	last data points. */
	bezier[0] = data[0];
	bezier[3] = data[len - 1];

	for i in 0..len {
		/* Bezier control point coefficients. */
		let b0 = (1. - u_prime[i]) * (1. - u_prime[i]) * (1. - u_prime[i]);
		let b1 = 3. * u_prime[i] * (1. - u_prime[i]) * (1. - u_prime[i]);
		let b2 = 3. * u_prime[i] * u_prime[i] * (1. - u_prime[i]);
		let b3 = u_prime[i] * u_prime[i] * u_prime[i];

		/* rhs for eqn */
		let a1 = b1 * t_hat_1;
		let a2 = b2 * t_hat_2;

		c[0][0] += a1.dot(a1);
		c[0][1] += a1.dot(a2);
		c[1][0] = c[0][1];
		c[1][1] += a2.dot(a2);

		/* Additional offset to the data point from the predicted point if we were to set bezier[1]
		to bezier[0] and bezier[2] to bezier[3]. */
		let shortfall = data[i] - ((b0 + b1) * bezier[0]) - ((b2 + b3) * bezier[3]);
		x[0] += a1.dot(shortfall);
		x[1] += a2.dot(shortfall);
	}

	/* We've constructed a pair of equations in the form of a matrix product C * alpha = X.
	Now solve for alpha. */

	/* Compute the determinants of C and X. */
	let det_c0_c1 = c[0][0] * c[1][1] - c[1][0] * c[0][1];
	let [mut alpha_l, mut alpha_r] = if det_c0_c1 != 0. {
		/* Apparently Kramer's rule. */
		let det_c0_x = c[0][0] * x[1] - c[0][1] * x[0];
		let det_x_c1 = x[0] * c[1][1] - x[1] * c[0][1];
		[det_x_c1 / det_c0_c1, det_c0_x / det_c0_c1]
	} else {
		/* The matrix is under-determined.  Try requiring alpha_l == alpha_r.
		 *
		 * One way of implementing the constraint alpha_l == alpha_r is to treat them as the same
		 * variable in the equations.  We can do this by adding the columns of C to form a single
		 * column, to be multiplied by alpha to give the column vector X.
		 *
		 * We try each row in turn.
		 */
		let c0 = c[0][0] + c[0][1];
		if c0 != 0. {
			[x[0] / c0; 2]
		} else {
			let c1 = c[1][0] + c[1][1];
			if c1 != 0. {
				[x[1] / c1; 2]
			} else {
				/* Let the below code handle this. */
				[0.; 2]
			}
		}
	};

	/* If alpha negative, use the Wu/Barsky heuristic (see text).  (If alpha is 0, you get
	coincident control points that lead to divide by zero in any subsequent
	NewtonRaphsonRootFind() call.) */
	// \todo Check whether this special-casing is necessary now that
	// NewtonRaphsonRootFind handles non-positive denominator.
	if alpha_l < 1.0e-6 || alpha_r < 1.0e-6 {
		alpha_l = data[0].distance(data[len - 1]) / 3.;
		alpha_r = alpha_l;
	}

	/* Control points 1 and 2 are positioned an alpha distance out on the tangent vectors, left and
	right, respectively. */
	bezier[1] = alpha_l * t_hat_1 + bezier[0];
	bezier[2] = alpha_r * t_hat_2 + bezier[3];
}
/*
 * ComputeLeftTangent, ComputeRightTangent, ComputeCenterTangent :
 * Approximate unit tangents at endpoints and "center" of digitized curve
 */

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L706
fn darray_left_tangent_simple(d: &[DVec2]) -> DVec2 {
	let len = d.len();
	assert!(len >= 2);
	assert!(d[0] != d[1]);
	(d[1] - d[0]).normalize()
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L724
fn darray_right_tangent_simple(d: &[DVec2]) -> DVec2 {
	let len = d.len();
	assert!(2 <= len);
	let last = len - 1;
	let prev = last - 1;
	assert!(d[last] != d[prev]);
	(d[prev] - d[last]).normalize()
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L745
fn darray_left_tangent(d: &[DVec2], tolerance_sq: f64) -> DVec2 {
	let len = d.len();
	assert!(2 <= len);
	assert!(0. <= tolerance_sq);
	let mut i = 1;
	loop {
		let pi = d[i];
		let t = pi - d[0];
		let distsq = t.length_squared();
		if tolerance_sq < distsq {
			return (t).normalize();
		}
		i += 1;
		if i == len {
			return if distsq == 0. { darray_left_tangent_simple(d) } else { (t).normalize() };
		}
	}
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L776
fn darray_right_tangent(d: &[DVec2], tolerance_sq: f64) -> DVec2 {
	let len = d.len();
	assert!(2 <= len);
	assert!(0. <= tolerance_sq);
	let last = len - 1;
	let mut i = last - 1;
	loop {
		let pi = d[i];
		let t = pi - d[last];
		let dist_sq = t.length_squared();
		if tolerance_sq < dist_sq {
			return t.normalize_or_zero();
		}
		if i == 0 {
			return if dist_sq == 0. { darray_right_tangent_simple(d) } else { (t).normalize_or_zero() };
		}
		i -= 1;
	}
}
// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L360
fn generate_bezier(bezier: &mut [DVec2], data: &[DVec2], u: &[f64], t_hat_1: DVec2, t_hat_2: DVec2, tolerance_sq: f64) {
	let est1 = t_hat_1 == DVec2::ZERO;
	let est2 = t_hat_2 == DVec2::ZERO;
	let mut est_t_hat_1 = if est1 { darray_left_tangent(data, tolerance_sq) } else { t_hat_1 };
	let est_t_hat_2 = if est2 { darray_right_tangent(data, tolerance_sq) } else { t_hat_2 };
	estimate_lengths(bezier, data, u, est_t_hat_1, est_t_hat_2);
	/* We find that darray_right_tangent tends to produce better results
	for our current freehand tool than full estimation. */
	if est1 {
		estimate_bi(bezier, 1, data, u);
		if bezier[1] != bezier[0] {
			est_t_hat_1 = (bezier[1] - bezier[0]).normalize_or_zero();
		}
		estimate_lengths(bezier, data, u, est_t_hat_1, est_t_hat_2);
	}
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L492
fn estimate_bi(bezier: &mut [DVec2], ei: usize, data: &[DVec2], u: &[f64]) {
	if !(1..=2).contains(&ei) {
		return;
	}
	let oi = 3 - ei;
	let mut num = [0., 0.];
	let mut den = 0.;
	for i in 0..data.len() {
		let ui = u[i];
		let b = [((1. - ui) * (1. - ui) * (1. - ui)), (3. * ui * (1. - ui) * (1. - ui)), (3. * ui * ui * (1. - ui)), (ui * ui * ui)];

		for d in 0..2 {
			num[d] += b[ei] * (b[0] * bezier[0][d] + b[oi] * bezier[oi][d] + b[3] * bezier[3][d] + -data[i][d]);
		}
		den -= b[ei] * b[ei];
	}

	if den != 0. {
		for d in 0..2 {
			bezier[ei][d] = num[d] / den;
		}
	} else {
		bezier[ei] = (oi as f64 * bezier[0] + ei as f64 * bezier[3]) / 3.;
	}
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L898
fn compute_max_error_ratio(d: &[DVec2], u: &[f64], bezier_curve: &[DVec2], tolerance: f64, mut split_point: usize) -> (usize, f64) {
	let len = u.len();
	assert!(2 <= len);
	let last = len - 1;
	assert!(bezier_curve[0] == d[0]);
	assert!(bezier_curve[3] == d[last]);
	assert!(u[0] == 0.);
	assert!(u[last] == 1.);
	/* I.e. assert that the error for the first & last points is zero.
	 * Otherwise we should include those points in the below loop.
	 * The assertion is also necessary to ensure 0 < splitPoint < last.
	 */

	let mut max_distance_sq = 0.; /* Maximum error */
	let mut max_hook_ratio = 0.;
	let mut snap_end = 0;
	let mut previous_point = bezier_curve[0];
	for i in 1..=last {
		let current_point = crate::Bezier::from_cubic_dvec2(bezier_curve[0], bezier_curve[1], bezier_curve[2], bezier_curve[3]).evaluate(crate::TValue::Parametric(u[i]));
		let distsq = (current_point - d[i]).length_squared();
		if distsq > max_distance_sq {
			max_distance_sq = distsq;
			split_point = i;
		}
		let hook_ratio = compute_hook(previous_point, current_point, 0.5 * (u[i - 1] + u[i]), bezier_curve, tolerance);
		if max_hook_ratio < hook_ratio {
			max_hook_ratio = hook_ratio;
			snap_end = i;
		}
		previous_point = current_point;
	}

	let dist_ratio = (max_distance_sq).sqrt() / tolerance;
	let error_ratio = if max_hook_ratio <= dist_ratio {
		dist_ratio
	} else {
		assert!(0 < snap_end);
		split_point = snap_end - 1;
		-max_hook_ratio
	};
	assert!(error_ratio == 0. || ((split_point < last) && (split_point != 0 || error_ratio < 0.)));
	(split_point, error_ratio)
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L969
fn compute_hook(a: DVec2, b: DVec2, u: f64, bezier_curve: &[DVec2], tolerance: f64) -> f64 {
	let point = crate::Bezier::from_cubic_dvec2(bezier_curve[0], bezier_curve[1], bezier_curve[2], bezier_curve[3]).evaluate(crate::TValue::Parametric(u));

	let distance = ((a + b) / 2.).distance(point);
	if distance < tolerance {
		return 0.;
	}
	let allowed = a.distance(b) + tolerance;
	distance / allowed
}

/// A value from 0..1 for each point in the path containing the total distance from the start along the path
// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier-utils.cpp#L833
fn chord_length_parameterize(d: &[DVec2]) -> Vec<f64> {
	let len = d.len();
	if len < 2 {
		return Vec::new();
	}

	// First let u[i] equal the distance travelled along the path from d[0] to d[i]
	let mut u = vec![0.; len];
	for i in 1..len {
		let dist = d[i].distance(d[i - 1]);
		u[i] = u[i - 1] + dist;
	}

	// Scale to 0 ..= 1
	let tot_len = u[len - 1];
	if tot_len <= 0. {
		return Vec::new();
	}
	if tot_len.is_finite() {
		for u in u.iter_mut().skip(1) {
			*u /= tot_len;
		}
	} else {
		// fallback to even space
		for (i, u) in u.iter_mut().enumerate().skip(1) {
			*u = i as f64 / (len - 1) as f64;
		}
	}

	// Ensure last is exactly 1.
	u[len - 1] = 1.;
	u
}

impl<PointId: crate::Identifier> crate::Subpath<PointId> {
	pub fn fit_cubic(points: &[DVec2], max_segs: usize, tangent: DVec2, tolerance_sq: f64) -> Option<Self> {
		let mut b = vec![DVec2::ZERO; 4 * max_segs];
		let len = bezier_fit_cubic_full(&mut b, points, tangent, DVec2::ZERO, tolerance_sq, max_segs)?;
		if len < 1 {
			return None;
		}
		let beziers = (0..len)
			.map(|i| crate::Bezier::from_cubic_dvec2(b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]))
			.collect::<Vec<_>>();
		Some(Self::from_beziers(&beziers, false))
	}
}

#[test]
fn generate_bezier_test() {
	let src_b = vec![DVec2::new(5., -3.), DVec2::new(8., 0.), DVec2::new(4., 2.), DVec2::new(3., 3.)];
	let t = [
		0., 0.001, 0.03, 0.05, 0.09, 0.13, 0.18, 0.25, 0.29, 0.33, 0.39, 0.44, 0.51, 0.57, 0.62, 0.69, 0.75, 0.81, 0.91, 0.93, 0.97, 0.98, 0.999, 1.,
	];

	let data = t
		.iter()
		.map(|&t| crate::Bezier::from_cubic_dvec2(src_b[0], src_b[1], src_b[2], src_b[3]).evaluate(crate::TValue::Parametric(t)))
		.collect::<Vec<_>>();
	let t_hat_1 = (src_b[1] - src_b[0]).normalize();
	let t_hat_2 = (src_b[2] - src_b[3]).normalize();

	let mut est_b = vec![DVec2::ZERO; 4];
	generate_bezier(&mut est_b, &data, &t, t_hat_1, t_hat_2, 1.);

	assert_eq!(src_b, est_b);
}
