use glam::DVec2;

use crate::{Bezier, BezierHandles};

impl std::ops::Index<usize> for Bezier {
	type Output = DVec2;
	fn index(&self, index: usize) -> &Self::Output {
		match &self.handles {
			BezierHandles::Linear => [&self.start, &self.end][index],
			BezierHandles::Quadratic { handle } => [&self.start, handle, &self.end][index],
			BezierHandles::Cubic { handle_start, handle_end } => [&self.start, handle_start, handle_end, &self.end][index],
		}
	}
}

#[derive(Debug, Clone)]
pub struct SymetricalBasis(pub Vec<DVec2>);

impl SymetricalBasis {
	#[must_use]
	fn derivative(&self) -> SymetricalBasis {
		let mut c = SymetricalBasis(vec![DVec2::ZERO; self.len()]);
		if self.iter().all(|x| x.abs_diff_eq(DVec2::ZERO, 1e-5)) {
			return c;
		}
		for k in 0..(self.len() - 1) {
			let d = (2. * k as f64 + 1.) * (self[k][1] - self[k][0]);

			c[k][0] = d + (k as f64 + 1.) * self[k + 1][0];
			c[k][1] = d - (k as f64 + 1.) * self[k + 1][1];
		}
		let k = self.len() - 1;
		let d = (2. * k as f64 + 1.) * (self[k][1] - self[k][0]);
		if d == 0. && k > 0 {
			c.pop();
		} else {
			c[k][0] = d;
			c[k][1] = d;
		}
		c
	}
	fn normalize(&mut self) {
		while self.len() > 1 && self.last().is_some_and(|x| x.abs_diff_eq(DVec2::ZERO, 1e-5)) {
			self.pop();
		}
	}
	#[must_use]
	pub fn to_bezier1d(&self) -> Bezier1d {
		let sb = self;
		assert!(!sb.is_empty());

		let n;
		let even;

		let mut q = sb.len();
		if sb[q - 1][0] == sb[q - 1][1] {
			even = true;
			q -= 1;
			n = 2 * q;
		} else {
			even = false;
			n = 2 * q - 1;
		}

		let mut bz = Bezier1d(vec![0.; n + 1]);
		for k in 0..q {
			let mut tjk = 1.;
			for j in k..(n - k) {
				// j <= n-k-1
				bz[j] += tjk * sb[k][0];
				bz[n - j] += tjk * sb[k][1]; // n-k <-> [k][1]
				tjk = binomial_increment_k(tjk, n - 2 * k - 1, j - k);
			}
		}
		if even {
			bz[q] += sb[q][0];
		}
		// the resulting coefficients are with respect to the scaled Bernstein
		// basis so we need to divide them by (n, j) binomial coefficient
		let mut bcj = n as f64;
		for j in 1..n {
			bz[j] /= bcj;
			bcj = binomial_increment_k(bcj, n, j);
		}
		bz[0] = sb[0][0];
		bz[n] = sb[0][1];
		bz
	}

	#[must_use]
	fn roots(&self) -> Vec<f64> {
		let s = self;
		match s.len() {
			0 => Vec::new(),
			1 => {
				let mut res = Vec::new();
				let d = s[0].x - s[0].y;
				if d != 0. {
					let r = s[0].x / d;
					if (0. ..=1.).contains(&r) {
						res.push(r);
					}
				}
				res
			}
			_ => {
				let mut bz = s.to_bezier1d();
				let mut solutions = Vec::new();
				if bz.len() == 0 || bz.iter().all(|&x| (x - bz[0]).abs() < 1e-5) {
					return solutions;
				}
				while bz[0] == 0. {
					bz = bz.deflate();
					solutions.push(0.);
				}
				// Linear
				if bz.len() - 1 == 1 {
					if bz[0].signum() != bz[1].signum() {
						let d = bz[0] - bz[1];
						if d != 0. {
							let r = bz[0] / d;
							if (0. ..=1.).contains(&r) {
								solutions.push(r);
							}
						}
					}
					return solutions;
				}
				bz.find_bernstein_roots(&mut solutions, 0, 0., 1.);

				solutions.sort_by(f64::total_cmp);

				solutions
			}
		}
	}
}

