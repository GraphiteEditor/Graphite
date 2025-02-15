/// Find real roots of cubic equation.
///
/// The implementation is not (yet) fully robust, but it does handle the case
/// where `c3` is zero (in that case, solving the quadratic equation).
///
/// See: <https://momentsingraphics.de/CubicRoots.html>
///
/// That implementation is in turn based on Jim Blinn's "How to Solve a Cubic
/// Equation", which is masterful.
///
/// Return values of x for which c0 + c1 x + c2 x² + c3 x³ = 0.
pub fn solve_cubic(c0: f64, c1: f64, c2: f64, c3: f64) -> [Option<f64>; 3] {
	let c3_recip = c3.recip();
	const ONETHIRD: f64 = 1. / 3.;
	let scaled_c2 = c2 * (ONETHIRD * c3_recip);
	let scaled_c1 = c1 * (ONETHIRD * c3_recip);
	let scaled_c0 = c0 * c3_recip;
	if !(scaled_c0.is_finite() && scaled_c1.is_finite() && scaled_c2.is_finite()) {
		// cubic coefficient is zero or nearly so.
		let [a, b] = solve_quadratic(c0, c1, c2);
		return [a, b, None];
	}
	let (c0, c1, c2) = (scaled_c0, scaled_c1, scaled_c2);
	// (d0, d1, d2) is called "Delta" in article
	let d0 = (-c2).mul_add(c2, c1);
	let d1 = (-c1).mul_add(c2, c0);
	let d2 = c2 * c0 - c1 * c1;
	// d is called "Discriminant"
	let d = 4.0 * d0 * d2 - d1 * d1;
	// de is called "Depressed.x", Depressed.y = d0
	let de = (-2.0 * c2).mul_add(d0, d1);
	// TODO: handle the cases where these intermediate results overflow.
	if d < 0.0 {
		let sq = (-0.25 * d).sqrt();
		let r = -0.5 * de;
		let t1 = (r + sq).cbrt() + (r - sq).cbrt();
		[Some(t1 - c2), None, None]
	} else if d == 0.0 {
		let t1 = (-d0).sqrt().copysign(de);
		[Some(t1 - c2), Some(-2.0 * t1 - c2), None]
	} else {
		let th = d.sqrt().atan2(-de) * ONETHIRD;
		// (th_cos, th_sin) is called "CubicRoot"
		let (th_sin, th_cos) = th.sin_cos();
		// (r0, r1, r2) is called "Root"
		let r0 = th_cos;
		let ss3 = th_sin * 3.0f64.sqrt();
		let r1 = 0.5 * (-th_cos + ss3);
		let r2 = 0.5 * (-th_cos - ss3);
		let t = 2.0 * (-d0).sqrt();
		[Some(t.mul_add(r0, -c2)), Some(t.mul_add(r1, -c2)), Some(t.mul_add(r2, -c2))]
	}
}

/// Find real roots of quadratic equation.
///
/// Return values of x for which c0 + c1 x + c2 x² = 0.
///
/// This function tries to be quite numerically robust. If the equation
/// is nearly linear, it will return the root ignoring the quadratic term;
/// the other root might be out of representable range. In the degenerate
/// case where all coefficients are zero, so that all values of x satisfy
/// the equation, a single `0.0` is returned.
pub fn solve_quadratic(c0: f64, c1: f64, c2: f64) -> [Option<f64>; 2] {
	let sc0 = c0 * c2.recip();
	let sc1 = c1 * c2.recip();
	if !sc0.is_finite() || !sc1.is_finite() {
		// c2 is zero or very small, treat as linear eqn
		let root = -c0 / c1;
		if root.is_finite() {
			return [Some(root), None];
		} else if c0 == 0.0 && c1 == 0.0 {
			// Degenerate case
			return [Some(0.0), None];
		}
	}
	let arg = sc1 * sc1 - 4. * sc0;
	let root1 = if !arg.is_finite() {
		// Likely, calculation of sc1 * sc1 overflowed. Find one root
		// using sc1 x + x² = 0, other root as sc0 / root1.
		-sc1
	} else {
		if arg < 0.0 {
			return [None, None];
		} else if arg == 0.0 {
			return [Some(-0.5 * sc1), None];
		}
		// See https://math.stackexchange.com/questions/866331
		-0.5 * (sc1 + arg.sqrt().copysign(sc1))
	};
	let root2 = sc0 / root1;
	if root2.is_finite() {
		// Sort just to be friendly and make results deterministic.
		if root2 > root1 {
			[Some(root1), Some(root2)]
		} else {
			[Some(root2), Some(root1)]
		}
	} else {
		[Some(root1), None]
	}
}

