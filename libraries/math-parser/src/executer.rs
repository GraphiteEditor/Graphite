use crate::ast::{BinaryOp, Clause, Literal, Local, MatrixNode, Node, SortedCase, UnaryOp, ValueNode};
use crate::constants::{Builtin, MatrixToValue, ValueOfRegions, builtin_function, suffixed_function};
use crate::context::{EvalContext, FunctionProvider, ValueProvider};
use crate::lexer::Constant;
use crate::matrix::{Matrix, Region};
use crate::object::Object;
use crate::quaternion::Quaternion;
use crate::value::{Number, Value};
use std::cell::OnceCell;
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

	#[error("A smoothstep across a flat range has no width to ease over")]
	FlatSmoothstep,

	#[error("A smoothstep's continuity is a whole number from 0 to 3")]
	SmoothstepContinuity,

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

/// Where an expression is evaluated: the host's context, and the innermost frame of `where` definitions or call arguments around it.
struct Scope<'a, V: ValueProvider, F: FunctionProvider> {
	context: &'a EvalContext<V, F>,
	frame: Option<&'a Frame<'a>>,
}

impl<'a, V: ValueProvider, F: FunctionProvider> Scope<'a, V, F> {
	fn root(context: &'a EvalContext<V, F>) -> Self {
		Self { context, frame: None }
	}

	/// The frame `depth` levels out from the innermost, which always exists for a tree from the sort pass.
	fn frame(&self, depth: usize) -> Result<&'a Frame<'a>, EvalError> {
		let mut frame = self.frame;
		for _ in 0..depth {
			frame = frame.and_then(|frame| frame.parent);
		}
		frame.ok_or(EvalError::OperatorTypeError)
	}
}

/// The definitions of one `where` clause, or the arguments of one call, each evaluated on its first read and at most once.
struct Frame<'a> {
	/// The frame lexically around this one, which for a call is the frame of the clause defining the function.
	parent: Option<&'a Frame<'a>>,
	definitions: Definitions<'a>,
	/// Each definition's result, once read.
	results: &'a [OnceCell<Object>],
}

enum Definitions<'a> {
	/// A clause's definitions, evaluated within the clause so each may read the others.
	Clause(&'a Clause),
	/// A call's arguments, evaluated where the call is.
	Arguments { arguments: &'a [Node], caller: Option<&'a Frame<'a>> },
}

/// A read through the names `where` clauses define, of the sort `T` evaluates to.
enum Bound<'a, T> {
	Local(Local),
	Call(Local, &'a [Node]),
	Where(&'a Clause, &'a T),
}

// Each sort reads through `where` definitions by an outlined function, keeping its frames out of the recursive evaluator's, and
// called from several sites since wasm-opt inlines a function with one call site
#[inline(never)]
fn bound_value<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, bound: Bound<ValueNode>) -> Result<Value, EvalError> {
	match bound {
		Bound::Local(local) => read(scope, local)?.as_value().copied().ok_or(EvalError::OperatorTypeError),
		Bound::Call(function, arguments) => call(scope, function, arguments, |body, scope| match body {
			Node::Value(body) => body.eval_in(scope),
			Node::Matrix(_) => Err(EvalError::OperatorTypeError),
		}),
		Bound::Where(clause, body) => enter(scope, scope.frame, Definitions::Clause(clause), clause.values.len(), |scope| body.eval_in(scope)),
	}
}

#[inline(never)]
fn bound_matrix<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, bound: Bound<MatrixNode>) -> Result<Matrix, EvalError> {
	match bound {
		Bound::Local(local) => read(scope, local)?.as_matrix().copied().ok_or(EvalError::OperatorTypeError),
		Bound::Call(function, arguments) => call(scope, function, arguments, |body, scope| match body {
			Node::Matrix(body) => body.eval_in(scope),
			Node::Value(_) => Err(EvalError::OperatorTypeError),
		}),
		Bound::Where(clause, body) => enter(scope, scope.frame, Definitions::Clause(clause), clause.values.len(), |scope| body.eval_in(scope)),
	}
}

