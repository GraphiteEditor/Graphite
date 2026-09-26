use crate::ast::{BinaryOp, Literal, MatrixNode, Node, SortedCase, UnaryOp, ValueNode};
use crate::constants::{Builtin, MatrixToValue, ValueOfRegions, builtin_function, suffixed_function};
use crate::context::{EvalContext, FunctionProvider, ValueProvider};
use crate::lexer::Constant;
use crate::matrix::{Matrix, Region};
use crate::object::Object;
use crate::quaternion::Quaternion;
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

	#[error("Logic requires values of exactly 0 (false) or 1 (true)")]
	NotATruthValue,

	#[error("Indeterminate result, like `0/0` or `∞ - ∞`")]
	Indeterminate,

	#[error("More than one piecewise case holds, where the cases must be disjoint")]
	OverlappingCases,

	#[error("No piecewise case holds, and there is no `otherwise` case")]
	NoCaseHolds,

	#[error("A singular matrix has no inverse")]
	SingularMatrix,

	#[error("A matrix power must be a whole number")]
	FractionalMatrixPower,

	#[error("A singular range has no interior")]
	SingularRange,

	#[error("Remapping from a flat range is ambiguous, since a value lies at every position along its flat side")]
	FlatRemapSource,

	#[error("Only a matrix without translation has a transpose")]
	AffineTranspose,

	#[error("Value of {0} is not a number")]
	NotANumber(String),
}

/// Settles an operation's result: no operation may produce NaN, so an indeterminate form is an error, and the value takes
/// its canonical form so that a zero imaginary part or a signed zero never changes a later result.
// Inlined with the canonical form it calls, since these run after every operation where a call would cost as much as the work
#[inline(always)]
fn settle(value: Value) -> Result<Value, EvalError> {
	let Value::Number(number) = value;
	if number.is_nan() {
		return Err(EvalError::Indeterminate);
	}
	Ok(Value::Number(number.canonical()))
}

/// Settles a matrix result like [`settle`] does a value, with signed zeros made plain zero.
pub(crate) fn settle_matrix(matrix: Matrix) -> Result<Matrix, EvalError> {
	if matrix.is_nan() {
		return Err(EvalError::Indeterminate);
	}
	Ok(matrix.map(|entry| if entry == 0. { 0. } else { entry }))
}

/// The canonical form of a value the host supplied for `name`, like [`settle`], except that NaN (which only a host can
/// supply) is an error naming its source.
#[inline(always)]
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
	let constant = |name: &str| Constant::from_name(name).map(|constant| Value::Number(constant.value()));

	match name.strip_prefix('\\') {
		Some(builtin_name) => constant(builtin_name),
		None => context.get_value(name).or_else(|| constant(name)),
	}
}

/// Resolves a matrix's name like [`resolve_value`], with `I` the one builtin.
fn resolve_matrix<V: ValueProvider, F: FunctionProvider>(context: &EvalContext<V, F>, name: &str) -> Option<Matrix> {
	let constant = |name: &str| (name == "I").then_some(Matrix::IDENTITY);

	match name.strip_prefix('\\') {
		Some(builtin_name) => constant(builtin_name),
		None => context.get_matrix(name).or_else(|| constant(name)),
	}
}

/// The case whose condition holds, or `None` where none does: every condition is evaluated and must be a truth value, and
/// at most one may hold, since the cases are unordered.
#[inline(always)]
fn holding_case<'a, T, V: ValueProvider, F: FunctionProvider>(context: &EvalContext<V, F>, cases: &'a [SortedCase<T>]) -> Result<Option<&'a T>, EvalError> {
	let mut holding = None;
	let mut overlapping = false;
	for case in cases {
		let Value::Number(condition) = case.condition.eval(context)?;
		match condition.as_bool() {
			Some(false) => {}
			Some(true) if holding.is_none() => holding = Some(&case.value),
			Some(true) => overlapping = true,
			None => return Err(EvalError::NotATruthValue),
		}
	}
	if overlapping {
		return Err(EvalError::OverlappingCases);
	}
	Ok(holding)
}