/// Compute epsilon relative to coefficient.
///
/// A helper function from the Orellana and De Michele paper.
fn eps_rel(raw: f64, a: f64) -> f64 {
	if a == 0.0 {
		raw.abs()
	} else {
		((raw - a) / a).abs()
	}
}

/// Find real roots of a quartic equation.
///
/// This is a fairly literal implementation of the method described in:
/// Algorithm 1010: Boosting Efficiency in Solving Quartic Equations with
/// No Compromise in Accuracy, Orellana and De Michele, ACM
/// Transactions on Mathematical Software, Vol. 46, No. 2, May 2020.
pub fn solve_quartic(c0: f64, c1: f64, c2: f64, c3: f64, c4: f64) -> [Option<f64>; 4] {
	if c4 == 0.0 {
		let [a, b, c] = solve_cubic(c0, c1, c2, c3);
		return [a, b, c, None];
	}
	if c0 == 0.0 {
		// Note: appends 0 root at end, doesn't sort. We might want to do that.
		let [a, b, c] = solve_cubic(c1, c2, c3, c4);
		return [a, b, c, Some(0.0)];
	}
	let a = c3 / c4;
	let b = c2 / c4;
	let c = c1 / c4;
	let d = c0 / c4;
	if let Some(result) = solve_quartic_inner(a, b, c, d, false) {
		return result;
	}
	// Do polynomial rescaling
	const K_Q: f64 = 7.16e76;
	for rescale in [false, true] {
		if let Some(result) = solve_quartic_inner(a / K_Q, b / K_Q.powi(2), c / K_Q.powi(3), d / K_Q.powi(4), rescale) {
			let [a, b, c, d] = result;
			return [a.map(|x| x * K_Q), b.map(|x| x * K_Q), c.map(|x| x * K_Q), d.map(|x| x * K_Q)];
		}
	}
	// Overflow happened, just return no roots.
	Default::default()
}

fn solve_quartic_inner(a: f64, b: f64, c: f64, d: f64, rescale: bool) -> Option<[Option<f64>; 4]> {
	factor_quartic_inner(a, b, c, d, rescale).map(|quadratics| {
		let mut quartics = quadratics.into_iter().flatten().flat_map(|(a, b)| solve_quadratic(b, a, 1.0));
		[quartics.next().flatten(), quartics.next().flatten(), quartics.next().flatten(), quartics.next().flatten()]
	})
}

