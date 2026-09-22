use crate::ast::{BinaryOp, UnaryOp};
use std::f64::consts::PI;
use std::ops::Mul;

pub type Complex = num_complex::Complex<f64>;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Value {
	Number(Number),
}

/// Generates accessors reading the value rounded to the nearest whole number of the target integer type.
macro_rules! integer_accessors {
	($($fn_name:ident: $int:ty),* $(,)?) => {
		$(
			#[doc = concat!("Reads the value rounded to the nearest whole `", stringify!($int), "`, or `None` if it isn't a real number, isn't finite, or lies outside the type's range.")]
			pub fn $fn_name(&self) -> Option<$int> {
				let rounded = self.as_real()?.round();
				// The MAX comparison is one float rounding step generous for the widest types, where the cast saturates
				(rounded.is_finite() && rounded >= <$int>::MIN as f64 && rounded <= <$int>::MAX as f64).then_some(rounded as $int)
			}
		)*
	};
}

impl Value {
	pub fn from_f64(x: f64) -> Self {
		Self::Number(Number::Real(x))
	}

	pub fn as_real(&self) -> Option<f64> {
		let Self::Number(number) = self;
		number.as_real()
	}

	/// Reads the value as a single-precision float, or `None` if it isn't a real number.
	pub fn as_f32(&self) -> Option<f32> {
		self.as_real().map(|real| real as f32)
	}

	/// Reads the value as a truth value, or `None` unless it is exactly 0 or 1.
	pub fn as_bool(&self) -> Option<bool> {
		let Self::Number(number) = self;
		number.as_bool()
	}

	integer_accessors! {
		as_u8: u8,
		as_u16: u16,
		as_u32: u32,
		as_u64: u64,
		as_u128: u128,
		as_i8: i8,
		as_i16: i16,
		as_i32: i32,
		as_i64: i64,
		as_i128: i128,
	}
}

impl From<f64> for Value {
	fn from(x: f64) -> Self {
		Self::from_f64(x)
	}
}

impl From<Complex> for Value {
	fn from(complex: Complex) -> Self {
		Self::Number(Number::Complex(complex))
	}
}

impl core::fmt::Display for Value {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Value::Number(num) => num.fmt(f),
		}
	}
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Number {
	Real(f64),
	Complex(Complex),
}

impl std::fmt::Display for Number {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Number::Real(real) => real.fmt(f),
			Number::Complex(complex) => complex.fmt(f),
		}
	}
}

impl Number {
	/// Reads the number as a real, or `None` if it has an imaginary part.
	pub fn as_real(self) -> Option<f64> {
		match self {
			Number::Real(real) => Some(real),
			// Canonical form stores a zero imaginary part as a real, so a canonical complex number is never real
			Number::Complex(_) => None,
		}
	}

	/// Widens the number into the complex plane, since every real number is a complex number without an imaginary part.
	pub fn as_complex(self) -> Complex {
		match self {
			Number::Real(real) => Complex::new(real, 0.),
			Number::Complex(complex) => complex,
		}
	}

	/// The truth value of a logical operand, which must be exactly 0 or 1: any other number is not a truth value, so logic on it is an error rather than a guess.
	pub fn as_bool(self) -> Option<bool> {
		match self {
			Number::Real(0.) => Some(false),
			Number::Real(1.) => Some(true),
			_ => None,
		}
	}

	/// The number's canonical form: a zero imaginary part is dropped, since `n + 0i` is exactly `n`, and a signed zero is plain zero, so no zero-valued part can ever change a result.
	pub fn canonical(self) -> Number {
		let unsigned_zero = |x: f64| if x == 0. { 0. } else { x };
		match self {
			Number::Real(real) => Number::Real(unsigned_zero(real)),
			Number::Complex(complex) if complex.im == 0. => Number::Real(unsigned_zero(complex.re)),
			Number::Complex(complex) => Number::Complex(Complex::new(unsigned_zero(complex.re), unsigned_zero(complex.im))),
		}
	}

