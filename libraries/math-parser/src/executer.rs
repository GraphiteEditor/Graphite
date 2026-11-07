use crate::ast::{BinaryOp, BuiltinCall, Clause, Literal, Local, MatrixNode, Measure, Node, SortedCase, UnaryOp, ValueNode};
use crate::constants::{Builtin, MatrixToValue, SuffixedFunction, ValueOfRegions, builtin_function, suffixed_function};
use crate::context::{EvalContext, FunctionProvider, ValueProvider};
use crate::lexer::Constant;
use crate::matrix::{Matrix, Region};
use crate::object::Object;
use crate::quantity::{self, MatrixQuantity, MatrixValueUnit, Quantity, RegionResult, UnitRule, unit_rule};
use crate::quaternion::Quaternion;
use crate::units::{Unit, unit_named};
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

	#[error("A matrix power must be an integer")]
	FractionalMatrixPower,

	#[error("A singular range has no interior")]
	SingularRange,

	#[error("Remapping from a flat range is ambiguous, since a value lies at every position along its flat side")]
	FlatRemapSource,

	#[error("A smoothstep across a flat range has no width to ease over")]
	FlatSmoothstep,

	#[error("A smoothstep's continuity is an integer from 0 to 3")]
	SmoothstepContinuity,

	#[error("Only a matrix without translation has a transpose")]
	AffineTranspose,

	#[error("Value of {0} is not a number")]
	NotANumber(String),

	#[error(transparent)]
	Units(Box<UnitMismatch>),

	#[error("An exponent in an angle unit rotates, so it can't also have a plain part")]
	AngleExponentWithPlainPart,

	#[error("The units are too far apart in size to combine")]
	UnitOverflow,
}

