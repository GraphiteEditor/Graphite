use crate::ast::{BinaryOp, UnaryOp};
use num_complex::ComplexFloat;
use std::f64::consts::PI;

pub type Complex = num_complex::Complex<f64>;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Value {
	Number(Number),
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
}

impl From<f64> for Value {
	fn from(x: f64) -> Self {
		Self::from_f64(x)
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
			Number::Real(real) => match op {
				UnaryOp::Neg => Number::Real(-real),
				UnaryOp::Sqrt => Number::Real(real.sqrt()),

				UnaryOp::Fac => todo!("Implement factorial"),
			},

			Number::Complex(complex) => match op {
				UnaryOp::Neg => Number::Complex(-complex),
				UnaryOp::Sqrt => Number::Complex(complex.sqrt()),

				UnaryOp::Fac => todo!("Implement factorial"),
			},
		}
	}

	pub fn from_f64(x: f64) -> Self {
		Self::Real(x)
	}
}