	/// Whether any part is NaN, which no operation may produce: an indeterminate form is an evaluation error instead.
	pub fn is_nan(self) -> bool {
		match self {
			Number::Real(real) => real.is_nan(),
			Number::Complex(complex) => complex.re.is_nan() || complex.im.is_nan(),
		}
	}

	pub fn binary_op(self, op: BinaryOp, other: Number) -> Option<Number> {
		// Logic and equality work uniformly across real and complex operands
		match op {
			BinaryOp::And | BinaryOp::Or => {
				let (Some(lhs), Some(rhs)) = (self.as_bool(), other.as_bool()) else { return None };
				let result = if matches!(op, BinaryOp::And) { lhs && rhs } else { lhs || rhs };
				return Some(Number::Real(result as u8 as f64));
			}
			BinaryOp::Eq | BinaryOp::Neq => {
				let equal = match (self, other) {
					(Number::Real(lhs), Number::Real(rhs)) => lhs == rhs,
					(Number::Complex(lhs), Number::Complex(rhs)) => lhs == rhs,
					(Number::Real(real), Number::Complex(complex)) | (Number::Complex(complex), Number::Real(real)) => complex == Complex::new(real, 0.),
				};
				return Some(Number::Real((equal != matches!(op, BinaryOp::Neq)) as u8 as f64));
			}
			_ => {}
		}

		match (self, other) {
			(Number::Real(lhs), Number::Real(rhs)) => {
				let result = match op {
					BinaryOp::Add => lhs + rhs,
					BinaryOp::Sub => lhs - rhs,
					BinaryOp::Mul => lhs * rhs,
					BinaryOp::Div => lhs / rhs,
					BinaryOp::Pow => {
						// A negative base under a fractional exponent has no real power, so it climbs to the principal complex one
						let power = lhs.powf(rhs);
						if power.is_nan() {
							return Some(Number::Complex(Complex::new(lhs, 0.).powf(rhs)));
						}
						power
					}
					BinaryOp::Leq => (lhs <= rhs) as u8 as f64,
					BinaryOp::Lt => (lhs < rhs) as u8 as f64,
					BinaryOp::Geq => (lhs >= rhs) as u8 as f64,
					BinaryOp::Gt => (lhs > rhs) as u8 as f64,
					BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::Neq => unreachable!("handled above"),
				};

				Some(Number::Real(result))
			}

			(Number::Complex(lhs), Number::Complex(rhs)) => {
				let result = match op {
					BinaryOp::Add => lhs + rhs,
					BinaryOp::Sub => lhs - rhs,
					BinaryOp::Mul => lhs * rhs,
					BinaryOp::Div => complex_divide(lhs, rhs),
					BinaryOp::Pow if rhs.im == 0. => return complex_real_power(lhs, rhs.re),
					BinaryOp::Pow => lhs.powc(rhs),
					BinaryOp::Leq | BinaryOp::Lt | BinaryOp::Geq | BinaryOp::Gt => {
						return None;
					}
					BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::Neq => unreachable!("handled above"),
				};
				Some(Number::Complex(result))
			}

			(Number::Real(lhs), Number::Complex(rhs)) => {
				let lhs_complex = Complex::new(lhs, 0.);
				let result = match op {
					BinaryOp::Add => lhs_complex + rhs,
					BinaryOp::Sub => lhs_complex - rhs,
					BinaryOp::Mul => lhs_complex * rhs,
					BinaryOp::Div => complex_divide(lhs_complex, rhs),
					BinaryOp::Pow => lhs_complex.powc(rhs),
					_ => return None,
				};
				Some(Number::Complex(result))
			}

			(Number::Complex(lhs), Number::Real(rhs)) => {
				let rhs_complex = Complex::new(rhs, 0.);
				let result = match op {
					BinaryOp::Add => lhs + rhs_complex,
					BinaryOp::Sub => lhs - rhs_complex,
					BinaryOp::Mul => lhs * rhs_complex,
					BinaryOp::Div if rhs == 0. => complex_over_zero(lhs, rhs),
					BinaryOp::Div => lhs / rhs,
					BinaryOp::Pow => return complex_real_power(lhs, rhs),
					_ => return None,
				};
				Some(Number::Complex(result))
			}
		}
	}

