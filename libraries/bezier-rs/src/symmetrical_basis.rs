/*
 * Modifications and Rust port copyright (C) 2024 by 0Hypercube.
 *
 * Original version by lib2geom: <https://gitlab.com/inkscape/lib2geom>
 *
 * The entirety of this file is specially licensed under MPL 1.1 terms:
 *
 *  Original Authors:
 *   Nathan Hurst <njh@mail.csse.monash.edu.au>
 *   Michael Sloan <mgsloan@gmail.com>
 *   Marco Cecchetti <mrcekets at gmail.com>
 *   MenTaLguY <mental@rydia.net>
 *   Michael Sloan <mgsloan@gmail.com>
 *   Nathan Hurst <njh@njhurst.com>
 *   Krzysztof Kosiński <tweenk.pl@gmail.com>
 *   And additional authors listed in the version control history of the following files:
 *   - https://gitlab.com/inkscape/lib2geom/-/blob/master/include/2geom/sbasis.h
 *   - https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/sbasis.cpp
 *   - https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/sbasis-to-bezier.cpp
 *   - https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier.cpp
 *   - https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/solve-bezier.cpp
 *   - https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/solve-bezier-one-d.cpp
 *
 * Copyright (C) 2006-2015 Original Authors
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

use crate::{Bezier, BezierHandles};

use glam::DVec2;

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

// https://gitlab.com/inkscape/lib2geom/-/blob/master/include/2geom/sbasis.h#L70
#[derive(Debug, Clone)]
pub(crate) struct SymmetricalBasis(pub Vec<DVec2>);

impl SymmetricalBasis {
	// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/sbasis.cpp#L323
	#[must_use]
	fn derivative(&self) -> SymmetricalBasis {
		let mut c = SymmetricalBasis(vec![DVec2::ZERO; self.len()]);
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

	// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/sbasis-to-bezier.cpp#L86
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

	fn normalize(&mut self) {
		while self.len() > 1 && self.last().is_some_and(|x| x.abs_diff_eq(DVec2::ZERO, 1e-5)) {
			self.pop();
		}
	}

	#[must_use]
	pub(crate) fn roots(&self) -> Vec<f64> {
		match self.len() {
			0 => Vec::new(),
			1 => {
				let mut res = Vec::new();
				let d = self[0].x - self[0].y;
				if d != 0. {
					let r = self[0].x / d;
					if (0. ..=1.).contains(&r) {
						res.push(r);
					}
				}
				res
			}
			_ => {
				let mut bz = self.to_bezier1d();
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

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/sbasis.cpp#L228
impl<'a> std::ops::Mul for &'a SymmetricalBasis {
	type Output = SymmetricalBasis;
	fn mul(self, b: Self) -> Self::Output {
		let a = self;
		if a.iter().all(|x| x.abs_diff_eq(DVec2::ZERO, 1e-5)) || b.iter().all(|x| x.abs_diff_eq(DVec2::ZERO, 1e-5)) {
			return SymmetricalBasis(vec![DVec2::ZERO]);
		}
		let mut c = SymmetricalBasis(vec![DVec2::ZERO; a.len() + b.len()]);

		for j in 0..b.len() {
			for i in j..(a.len() + j) {
				let tri = (b[j][1] - b[j][0]) * (a[i - j][1] - a[i - j][0]);
				c[i + 1] += DVec2::splat(-tri);
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

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/sbasis.cpp#L88
impl std::ops::Add for SymmetricalBasis {
	type Output = SymmetricalBasis;
	fn add(self, b: Self) -> Self::Output {
		let a = self;
		let out_size = a.len().max(b.len());
		let min_size = a.len().min(b.len());
		let mut result = SymmetricalBasis(vec![DVec2::ZERO; out_size]);
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

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/sbasis.cpp#L110
impl std::ops::Sub for SymmetricalBasis {
	type Output = SymmetricalBasis;
	fn sub(self, b: Self) -> Self::Output {
		let a = self;
		let out_size = a.len().max(b.len());
		let min_size = a.len().min(b.len());
		let mut result = SymmetricalBasis(vec![DVec2::ZERO; out_size]);
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

impl std::ops::Deref for SymmetricalBasis {
	type Target = Vec<DVec2>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl std::ops::DerefMut for SymmetricalBasis {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

#[derive(Debug, Clone)]
pub(crate) struct SymmetricalBasisPair {
	pub x: SymmetricalBasis,
	pub y: SymmetricalBasis,
}

impl SymmetricalBasisPair {
	#[must_use]
	pub fn derivative(&self) -> Self {
		Self {
			x: self.x.derivative(),
			y: self.y.derivative(),
		}
	}

	#[must_use]
	pub fn dot(&self, other: &Self) -> SymmetricalBasis {
		(&self.x * &other.x) + (&self.y * &other.y)
	}

	#[must_use]
	pub fn cross(&self, rhs: &Self) -> SymmetricalBasis {
		(&self.x * &rhs.y) - (&self.y * &rhs.x)
	}
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/include/2geom/sbasis.h#L337
impl std::ops::Sub<DVec2> for SymmetricalBasisPair {
	type Output = SymmetricalBasisPair;
	fn sub(self, rhs: DVec2) -> Self::Output {
		let sub = |a: &SymmetricalBasis, b: f64| {
			if a.iter().all(|x| x.abs_diff_eq(DVec2::ZERO, 1e-5)) {
				return SymmetricalBasis(vec![DVec2::splat(-b)]);
			}
			let mut result = a.clone();
			result[0] -= DVec2::splat(b);
			result
		};

		Self {
			x: sub(&self.x, rhs.x),
			y: sub(&self.y, rhs.y),
		}
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
	const MAX_DEPTH: u32 = 53;

	// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier.cpp#L176
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

	// https://gitlab.com/inkscape/lib2geom/-/blob/master/include/2geom/bezier.h#L55
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

	// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/solve-bezier.cpp#L258
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

	// https://gitlab.com/inkscape/lib2geom/-/blob/master/include/2geom/bezier.h#L78
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

	// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/bezier.cpp#L282
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

	// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/solve-bezier-one-d.cpp#L76
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

			let mut d_solutions = Vec::new();
			dbz.find_bernstein_roots(&mut d_solutions, 0, left_t, right_t);
			d_solutions.sort_by(f64::total_cmp);

			let mut d_split_t = 0.5;
			if !d_solutions.is_empty() {
				d_split_t = d_solutions[0];
				split_t = left_t + (right_t - left_t) * d_split_t;
			}

			[left, right] = bz.casteljau_subdivision(d_split_t);
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

// https://gitlab.com/inkscape/lib2geom/-/blob/master/include/2geom/choose.h#L61
/// Given a multiple of binomial(n, k), modify it to the same multiple of binomial(n, k + 1).
#[must_use]
fn binomial_increment_k(b: f64, n: usize, k: usize) -> f64 {
	b * (n as f64 - k as f64) / (k + 1) as f64
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/include/2geom/choose.h#L52
/// Given a multiple of binomial(n, k), modify it to the same multiple of binomial(n - 1, k).
#[must_use]
fn binomial_decrement_n(b: f64, n: usize, k: usize) -> f64 {
	b * (n as f64 - k as f64) / n as f64
}

// https://gitlab.com/inkscape/lib2geom/-/blob/master/src/2geom/sbasis-to-bezier.cpp#L86
#[must_use]
pub(crate) fn to_symmetrical_basis_pair(bezier: Bezier) -> SymmetricalBasisPair {
	let n = match bezier.handles {
		BezierHandles::Linear => 1,
		BezierHandles::Quadratic { .. } => 2,
		BezierHandles::Cubic { .. } => 3,
	};
	let q = (n + 1) / 2;
	let even = n % 2 == 0;
	let mut sb = SymmetricalBasisPair {
		x: SymmetricalBasis(vec![DVec2::ZERO; q + even as usize]),
		y: SymmetricalBasis(vec![DVec2::ZERO; q + even as usize]),
	};

	let mut nck = 1.;
	for k in 0..q {
		let mut tjk = nck;
		for j in k..q {
			sb.x[j][0] += tjk * bezier[k].x;
			sb.x[j][1] += tjk * bezier[n - k].x;
			sb.y[j][0] += tjk * bezier[k].y;
			sb.y[j][1] += tjk * bezier[n - k].y;
			tjk = binomial_increment_k(tjk, n - j - k, j - k);
			tjk = binomial_decrement_n(tjk, n - j - k, j - k + 1);
			tjk = -tjk;
		}
		tjk = -nck;
		for j in (k + 1)..q {
			sb.x[j][0] += tjk * bezier[n - k].x;
			sb.x[j][1] += tjk * bezier[k].x;
			sb.y[j][0] += tjk * bezier[n - k].y;
			sb.y[j][1] += tjk * bezier[k].y;
			tjk = binomial_increment_k(tjk, n - j - k - 1, j - k - 1);
			tjk = binomial_decrement_n(tjk, n - j - k - 1, j - k);
			tjk = -tjk;
		}
		nck = binomial_increment_k(nck, n, k);
	}
	if even {
		let mut tjk = if q % 2 == 1 { -1. } else { 1. };
		for k in 0..q {
			sb.x[q][0] += tjk * (bezier[k].x + bezier[n - k].x);
			sb.y[q][0] += tjk * (bezier[k].y + bezier[n - k].y);
			tjk = binomial_increment_k(tjk, n, k);
			tjk = -tjk;
		}
		sb.x[q][0] += tjk * bezier[q].x;
		sb.x[q][1] = sb.x[q][0];
		sb.y[q][0] += tjk * bezier[q].y;
		sb.y[q][1] = sb.y[q][0];
	}
	sb.x[0][0] = bezier[0].x;
	sb.x[0][1] = bezier[n].x;
	sb.y[0][0] = bezier[0].y;
	sb.y[0][1] = bezier[n].y;

	sb
}

#[cfg(test)]
mod tests {
	use super::*;

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
}
