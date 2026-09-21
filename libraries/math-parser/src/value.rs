use crate::ast::{BinaryOp, UnaryOp};

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
		match self {
			Self::Number(Number::Real(val)) => Some(*val),
			_ => None,
		}
	}

	/// Reads the value as a single-precision float, or `None` if it isn't a real number.
	pub fn as_f32(&self) -> Option<f32> {
		self.as_real().map(|real| real as f32)
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
	/// Widens the number into the complex plane, since every real number is a complex number without an imaginary part.
	pub fn as_complex(self) -> Complex {
		match self {
			Number::Real(real) => Complex::new(real, 0.),
			Number::Complex(complex) => complex,
		}
	}

	/// The value's truthiness for conditions and logic operators, or `None` for NaN values, which poison the result rather than acting as a boolean.
	pub fn as_bool(self) -> Option<bool> {
		match self {
			Number::Real(real) => (!real.is_nan()).then_some(real != 0.),
			Number::Complex(complex) => (!complex.re.is_nan() && !complex.im.is_nan()).then_some(complex != Complex::ZERO),
		}
	}

	pub fn binary_op(self, op: BinaryOp, other: Number) -> Option<Number> {
		// Logic and equality work uniformly across real and complex operands
		match op {
			BinaryOp::And | BinaryOp::Or => {
				let (Some(lhs), Some(rhs)) = (self.as_bool(), other.as_bool()) else {
					return Some(Number::Real(f64::NAN));
				};
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
					BinaryOp::Modulo => lhs % rhs,
					BinaryOp::Pow => lhs.powf(rhs),
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
					BinaryOp::Div => lhs / rhs,
					BinaryOp::Modulo => lhs % rhs,
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
					BinaryOp::Div => lhs_complex / rhs,
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
					BinaryOp::Div => lhs / rhs_complex,
					BinaryOp::Pow => lhs.powf(rhs),
					_ => return None,
				};
				Some(Number::Complex(result))
			}
		}
	}

	pub fn unary_op(self, op: UnaryOp) -> Number {
		if matches!(op, UnaryOp::Not) {
			return match self.as_bool() {
				Some(boolean) => Number::Real(!boolean as u8 as f64),
				None => Number::Real(f64::NAN),
			};
		}

		match self {
			Number::Real(real) => match op {
				UnaryOp::Pos => Number::Real(real),
				UnaryOp::Neg => Number::Real(-real),
				UnaryOp::Fac => {
					// n! for real n: use integer semantics when n is a
					// non-negative integer, otherwise return NaN.
					if !real.is_finite() {
						return Number::Real(f64::NAN);
					}
					let truncated = real.trunc();
					if truncated < 0. || (real - truncated).abs() > f64::EPSILON {
						return Number::Real(f64::NAN);
					}

					// Return infinity above 170! since that overflows f64, which also keeps huge inputs from spinning the loop
					let n = truncated as u64;
					if n > 170 {
						return Number::Real(f64::INFINITY);
					}
					let mut acc = 1_f64;
					for k in 1..=n {
						acc *= k as f64;
					}
					Number::Real(acc)
				}
				UnaryOp::Not => unreachable!("handled above"),
			},

			Number::Complex(complex) => match op {
				UnaryOp::Pos => Number::Complex(complex),
				UnaryOp::Neg => Number::Complex(-complex),
				UnaryOp::Fac => Number::Complex(Complex::new(f64::NAN, f64::NAN)),
				UnaryOp::Not => unreachable!("handled above"),
			},
		}
	}

	pub fn from_f64(x: f64) -> Self {
		Self::Real(x)
	}
}