	pub fn unary_op(self, op: UnaryOp) -> Option<Number> {
		match op {
			UnaryOp::Pos => Some(self),
			UnaryOp::Neg => Some(match self {
				Number::Real(real) => Number::Real(-real),
				Number::Complex(complex) => Number::Complex(-complex),
			}),
			UnaryOp::Not => self.as_bool().map(|boolean| Number::Real(!boolean as u8 as f64)),
			UnaryOp::Magnitude => Some(Number::Real(match self {
				Number::Real(real) => real.abs(),
				Number::Complex(complex) => complex.norm(),
			})),
			UnaryOp::Fac => Some(match self {
				Number::Real(real) => Number::Real(real_factorial(real)?),
				Number::Complex(complex) => Number::Complex(complex_gamma(complex + 1.)),
			}),
		}
	}

	pub fn from_f64(x: f64) -> Self {
		Self::Real(x)
	}
}

/// Division by a real zero, sending each nonzero part to the infinity of its own sign as an overflow would, so `i / 0` is `∞i`.
fn complex_over_zero(dividend: Complex, zero: f64) -> Complex {
	let part_over_zero = |part: f64| if part == 0. { 0. } else { part / zero };
	Complex::new(part_over_zero(dividend.re), part_over_zero(dividend.im))
}

/// Complex division by Smith's algorithm, scaling by the divisor's larger part so neither `|divisor|²` nor the quotient
/// overflows or underflows while the answer fits, and a simple quotient like `(1 + i) / (1 - i)` stays exact.
pub(crate) fn complex_divide(dividend: Complex, divisor: Complex) -> Complex {
	// Every part is halved when one passes half of f64::MAX, keeping the quotient while the sums below stay in range
	let largest_part = [dividend.re, dividend.im, divisor.re, divisor.im].into_iter().map(f64::abs).fold(0., f64::max);
	let (dividend, divisor) = if largest_part > f64::MAX / 2. { (dividend / 2., divisor / 2.) } else { (dividend, divisor) };

	let Complex { re: a, im: b } = dividend;
	let Complex { re: c, im: d } = divisor;
	if c.abs() >= d.abs() {
		let ratio = d / c;
		let denominator = c + d * ratio;
		Complex::new((a + b * ratio) / denominator, (b - a * ratio) / denominator)
	} else {
		let ratio = c / d;
		let denominator = d + c * ratio;
		Complex::new((a * ratio + b) / denominator, (b * ratio - a) / denominator)
	}
}

/// A complex base under a real exponent: a whole exponent multiplies out exactly by squaring, so `i^2` is `-1` where the polar
/// form leaves a `sin(π)` residue, and any other exponent takes the polar form.
fn complex_real_power(base: Complex, exponent: f64) -> Option<Number> {
	if exponent.fract() != 0. || exponent.abs() >= u128::MAX as f64 {
		return Some(Number::Complex(base.powf(exponent)));
	}

	let power = match exponent.abs() as u128 {
		0 => Complex::from(1.),
		count => whole_power(base, count),
	};
	// An overflowed product has NaN cross terms, so the polar form takes over with the overflow's direction
	if Number::Complex(power).is_nan() {
		return Some(Number::Complex(base.powf(exponent)));
	}

	if exponent < 0. {
		return Number::Real(1.).binary_op(BinaryOp::Div, Number::Complex(power).canonical());
	}
	Some(Number::Complex(power))
}

/// `base^n` for a whole `n` of at least 1 by repeated squaring, in O(log n) multiplications.
fn whole_power<T: Copy + Mul<Output = T>>(mut base: T, mut exponent: u128) -> T {
	while exponent & 1 == 0 {
		base = base * base;
		exponent >>= 1;
	}

	let mut result = base;
	while exponent > 1 {
		exponent >>= 1;
		base = base * base;
		if exponent & 1 == 1 {
			result = result * base;
		}
	}
	result
}