/// Why units don't fit an operation, boxed within [`EvalError`] so the error every evaluation step returns stays small.
#[derive(Debug, Error)]
pub enum UnitMismatch {
	#[error("A value in `{left}` can't be {verb} one in `{right}`, since they measure different things")]
	IncompatibleUnits { verb: &'static str, left: String, right: String },

	#[error("A plain number can't be {verb} a value in `{unit}`, so give it a unit too, like `1 {unit}`")]
	PlainWithUnit { verb: &'static str, unit: String },

	#[error("`{function}` takes plain numbers, not a value in `{unit}`")]
	UnitsNotAccepted { function: String, unit: String },

	#[error("`{function}` takes a plain number, since its result would depend on the unit, so round a value in `{unit}` with `snap(x, 1 {unit})`")]
	RoundingWithUnits { function: String, unit: String },

	#[error("`{function}` takes an angle or a plain number, not a value in `{unit}`")]
	AngleExpected { function: String, unit: String },

	#[error("The power leaves `{0}` with a fractional exponent")]
	FractionalUnitPower(String),

	#[error("An exponent can't be in `{0}`")]
	ExponentWithUnits(String),
}

impl From<UnitMismatch> for EvalError {
	fn from(mismatch: UnitMismatch) -> Self {
		Self::Units(Box::new(mismatch))
	}
}

/// Settles an operation's result: no operation may produce NaN, so an indeterminate form is an error, and the value takes
/// its canonical form so that a zero imaginary part or a signed zero never changes a later result.
// Inlined with the canonical form it calls, since these run after every operation where a call would cost as much as the work
#[inline(always)]
fn settle(quantity: Quantity) -> Result<Quantity, EvalError> {
	let number = quantity.number();
	if number.is_nan() {
		return Err(EvalError::Indeterminate);
	}
	Ok(Quantity::new(Value::Number(number.canonical()), quantity.unit))
}

/// Settles a matrix result like [`settle`] does a value, with signed zeros made plain zero.
pub(crate) fn settle_matrix(matrix: Matrix) -> Result<Matrix, EvalError> {
	if matrix.is_nan() {
		return Err(EvalError::Indeterminate);
	}
	Ok(matrix.map(|entry| if entry == 0. { 0. } else { entry }))
}

fn settle_matrix_quantity(matrix: MatrixQuantity) -> Result<MatrixQuantity, EvalError> {
	Ok(MatrixQuantity {
		matrix: settle_matrix(matrix.matrix)?,
		..matrix
	})
}

/// The binary operation on two evaluated values, settled.
#[inline(always)]
fn binary(lhs: Quantity, op: BinaryOp, rhs: Quantity, pixels_per_inch: f64) -> Result<Quantity, EvalError> {
	if !lhs.is_plain() || !rhs.is_plain() {
		return settle(quantity::unit_binary_op(lhs, op, rhs, pixels_per_inch)?);
	}

	// Plain numbers, the common case, take the arithmetic directly
	let rejected = if matches!(op, BinaryOp::And | BinaryOp::Or) {
		EvalError::NotATruthValue
	} else {
		EvalError::OperatorTypeError
	};
	let number = lhs.number().binary_op(op, rhs.number()).ok_or(rejected)?;
	settle(Quantity::of_number(number, Unit::NONE))
}

/// A quotient by a measure, which is the whole divisor where the dividend has a unit, and otherwise divides by the measure's
/// number before its units multiply in. Kept out of the evaluator's frame, which nesting multiplies.
#[inline(never)]
fn measure_quotient<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, dividend: &ValueNode, measure: &Measure) -> Result<Quantity, EvalError> {
	let pixels_per_inch = scope.pixels_per_inch();
	let dividend = dividend.eval_in(scope)?;
	let number = measure.number.eval_in(scope)?;
	let units = measure
		.units
		.iter()
		.try_fold(Quantity::plain(Value::from_i64(1)), |units, unit| binary(units, BinaryOp::Mul, unit.eval_in(scope)?, pixels_per_inch))?;

	// A name the host or a `where` definition binds is no unit, so the measure then reads left to right as written
	fn is_unit<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, node: &ValueNode) -> bool {
		match node {
			ValueNode::Unit { name, .. } => name.starts_with('\\') || scope.context.get_quantity(name).is_none(),
			ValueNode::BinOp { lhs, op: BinaryOp::Pow, .. } => is_unit(scope, lhs),
			_ => false,
		}
	}
	if measure.units.iter().all(|unit| is_unit(scope, unit)) && !dividend.unit.dimension().is_none() {
		binary(dividend, BinaryOp::Div, binary(number, BinaryOp::Mul, units, pixels_per_inch)?, pixels_per_inch)
	} else {
		binary(binary(dividend, BinaryOp::Div, number, pixels_per_inch)?, BinaryOp::Mul, units, pixels_per_inch)
	}
}

/// The canonical form of a value the host supplied for `name`, like [`settle`], except that NaN (which only a host can
/// supply) is an error naming its source.
#[inline(always)]
fn canonical_host_value(name: &str, quantity: Quantity) -> Result<Quantity, EvalError> {
	let number = quantity.number();
	if number.is_nan() {
		return Err(EvalError::NotANumber(name.to_string()));
	}
	Ok(Quantity::new(Value::Number(number.canonical()), quantity.unit))
}

/// Resolves a name against the environment before the builtin constants and units, so a binding of exactly that spelling shadows
/// the builtin. The `\` prefix skips the environment.
fn resolve_value<V: ValueProvider, F: FunctionProvider>(context: &EvalContext<V, F>, name: &str) -> Option<Quantity> {
	let builtin = |name: &str| {
		Constant::from_name(name)
			.map(|constant| Quantity::plain(Value::Number(constant.value())))
			.or_else(|| unit_named(name).map(|unit| Quantity::new(Value::from_i64(1), Unit::of(unit))))
	};

	match name.strip_prefix('\\') {
		Some(builtin_name) => builtin(builtin_name),
		None => context.get_quantity(name).or_else(|| builtin(name)),
	}
}

