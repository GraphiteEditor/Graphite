use crate::value::{Complex, part_product, power_of_two_scale, rescaled_product};
use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// A quaternion `w + x i + y j + z k`, the language's one value type in full generality: every narrower value is a
/// quaternion whose remaining parts are zero, with `ijk = xyz`. The real part `w` is the weight, not a homogeneous coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Quaternion {
	pub w: f64,
	pub x: f64,
	pub y: f64,
	pub z: f64,
}

impl Quaternion {
	pub const ONE: Self = Self::new(1., 0., 0., 0.);
	pub const I: Self = Self::new(0., 1., 0., 0.);
	pub const J: Self = Self::new(0., 0., 1., 0.);
	pub const K: Self = Self::new(0., 0., 0., 1.);

	pub const fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
		Self { w, x, y, z }
	}

	/// Embeds a complex number as its `1` and `i` parts.
	pub fn from_complex(complex: Complex) -> Self {
		Self::new(complex.re, complex.im, 0., 0.)
	}

	/// Every part set to the one value.
	pub fn splat(part: f64) -> Self {
		Self::new(part, part, part, part)
	}

	/// Builds a quaternion from its parts in the order the bases `1`, `i`, `j`, `k` are written.
	pub fn from_parts([w, x, y, z]: [f64; 4]) -> Self {
		Self::new(w, x, y, z)
	}

	/// The parts in the order the bases `1`, `i`, `j`, `k` are written.
	pub fn parts(self) -> [f64; 4] {
		[self.w, self.x, self.y, self.z]
	}

	/// Whether the vector part is zero, leaving a real number.
	pub fn is_real(self) -> bool {
		self.x == 0. && self.y == 0. && self.z == 0.
	}

	/// Applies a function to every part.
	pub fn map(self, function: impl Fn(f64) -> f64) -> Self {
		Self::new(function(self.w), function(self.x), function(self.y), function(self.z))
	}

	/// Combines two quaternions part by part.
	pub fn zip(self, other: Self, function: impl Fn(f64, f64) -> f64) -> Self {
		Self::new(function(self.w, other.w), function(self.x, other.x), function(self.y, other.y), function(self.z, other.z))
	}

	/// The conjugate, which negates the vector part.
	pub fn conj(self) -> Self {
		Self::new(self.w, -self.x, -self.y, -self.z)
	}

	/// The inner product over all four parts, taken as `[1](a * conj(b))` so it shares the product's handling of zero parts and overflow.
	pub fn dot(self, other: Self) -> f64 {
		(self * other.conj()).w
	}

	pub fn norm_squared(self) -> f64 {
		self.dot(self)
	}

	/// The Euclidean magnitude over all four parts.
	pub fn norm(self) -> f64 {
		root_sum_of_squares(&self.parts())
	}

	/// The magnitude of the vector part alone.
	pub fn vector_norm(self) -> f64 {
		root_sum_of_squares(&[self.x, self.y, self.z])
	}

	/// Scales to unit magnitude, or `None` at zero, where no direction exists. An infinite quaternion points along its infinite parts.
	pub fn normalized(self) -> Option<Self> {
		let direction = if self.parts().iter().any(|part| part.is_infinite()) {
			self.map(|part| if part.is_infinite() { part.signum() } else { 0. })
		} else {
			self
		};

		// Dividing by a power of two near the largest part is exact and keeps a huge quaternion's norm finite
		let scale = power_of_two_scale(direction.parts().into_iter());
		let scaled = direction.map(|part| part / scale);
		let norm = scaled.norm();
		(norm != 0.).then(|| scaled.map(|part| part / norm))
	}

	/// The multiplicative inverse, `conj(q) / |q|²`, over parts divided by a power of two near the largest so the square neither overflows nor underflows.
	pub fn inverse(self) -> Self {
		// The limit at an infinite magnitude, where an infinite part over the norm would be NaN
		if self.parts().iter().any(|part| part.is_infinite()) {
			return Self::splat(0.);
		}

		let scale = power_of_two_scale(self.parts().into_iter());
		let scaled = self.map(|part| part / scale);
		scaled.conj().map(|part| part / scaled.norm_squared() / scale)
	}

	/// Applies a complex function in the quaternion's own complex plane, the one spanned by `1` and its normalized vector
	/// part, which is how every function of one variable extends past the complex numbers. A quaternion with no vector part
	/// takes its answer in the `1, i` plane, like a real does.
	pub fn in_plane(self, function: impl Fn(Complex) -> Complex) -> Self {
		let result = function(Complex::new(self.w, self.vector_norm()));
		let Some(direction) = Self::new(0., self.x, self.y, self.z).normalized() else {
			return Self::from_complex(result);
		};

		// A zero part of the direction stays zero beside an infinite imaginary part
		let vector = direction.map(|part| part_product(part, result.im, false));
		Self::new(result.re, vector.x, vector.y, vector.z)
	}

	pub fn ln(self) -> Self {
		self.in_plane(Complex::ln)
	}

	pub fn exp(self) -> Self {
		self.in_plane(Complex::exp)
	}

	/// The power `q^p = exp(ln(q) * p)`, which pins the operand order for exponents that do not commute with the base.
	pub fn pow(self, exponent: Self) -> Self {
		(self.ln() * exponent).exp()
	}
}