/// The factorial of a real number: the exact product for a whole number, and `x! = Γ(x + 1)` past the whole numbers, or
/// `None` at the negative integers (the gamma function's poles, where no signed limit exists) and at -∞.
fn real_factorial(x: f64) -> Option<f64> {
	// The factorial overflows f64 from 171! on, and returning early keeps a huge whole number from spinning the product loop
	if x > 171. {
		return Some(f64::INFINITY);
	}

	if x.fract() == 0. {
		(x >= 0.).then(|| (1..=x as u64).fold(1., |accumulated, k| accumulated * k as f64))
	} else {
		x.is_finite().then(|| real_gamma(x + 1.))
	}
}

/// The Lanczos approximation's shift, for which [`LANCZOS_COEFFICIENTS`] give 15 significant digits near 1, falling to 13 by the f64 overflow limit.
const LANCZOS_G: f64 = 7.;
const LANCZOS_COEFFICIENTS: [f64; 9] = [
	0.9999999999998099,
	676.5203681218851,
	-1259.1392167224028,
	771.3234287776531,
	-176.6150291621406,
	12.507343278686905,
	-0.13857109526572012,
	9.984369578019572e-6,
	1.5056327351493116e-7,
];

/// The gamma function on the reals by the Lanczos approximation. Below 1/2, where the series loses accuracy, it reflects
/// through `Γ(x) Γ(1 - x) = π / sin(πx)`.
fn real_gamma(x: f64) -> f64 {
	if x < 0.5 {
		return PI / (PI * x).sin() / real_gamma(1. - x);
	}

	let x = x - 1.;
	let series = (1..LANCZOS_COEFFICIENTS.len()).fold(LANCZOS_COEFFICIENTS[0], |sum, k| sum + LANCZOS_COEFFICIENTS[k] / (x + k as f64));
	let t = x + LANCZOS_G + 0.5;

	// One exponential for the whole `t^(x + 1/2) e^-t` factor, so past f64's range it is ∞ rather than the NaN of ∞ × 0
	(2. * PI).sqrt() * ((x + 0.5) * t.ln() - t).exp() * series
}

/// The gamma function over the complex plane, taken as one exponential of its logarithm so a magnitude past f64's range is ∞ or 0.
fn complex_gamma(z: Complex) -> Complex {
	complex_log_gamma(z).exp()
}

/// The natural logarithm of the gamma function over the complex plane, by the same approximation and reflection as [`real_gamma`].
pub(crate) fn complex_log_gamma(z: Complex) -> Complex {
	if z.re < 0.5 {
		// Shifting the real part into `[0, 1)` keeps the sine exact, with a half turn (a sign) for each odd shift
		let shift = z.re.floor();
		let log_sin = complex_log_sin(PI * (z - shift)) + Complex::new(0., if shift.rem_euclid(2.) == 0. { 0. } else { PI });
		return PI.ln() - log_sin - complex_log_gamma(1. - z);
	}

	let z = z - 1.;
	let series = (1..LANCZOS_COEFFICIENTS.len()).fold(Complex::from(LANCZOS_COEFFICIENTS[0]), |sum, k| sum + LANCZOS_COEFFICIENTS[k] / (z + k as f64));
	let t = z + LANCZOS_G + 0.5;

	0.5 * (2. * PI).ln() + (z + 0.5) * t.ln() - t + series.ln()
}

/// `ln sin(w)` without forming `sin(w)`, which overflows past an imaginary part of about 710: with `s` the sign of that part,
/// `sin(w) = e^(-siw) (e^(2siw) - 1) / (2si)`, and the growing exponential becomes a plain shift of the logarithm.
fn complex_log_sin(w: Complex) -> Complex {
	let s = if w.im < 0. { -1. } else { 1. };
	let siw = Complex::new(0., s) * w;

	-siw + (((2. * siw).exp() - 1.) / Complex::new(0., 2. * s)).ln()
}