/// Resolves a matrix's name like [`resolve_value`], with `I` the one builtin.
fn resolve_matrix<V: ValueProvider, F: FunctionProvider>(context: &EvalContext<V, F>, name: &str) -> Option<MatrixQuantity> {
	let constant = |name: &str| (name == "I").then_some(MatrixQuantity::plain(Matrix::IDENTITY));

	match name.strip_prefix('\\') {
		Some(builtin_name) => constant(builtin_name),
		None => context.get_matrix_quantity(name).or_else(|| constant(name)),
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

	fn pixels_per_inch(&self) -> f64 {
		self.context.pixels_per_inch()
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
fn bound_value<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, bound: Bound<ValueNode>) -> Result<Quantity, EvalError> {
	match bound {
		Bound::Local(local) => read(scope, local)?.as_quantity().copied().ok_or(EvalError::OperatorTypeError),
		Bound::Call(function, arguments) => call(scope, function, arguments, |body, scope| match body {
			Node::Value(body) => body.eval_in(scope),
			Node::Matrix(_) => Err(EvalError::OperatorTypeError),
		}),
		Bound::Where(clause, body) => enter(scope, scope.frame, Definitions::Clause(clause), clause.values.len(), |scope| body.eval_in(scope)),
	}
}

#[inline(never)]
fn bound_matrix<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, bound: Bound<MatrixNode>) -> Result<MatrixQuantity, EvalError> {
	match bound {
		Bound::Local(local) => read(scope, local)?.as_matrix_quantity().copied().ok_or(EvalError::OperatorTypeError),
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
		let condition = case.condition.eval_in(scope)?;
		if !condition.is_plain() {
			return Err(EvalError::NotATruthValue);
		}
		match condition.number().as_bool() {
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
	Value(Quantity),
	Matrix(MatrixQuantity),
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
fn matrix_binary_op(lhs: Operand, op: BinaryOp, rhs: Operand, pixels_per_inch: f64) -> Result<MatrixQuantity, EvalError> {
	use BinaryOp as Op;
	let result = match (lhs, op, rhs) {
		(Operand::Value(q), Op::Mul, Operand::Matrix(b)) => {
			let left_multiplication = MatrixQuantity {
				matrix: Matrix::left_multiplication(q.number().to_quaternion()),
				linear: q.unit,
				translation: Unit::NONE,
			};
			left_multiplication.compose(&b, pixels_per_inch)?
		}
		(Operand::Matrix(a), Op::Mul, Operand::Matrix(b)) => a.compose(&b, pixels_per_inch)?,
		(lhs, Op::Div, Operand::Matrix(b)) => return matrix_binary_op(lhs, Op::Mul, Operand::Matrix(b.inverse(pixels_per_inch)?), pixels_per_inch),
		(Operand::Matrix(a), Op::Add | Op::Sub, Operand::Matrix(b)) => a.sum(op, &b, pixels_per_inch)?,
		(Operand::Matrix(a), Op::Add | Op::Sub, Operand::Value(t)) => a.translated(op, t, pixels_per_inch)?,
		(Operand::Value(t), Op::Add, Operand::Matrix(a)) => a.translated(Op::Add, t, pixels_per_inch)?,
		(Operand::Value(t), Op::Sub, Operand::Matrix(a)) => a.negated().translated(Op::Add, t, pixels_per_inch)?,
		(Operand::Matrix(a), Op::Pow, Operand::Value(exponent)) => {
			if !exponent.is_plain() {
				return Err(EvalError::from(UnitMismatch::ExponentWithUnits(exponent.unit.to_string())));
			}
			// A whole exponent is a composition power, a negative one of the inverse, with an integer read exactly past 2^53
			let whole = match exponent.number() {
				Number::Integer(integer) => integer,
				real => real
					.as_real()
					.filter(|real| real.fract() == 0. && real.abs() < i64::MAX as f64)
					.ok_or(EvalError::FractionalMatrixPower)? as i64,
			};
			a.power(whole, pixels_per_inch)?
		}
		_ => return Err(EvalError::OperatorTypeError),
	};
	settle_matrix_quantity(result)
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
	pub fn eval<V: ValueProvider, F: FunctionProvider>(&self, context: &EvalContext<V, F>) -> Result<Quantity, EvalError> {
		self.eval_in(&Scope::root(context))
	}

	fn eval_in<V: ValueProvider, F: FunctionProvider>(&self, scope: &Scope<'_, V, F>) -> Result<Quantity, EvalError> {
		match self {
			ValueNode::Lit(lit) => match lit {
				Literal::Integer(integer) => Ok(Quantity::plain(Value::from_i64(*integer))),
				Literal::Float(float) => Ok(Quantity::plain(Value::from_f64(*float))),
			},

			ValueNode::BinOp { lhs, op, rhs } => binary(lhs.eval_in(scope)?, *op, rhs.eval_in(scope)?, scope.pixels_per_inch()),
			ValueNode::MeasureQuotient { dividend, measure } => measure_quotient(scope, dividend, measure),
			ValueNode::UnaryOp { expr, op } => {
				let operand = expr.eval_in(scope)?;
				if !operand.is_plain() {
					return settle(quantity::unary_op(operand, *op)?);
				}

				let rejected = if *op == UnaryOp::Not { EvalError::NotATruthValue } else { EvalError::OperatorTypeError };
				settle(Quantity::of_number(operand.number().unary_op(*op).ok_or(rejected)?, Unit::NONE))
			}
			ValueNode::Comparison { first, rest } => comparison_chain(scope, first, rest),
			ValueNode::Var(name) => {
				let value = resolve_value(scope.context, name).ok_or_else(|| EvalError::MissingValue(name.clone()))?;
				canonical_host_value(name, value)
			}
			ValueNode::Unit { name, unit } => {
				// A host's binding of the spelling shadows the unit, unless the `\` prefix asks for the unit
				let bound = (!name.starts_with('\\')).then(|| scope.context.get_quantity(name)).flatten();
				match bound {
					Some(value) => canonical_host_value(name, value),
					None => Ok(Quantity::new(Value::from_i64(1), Unit::of(*unit))),
				}
			}
			ValueNode::FnCall { name, expr } => function_call(scope, name, expr),
			ValueNode::BuiltinCall(call) => builtin_call(scope, call),
			// Only the chosen value is evaluated, so an error in any other case's value is never raised
			ValueNode::Piecewise { cases, otherwise } => match (holding_case(scope, cases)?, otherwise) {
				(Some(value), _) => value.eval_in(scope),
				(None, Some(otherwise)) => otherwise.eval_in(scope),
				(None, None) => Err(EvalError::NoCaseHolds),
			},
			ValueNode::Apply { matrix, value } => value_of_matrix(scope, MatrixValueCase::Apply(matrix, value)),
			ValueNode::OfMatrix { function, unit, matrix } => value_of_matrix(scope, MatrixValueCase::OfMatrix(*function, *unit, matrix)),
			ValueNode::OfValueAndRegions {
				function,
				result,
				value,
				regions,
				trailing,
			} => value_of_matrix(scope, MatrixValueCase::OfValueAndRegions(*function, *result, value, regions, trailing.as_deref())),
			ValueNode::MatrixComparison { matrices, distinct } => value_of_matrix(scope, MatrixValueCase::Comparison(matrices, *distinct)),
			ValueNode::Local(local) => bound_value(scope, Bound::Local(*local)),
			ValueNode::Call { function, arguments } => bound_value(scope, Bound::Call(*function, arguments)),
			ValueNode::Where { clause, body } => bound_value(scope, Bound::Where(clause, body)),
		}
	}
}

/// A chain of comparisons, outlined to keep its buffers out of the recursive evaluator's frame.
#[inline(never)]
fn comparison_chain<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, first: &ValueNode, rest: &[(BinaryOp, ValueNode)]) -> Result<Quantity, EvalError> {
	let first = first.eval_in(scope)?;
	let rest = rest
		.iter()
		.map(|(op, operand)| operand.eval_in(scope).map(|quantity| (*op, quantity)))
		.collect::<Result<Vec<(BinaryOp, Quantity)>, EvalError>>()?;
	let (first, rest) = quantity::comparison_operands(first, &rest, scope.pixels_per_inch())?;

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
	Ok(Quantity::plain(Value::from_bool(holds)))
}

/// A call of a builtin found as the expression was read, kept apart from the recursive evaluator so its argument buffers stay out of the evaluator's frame in unoptimized builds.
fn builtin_call<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, call: &BuiltinCall) -> Result<Quantity, EvalError> {
	if call.suffix.is_some() && call.arguments.len() != 1 {
		return Err(EvalError::TypeError);
	}

	// Arguments land in stack buffers when they fit (builtins take at most 5), each value beside its unit
	let count = call.arguments.len() + usize::from(call.suffix.is_some());
	let mut stack_values = [Value::from_i64(0); 5];
	let mut stack_units = [Unit::NONE; 5];
	let (mut heap_values, mut heap_units): (Vec<Value>, Vec<Unit>);
	let (values, units): (&mut [Value], &mut [Unit]) = if count <= stack_values.len() {
		(&mut stack_values[..count], &mut stack_units[..count])
	} else {
		heap_values = vec![Value::from_i64(0); count];
		heap_units = vec![Unit::NONE; count];
		(&mut heap_values, &mut heap_units)
	};

	for ((value, unit), argument) in values.iter_mut().zip(units.iter_mut()).zip(&call.arguments) {
		let Quantity {
			value: argument_value,
			unit: argument_unit,
		} = argument.eval_in(scope)?;
		*value = argument_value;
		*unit = argument_unit;
	}

	// A host may supply a function the expression wasn't read with, which shadows the builtin as it would have then
	if let Some(host_name) = &call.host_name
		&& scope.context.provides_function(host_name)
	{
		let arguments: Vec<Quantity> = values.iter().zip(units.iter()).take(call.arguments.len()).map(|(value, unit)| Quantity::new(*value, *unit)).collect();
		if let Some(value) = host_function(scope, host_name, &arguments)? {
			return settle(canonical_host_value(host_name, Quantity::plain(value))?);
		}
	}

	if let Some(suffix) = call.suffix {
		values[count - 1] = Value::from_f64(suffix);
	}

	settle(quantity::run_builtin_on_parts(&call.name, call.rule, call.function, values, units, scope.pixels_per_inch())?)
}

/// A call of a host function or a builtin, outlined to keep its argument buffer out of the recursive evaluator's frame.
#[inline(never)]
fn function_call<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, name: &str, expr: &[ValueNode]) -> Result<Quantity, EvalError> {
	// Arguments land in a stack buffer when they fit (builtins take at most 5), avoiding a heap allocation per call
	let mut stack_values = [Quantity::plain(Value::from_i64(0)); 5];
	let heap_values: Vec<Quantity>;
	let arguments: &[Quantity] = if expr.len() <= stack_values.len() {
		for (slot, argument) in stack_values.iter_mut().zip(expr) {
			*slot = argument.eval_in(scope)?;
		}
		&stack_values[..expr.len()]
	} else {
		heap_values = expr.iter().map(|argument| argument.eval_in(scope)).collect::<Result<Vec<Quantity>, EvalError>>()?;
		&heap_values
	};

	// A host-supplied function shadows the builtin of the same name, unless the `\` prefix asks for the language's own
	let (prefixed, bare_name) = match name.strip_prefix('\\') {
		Some(bare_name) => (true, bare_name),
		None => (false, name),
	};

	if !prefixed && let Some(value) = host_function(scope, bare_name, arguments)? {
		settle(canonical_host_value(bare_name, Quantity::plain(value))?)
	} else if let Some(Builtin::Values { function, .. }) = builtin_function(bare_name) {
		let rule = unit_rule(bare_name).unwrap_or(UnitRule::Plain);
		settle(quantity::run_builtin(bare_name, rule, function, arguments, scope.pixels_per_inch())?)
	} else if let Some(SuffixedFunction { name: base_name, function, argument }) = suffixed_function(bare_name) {
		// A base-suffixed call like `log10(x)` runs the two-argument form with the suffix baked in as its second argument
		let [value] = arguments else { return Err(EvalError::TypeError) };
		let rule = unit_rule(base_name).unwrap_or(UnitRule::Plain);
		let arguments = [*value, Quantity::plain(Value::from_f64(argument))];
		settle(quantity::run_builtin(base_name, rule, function, &arguments, scope.pixels_per_inch())?)
	} else if let Some(value) = resolve_value(scope.context, name)
		&& let [argument] = arguments
	{
		// A known value applied to one argument is implicit multiplication, so `x(2)` matches `2(3)` and `i(16)`
		let value = canonical_host_value(name, value)?;
		settle(quantity::binary_op(value, BinaryOp::Mul, *argument, scope.pixels_per_inch())?)
	} else {
		Err(EvalError::MissingFunction(name.to_string()))
	}
}

/// Runs the host's function of the name, which takes plain numbers, or gives `None` where the host has none.
fn host_function<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, name: &str, arguments: &[Quantity]) -> Result<Option<Value>, EvalError> {
	if !scope.context.provides_function(name) {
		return Ok(None);
	}
	let arguments: Vec<Quantity> = arguments.iter().map(|argument| argument.plain_if_dimensionless()).collect();
	if let Some(argument) = arguments.iter().find(|argument| !argument.is_plain()) {
		return Err(EvalError::from(UnitMismatch::UnitsNotAccepted {
			function: name.to_string(),
			unit: argument.unit.to_string(),
		}));
	}

	let values: Vec<Value> = arguments.iter().map(|argument| argument.value).collect();
	Ok(scope.context.run_function(name, &values))
}