/// A definition or argument's result, evaluated on its first read: a clause's definition within the clause, so it can read the
/// others, and an argument where the call is.
fn read<'a, V: ValueProvider, F: FunctionProvider>(scope: &Scope<'a, V, F>, local: Local) -> Result<&'a Object, EvalError> {
	let frame = scope.frame(local.depth)?;
	let result = frame.results.get(local.index).ok_or(EvalError::OperatorTypeError)?;
	if let Some(object) = result.get() {
		return Ok(object);
	}

	let (definition, within) = match frame.definitions {
		Definitions::Clause(clause) => (clause.values.get(local.index), Some(frame)),
		Definitions::Arguments { arguments, caller } => (arguments.get(local.index), caller),
	};
	let object = definition.ok_or(EvalError::OperatorTypeError)?.eval_in(&Scope {
		context: scope.context,
		frame: within,
	})?;
	Ok(result.get_or_init(|| object))
}

/// Runs a function a `where` clause defines, whose body reads its parameters within the scope defining the function, lexically.
fn call<'a, V: ValueProvider, F: FunctionProvider, T>(
	scope: &Scope<'a, V, F>,
	function: Local,
	arguments: &'a [Node],
	evaluate: impl FnOnce(&Node, &Scope<'_, V, F>) -> Result<T, EvalError>,
) -> Result<T, EvalError> {
	let definer = scope.frame(function.depth)?;
	let Definitions::Clause(clause) = definer.definitions else {
		return Err(EvalError::OperatorTypeError);
	};
	let body = clause.functions.get(function.index).ok_or(EvalError::OperatorTypeError)?;

	let definitions = Definitions::Arguments { arguments, caller: scope.frame };
	enter(scope, Some(definer), definitions, arguments.len(), |scope| evaluate(body, scope))
}

/// Evaluates a body within a new frame of unread definitions, held on the stack when there are few.
fn enter<'a, V: ValueProvider, F: FunctionProvider, T>(
	scope: &Scope<'a, V, F>,
	parent: Option<&'a Frame<'a>>,
	definitions: Definitions<'a>,
	count: usize,
	body: impl FnOnce(&Scope<'_, V, F>) -> Result<T, EvalError>,
) -> Result<T, EvalError> {
	const STACK_RESULTS: usize = 4;
	let stack_results: [OnceCell<Object>; STACK_RESULTS];
	let heap_results: Vec<OnceCell<Object>>;
	let results: &[OnceCell<Object>] = if count <= STACK_RESULTS {
		stack_results = Default::default();
		&stack_results[..count]
	} else {
		heap_results = (0..count).map(|_| OnceCell::new()).collect();
		&heap_results
	};

	let frame = Frame { parent, definitions, results };
	body(&Scope {
		context: scope.context,
		frame: Some(&frame),
	})
}