/// Factor a quartic into two quadratics.
///
/// Attempt to factor a quartic equation into two quadratic equations. Returns `None` either if there
/// is overflow (in which case rescaling might succeed) or the factorization would result in
/// complex coefficients.
///
/// Discussion question: distinguish the two cases in return value?
pub fn factor_quartic_inner(a: f64, b: f64, c: f64, d: f64, rescale: bool) -> Option<[Option<(f64, f64)>; 2]> {
	let calc_eps_q = |a1, b1, a2, b2| {
		let eps_a = eps_rel(a1 + a2, a);
		let eps_b = eps_rel(b1 + a1 * a2 + b2, b);
		let eps_c = eps_rel(b1 * a2 + a1 * b2, c);
		eps_a + eps_b + eps_c
	};
	let calc_eps_t = |a1, b1, a2, b2| calc_eps_q(a1, b1, a2, b2) + eps_rel(b1 * b2, d);
	let disc = 9. * a * a - 24. * b;
	let s = if disc >= 0.0 { -2. * b / (3. * a + disc.sqrt().copysign(a)) } else { -0.25 * a };
	let a_prime = a + 4. * s;
	let b_prime = b + 3. * s * (a + 2. * s);
	let c_prime = c + s * (2. * b + s * (3. * a + 4. * s));
	let d_prime = d + s * (c + s * (b + s * (a + s)));
	let g_prime;
	let h_prime;
	const K_C: f64 = 3.49e102;
	if rescale {
		let a_prime_s = a_prime / K_C;
		let b_prime_s = b_prime / K_C;
		let c_prime_s = c_prime / K_C;
		let d_prime_s = d_prime / K_C;
		g_prime = a_prime_s * c_prime_s - (4. / K_C) * d_prime_s - (1. / 3.) * b_prime_s.powi(2);
		h_prime = (a_prime_s * c_prime_s + (8. / K_C) * d_prime_s - (2. / 9.) * b_prime_s.powi(2)) * (1. / 3.) * b_prime_s - c_prime_s * (c_prime_s / K_C) - a_prime_s.powi(2) * d_prime_s;
	} else {
		g_prime = a_prime * c_prime - 4. * d_prime - (1. / 3.) * b_prime.powi(2);
		h_prime = (a_prime * c_prime + 8. * d_prime - (2. / 9.) * b_prime.powi(2)) * (1. / 3.) * b_prime - c_prime.powi(2) - a_prime.powi(2) * d_prime;
	}
	if !(g_prime.is_finite() && h_prime.is_finite()) {
		return None;
	}
	let phi = depressed_cubic_dominant(g_prime, h_prime);
	let phi = if rescale { phi * K_C } else { phi };
	let l_1 = a * 0.5;
	let l_3 = (1. / 6.) * b + 0.5 * phi;
	let delt_2 = c - a * l_3;
	let d_2_cand_1 = (2. / 3.) * b - phi - l_1 * l_1;
	let l_2_cand_1 = 0.5 * delt_2 / d_2_cand_1;
	let l_2_cand_2 = 2. * (d - l_3 * l_3) / delt_2;
	let d_2_cand_2 = 0.5 * delt_2 / l_2_cand_2;
	let d_2_cand_3 = d_2_cand_1;
	let l_2_cand_3 = l_2_cand_2;
	let mut d_2_best = 0.0;
	let mut l_2_best = 0.0;
	let mut eps_l_best = 0.0;
	for (i, (d_2, l_2)) in [(d_2_cand_1, l_2_cand_1), (d_2_cand_2, l_2_cand_2), (d_2_cand_3, l_2_cand_3)].iter().enumerate() {
		let eps_0 = eps_rel(d_2 + l_1 * l_1 + 2. * l_3, b);
		let eps_1 = eps_rel(2. * (d_2 * l_2 + l_1 * l_3), c);
		let eps_2 = eps_rel(d_2 * l_2 * l_2 + l_3 * l_3, d);
		let eps_l = eps_0 + eps_1 + eps_2;
		if i == 0 || eps_l < eps_l_best {
			d_2_best = *d_2;
			l_2_best = *l_2;
			eps_l_best = eps_l;
		}
	}
	let d_2 = d_2_best;
	let l_2 = l_2_best;
	let mut alpha_1;
	let mut beta_1;
	let mut alpha_2;
	let mut beta_2;

	if d_2 < 0.0 {
		let sq = (-d_2).sqrt();
		alpha_1 = l_1 + sq;
		beta_1 = l_3 + sq * l_2;
		alpha_2 = l_1 - sq;
		beta_2 = l_3 - sq * l_2;
		if beta_2.abs() < beta_1.abs() {
			beta_2 = d / beta_1;
		} else if beta_2.abs() > beta_1.abs() {
			beta_1 = d / beta_2;
		}
		let cands;
		if alpha_1.abs() != alpha_2.abs() {
			if alpha_1.abs() < alpha_2.abs() {
				let a1_cand_1 = (c - beta_1 * alpha_2) / beta_2;
				let a1_cand_2 = (b - beta_2 - beta_1) / alpha_2;
				let a1_cand_3 = a - alpha_2;
				// Note: cand 3 is first because it is infallible, simplifying logic
				cands = [(a1_cand_3, alpha_2), (a1_cand_1, alpha_2), (a1_cand_2, alpha_2)];
			} else {
				let a2_cand_1 = (c - alpha_1 * beta_2) / beta_1;
				let a2_cand_2 = (b - beta_2 - beta_1) / alpha_1;
				let a2_cand_3 = a - alpha_1;
				cands = [(alpha_1, a2_cand_3), (alpha_1, a2_cand_1), (alpha_1, a2_cand_2)];
			}
			let mut eps_q_best = 0.0;
			for (i, (a1, a2)) in cands.iter().enumerate() {
				if a1.is_finite() && a2.is_finite() {
					let eps_q = calc_eps_q(*a1, beta_1, *a2, beta_2);
					if i == 0 || eps_q < eps_q_best {
						alpha_1 = *a1;
						alpha_2 = *a2;
						eps_q_best = eps_q;
					}
				}
			}
		}
	} else if d_2 == 0.0 {
		let d_3 = d - l_3 * l_3;
		alpha_1 = l_1;
		beta_1 = l_3 + (-d_3).sqrt();
		alpha_2 = l_1;
		beta_2 = l_3 - (-d_3).sqrt();
		if beta_1.abs() > beta_2.abs() {
			beta_2 = d / beta_1;
		} else if beta_2.abs() > beta_1.abs() {
			beta_1 = d / beta_2;
		}
		// TODO: handle case d_2 is very small?
	} else {
		// This case means no real roots; in the most general case we might want
		// to factor into quadratic equations with complex coefficients.
		return None;
	}
	// Newton-Raphson iteration on alpha/beta coeff's.
	let mut eps_t = calc_eps_t(alpha_1, beta_1, alpha_2, beta_2);
	for _ in 0..8 {
		if eps_t == 0.0 {
			break;
		}
		let f_0 = beta_1 * beta_2 - d;
		let f_1 = beta_1 * alpha_2 + alpha_1 * beta_2 - c;
		let f_2 = beta_1 + alpha_1 * alpha_2 + beta_2 - b;
		let f_3 = alpha_1 + alpha_2 - a;
		let c_1 = alpha_1 - alpha_2;
		let det_j = beta_1 * beta_1 - beta_1 * (alpha_2 * c_1 + 2. * beta_2) + beta_2 * (alpha_1 * c_1 + beta_2);
		if det_j == 0.0 {
			break;
		}
		let inv = det_j.recip();
		let c_2 = beta_2 - beta_1;
		let c_3 = beta_1 * alpha_2 - alpha_1 * beta_2;
		let dz_0 = c_1 * f_0 + c_2 * f_1 + c_3 * f_2 - (beta_1 * c_2 + alpha_1 * c_3) * f_3;
		let dz_1 = (alpha_1 * c_1 + c_2) * f_0 - beta_1 * c_1 * f_1 - beta_1 * c_2 * f_2 - beta_1 * c_3 * f_3;
		let dz_2 = -c_1 * f_0 - c_2 * f_1 - c_3 * f_2 + (alpha_2 * c_3 + beta_2 * c_2) * f_3;
		let dz_3 = -(alpha_2 * c_1 + c_2) * f_0 + beta_2 * c_1 * f_1 + beta_2 * c_2 * f_2 + beta_2 * c_3 * f_3;
		let a1 = alpha_1 - inv * dz_0;
		let b1 = beta_1 - inv * dz_1;
		let a2 = alpha_2 - inv * dz_2;
		let b2 = beta_2 - inv * dz_3;
		let new_eps_t = calc_eps_t(a1, b1, a2, b2);
		// We break if the new eps is equal, paper keeps going
		if new_eps_t < eps_t {
			alpha_1 = a1;
			beta_1 = b1;
			alpha_2 = a2;
			beta_2 = b2;
			eps_t = new_eps_t;
		} else {
			break;
		}
	}
	Some([Some((alpha_1, beta_1)), Some((alpha_2, beta_2))])
}

