use crate::ast::{BinaryOp, UnaryOp};

type Complex = num_complex::Complex<f64>;

#[derive(Debug, PartialEq)]
pub enum Number {
	Real(f64),
	Complex(Complex),
}

impl Number {
	pub fn binary_op(self, op: BinaryOp, other: Number) -> Number {
		match (self, other) {
			(Number::Real(lhs), Number::Real(rhs)) => {
				let result = match op {
					BinaryOp::Add => lhs + rhs,
					BinaryOp::Sub => lhs - rhs,
					BinaryOp::Mul => lhs * rhs,
					BinaryOp::Div => lhs / rhs,
					BinaryOp::Pow => lhs.powf(rhs),
				};
				Number::Real(result)
			}

			(Number::Complex(lhs), Number::Complex(rhs)) => {
				let result = match op {
					BinaryOp::Add => lhs + rhs,
					BinaryOp::Sub => lhs - rhs,
					BinaryOp::Mul => lhs * rhs,
					BinaryOp::Div => lhs / rhs,
					BinaryOp::Pow => lhs.powc(rhs),
				};
				Number::Complex(result)
			}

			(Number::Real(lhs), Number::Complex(rhs)) => {
				let lhs_complex = Complex::new(lhs, 0.0);
				let result = match op {
					BinaryOp::Add => lhs_complex + rhs,
					BinaryOp::Sub => lhs_complex - rhs,
					BinaryOp::Mul => lhs_complex * rhs,
					BinaryOp::Div => lhs_complex / rhs,
					BinaryOp::Pow => lhs_complex.powc(rhs),
				};
				Number::Complex(result)
			}

			(Number::Complex(lhs), Number::Real(rhs)) => {
				let rhs_complex = Complex::new(rhs, 0.0);
				let result = match op {
					BinaryOp::Add => lhs + rhs_complex,
					BinaryOp::Sub => lhs - rhs_complex,
					BinaryOp::Mul => lhs * rhs_complex,
					BinaryOp::Div => lhs / rhs_complex,
					BinaryOp::Pow => lhs.powf(rhs),
				};
				Number::Complex(result)
			}
		}
	}

	pub fn unary_op(self, op: UnaryOp) -> Number {
		match self {
			Number::Real(real) => {
				let result = match op {
					UnaryOp::Neg => Number::Real(-real),
					UnaryOp::Sqrt => Number::Real(real.sqrt()),

					UnaryOp::Sin => Number::Real(real.sin()),
					UnaryOp::Cos => Number::Real(real.cos()),
					UnaryOp::Tan => Number::Real(real.tan()),
					UnaryOp::Csc => Number::Real(1.0 / real.sin()),
					UnaryOp::Sec => Number::Real(1.0 / real.cos()),
					UnaryOp::Cot => Number::Real(1.0 / real.tan()),

					UnaryOp::InvSin => Number::Real(real.asin()),
					UnaryOp::InvCos => Number::Real(real.acos()),
					UnaryOp::InvTan => Number::Real(real.atan()),

					_ => unreachable!(),
				};
				result
			}

			Number::Complex(complex) => {
				let result = match op {
					UnaryOp::Neg => Number::Complex(-complex),
					UnaryOp::Sqrt => Number::Complex(complex.sqrt()),

					UnaryOp::Sin => Number::Complex(complex.sin()),
					UnaryOp::Cos => Number::Complex(complex.cos()),
					UnaryOp::Tan => Number::Complex(complex.tan()),

					UnaryOp::Csc => Number::Complex(Complex::new(1.0, 0.0) / complex.sin()),
					UnaryOp::Sec => Number::Complex(Complex::new(1.0, 0.0) / complex.cos()),
					UnaryOp::Cot => Number::Complex(Complex::new(1.0, 0.0) / complex.tan()),

					UnaryOp::InvSin => Number::Complex(complex.asin()),
					UnaryOp::InvCos => Number::Complex(complex.acos()),
					UnaryOp::InvTan => Number::Complex(complex.atan()),

					_ => unreachable!(),
				};
				result
			}
		}
	}

	pub fn from_f64(x: f64) -> Self {
		Self::Real(x)
	}
}

#[derive(Debug, PartialEq)]
pub enum Value {
	Number(Number),
}

impl Value {
	pub fn from_f64(x: f64) -> Self {
		Self::Number(Number::Real(x))
	}
	/// Attempt to convert to a real number
	pub fn as_real(&self) -> Option<f64> {
		match self {
			Self::Complex(real, imaginary) if imaginary.abs() < f64::EPSILON => Some(*real),
			_ => None,
		}
	}
}

impl From<f64> for Value {
	fn from(x: f64) -> Self {
		Self::from_f64(x)
	}
}

impl core::fmt::Display for Value {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(real) = self.as_real() {
			return real.fmt(f);
		}
		match self {
			Value::Complex(real, imaginary) => write!(f, "{real}{imaginary:+}i"),
		}
	}
}