impl<'a> std::ops::Mul for &'a SymetricalBasis {
	type Output = SymetricalBasis;
	fn mul(self, b: Self) -> Self::Output {
		let a = self;
		if a.iter().all(|x| x.abs_diff_eq(DVec2::ZERO, 1e-5)) || b.iter().all(|x| x.abs_diff_eq(DVec2::ZERO, 1e-5)) {
			return SymetricalBasis(vec![DVec2::ZERO]);
		}
		let mut c = SymetricalBasis(vec![DVec2::ZERO; a.len() + b.len()]);

		for j in 0..b.len() {
			for i in j..(a.len() + j) {
				let tri = (b[j][1] - b[j][0]) * (a[i - j][1] - a[i - j][0]);
				c[i+1/*shift*/] += DVec2::splat(-tri);
			}
		}
		for j in 0..b.len() {
			for i in j..(a.len() + j) {
				for dim in 0..2 {
					c[i][dim] += b[j][dim] * a[i - j][dim];
				}
			}
		}
		c.normalize();
		c
	}
}

impl std::ops::Add for SymetricalBasis {
	type Output = SymetricalBasis;
	fn add(self, b: Self) -> Self::Output {
		let a = self;
		let out_size = a.len().max(b.len());
		let min_size = a.len().min(b.len());
		let mut result = SymetricalBasis(vec![DVec2::ZERO; out_size]);
		for i in 0..min_size {
			result[i] = a[i] + b[i];
		}
		for i in min_size..a.len() {
			result[i] = a[i];
		}
		for i in min_size..b.len() {
			result[i] = b[i];
		}
		result
	}
}

impl std::ops::Sub for SymetricalBasis {
	type Output = SymetricalBasis;
	fn sub(self, b: Self) -> Self::Output {
		let a = self;
		let out_size = a.len().max(b.len());
		let min_size = a.len().min(b.len());
		let mut result = SymetricalBasis(vec![DVec2::ZERO; out_size]);
		for i in 0..min_size {
			result[i] = a[i] - b[i];
		}
		for i in min_size..a.len() {
			result[i] = a[i];
		}
		for i in min_size..b.len() {
			result[i] = -b[i];
		}
		result
	}
}