/// Dominant root of depressed cubic x^3 + gx + h = 0.
///
/// Section 2.2 of Orellana and De Michele.
// Note: some of the techniques in here might be useful to improve the
// cubic solver, and vice versa.
fn depressed_cubic_dominant(g: f64, h: f64) -> f64 {
	let q = (-1. / 3.) * g;
	let r = 0.5 * h;
	let phi_0;
	let k = if q.abs() < 1e102 && r.abs() < 1e154 {
		None
	} else if q.abs() < r.abs() {
		Some(1. - q * (q / r).powi(2))
	} else {
		Some(q.signum() * ((r / q).powi(2) / q - 1.0))
	};
	if k.is_some() && r == 0.0 {
		if g > 0.0 {
			phi_0 = 0.0;
		} else {
			phi_0 = (-g).sqrt();
		}
	} else if k.map(|k| k < 0.0).unwrap_or_else(|| r * r < q.powi(3)) {
		let t = if k.is_some() { r / q / q.sqrt() } else { r / q.powi(3).sqrt() };
		phi_0 = -2. * q.sqrt() * (t.abs().acos() * (1. / 3.)).cos().copysign(t);
	} else {
		let a = if let Some(k) = k {
			if q.abs() < r.abs() {
				-r * (1. + k.sqrt())
			} else {
				-r - (q.abs().sqrt() * q * k.sqrt()).copysign(r)
			}
		} else {
			-r - (r * r - q.powi(3)).sqrt().copysign(r)
		}
		.cbrt();
		let b = if a == 0.0 { 0.0 } else { q / a };
		phi_0 = a + b;
	}
	// Refine with Newton-Raphson iteration
	let mut x = phi_0;
	let mut f = (x * x + g) * x + h;
	const EPS_M: f64 = 2.22045e-16;
	if f.abs() < EPS_M * x.powi(3).max(g * x).max(h) {
		return x;
	}
	for _ in 0..8 {
		let delt_f = 3. * x * x + g;
		if delt_f == 0.0 {
			break;
		}
		let new_x = x - f / delt_f;
		let new_f = (new_x * new_x + g) * new_x + h;
		if new_f == 0.0 {
			return new_x;
		}
		if new_f.abs() >= f.abs() {
			break;
		}
		x = new_x;
		f = new_f;
	}
	x
}