/// The root of the sum of squares, with the parts divided by a power of two near the largest while squaring so none can overflow.
fn root_sum_of_squares(parts: &[f64]) -> f64 {
	let scale = power_of_two_scale(parts.iter().copied());
	parts.iter().map(|part| (part / scale).powi(2)).sum::<f64>().sqrt() * scale
}

impl Add for Quaternion {
	type Output = Self;
	fn add(self, other: Self) -> Self {
		self.zip(other, |a, b| a + b)
	}
}

impl Sub for Quaternion {
	type Output = Self;
	fn sub(self, other: Self) -> Self {
		self.zip(other, |a, b| a - b)
	}
}

impl Neg for Quaternion {
	type Output = Self;
	fn neg(self) -> Self {
		self.map(|part| -part)
	}
}

/// The Hamilton product over [`part_product`], so two pure vectors multiply to `-dot + cross`.
impl Mul for Quaternion {
	type Output = Self;
	fn mul(self, other: Self) -> Self {
		Self::from_parts(rescaled_product(self.parts(), other.parts(), |a, b| {
			let (a, b) = (Self::from_parts(a), Self::from_parts(b));
			let zero_operand = a == Self::splat(0.) || b == Self::splat(0.);
			let term = |x: f64, y: f64| part_product(x, y, zero_operand);
			[
				term(a.w, b.w) - term(a.x, b.x) - term(a.y, b.y) - term(a.z, b.z),
				term(a.w, b.x) + term(a.x, b.w) + term(a.y, b.z) - term(a.z, b.y),
				term(a.w, b.y) - term(a.x, b.z) + term(a.y, b.w) + term(a.z, b.x),
				term(a.w, b.z) + term(a.x, b.y) - term(a.y, b.x) + term(a.z, b.w),
			]
		}))
	}
}

/// Division is multiplication by the inverse on the right, `x / y = x * y⁻¹`.
impl Div for Quaternion {
	type Output = Self;
	// The product is the definition here, not the mistaken operator clippy suspects in a `Div` impl
	#[allow(clippy::suspicious_arithmetic_impl)]
	fn div(self, other: Self) -> Self {
		// Both scale by the divisor's power of two, which is exact and keeps a tiny divisor's inverse from overflowing
		let scale = power_of_two_scale(other.parts().into_iter());
		self.map(|part| part / scale) * other.map(|part| part / scale).inverse()
	}
}

impl fmt::Display for Quaternion {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// Only the nonzero parts are written, each with its basis, in the style `1+2i-3j`
		let mut written = false;
		for (part, basis) in self.parts().into_iter().zip(["", "i", "j", "k"]) {
			if part == 0. {
				continue;
			}
			if written && part.is_sign_positive() {
				f.write_str("+")?;
			}
			write!(f, "{part}{basis}")?;
			written = true;
		}

		if !written {
			f.write_str("0")?;
		}
		Ok(())
	}
}
