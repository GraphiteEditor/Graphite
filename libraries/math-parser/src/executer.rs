use crate::ast::{BinaryOp, Literal, Node};
use crate::constants::{builtin_function, suffixed_function};
use crate::context::{EvalContext, FunctionProvider, ValueProvider};
use crate::lexer::Constant;
use crate::value::{Number, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
	#[error("Missing value: {0}")]
	MissingValue(String),

	#[error("Missing function: {0}")]
	MissingFunction(String),

	#[error("Invalid arguments for function call")]
	TypeError,

	#[error("Unsupported operand types for operator")]
	OperatorTypeError,

	#[error("Indeterminate result, like `0/0` or `∞ - ∞`")]
	Indeterminate,

	#[error("Value of {0} is not a number")]
	NotANumber(String),
}

/// Settles an operation's result: no operation may produce NaN, so an indeterminate form is an error, and the value takes
/// its canonical form so that a zero imaginary part or a signed zero never changes a later result.
fn settle(value: Value) -> Result<Value, EvalError> {
	let Value::Number(number) = value;
	if number.is_nan() {
		return Err(EvalError::Indeterminate);
	}
	Ok(Value::Number(number.canonical()))
}

/// The canonical form of a value the host supplied for `name`, like [`settle`], except that NaN (which only a host can
/// supply) is an error naming its source.
fn canonical_host_value(name: &str, value: Value) -> Result<Value, EvalError> {
	let Value::Number(number) = value;
	if number.is_nan() {
		return Err(EvalError::NotANumber(name.to_string()));
	}
	Ok(Value::Number(number.canonical()))
}

/// Resolves a name against the environment before the builtin constants, so a binding of exactly that spelling shadows the builtin.
/// The `\` prefix skips the environment.
fn resolve_value<V: ValueProvider, F: FunctionProvider>(context: &EvalContext<V, F>, name: &str) -> Option<Value> {
	let constant = |name: &str| {
		Constant::from_name(name).map(|constant| match constant.value() {
			Literal::Float(real) => Value::from_f64(real),
			Literal::Complex(complex) => Value::Number(Number::Complex(complex)),
		})
	};

	match name.strip_prefix('\\') {
		Some(builtin_name) => constant(builtin_name),
		None => context.get_value(name).or_else(|| constant(name)),
	}
}

impl Node {
	pub fn eval<V: ValueProvider, F: FunctionProvider>(&self, context: &EvalContext<V, F>) -> Result<Value, EvalError> {
		match self {
			Node::Lit(lit) => match lit {
				Literal::Float(num) => Ok(Value::from_f64(*num)),
				Literal::Complex(num) => Ok(Value::Number(Number::Complex(*num))),
			},

			Node::BinOp { lhs, op, rhs } => match (lhs.eval(context)?, rhs.eval(context)?) {
				(Value::Number(lhs), Value::Number(rhs)) => settle(Value::Number(lhs.binary_op(*op, rhs).ok_or(EvalError::OperatorTypeError)?)),
			},
			Node::UnaryOp { expr, op } => match expr.eval(context)? {
				Value::Number(num) => settle(Value::Number(num.unary_op(*op).ok_or(EvalError::OperatorTypeError)?)),
			},
			Node::Var(name) => {
				let value = resolve_value(context, name).ok_or_else(|| EvalError::MissingValue(name.clone()))?;
				canonical_host_value(name, value)
			}
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

				// A host-supplied function shadows the builtin of the same name, unless the `\` prefix asks for the language's own
				let (prefixed, bare_name) = match name.strip_prefix('\\') {
					Some(bare_name) => (true, bare_name),
					None => (false, name.as_str()),
				};

				if !prefixed && let Some(value) = context.run_function(bare_name, values) {
					settle(canonical_host_value(bare_name, value)?)
				} else if let Some(function) = builtin_function(bare_name) {
					settle(function(values).ok_or(EvalError::TypeError)?)
				} else if let Some((function, base)) = suffixed_function(bare_name) {
					// A base-suffixed call like `log10(x)` runs the two-argument form with the suffix baked in as its second argument
					let [value] = values else { return Err(EvalError::TypeError) };
					settle(function(&[*value, Value::from_f64(base)]).ok_or(EvalError::TypeError)?)
				} else if let Some(value) = resolve_value(context, name)
					&& let [Value::Number(argument)] = values
				{
					// A known value applied to one argument is implicit multiplication, so `x(2)` matches `2(3)` and `i(16)`
					let Value::Number(value) = canonical_host_value(name, value)?;
					settle(Value::Number(value.binary_op(BinaryOp::Mul, *argument).ok_or(EvalError::OperatorTypeError)?))
				} else {
					Err(EvalError::MissingFunction(name.to_string()))
				}
			}
			Node::Conditional { condition, if_block, else_block } => {
				let Value::Number(number) = condition.eval(context)?;
				if number.as_bool() { if_block.eval(context) } else { else_block.eval(context) }
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
		test_sqrt: Value::from_f64(2.) => Node::FnCall {
			name: "sqrt".to_string(),
			expr: vec![Node::Lit(Literal::Float(4.))],
		},
		 test_power: Value::from_f64(8.) => Node::BinOp {
			 lhs: Box::new(Node::Lit(Literal::Float(2.))),
			 op: BinaryOp::Pow,
			 rhs: Box::new(Node::Lit(Literal::Float(3.))),
		 },
	}
}