/// An operation with a matrix result: a value on the left acts as `L_q`, matrices compose, sums are pointwise or attach a
/// value as translation, division is times-inverse, and powers are whole.
fn matrix_binary_op(lhs: Object, op: BinaryOp, rhs: Object) -> Result<Matrix, EvalError> {
	use BinaryOp as Op;
	match (lhs, op, rhs) {
		(Object::Value(Value::Number(q)), Op::Mul, Object::Matrix(b)) => settle_matrix(Matrix::left_multiplication(q.to_quaternion()).compose(*b)),
		(Object::Matrix(a), Op::Mul, Object::Matrix(b)) => settle_matrix(a.compose(*b)),
		(lhs, Op::Div, Object::Matrix(b)) => matrix_binary_op(lhs, Op::Mul, Object::from(b.inverse().ok_or(EvalError::SingularMatrix)?)),
		(Object::Matrix(a), Op::Add, Object::Matrix(b)) => settle_matrix(*a + *b),
		(Object::Matrix(a), Op::Sub, Object::Matrix(b)) => settle_matrix(*a - *b),
		(Object::Matrix(a), Op::Add, Object::Value(Value::Number(t))) | (Object::Value(Value::Number(t)), Op::Add, Object::Matrix(a)) => settle_matrix(a.translated(t.to_quaternion())),
		(Object::Matrix(a), Op::Sub, Object::Value(Value::Number(t))) => settle_matrix(a.translated(-t.to_quaternion())),
		(Object::Value(Value::Number(t)), Op::Sub, Object::Matrix(a)) => settle_matrix((-*a).translated(t.to_quaternion())),
		(Object::Matrix(a), Op::Pow, Object::Value(Value::Number(exponent))) => {
			// A whole exponent is a composition power, a negative one of the inverse, with an integer read exactly past 2^53
			let whole = match exponent {
				Number::Integer(integer) => integer,
				real => real
					.as_real()
					.filter(|real| real.fract() == 0. && real.abs() < i64::MAX as f64)
					.ok_or(EvalError::FractionalMatrixPower)? as i64,
			};
			settle_matrix(a.power(whole).ok_or(EvalError::SingularMatrix)?)
		}
		_ => Err(EvalError::OperatorTypeError),
	}
}

impl Node {
	#[inline]
	pub fn eval<V: ValueProvider, F: FunctionProvider>(&self, context: &EvalContext<V, F>) -> Result<Object, EvalError> {
		match self {
			Node::Value(value) => value.eval(context).map(Object::Value),
			Node::Matrix(matrix) => matrix.eval(context).map(Object::from),
		}
	}
}

