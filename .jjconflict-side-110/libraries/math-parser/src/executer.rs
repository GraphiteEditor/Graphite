use crate::ast::{BinaryOp, Literal, Node};
use crate::constants::builtin_function;
use crate::context::{EvalContext, FunctionProvider, ValueProvider};
use crate::value::{Number, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
	#[error("Missing value: {0}")]
	MissingValue(String),

	#[error("Missing function: {0}")]
	MissingFunction(String),

	#[error("Wrong argument types for function call")]
	TypeError,

	#[error("Unsupported operand types for operator")]
	OperatorTypeError,
}

impl Node {
	pub fn eval<V: ValueProvider, F: FunctionProvider>(&self, context: &EvalContext<V, F>) -> Result<Value, EvalError> {
		match self {
			Node::Lit(lit) => match lit {
				Literal::Float(num) => Ok(Value::from_f64(*num)),
				Literal::Complex(num) => Ok(Value::Number(Number::Complex(*num))),
			},

			Node::BinOp { lhs, op, rhs } => match (lhs.eval(context)?, rhs.eval(context)?) {
				(Value::Number(lhs), Value::Number(rhs)) => Ok(Value::Number(lhs.binary_op(*op, rhs).ok_or(EvalError::OperatorTypeError)?)),
			},
			Node::UnaryOp { expr, op } => match expr.eval(context)? {
				Value::Number(num) => Ok(Value::Number(num.unary_op(*op))),
			},
			Node::Var(name) => context.get_value(name).ok_or_else(|| EvalError::MissingValue(name.clone())),
			Node::FnCall { name, expr } => {
				// Arguments land in a stack buffer when they fit (builtins take at most 5), avoiding a heap allocation per call
				let mut stack_values = [Value::from_f64(0.); 5];
				let heap_values: Vec<Value>;
				let values: &[Value] = if expr.len() <= stack_values.len() {
					for (slot, argument) in stack_values.iter_mut().zip(expr) {
						*slot = argument.eval(context)?;
					}
					&stack_values[..expr.len()]
				} else {
					heap_values = expr.iter().map(|argument| argument.eval(context)).collect::<Result<Vec<Value>, EvalError>>()?;
					&heap_values
				};

				if let Some(function) = builtin_function(name) {
					function(values).ok_or(EvalError::TypeError)
				} else if let Some(val) = context.run_function(name, values) {
					Ok(val)
				} else if let Some(Value::Number(value)) = context.get_value(name)
					&& let [Value::Number(argument)] = values
				{
					// A known value applied to one argument is implicit multiplication, so `x(2)` matches `2(3)` and `i(16)`
					Ok(Value::Number(value.binary_op(BinaryOp::Mul, *argument).ok_or(EvalError::OperatorTypeError)?))
				} else {
					Err(EvalError::MissingFunction(name.to_string()))
				}
			}
			Node::Conditional { condition, if_block, else_block } => {
				// A NaN condition yields NaN rather than arbitrarily picking a branch
				let Value::Number(number) = condition.eval(context)?;
				let Some(condition) = number.as_bool() else { return Ok(Value::from_f64(f64::NAN)) };

				if condition { if_block.eval(context) } else { else_block.eval(context) }
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::ast::{BinaryOp, Literal, Node, UnaryOp};
	use crate::context::{EvalContext, NothingMap, ValueProvider};
	use crate::value::Value;

	struct SingleValue(f64);

	impl ValueProvider for SingleValue {
		fn get_value(&self, name: &str) -> Option<Value> {
			(name == "x").then(|| Value::from_f64(self.0))
		}
	}

	#[test]
	fn known_value_with_one_argument_multiplies() {
		// `x(2)` juxtaposes like `2(3)` and `i(16)` instead of silently discarding the argument
		let call = Node::FnCall {
			name: "x".to_string(),
			expr: vec![Node::Lit(Literal::Float(2.))],
		};
		let result = call.eval(&EvalContext::new(SingleValue(5.), NothingMap)).unwrap();
		assert_eq!(result, Value::from_f64(10.));
	}

	#[test]
	fn known_value_with_multiple_arguments_is_an_error() {
		let call = Node::FnCall {
			name: "x".to_string(),
			expr: vec![Node::Lit(Literal::Float(1.)), Node::Lit(Literal::Float(2.))],
		};
		assert!(call.eval(&EvalContext::new(SingleValue(5.), NothingMap)).is_err());
	}

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
		test_addition: Value::from_f64(7.) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(3.))),
			op: BinaryOp::Add,
			rhs: Box::new(Node::Lit(Literal::Float(4.))),
		},
		test_subtraction: Value::from_f64(1.) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(5.))),
			op: BinaryOp::Sub,
			rhs: Box::new(Node::Lit(Literal::Float(4.))),
		},
		test_multiplication: Value::from_f64(12.) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(3.))),
			op: BinaryOp::Mul,
			rhs: Box::new(Node::Lit(Literal::Float(4.))),
		},
		test_division: Value::from_f64(2.5) => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(5.))),
			op: BinaryOp::Div,
			rhs: Box::new(Node::Lit(Literal::Float(2.))),
		},
		test_negation: Value::from_f64(-3.) => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Float(3.))),
			op: UnaryOp::Neg,
		},
		test_sqrt: Value::from_f64(2.) => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Float(4.))),
			op: UnaryOp::Sqrt,
		},
		 test_power: Value::from_f64(8.) => Node::BinOp {
			 lhs: Box::new(Node::Lit(Literal::Float(2.))),
			 op: BinaryOp::Pow,
			 rhs: Box::new(Node::Lit(Literal::Float(3.))),
		 },
	}
}