impl std::ops::Deref for SymetricalBasis {
	type Target = Vec<DVec2>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl std::ops::DerefMut for SymetricalBasis {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

#[derive(Debug, Clone)]
pub struct SymetricalBasisPair {
	pub x: SymetricalBasis,
	pub y: SymetricalBasis,
}
impl SymetricalBasisPair {
	#[must_use]
	pub fn derivative(&self) -> Self {
		Self {
			x: self.x.derivative(),
			y: self.y.derivative(),
		}
	}

	#[must_use]
	pub fn dot(&self, other: &Self) -> SymetricalBasis {
		(&self.x * &other.x) + (&self.y * &other.y)
	}
	#[must_use]
	pub fn cross(&self, rhs: &Self) -> SymetricalBasis {
		(&self.x * &rhs.y) - (&self.y * &rhs.x)
	}
}

#[derive(Debug, Clone)]
pub struct Bezier1d(pub Vec<f64>);

impl std::ops::Deref for Bezier1d {
	type Target = Vec<f64>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl std::ops::DerefMut for Bezier1d {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl Bezier1d {
	const MAX_DEPTH: u32 = 32;
	#[must_use]
	fn deflate(&self) -> Self {
		let bz = self;
		if bz.is_empty() {
			return Bezier1d(Vec::new());
		}
		let n = bz.len() - 1;
		let mut b = Bezier1d(vec![0.; n]);
		for i in 0..n {
			b[i] = (n as f64 * bz[i + 1]) / (i as f64 + 1.)
		}
		b
	}

	/// Compute the value of a Bernstein-Bezier polynomial using a Horner-like fast evaluation scheme.
	#[must_use]
	fn value_at(&self, t: f64) -> f64 {
		let bz = self;
		let order = bz.len() - 1;
		let u = 1.0 - t;
		let mut bc = 1.;
		let mut tn = 1.;
		let mut tmp = bz[0] * u;
		for i in 1..order {
			tn *= t;
			bc = bc * (order as f64 - i as f64 + 1.) / i as f64;
			tmp = (tmp + tn * bc * bz[i]) * u;
		}
		tmp + tn * t * bz[bz.len() - 1]
	}

	#[must_use]
	fn secant(&self) -> f64 {
		let bz = self;
		let mut s = 0.;
		let mut t = 1.;
		let e = 1e-14;
		let mut side = 0;
		let mut r = 0.;
		let mut fs = bz[0];
		let mut ft = bz[bz.len() - 1];

		for _n in 0..100 {
			r = (fs * t - ft * s) / (fs - ft);
			if (t - s).abs() < e * (t + s).abs() {
				return r;
			}

			let fr = self.value_at(r);

			if fr * ft > 0. {
				t = r;
				ft = fr;
				if side == -1 {
					fs /= 2.;
				}
				side = -1;
			} else if fs * fr > 0. {
				s = r;
				fs = fr;
				if side == 1 {
					ft /= 2.;
				}
				side = 1;
			} else {
				break;
			}
		}
		r
	}

	fn casteljau_subdivision(&self, t: f64) -> [Self; 2] {
		let v = self;
		let order = v.len() - 1;
		let mut left = v.clone();
		let mut right = v.clone();

		// The Horner-like scheme gives very slightly different results, but we need
		// the result of subdivision to match exactly with Bezier's valueAt function.
		let val = v.value_at(t);
		for i in (1..=order).rev() {
			left[i - 1] = right[0];
			for j in i..v.len() {
				right[j - 1] = right[j - 1] + ((right[j] - right[j - 1]) * t);
			}
		}
		right[0] = val;
		left[order] = right[0];
		[left, right]
	}

	fn derivative(&self) -> Self {
		let bz = self;
		if bz.len() - 1 == 1 {
			return Bezier1d(vec![bz[1] - bz[0]]);
		}
		let mut der = Bezier1d(vec![0.; bz.len() - 1]);

		for i in 0..(bz.len() - 1) {
			der[i] = (bz.len() - 1) as f64 * (bz[i + 1] - bz[i]);
		}
		der
	}

	/// given an equation in Bernstein-Bernstein form, find all roots between left_t and right_t
	fn find_bernstein_roots(&self, solutions: &mut Vec<f64>, depth: u32, left_t: f64, right_t: f64) {
		let bz = self;
		let mut n_crossings = 0;

		let mut old_sign = bz[0].signum();
		for i in 1..bz.len() {
			let sign = bz[i].signum();
			if sign != 0. {
				if sign != old_sign && old_sign != 0. {
					n_crossings += 1;
				}
				old_sign = sign;
			}
		}
		// if last control point is zero, that counts as crossing too
		if bz[bz.len() - 1].signum() == 0. {
			n_crossings += 1;
		}
		// no solutions
		if n_crossings == 0 {
			return;
		}
		// Unique solution
		if n_crossings == 1 {
			// Stop recursion when the tree is deep enough - return 1 solution at midpoint
			if depth > Self::MAX_DEPTH {
				let ax = right_t - left_t;
				let ay = bz.last().unwrap() - bz[0];

				solutions.push(left_t - ax * bz[0] / ay);
				return;
			}

			let r = bz.secant();
			solutions.push(r * right_t + (1. - r) * left_t);
			return;
		}
		// solve recursively after subdividing control polygon
		let o = bz.len() - 1;
		let mut left = Bezier1d(vec![0.; o + 1]);
		let mut right = bz.clone();
		let mut split_t = (left_t + right_t) * 0.5;

		// If subdivision is working poorly, split around the leftmost root of the derivative
		if depth > 2 {
			let dbz = bz.derivative();

			let mut dsolutions = Vec::new();
			dbz.find_bernstein_roots(&mut dsolutions, 0, left_t, right_t);
			dsolutions.sort_by(f64::total_cmp);

			let mut dsplit_t = 0.5;
			if !dsolutions.is_empty() {
				dsplit_t = dsolutions[0];
				split_t = left_t + (right_t - left_t) * dsplit_t;
			}

			[left, right] = bz.casteljau_subdivision(dsplit_t);
		} else {
			// split at midpoint, because it is cheap
			left[0] = right[0];
			for i in 1..bz.len() {
				for j in 0..(bz.len() - i) {
					right[j] = (right[j] + right[j + 1]) * 0.5;
				}
				left[i] = right[0];
			}
		}
		// Solution is exactly on the subdivision point
		left.reverse();
		while right.len() - 1 > 0 && (right[0]).abs() <= 1e-10 {
			// Deflate
			right = right.deflate();
			left = left.deflate();
			solutions.push(split_t);
		}
		left.reverse();
		if right.len() - 1 > 0 {
			left.find_bernstein_roots(solutions, depth + 1, left_t, split_t);
			right.find_bernstein_roots(solutions, depth + 1, split_t, right_t);
		}
	}
}

impl std::ops::Sub<DVec2> for SymetricalBasisPair {
	type Output = SymetricalBasisPair;
	fn sub(self, rhs: DVec2) -> Self::Output {
		fn sub(a: &SymetricalBasis, b: f64) -> SymetricalBasis {
			if a.iter().all(|x| x.abs_diff_eq(DVec2::ZERO, 1e-5)) {
				return SymetricalBasis(vec![DVec2::splat(-b)]);
			}
			let mut result = a.clone();
			result[0] -= DVec2::splat(b);
			result
		}
		Self {
			x: sub(&self.x, rhs.x),
			y: sub(&self.y, rhs.y),
		}
	}
}

/// Given a multiple of binomial(n, k), modify it to the same multiple of binomial(n, k + 1).
#[must_use]
fn binomial_increment_k(b: f64, n: usize, k: usize) -> f64 {
	b * (n as f64 - k as f64) / (k + 1) as f64
}

/// Given a multiple of binomial(n, k), modify it to the same multiple of binomial(n - 1, k).
#[must_use]
fn binomial_decrement_n(b: f64, n: usize, k: usize) -> f64 {
	b * (n as f64 - k as f64) / n as f64
}

impl Bezier {
	/// Get roots as [[x], [y]]
	#[must_use]
	pub fn roots(&self) -> [Vec<f64>; 2] {
		let s_basis = self.to_symetrical_basis_pair();
		[s_basis.x.roots(), s_basis.y.roots()]
	}

	#[must_use]
	pub fn to_symetrical_basis_pair(&self) -> SymetricalBasisPair {
		let n = match self.handles {
			BezierHandles::Linear => 1,
			BezierHandles::Quadratic { .. } => 2,
			BezierHandles::Cubic { .. } => 3,
		};
		let q = (n + 1) / 2;
		let even = n % 2 == 0;
		let mut sb = SymetricalBasisPair {
			x: SymetricalBasis(vec![DVec2::ZERO; q + even as usize]),
			y: SymetricalBasis(vec![DVec2::ZERO; q + even as usize]),
		};

		let mut nck = 1.;
		for k in 0..q {
			let mut tjk = nck;
			for j in k..q {
				sb.x[j][0] += tjk * self[k].x;
				sb.x[j][1] += tjk * self[n - k].x;
				sb.y[j][0] += tjk * self[k].y;
				sb.y[j][1] += tjk * self[n - k].y;
				tjk = binomial_increment_k(tjk, n - j - k, j - k);
				tjk = binomial_decrement_n(tjk, n - j - k, j - k + 1);
				tjk = -tjk;
			}
			tjk = -nck;
			for j in (k + 1)..q {
				sb.x[j][0] += tjk * self[n - k].x;
				sb.x[j][1] += tjk * self[k].x;
				sb.y[j][0] += tjk * self[n - k].y;
				sb.y[j][1] += tjk * self[k].y;
				tjk = binomial_increment_k(tjk, n - j - k - 1, j - k - 1);
				tjk = binomial_decrement_n(tjk, n - j - k - 1, j - k);
				tjk = -tjk;
			}
			nck = binomial_increment_k(nck, n, k);
		}
		if even {
			let mut tjk = if q % 2 == 1 { -1. } else { 1. };
			for k in 0..q {
				sb.x[q][0] += tjk * (self[k].x + self[n - k].x);
				sb.y[q][0] += tjk * (self[k].y + self[n - k].y);
				tjk = binomial_increment_k(tjk, n, k);
				tjk = -tjk;
			}
			sb.x[q][0] += tjk * self[q].x;
			sb.x[q][1] = sb.x[q][0];
			sb.y[q][0] += tjk * self[q].y;
			sb.y[q][1] = sb.y[q][0];
		}
		sb.x[0][0] = self[0].x;
		sb.x[0][1] = self[n].x;
		sb.y[0][0] = self[0].y;
		sb.y[0][1] = self[n].y;

		sb
	}

	/// Find the t value such that the tangent at t is equal to the direction from t on the curve to the specified point.
	#[must_use]
	pub fn tangent_to_point(&self, point: DVec2) -> Vec<f64> {
		let sbasis = self.to_symetrical_basis_pair();
		let derivative = sbasis.derivative();
		let sub = sbasis - point;
		let cross = sub.cross(&derivative);
		SymetricalBasis::roots(&cross)
	}

	/// Find the t value such that the normal at t is equal to the direction from t on the curve to the specified point.
	#[must_use]
	pub fn normal_to_point(&self, point: DVec2) -> Vec<f64> {
		let sbasis = self.to_symetrical_basis_pair();
		let derivative = sbasis.derivative();
		let sub = sbasis - point;
		let cross = sub.dot(&derivative);
		SymetricalBasis::roots(&cross)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::*;

	#[test]
	fn find_bernstein_roots() {
		let bz = Bezier1d(vec![50.0, -100.0, 170.0]);
		let mut solutions = Vec::new();
		bz.find_bernstein_roots(&mut solutions, 0, 0., 1.);

		solutions.sort_by(f64::total_cmp);
		for &t in &solutions {
			assert!(bz.value_at(t,).abs() < 1e-5, "roots should be roots {} {}", t, bz.value_at(t,));
		}
	}

	#[test]
	fn tangent_at_point() {
		let validate = |bz: Bezier, p: DVec2| {
			let solutions = bz.tangent_to_point(p);
			assert_ne!(solutions.len(), 0);
			for t in solutions {
				let pos = bz.evaluate(TValue::Parametric(t));
				let expected_tangent = (pos - p).normalize();
				let tangent = bz.tangent(TValue::Parametric(t));
				assert!(expected_tangent.perp_dot(tangent).abs() < 0.2, "Expected tangent {expected_tangent} found {tangent} pos {pos}")
			}
		};
		let bz = Bezier::from_quadratic_coordinates(55., 50., 165., 30., 185., 170.);
		let p = DVec2::new(193., 83.);
		validate(bz, p);

		let bz = Bezier::from_cubic_coordinates(55., 30., 18., 139., 175., 30., 185., 160.);
		let p = DVec2::new(127., 121.);
		validate(bz, p);
	}

	#[test]
	fn normal_at_point() {
		let validate = |bz: Bezier, p: DVec2| {
			let solutions = bz.normal_to_point(p);
			assert_ne!(solutions.len(), 0);
			for t in solutions {
				let pos = bz.evaluate(TValue::Parametric(t));
				let expected_normal = (pos - p).normalize();
				let normal = bz.normal(TValue::Parametric(t));
				assert!(expected_normal.perp_dot(normal).abs() < 0.2, "Expected normal {expected_normal} found {normal} pos {pos}")
			}
		};

		let bz = Bezier::from_linear_coordinates(50., 50., 100., 100.);
		let p = DVec2::new(100., 50.);
		validate(bz, p);

		let bz = Bezier::from_quadratic_coordinates(55., 50., 165., 30., 185., 170.);
		let p = DVec2::new(193., 83.);
		validate(bz, p);

		let bz = Bezier::from_cubic_coordinates(55., 30., 18., 139., 175., 30., 185., 160.);
		let p = DVec2::new(127., 121.);
		validate(bz, p);

		let bz = Bezier::from_cubic_coordinates(55.0, 30.0, 85.0, 140.0, 175.0, 30.0, 185.0, 160.0);
		let p = DVec2::new(17., 172.);
		validate(bz, p);
	}
}