/// A value computed from matrices.
enum MatrixValueCase<'a> {
	Apply(&'a MatrixNode, &'a ValueNode),
	OfMatrix(MatrixToValue, MatrixValueUnit, &'a MatrixNode),
	OfValueAndRegions(ValueOfRegions, RegionResult, &'a ValueNode, &'a [MatrixNode], Option<&'a ValueNode>),
	Comparison(&'a [MatrixNode], bool),
}

/// A region with the unit of the values it holds.
#[derive(Clone, Copy)]
struct UnitRegion {
	region: Region,
	unit: Unit,
}

// One function for every value taken from a matrix, called from several sites, since wasm-opt inlines a function with one call site back into the evaluator whatever its attributes say
#[cold]
#[inline(never)]
fn value_of_matrix<V: ValueProvider, F: FunctionProvider>(scope: &Scope<'_, V, F>, case: MatrixValueCase) -> Result<Quantity, EvalError> {
	let pixels_per_inch = scope.pixels_per_inch();
	match case {
		MatrixValueCase::Apply(matrix, value) => {
			let matrix = matrix.eval_in(scope)?;
			let value = value.eval_in(scope)?;
			settle(matrix.apply(value, pixels_per_inch)?)
		}
		MatrixValueCase::OfMatrix(function, unit, matrix) => {
			let matrix = matrix.eval_in(scope)?;
			let unit = quantity::matrix_value_result_unit(unit, &matrix)?;
			settle(Quantity::new(function(matrix.matrix), unit))
		}
		MatrixValueCase::OfValueAndRegions(function, result_unit, value, regions, trailing) => {
			let value = value.eval_in(scope)?;

			// A range literal is kept by its corners, which may be infinite where no matrix can hold them
			let region_of = |region: &MatrixNode| -> Result<UnitRegion, EvalError> {
				match region {
					MatrixNode::Range { from, to } => {
						let (from, to, unit) = quantity::range_corners(from.eval_in(scope)?, to.eval_in(scope)?, pixels_per_inch)?;
						Ok(UnitRegion {
							region: Region::Range(from, to),
							unit,
						})
					}
					region => {
						let matrix = region.eval_in(scope)?;
						let unit = matrix.region_unit();
						Ok(UnitRegion {
							region: Region::Map(matrix.as_region_in(unit, pixels_per_inch)?),
							unit,
						})
					}
				}
			};

			// The range functions take at most two regions as parsed, which land in a stack buffer, while a longer list a host built goes to the heap
			let placeholder = UnitRegion {
				region: Region::Range(Number::Integer(0), Number::Integer(0)),
				unit: Unit::NONE,
			};
			let mut stack_regions = [placeholder; 2];
			let heap_regions: Vec<UnitRegion>;
			let unit_regions: &[UnitRegion] = if regions.len() <= stack_regions.len() {
				for (slot, region) in stack_regions.iter_mut().zip(regions) {
					*slot = region_of(region)?;
				}
				&stack_regions[..regions.len()]
			} else {
				heap_regions = regions.iter().map(region_of).collect::<Result<Vec<UnitRegion>, EvalError>>()?;
				&heap_regions
			};

			// The value is measured in the first region's unit, and a remap's result in the last's
			let Some(first) = unit_regions.first() else { return Err(EvalError::TypeError) };
			let value = Quantity::of_number(quantity::into_region_unit(value, first.unit, pixels_per_inch)?, Unit::NONE);
			let mut stack_plain_regions = [placeholder.region; 2];
			let heap_plain_regions: Vec<Region>;
			let plain_regions: &[Region] = if unit_regions.len() <= stack_plain_regions.len() {
				for (slot, unit_region) in stack_plain_regions.iter_mut().zip(unit_regions) {
					*slot = unit_region.region;
				}
				&stack_plain_regions[..unit_regions.len()]
			} else {
				heap_plain_regions = unit_regions.iter().map(|unit_region| unit_region.region).collect();
				&heap_plain_regions
			};

			let trailing = match trailing.map(|trailing| trailing.eval_in(scope)).transpose()? {
				Some(trailing) if !trailing.is_plain() => return Err(EvalError::SmoothstepContinuity),
				trailing => trailing.map(|trailing| trailing.value),
			};
			let result = function(value.value, plain_regions, trailing)?;

			let unit = match result_unit {
				RegionResult::FirstRegion => first.unit,
				RegionResult::LastRegion => unit_regions.last().map_or(Unit::NONE, |region| region.unit),
				RegionResult::Plain => Unit::NONE,
			};
			settle(Quantity::new(result, unit))
		}
		MatrixValueCase::Comparison(matrices, distinct) => {
			// A chain lands in a stack buffer when it fits, the usual case
			let mut stack_matrices = [MatrixQuantity::plain(Matrix::ZERO); 4];
			let heap_matrices: Vec<MatrixQuantity>;
			let matrices: &[MatrixQuantity] = if matrices.len() <= stack_matrices.len() {
				for (slot, matrix) in stack_matrices.iter_mut().zip(matrices) {
					*slot = matrix.eval_in(scope)?;
				}
				&stack_matrices[..matrices.len()]
			} else {
				heap_matrices = matrices.iter().map(|matrix| matrix.eval_in(scope)).collect::<Result<Vec<MatrixQuantity>, EvalError>>()?;
				&heap_matrices
			};

			// Matrices compare in the first one's units, which every other must share in dimension
			let Some(first) = matrices.first() else { return Err(EvalError::TypeError) };
			let translation_unit = matrices.iter().map(|matrix| matrix.translation).find(|unit| !unit.is_none()).unwrap_or(Unit::NONE);
			let in_first_units = |matrix: &MatrixQuantity| -> Result<Matrix, EvalError> {
				if matrix.linear == first.linear && matrix.translation == translation_unit {
					return Ok(matrix.matrix);
				}
				matrix.in_units(first.linear, translation_unit, pixels_per_inch)
			};
			let plain = matrices.iter().map(in_first_units).collect::<Result<Vec<Matrix>, EvalError>>()?;

			let holds = if distinct {
				plain.iter().enumerate().all(|(index, a)| plain[index + 1..].iter().all(|b| !a.same_entries(*b)))
			} else {
				plain.windows(2).all(|pair| pair[0].same_entries(pair[1]))
			};
			Ok(Quantity::plain(Value::from_bool(holds)))
		}
	}
}

impl MatrixNode {
	pub fn eval<V: ValueProvider, F: FunctionProvider>(&self, context: &EvalContext<V, F>) -> Result<MatrixQuantity, EvalError> {
		self.eval_in(&Scope::root(context))
	}

	fn eval_in<V: ValueProvider, F: FunctionProvider>(&self, scope: &Scope<'_, V, F>) -> Result<MatrixQuantity, EvalError> {
		let pixels_per_inch = scope.pixels_per_inch();
		match self {
			MatrixNode::Var(name) => {
				let matrix = resolve_matrix(scope.context, name).ok_or_else(|| EvalError::MissingValue(name.clone()))?;
				if matrix.matrix.is_nan() {
					return Err(EvalError::NotANumber(name.clone()));
				}
				settle_matrix_quantity(matrix)
			}
			MatrixNode::Literal { entries, by_rows } => {
				if entries.len() > 4 {
					return Err(EvalError::TypeError);
				}
				let quantities = entries.iter().map(|entry| entry.eval_in(scope)).collect::<Result<Vec<Quantity>, EvalError>>()?;
				let (quaternions, unit) = quantity::literal_entries(&quantities, pixels_per_inch)?;

				let matrix = if *by_rows { Matrix::from_rows(&quaternions) } else { Matrix::from_columns(&quaternions) };
				settle_matrix_quantity(MatrixQuantity {
					matrix: matrix.ok_or(EvalError::TypeError)?,
					linear: unit,
					translation: Unit::NONE,
				})
			}
			MatrixNode::FromValues { function, units, arguments } => {
				// The builders take at most three arguments as parsed, which land in a stack buffer, while a longer list a host built goes to the heap
				let mut stack_values = [Quantity::plain(Value::from_i64(0)); 3];
				let heap_values: Vec<Quantity>;
				let quantities: &[Quantity] = if arguments.len() <= stack_values.len() {
					for (slot, argument) in stack_values.iter_mut().zip(arguments) {
						*slot = argument.eval_in(scope)?;
					}
					&stack_values[..arguments.len()]
				} else {
					heap_values = arguments.iter().map(|argument| argument.eval_in(scope)).collect::<Result<Vec<Quantity>, EvalError>>()?;
					&heap_values
				};

				let (values, linear) = quantity::builder_arguments(*units, quantities)?;
				settle_matrix_quantity(MatrixQuantity {
					matrix: function(&values).ok_or(EvalError::TypeError)?,
					linear,
					translation: Unit::NONE,
				})
			}
			MatrixNode::Range { from, to } => {
				let (from, to, unit) = quantity::range_corners(from.eval_in(scope)?, to.eval_in(scope)?, pixels_per_inch)?;
				let matrix = Matrix::range(from.to_quaternion(), to.to_quaternion());
				let translation = if matrix.translation == Quaternion::ZERO { Unit::NONE } else { unit };
				settle_matrix_quantity(MatrixQuantity { matrix, linear: unit, translation })
			}
			MatrixNode::OfMatrix { function, matrix } => {
				let matrix = matrix.eval_in(scope)?;
				let result = function(matrix.matrix);
				let translation = if result.translation == Quaternion::ZERO { Unit::NONE } else { matrix.translation };
				settle_matrix_quantity(MatrixQuantity {
					matrix: result,
					linear: matrix.linear,
					translation,
				})
			}
			MatrixNode::BinOp { lhs, op, rhs } => matrix_binary_op(lhs.operand(scope)?, *op, rhs.operand(scope)?, pixels_per_inch),
			MatrixNode::UnaryOp { expr, op } => {
				let matrix = expr.eval_in(scope)?;
				match op {
					UnaryOp::Pos => Ok(matrix),
					UnaryOp::Neg => settle_matrix_quantity(matrix.negated()),
					UnaryOp::Transpose if matrix.matrix.is_linear() => settle_matrix_quantity(MatrixQuantity {
						matrix: matrix.matrix.transposed(),
						..matrix
					}),
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
		assert_eq!(result.value, Value::from_f64(10.));
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
					assert_eq!(result.value, $expected);
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