/// Find real roots of a quintic equation.
///
/// Return values of x for which c0 + c1 x + c2 x^2 + c3 x^3 + c4 x^4 + c5 x^5 = 0.
pub fn solve_quintic(c0: f64, c1: f64, c2: f64, c3: f64, c4: f64, c5: f64) -> [Option<f64>; 5] {
	if c5 == 0.0 {
		let [a, b, c, d] = solve_quartic(c0, c1, c2, c3, c4);
		return [a, b, c, d, None];
	}

	// Normalize coefficients
	let c0 = c0 / c5;
	let c1 = c1 / c5;
	let c2 = c2 / c5;
	let c3 = c3 / c5;
	let c4 = c4 / c5;

	// Define the quintic function
	let quintic_fn = |x: f64| c0 + x * (c1 + x * (c2 + x * (c3 + x * c4)));

	// Find potential root intervals
	let mut roots = Vec::new();
	let mut a = -10.0; // Initial lower bound
	let mut ya = quintic_fn(a);

	for _ in 0..20 {
		let b = a + 1.0; // Increment upper bound
		let yb = quintic_fn(b);

		if ya.signum() != yb.signum() {
			// Root is likely in the interval [a, b]
			const EPSILON: f64 = 1e-7;
			const N0: usize = 1;
			const K1: f64 = 0.2;

			let root = solve_itp(quintic_fn, a, b, EPSILON, N0, K1, ya, yb);
			roots.push(Some(root));
		}

		a = b;
		ya = yb;
	}

	// Convert the Vec<Option<f64>> to a fixed-size array [Option<f64>; 5]
	let mut result: [Option<f64>; 5] = [None; 5];
	for (i, root) in roots.into_iter().enumerate().take(5) {
		result[i] = root;
	}

	result
}

