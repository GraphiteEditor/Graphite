use crate::ast::{Literal, Node};
use crate::constants::DEFAULT_FUNCTIONS;
use crate::context::{EvalContext, FunctionProvider, ValueProvider};
use crate::value::{Number, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
	#[error("Missing value: {0}")]
	MissingValue(String),

	#[error("Missing function: {0}")]
	MissingFunction(String),
	#[error("Wrong type for function call")]
	TypeError,
}

impl Node {
	pub fn eval<V: ValueProvider, F: FunctionProvider>(&self, context: &EvalContext<V, F>) -> Result<Value, EvalError> {
		match self {
			Node::Lit(lit) => match lit {
				Literal::Float(num) => Ok(Value::from_f64(*num)),
				Literal::Complex(num) => Ok(Value::Number(Number::Complex(*num))),
			},

			Node::BinOp { lhs, op, rhs } => match (lhs.eval(context)?, rhs.eval(context)?) {
				(Value::Number(lhs), Value::Number(rhs)) => Ok(Value::Number(lhs.binary_op(*op, rhs))),
			},
			Node::UnaryOp { expr, op } => match expr.eval(context)? {
				Value::Number(num) => Ok(Value::Number(num.unary_op(*op))),
			},
			Node::Var(name) => context.get_value(name).ok_or_else(|| EvalError::MissingValue(name.clone())),
			Node::FnCall { name, expr } => {
				let values = expr.iter().map(|expr| expr.eval(context)).collect::<Result<Vec<Value>, EvalError>>()?;
				if let Some(function) = DEFAULT_FUNCTIONS.get(&name.as_str()) {
					function(&values).ok_or(EvalError::TypeError)
				} else if let Some(val) = context.run_function(name, &values) {
					Ok(val)
				} else {
					context.get_value(name).ok_or_else(|| EvalError::MissingFunction(name.to_string()))
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::ast::{BinaryOp, Literal, Node, UnaryOp};
	use crate::context::{EvalContext, ValueMap};
	use crate::value::Value;

	macro_rules! eval_tests {
		($($name:ident: $expected:expr_2021 => $expr:expr_2021),* $(,)?) => {
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
		 test_power: Value::from_f64(8.0) => Node::BinOp {
			 lhs: Box::new(Node::Lit(Literal::Float(2.0))),
			 op: BinaryOp::Pow,
			 rhs: Box::new(Node::Lit(Literal::Float(3.0))),
		 },
	}
}