impl ValueNode {
	pub fn eval<V: ValueProvider, F: FunctionProvider>(&self, context: &EvalContext<V, F>) -> Result<Value, EvalError> {
		match self {
			ValueNode::Lit(lit) => match lit {
				Literal::Integer(integer) => Ok(Value::from_i64(*integer)),
				Literal::Float(float) => Ok(Value::from_f64(*float)),
			},

			ValueNode::BinOp { lhs, op, rhs } => match (lhs.eval(context)?, rhs.eval(context)?) {
				(Value::Number(lhs), Value::Number(rhs)) => {
					// Logic rejects operands that aren't truth values, while the other operators reject operand types they don't support
					let rejected = if matches!(op, BinaryOp::And | BinaryOp::Or) {
						EvalError::NotATruthValue
					} else {
						EvalError::OperatorTypeError
					};
					settle(Value::Number(lhs.binary_op(*op, rhs).ok_or(rejected)?))
				}
			},
			ValueNode::UnaryOp { expr, op } => match expr.eval(context)? {
				Value::Number(num) => {
					let rejected = if *op == UnaryOp::Not { EvalError::NotATruthValue } else { EvalError::OperatorTypeError };
					settle(Value::Number(num.unary_op(*op).ok_or(rejected)?))
				}
			},
			ValueNode::Comparison { first, rest } => {
				let Value::Number(first) = first.eval(context)?;
				let rest = rest
					.iter()
					.map(|(op, operand)| operand.eval(context).map(|Value::Number(number)| (*op, number)))
					.collect::<Result<Vec<(BinaryOp, Number)>, EvalError>>()?;

				// A `!=` chain asserts every pair distinct, while the ordered chains assert each adjacent pair's relation; every pair is checked so an unsupported comparison errors regardless of the others
				let holds = if rest.iter().all(|(op, _)| *op == BinaryOp::Neq) {
					let numbers: Vec<Number> = std::iter::once(first).chain(rest.iter().map(|(_, number)| *number)).collect();
					numbers.iter().enumerate().all(|(index, a)| numbers[index + 1..].iter().all(|b| a != b))
				} else {
					let mut holds = true;
					let mut previous = first;
					for (op, number) in rest {
						holds &= previous.binary_op(op, number).ok_or(EvalError::OperatorTypeError)?.as_bool() == Some(true);
						previous = number;
					}
					holds
				};
				Ok(Value::from_bool(holds))
			}
			ValueNode::Var(name) => {
				let value = resolve_value(context, name).ok_or_else(|| EvalError::MissingValue(name.clone()))?;
				canonical_host_value(name, value)
			}
			ValueNode::FnCall { name, expr } => {
				// Arguments land in a stack buffer when they fit (builtins take at most 5), avoiding a heap allocation per call
				let mut stack_values = [Value::from_i64(0); 5];
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
				} else if let Some(Builtin::Values { function, .. }) = builtin_function(bare_name) {
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
			// Only the chosen value is evaluated, so an error in any other case's value is never raised
			ValueNode::Piecewise { cases, otherwise } => match (holding_case(context, cases)?, otherwise) {
				(Some(value), _) => value.eval(context),
				(None, Some(otherwise)) => otherwise.eval(context),
				(None, None) => Err(EvalError::NoCaseHolds),
			},
			ValueNode::Apply { matrix, value } => value_of_matrix(context, MatrixValueCase::Apply(matrix, value)),
			ValueNode::OfMatrix { function, matrix } => value_of_matrix(context, MatrixValueCase::OfMatrix(*function, matrix)),
			ValueNode::OfValueAndRegions { function, value, regions } => value_of_matrix(context, MatrixValueCase::OfValueAndRegions(*function, value, regions)),
			ValueNode::MatrixComparison { matrices, distinct } => value_of_matrix(context, MatrixValueCase::Comparison(matrices, *distinct)),
		}
	}
}

/// A value computed from matrices.
enum MatrixValueCase<'a> {
	Apply(&'a MatrixNode, &'a ValueNode),
	OfMatrix(MatrixToValue, &'a MatrixNode),
	OfValueAndRegions(ValueOfRegions, &'a ValueNode, &'a [MatrixNode]),
	Comparison(&'a [MatrixNode], bool),
}

// One function for every value taken from a matrix, called from several sites, since wasm-opt inlines a function with one call site back into the evaluator whatever its attributes say
#[cold]
#[inline(never)]
fn value_of_matrix<V: ValueProvider, F: FunctionProvider>(context: &EvalContext<V, F>, case: MatrixValueCase) -> Result<Value, EvalError> {
	match case {
		MatrixValueCase::Apply(matrix, value) => {
			let matrix = matrix.eval(context)?;
			let Value::Number(value) = value.eval(context)?;
			settle(Value::from(matrix.apply(value.to_quaternion())))
		}
		MatrixValueCase::OfMatrix(function, matrix) => settle(function(matrix.eval(context)?)),
		MatrixValueCase::OfValueAndRegions(function, value, regions) => {
			let value = value.eval(context)?;

			// A range literal is kept by its corners, which may be infinite where no matrix can hold them
			let regions = regions
				.iter()
				.map(|region| match region {
					MatrixNode::Range { from, to } => {
						let (Value::Number(from), Value::Number(to)) = (from.eval(context)?, to.eval(context)?);
						Ok(Region::Range(from.to_quaternion(), to.to_quaternion()))
					}
					region => region.eval(context).map(Region::Map),
				})
				.collect::<Result<Vec<Region>, EvalError>>()?;
			settle(function(value, &regions)?)
		}
		MatrixValueCase::Comparison(matrices, distinct) => {
			let matrices = matrices.iter().map(|matrix| matrix.eval(context)).collect::<Result<Vec<Matrix>, EvalError>>()?;
			let holds = if distinct {
				matrices.iter().enumerate().all(|(index, a)| matrices[index + 1..].iter().all(|b| !a.same_entries(*b)))
			} else {
				matrices.windows(2).all(|pair| pair[0].same_entries(pair[1]))
			};
			Ok(Value::from_bool(holds))
		}
	}
}

impl MatrixNode {
	pub fn eval<V: ValueProvider, F: FunctionProvider>(&self, context: &EvalContext<V, F>) -> Result<Matrix, EvalError> {
		match self {
			MatrixNode::Var(name) => {
				let matrix = resolve_matrix(context, name).ok_or_else(|| EvalError::MissingValue(name.clone()))?;
				if matrix.is_nan() {
					return Err(EvalError::NotANumber(name.clone()));
				}
				settle_matrix(matrix)
			}
			MatrixNode::Literal { entries, by_rows } => {
				let mut quaternions = [Quaternion::ZERO; 4];
				if entries.len() > quaternions.len() {
					return Err(EvalError::TypeError);
				}
				for (slot, entry) in quaternions.iter_mut().zip(entries) {
					let Value::Number(number) = entry.eval(context)?;
					*slot = number.to_quaternion();
				}

				let entries = &quaternions[..entries.len()];
				let matrix = if *by_rows { Matrix::from_rows(entries) } else { Matrix::from_columns(entries) };
				settle_matrix(matrix.ok_or(EvalError::TypeError)?)
			}
			MatrixNode::FromValues { function, arguments } => {
				let values = arguments.iter().map(|argument| argument.eval(context)).collect::<Result<Vec<Value>, EvalError>>()?;
				settle_matrix(function(&values).ok_or(EvalError::TypeError)?)
			}
			MatrixNode::Range { from, to } => {
				let (Value::Number(from), Value::Number(to)) = (from.eval(context)?, to.eval(context)?);
				settle_matrix(Matrix::range(from.to_quaternion(), to.to_quaternion()))
			}
			MatrixNode::OfMatrix { function, matrix } => settle_matrix(function(matrix.eval(context)?)),
			MatrixNode::BinOp { lhs, op, rhs } => matrix_binary_op(lhs.eval(context)?, *op, rhs.eval(context)?),
			MatrixNode::UnaryOp { expr, op } => {
				let matrix = expr.eval(context)?;
				match op {
					UnaryOp::Pos => Ok(matrix),
					UnaryOp::Neg => settle_matrix(-matrix),
					UnaryOp::Transpose if matrix.is_linear() => settle_matrix(matrix.transposed()),
					UnaryOp::Transpose => Err(EvalError::AffineTranspose),
					UnaryOp::Not | UnaryOp::Fac | UnaryOp::Magnitude => Err(EvalError::OperatorTypeError),
				}
			}
			MatrixNode::Piecewise { cases, otherwise } => match (holding_case(context, cases)?, otherwise) {
				(Some(matrix), _) => matrix.eval(context),
				(None, Some(otherwise)) => otherwise.eval(context),
				(None, None) => Err(EvalError::NoCaseHolds),
			},
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::context::NothingMap;

	struct SingleValue(f64);

	impl ValueProvider for SingleValue {
		fn get_value(&self, name: &str) -> Option<Value> {
			(name == "x").then(|| Value::from_f64(self.0))
		}
	}

	#[test]
	fn known_value_with_one_argument_multiplies() {
		// `x(2)` juxtaposes like `2(3)` and `i(16)` instead of silently discarding the argument
		let call = ValueNode::FnCall {
			name: "x".to_string(),
			expr: vec![ValueNode::Lit(Literal::Float(2.))],
		};
		let result = call.eval(&EvalContext::new(SingleValue(5.), NothingMap)).unwrap();
		assert_eq!(result, Value::from_f64(10.));
	}

	#[test]
	fn known_value_with_multiple_arguments_is_an_error() {
		let call = ValueNode::FnCall {
			name: "x".to_string(),
			expr: vec![ValueNode::Lit(Literal::Float(1.)), ValueNode::Lit(Literal::Float(2.))],
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
		test_addition: Value::from_f64(7.) => ValueNode::BinOp {
			lhs: Box::new(ValueNode::Lit(Literal::Float(3.))),
			op: BinaryOp::Add,
			rhs: Box::new(ValueNode::Lit(Literal::Float(4.))),
		},
		test_subtraction: Value::from_f64(1.) => ValueNode::BinOp {
			lhs: Box::new(ValueNode::Lit(Literal::Float(5.))),
			op: BinaryOp::Sub,
			rhs: Box::new(ValueNode::Lit(Literal::Float(4.))),
		},
		test_multiplication: Value::from_f64(12.) => ValueNode::BinOp {
			lhs: Box::new(ValueNode::Lit(Literal::Float(3.))),
			op: BinaryOp::Mul,
			rhs: Box::new(ValueNode::Lit(Literal::Float(4.))),
		},
		test_division: Value::from_f64(2.5) => ValueNode::BinOp {
			lhs: Box::new(ValueNode::Lit(Literal::Float(5.))),
			op: BinaryOp::Div,
			rhs: Box::new(ValueNode::Lit(Literal::Float(2.))),
		},
		test_negation: Value::from_f64(-3.) => ValueNode::UnaryOp {
			expr: Box::new(ValueNode::Lit(Literal::Float(3.))),
			op: UnaryOp::Neg,
		},
		test_sqrt: Value::from_f64(2.) => ValueNode::FnCall {
			name: "sqrt".to_string(),
			expr: vec![ValueNode::Lit(Literal::Float(4.))],
		},
		test_power: Value::from_f64(8.) => ValueNode::BinOp {
			lhs: Box::new(ValueNode::Lit(Literal::Float(2.))),
			op: BinaryOp::Pow,
			rhs: Box::new(ValueNode::Lit(Literal::Float(3.))),
		},
	}
}
