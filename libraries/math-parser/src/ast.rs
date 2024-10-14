use thiserror::Error;

use crate::value::Value;

#[derive(Debug, PartialEq)]
pub struct Unit {
	pub scale: f64,
	// Exponent of length unit (meters)
	pub length: i32,
	// Exponent of mass unit (kilograms)
	pub mass: i32,
	// Exponent of time unit (seconds)
	pub time: i32,
}

#[derive(Debug, PartialEq)]
pub enum Literal {
	Int(i32),
	Float(f64),
}

impl From<f64> for Literal {
	fn from(value: f64) -> Self {
		Self::Float(value)
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
	Add,
	Sub,
	Mul,
	Div,
	Pow,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnaryOp {
	Neg,
	Sqrt,
	Sin,
	Cos,
	Tan,
	Csc,
	Sec,
	Cot,
	InvSin,
	InvCos,
	InvTan,
	InvCsc,
	InvSec,
	InvCot,
	Fac,
}

#[derive(Debug, PartialEq)]
pub enum Node {
	Lit(Literal),
	Var(String),
	FnCall { name: String, expr: Box<Node> },
	GlobalVar(String),
	BinOp { lhs: Box<Node>, op: BinaryOp, rhs: Box<Node> },
	UnaryOp { expr: Box<Node>, op: UnaryOp },
}

#[derive(Debug, Error)]
pub enum EvalError {}

impl Node {
	pub fn eval(&self) -> Result<Value, EvalError> {
		match self {
			Node::Lit(lit) => match lit {
				Literal::Int(num) => Ok(Value::from_f64(*num as f64)),
				Literal::Float(num) => Ok(Value::from_f64(*num)),
			},

			Node::BinOp { lhs, op, rhs } => match (lhs.eval()?, rhs.eval()?) {
				(Value::Number(lhs), Value::Number(rhs)) => Ok(Value::Number(lhs.binary_op(*op, rhs))),
			},
			Node::UnaryOp { expr, op } => match expr.eval()? {
				Value::Number(num) => Ok(Value::Number(num.unary_op(*op))),
			},
			Node::Var(_) => todo!("implement vars"),
			Node::FnCall { .. } => todo!("implement function calls"),
			Node::GlobalVar(_) => todo!("Implement global vars"),
		}
	}
}

#[cfg(test)]
mod tests {

	use crate::{
		ast::{BinaryOp, Literal, Node, UnaryOp},
		value::Value,
	};

	macro_rules! eval_tests {
		($($name:ident: $expected:expr => $expr:expr),* $(,)?) => {
			$(
				#[test]
				fn $name() {
					let result = $expr.eval().unwrap();
					assert_eq!(result, $expected);
				}
			)*
		};
	}

	eval_tests! {
		test_addition: Value::from_f64(7.0) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Int(3))),
			op: BinaryOp::Add,
			rhs: Box::new(Node::Lit(Literal::Int(4))),
		},
		test_subtraction: Value::from_f64(1.0) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Int(5))),
			op: BinaryOp::Sub,
			rhs: Box::new(Node::Lit(Literal::Int(4))),
		},
		test_multiplication: Value::from_f64(12.0) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Int(3))),
			op: BinaryOp::Mul,
			rhs: Box::new(Node::Lit(Literal::Int(4))),
		},
		test_division: Value::from_f64(2.5) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(5.0))),
			op: BinaryOp::Div,
			rhs: Box::new(Node::Lit(Literal::Int(2))),
		},
		test_negation: Value::from_f64(-3.0) => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Int(3))),
			op: UnaryOp::Neg,
		},
		test_sqrt: Value::from_f64(2.0) => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Int(4))),
			op: UnaryOp::Sqrt,
		},
		test_sine: Value::from_f64(0.0) => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Float(0.0))),
			op: UnaryOp::Sin,
		},
		test_cosine: Value::from_f64(1.0) => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Float(0.0))),
			op: UnaryOp::Cos,
		},
		// test_power: Value::from_f64(8.0) => Node::BinOp {
		// 	lhs: Box::new(Node::Lit(Literal::Int(2))),
		// 	op: BinaryOp::Pow,
		// 	rhs: Box::new(Node::Lit(Literal::Int(3))),
		// },
	}
}