/// Solve an arbitrary function for a zero-crossing.
///
/// This uses the [ITP method], as described in the paper
/// [An Enhancement of the Bisection Method Average Performance Preserving Minmax Optimality].
///
/// The values of `ya` and `yb` are given as arguments rather than
/// computed from `f`, as the values may already be known, or they may
/// be less expensive to compute as special cases.
///
/// It is assumed that `ya < 0.0` and `yb > 0.0`, otherwise unexpected
/// results may occur.
///
/// The value of `epsilon` must be larger than 2^-63 times `b - a`,
/// otherwise integer overflow may occur. The `a` and `b` parameters
/// represent the lower and upper bounds of the bracket searched for a
/// solution.
///
/// The ITP method has tuning parameters. This implementation hardwires
/// k2 to 2, both because it avoids an expensive floating point
/// exponentiation, and because this value has been tested to work well
/// with curve fitting problems.
///
/// The `n0` parameter controls the relative impact of the bisection and
/// secant components. When it is 0, the number of iterations is
/// guaranteed to be no more than the number required by bisection (thus,
/// this method is strictly superior to bisection). However, when the
/// function is smooth, a value of 1 gives the secant method more of a
/// chance to engage, so the average number of iterations is likely
/// lower, though there can be one more iteration than bisection in the
/// worst case.
///
/// The `k1` parameter is harder to characterize, and interested users
/// are referred to the paper, as well as encouraged to do empirical
/// testing. To match the paper, a value of `0.2 / (b - a)` is
/// suggested, and this is confirmed to give good results.
///
/// When the function is monotonic, the returned result is guaranteed to
/// be within `epsilon` of the zero crossing. For more detailed analysis,
/// again see the paper.
///
/// [ITP method]: https://en.wikipedia.org/wiki/ITP_Method
/// [An Enhancement of the Bisection Method Average Performance Preserving Minmax Optimality]: https://dl.acm.org/doi/10.1145/3423597
#[allow(clippy::too_many_arguments)]
pub fn solve_itp(mut f: impl FnMut(f64) -> f64, mut a: f64, mut b: f64, epsilon: f64, n0: usize, k1: f64, mut ya: f64, mut yb: f64) -> f64 {
	let n1_2 = (((b - a) / epsilon).log2().ceil() - 1.0).max(0.0) as usize;
	let nmax = n0 + n1_2;
	let mut scaled_epsilon = epsilon * (1u64 << nmax) as f64;
	while b - a > 2.0 * epsilon {
		let x1_2 = 0.5 * (a + b);
		let r = scaled_epsilon - 0.5 * (b - a);
		let xf = (yb * a - ya * b) / (yb - ya);
		let sigma = x1_2 - xf;
		// This has k2 = 2 hardwired for efficiency.
		let delta = k1 * (b - a).powi(2);
		let xt = if delta <= (x1_2 - xf).abs() { xf + delta.copysign(sigma) } else { x1_2 };
		let xitp = if (xt - x1_2).abs() <= r { xt } else { x1_2 - r.copysign(sigma) };
		let yitp = f(xitp);
		if yitp > 0.0 {
			b = xitp;
			yb = yitp;
		} else if yitp < 0.0 {
			a = xitp;
			ya = yitp;
		} else {
			return xitp;
		}
		scaled_epsilon *= 0.5;
	}
	0.5 * (a + b)
}

/// A variant ITP solver that allows fallible functions.
///
/// Another difference: it returns the bracket that contains the root,
/// which may be important if the function has a discontinuity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn solve_itp_fallible<E>(mut f: impl FnMut(f64) -> Result<f64, E>, mut a: f64, mut b: f64, epsilon: f64, n0: usize, k1: f64, mut ya: f64, mut yb: f64) -> Result<(f64, f64), E> {
	let n1_2 = (((b - a) / epsilon).log2().ceil() - 1.0).max(0.0) as usize;
	let nmax = n0 + n1_2;
	let mut scaled_epsilon = epsilon * (1u64 << nmax) as f64;
	while b - a > 2.0 * epsilon {
		let x1_2 = 0.5 * (a + b);
		let r = scaled_epsilon - 0.5 * (b - a);
		let xf = (yb * a - ya * b) / (yb - ya);
		let sigma = x1_2 - xf;
		// This has k2 = 2 hardwired for efficiency.
		let delta = k1 * (b - a).powi(2);
		let xt = if delta <= (x1_2 - xf).abs() { xf + delta.copysign(sigma) } else { x1_2 };
		let xitp = if (xt - x1_2).abs() <= r { xt } else { x1_2 - r.copysign(sigma) };
		let yitp = f(xitp)?;
		if yitp > 0.0 {
			b = xitp;
			yb = yitp;
		} else if yitp < 0.0 {
			a = xitp;
			ya = yitp;
		} else {
			return Ok((xitp, xitp));
		}
		scaled_epsilon *= 0.5;
	}
	Ok((a, b))
}
