use thiserror::Error;

use crate::{
	ast::{Literal, Node},
	context::{EvalContext, ValueProvider},
	value::Value,
};

#[derive(Debug, Error)]
pub enum EvalError {
	#[error("Missing value: {0}")]
	MissingValue(String),
}

impl Node {
	pub fn eval<T: ValueProvider>(&self, context: &EvalContext<T>) -> Result<Value, EvalError> {
		match self {
			Node::Lit(lit) => match lit {
				Literal::Float(num) => Ok(Value::from_f64(*num)),
			},

			Node::BinOp { lhs, op, rhs } => match (lhs.eval(context)?, rhs.eval(context)?) {
				(Value::Number(lhs), Value::Number(rhs)) => Ok(Value::Number(lhs.binary_op(*op, rhs))),
			},
			Node::UnaryOp { expr, op } => match expr.eval(context)? {
				Value::Number(num) => Ok(Value::Number(num.unary_op(*op))),
			},
			Node::Var(name) => context.get_value(name).cloned().ok_or_else(|| EvalError::MissingValue(name.clone())),
			Node::FnCall { .. } => todo!("implement function calls"),
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::{
		ast::{BinaryOp, Literal, Node, UnaryOp},
		context::{EvalContext, ValueMap},
		value::Value,
	};

	macro_rules! eval_tests {
		($($name:ident: $expected:expr => $expr:expr),* $(,)?) => {
			$(
				#[test]
				fn $name() {
					let result = $expr.eval(&EvalContext::default()).unwrap();
					assert_eq!(result, $expected);
				}
			)*
		};
	}

	eval_tests! {
		test_addition: Value::from_f64(7.0) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(3.0))),
			op: BinaryOp::Add,
			rhs: Box::new(Node::Lit(Literal::Float(4.0))),
		},
		test_subtraction: Value::from_f64(1.0) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(5.0))),
			op: BinaryOp::Sub,
			rhs: Box::new(Node::Lit(Literal::Float(4.0))),
		},
		test_multiplication: Value::from_f64(12.0) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(3.0))),
			op: BinaryOp::Mul,
			rhs: Box::new(Node::Lit(Literal::Float(4.0))),
		},
		test_division: Value::from_f64(2.5) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(5.0))),
			op: BinaryOp::Div,
			rhs: Box::new(Node::Lit(Literal::Float(2.0))),
		},
		test_negation: Value::from_f64(-3.0) => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Float(3.0))),
			op: UnaryOp::Neg,
		},
		test_sqrt: Value::from_f64(2.0) => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Float(4.0))),
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
		 test_power: Value::from_f64(8.0) => Node::BinOp {
			 lhs: Box::new(Node::Lit(Literal::Float(2.0))),
			 op: BinaryOp::Pow,
			 rhs: Box::new(Node::Lit(Literal::Float(3.0))),
		 },
	}
}