/// The case whose condition holds, or `None` where none does: every condition is evaluated and must be a truth value, and
/// at most one may hold, since the cases are unordered.
#[inline(always)]
fn holding_case<'a, T, V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, cases: &'a [SortedCase<T>]) -> Result<Option<&'a T>, EvalError> {
	let mut holding = None;
	let mut overlapping = false;
	for case in cases {
		let Value::Number(condition) = case.condition.eval_in(scope)?;
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

/// An operand of a matrix operation, holding a matrix on the stack since the operation consumes it at once.
enum Operand {
	Value(Value),
	Matrix(Matrix),
}

impl Node {
	fn operand<V: ValueProvider, F: FunctionProvider>(&self, scope: &Scope<'_, V, F>) -> Result<Operand, EvalError> {
		match self {
			Node::Value(value) => value.eval_in(scope).map(Operand::Value),
			Node::Matrix(matrix) => matrix.eval_in(scope).map(Operand::Matrix),
		}
	}
}

/// An operation with a matrix result: a value on the left acts as `L_q`, matrices compose, sums are pointwise or attach a
/// value as translation, division is times-inverse, and powers are whole.
fn matrix_binary_op(lhs: Operand, op: BinaryOp, rhs: Operand) -> Result<Matrix, EvalError> {
	use BinaryOp as Op;
	match (lhs, op, rhs) {
		(Operand::Value(Value::Number(q)), Op::Mul, Operand::Matrix(b)) => settle_matrix(Matrix::left_multiplication(q.to_quaternion()).compose(b)),
		(Operand::Matrix(a), Op::Mul, Operand::Matrix(b)) => settle_matrix(a.compose(b)),
		(lhs, Op::Div, Operand::Matrix(b)) => matrix_binary_op(lhs, Op::Mul, Operand::Matrix(b.inverse().ok_or(EvalError::SingularMatrix)?)),
		(Operand::Matrix(a), Op::Add, Operand::Matrix(b)) => settle_matrix(a + b),
		(Operand::Matrix(a), Op::Sub, Operand::Matrix(b)) => settle_matrix(a - b),
		(Operand::Matrix(a), Op::Add, Operand::Value(Value::Number(t))) | (Operand::Value(Value::Number(t)), Op::Add, Operand::Matrix(a)) => settle_matrix(a.translated(t.to_quaternion())),
		(Operand::Matrix(a), Op::Sub, Operand::Value(Value::Number(t))) => settle_matrix(a.translated(-t.to_quaternion())),
		(Operand::Value(Value::Number(t)), Op::Sub, Operand::Matrix(a)) => settle_matrix((-a).translated(t.to_quaternion())),
		(Operand::Matrix(a), Op::Pow, Operand::Value(Value::Number(exponent))) => {
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

	fn eval_in<V: ValueProvider, F: FunctionProvider>(&self, scope: &Scope<'_, V, F>) -> Result<Object, EvalError> {
		match self {
			Node::Value(value) => value.eval_in(scope).map(Object::Value),
			Node::Matrix(matrix) => matrix.eval_in(scope).map(Object::from),
		}
	}
}

impl ValueNode {
	pub fn eval<V: ValueProvider, F: FunctionProvider>(&self, context: &EvalContext<V, F>) -> Result<Value, EvalError> {
		self.eval_in(&Scope::root(context))
	}

	fn eval_in<V: ValueProvider, F: FunctionProvider>(&self, scope: &Scope<'_, V, F>) -> Result<Value, EvalError> {
		match self {
			ValueNode::Lit(lit) => match lit {
				Literal::Integer(integer) => Ok(Value::from_i64(*integer)),
				Literal::Float(float) => Ok(Value::from_f64(*float)),
			},

			ValueNode::BinOp { lhs, op, rhs } => match (lhs.eval_in(scope)?, rhs.eval_in(scope)?) {
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
			ValueNode::UnaryOp { expr, op } => match expr.eval_in(scope)? {
				Value::Number(num) => {
					let rejected = if *op == UnaryOp::Not { EvalError::NotATruthValue } else { EvalError::OperatorTypeError };
					settle(Value::Number(num.unary_op(*op).ok_or(rejected)?))
				}
			},
			ValueNode::Comparison { first, rest } => {
				let Value::Number(first) = first.eval_in(scope)?;
				let rest = rest
					.iter()
					.map(|(op, operand)| operand.eval_in(scope).map(|Value::Number(number)| (*op, number)))
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
				let value = resolve_value(scope.context, name).ok_or_else(|| EvalError::MissingValue(name.clone()))?;
				canonical_host_value(name, value)
			}
			ValueNode::FnCall { name, expr } => {
				// Arguments land in a stack buffer when they fit (builtins take at most 5), avoiding a heap allocation per call
				let mut stack_values = [Value::from_i64(0); 5];
				let heap_values: Vec<Value>;
				let values: &[Value] = if expr.len() <= stack_values.len() {
					for (slot, argument) in stack_values.iter_mut().zip(expr) {
						*slot = argument.eval_in(scope)?;
					}
					&stack_values[..expr.len()]
				} else {
					heap_values = expr.iter().map(|argument| argument.eval_in(scope)).collect::<Result<Vec<Value>, EvalError>>()?;
					&heap_values
				};

				// A host-supplied function shadows the builtin of the same name, unless the `\` prefix asks for the language's own
				let (prefixed, bare_name) = match name.strip_prefix('\\') {
					Some(bare_name) => (true, bare_name),
					None => (false, name.as_str()),
				};

				if !prefixed && let Some(value) = scope.context.run_function(bare_name, values) {
					settle(canonical_host_value(bare_name, value)?)
				} else if let Some(Builtin::Values { function, .. }) = builtin_function(bare_name) {
					settle(function(values).ok_or(EvalError::TypeError)?)
				} else if let Some((function, base)) = suffixed_function(bare_name) {
					// A base-suffixed call like `log10(x)` runs the two-argument form with the suffix baked in as its second argument
					let [value] = values else { return Err(EvalError::TypeError) };
					settle(function(&[*value, Value::from_f64(base)]).ok_or(EvalError::TypeError)?)
				} else if let Some(value) = resolve_value(scope.context, name)
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
			ValueNode::Piecewise { cases, otherwise } => match (holding_case(scope, cases)?, otherwise) {
				(Some(value), _) => value.eval_in(scope),
				(None, Some(otherwise)) => otherwise.eval_in(scope),
				(None, None) => Err(EvalError::NoCaseHolds),
			},
			ValueNode::Apply { matrix, value } => value_of_matrix(scope, MatrixValueCase::Apply(matrix, value)),
			ValueNode::OfMatrix { function, matrix } => value_of_matrix(scope, MatrixValueCase::OfMatrix(*function, matrix)),
			ValueNode::OfValueAndRegions { function, value, regions, trailing } => value_of_matrix(scope, MatrixValueCase::OfValueAndRegions(*function, value, regions, trailing.as_deref())),
			ValueNode::MatrixComparison { matrices, distinct } => value_of_matrix(scope, MatrixValueCase::Comparison(matrices, *distinct)),
			ValueNode::Local(local) => bound_value(scope, Bound::Local(*local)),
			ValueNode::Call { function, arguments } => bound_value(scope, Bound::Call(*function, arguments)),
			ValueNode::Where { clause, body } => bound_value(scope, Bound::Where(clause, body)),
		}
	}
}

/// A value computed from matrices.
enum MatrixValueCase<'a> {
	Apply(&'a MatrixNode, &'a ValueNode),
	OfMatrix(MatrixToValue, &'a MatrixNode),
	OfValueAndRegions(ValueOfRegions, &'a ValueNode, &'a [MatrixNode], Option<&'a ValueNode>),
	Comparison(&'a [MatrixNode], bool),
}

// One function for every value taken from a matrix, called from several sites, since wasm-opt inlines a function with one call site back into the evaluator whatever its attributes say
#[cold]
#[inline(never)]
fn value_of_matrix<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, case: MatrixValueCase) -> Result<Value, EvalError> {
	match case {
		MatrixValueCase::Apply(matrix, value) => {
			let matrix = matrix.eval_in(scope)?;
			let Value::Number(value) = value.eval_in(scope)?;
			settle(Value::from(matrix.apply(value.to_quaternion())))
		}
		MatrixValueCase::OfMatrix(function, matrix) => settle(function(matrix.eval_in(scope)?)),
		MatrixValueCase::OfValueAndRegions(function, value, regions, trailing) => {
			let value = value.eval_in(scope)?;

			// A range literal is kept by its corners, which may be infinite where no matrix can hold them
			let region_of = |region: &MatrixNode| -> Result<Region, EvalError> {
				match region {
					MatrixNode::Range { from, to } => {
						let (Value::Number(from), Value::Number(to)) = (from.eval_in(scope)?, to.eval_in(scope)?);
						Ok(Region::Range(from, to))
					}
					region => region.eval_in(scope).map(Region::Map),
				}
			};

			// The range functions take at most two regions as parsed, which land in a stack buffer, while a longer list a host built goes to the heap
			let mut stack_regions = [Region::Range(Number::Integer(0), Number::Integer(0)); 2];
			let heap_regions: Vec<Region>;
			let regions: &[Region] = if regions.len() <= stack_regions.len() {
				for (slot, region) in stack_regions.iter_mut().zip(regions) {
					*slot = region_of(region)?;
				}
				&stack_regions[..regions.len()]
			} else {
				heap_regions = regions.iter().map(region_of).collect::<Result<Vec<Region>, EvalError>>()?;
				&heap_regions
			};

			let trailing = trailing.map(|trailing| trailing.eval_in(scope)).transpose()?;
			settle(function(value, regions, trailing)?)
		}
		MatrixValueCase::Comparison(matrices, distinct) => {
			// A chain lands in a stack buffer when it fits, the usual case
			let mut stack_matrices = [Matrix::ZERO; 4];
			let heap_matrices: Vec<Matrix>;
			let matrices: &[Matrix] = if matrices.len() <= stack_matrices.len() {
				for (slot, matrix) in stack_matrices.iter_mut().zip(matrices) {
					*slot = matrix.eval_in(scope)?;
				}
				&stack_matrices[..matrices.len()]
			} else {
				heap_matrices = matrices.iter().map(|matrix| matrix.eval_in(scope)).collect::<Result<Vec<Matrix>, EvalError>>()?;
				&heap_matrices
			};

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
		self.eval_in(&Scope::root(context))
	}

	fn eval_in<V: ValueProvider, F: FunctionProvider>(&self, scope: &Scope<'_, V, F>) -> Result<Matrix, EvalError> {
		match self {
			MatrixNode::Var(name) => {
				let matrix = resolve_matrix(scope.context, name).ok_or_else(|| EvalError::MissingValue(name.clone()))?;
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
					let Value::Number(number) = entry.eval_in(scope)?;
					*slot = number.to_quaternion();
				}

				let entries = &quaternions[..entries.len()];
				let matrix = if *by_rows { Matrix::from_rows(entries) } else { Matrix::from_columns(entries) };
				settle_matrix(matrix.ok_or(EvalError::TypeError)?)
			}
			MatrixNode::FromValues { function, arguments } => {
				// The builders take at most three arguments as parsed, which land in a stack buffer, while a longer list a host built goes to the heap
				let mut stack_values = [Value::from_i64(0); 3];
				let heap_values: Vec<Value>;
				let values: &[Value] = if arguments.len() <= stack_values.len() {
					for (slot, argument) in stack_values.iter_mut().zip(arguments) {
						*slot = argument.eval_in(scope)?;
					}
					&stack_values[..arguments.len()]
				} else {
					heap_values = arguments.iter().map(|argument| argument.eval_in(scope)).collect::<Result<Vec<Value>, EvalError>>()?;
					&heap_values
				};
				settle_matrix(function(values).ok_or(EvalError::TypeError)?)
			}
			MatrixNode::Range { from, to } => {
				let (Value::Number(from), Value::Number(to)) = (from.eval_in(scope)?, to.eval_in(scope)?);
				settle_matrix(Matrix::range(from.to_quaternion(), to.to_quaternion()))
			}
			MatrixNode::OfMatrix { function, matrix } => settle_matrix(function(matrix.eval_in(scope)?)),
			MatrixNode::BinOp { lhs, op, rhs } => matrix_binary_op(lhs.operand(scope)?, *op, rhs.operand(scope)?),
			MatrixNode::UnaryOp { expr, op } => {
				let matrix = expr.eval_in(scope)?;
				match op {
					UnaryOp::Pos => Ok(matrix),
					UnaryOp::Neg => settle_matrix(-matrix),
					UnaryOp::Transpose if matrix.is_linear() => settle_matrix(matrix.transposed()),
					UnaryOp::Transpose => Err(EvalError::AffineTranspose),
					UnaryOp::Not | UnaryOp::Fac | UnaryOp::Magnitude => Err(EvalError::OperatorTypeError),
				}
			}
			MatrixNode::Piecewise { cases, otherwise } => match (holding_case(scope, cases)?, otherwise) {
				(Some(matrix), _) => matrix.eval_in(scope),
				(None, Some(otherwise)) => otherwise.eval_in(scope),
				(None, None) => Err(EvalError::NoCaseHolds),
			},
			MatrixNode::Local(local) => bound_matrix(scope, Bound::Local(*local)),
			MatrixNode::Call { function, arguments } => bound_matrix(scope, Bound::Call(*function, arguments)),
			MatrixNode::Where { clause, body } => bound_matrix(scope, Bound::Where(clause, body)),
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
