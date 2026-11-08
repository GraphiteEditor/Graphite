//! Values and matrices with the units they are measured in, and the arithmetic that carries units through every operation and
//! builtin, so the builtins themselves only ever see plain numbers.

use crate::ast::{BinaryOp, UnaryOp};
use crate::constants::BuiltinFunction;
use crate::executer::{EvalError, UnitMismatch};
use crate::matrix::Matrix;
use crate::quaternion::Quaternion;
use crate::units::{Dimension, Size, Unit, UnitError, UnitId, common_unit};
use crate::value::{Number, Value};
use std::fmt;

/// A value with the unit it is measured in, which is [`Unit::NONE`] for a plain number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
	pub value: Value,
	pub unit: Unit,
}

impl Quantity {
	#[inline(always)]
	pub const fn plain(value: Value) -> Self {
		Self { value, unit: Unit::NONE }
	}

	#[inline(always)]
	pub const fn new(value: Value, unit: Unit) -> Self {
		Self { value, unit }
	}

	#[inline(always)]
	pub(crate) fn of_number(number: Number, unit: Unit) -> Self {
		Self::new(Value::Number(number), unit)
	}

	#[inline(always)]
	pub fn number(self) -> Number {
		let Value::Number(number) = self.value;
		number
	}

	#[inline(always)]
	pub fn is_plain(self) -> bool {
		self.unit.is_none()
	}

	pub fn dimension(self) -> Dimension {
		self.unit.dimension()
	}

	/// Converts the value into `unit`, which must measure the same dimension, or is an error. A plain value converts only into no unit.
	pub fn convert(self, unit: Unit, pixels_per_inch: f64) -> Result<Value, UnitError> {
		let factor = self.unit.conversion(unit)?;
		factor.apply(self.number(), pixels_per_inch).map(Value::Number).ok_or(UnitError::Overflow)
	}

	/// Reads the value in `unit` as a host field reads what was typed into it: a plain value is taken as already in `unit`, and
	/// one with a unit converts from it.
	pub fn read_in(self, unit: Unit, pixels_per_inch: f64) -> Result<Value, UnitError> {
		if self.is_plain() {
			return Ok(self.value);
		}
		self.convert(unit, pixels_per_inch)
	}

	/// Reads the value as a plain number: a dimensionless unit like `%` converts out, and an angle reads in radians, or `None` for
	/// any other dimension.
	pub fn as_plain(self) -> Option<Value> {
		match self.dimension() {
			Dimension::NONE => self.convert(Unit::NONE, 0.).ok(),
			Dimension::ANGLE => self.convert(radians(), 0.).ok(),
			_ => None,
		}
	}

	/// The value with any dimensionless unit like `%` converted out, so it can stand where a plain number is needed.
	pub(crate) fn plain_if_dimensionless(self) -> Self {
		if self.unit.is_dimensionless() {
			return self.convert(Unit::NONE, 0.).map(Self::plain).unwrap_or(self);
		}
		self
	}

	fn is_zero(self) -> bool {
		self.number() == Number::Integer(0)
	}
}

impl fmt::Display for Quantity {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.unit.is_none() {
			return self.value.fmt(f);
		}

		// A unit with only a denominator reads after the number's own `/`, like `5/s`
		let unit = self.unit.to_string();
		let unit = unit.strip_prefix("1/").map(|denominator| format!("/{denominator}")).unwrap_or(unit);
		let separator = if self.unit.attaches_without_space() || unit.starts_with('/') { "" } else { " " };

		// A value with several parts is grouped, so the unit applies to the whole of it
		match self.number() {
			Number::Complex(_) | Number::Quaternion(_) => write!(f, "({}){separator}{unit}", self.value),
			_ => write!(f, "{}{separator}{unit}", self.value),
		}
	}
}

pub(crate) const fn radians() -> Unit {
	Unit::of(UnitId::RADIAN)
}

/// Converts a number by a unit conversion factor.
fn scaled(number: Number, size: Size, pixels_per_inch: f64) -> Result<Number, EvalError> {
	if size.is_one() {
		return Ok(number);
	}
	size.apply(number, pixels_per_inch).ok_or(EvalError::UnitOverflow)
}

fn unit_error(error: UnitError, left: Unit, right: Unit, verb: &'static str) -> EvalError {
	match error {
		UnitError::Overflow => EvalError::UnitOverflow,
		UnitError::FractionalPower => EvalError::from(UnitMismatch::FractionalUnitPower(left.to_string())),
		UnitError::Incompatible if left.is_none() || right.is_none() => EvalError::from(UnitMismatch::PlainWithUnit {
			verb,
			unit: if left.is_none() { right } else { left }.to_string(),
		}),
		UnitError::Incompatible => EvalError::from(UnitMismatch::IncompatibleUnits {
			verb,
			left: left.to_string(),
			right: right.to_string(),
		}),
	}
}

/// Two operands in one unit, for a sum or a comparison: the unit a sum of theirs is measured in, where a plain zero takes the
/// other's unit, as zero is the same in every unit, while any other plain number can't mix with a unit.
fn in_common_unit(lhs: Quantity, rhs: Quantity, verb: &'static str, pixels_per_inch: f64) -> Result<(Number, Number, Unit), EvalError> {
	if lhs.unit == rhs.unit {
		return Ok((lhs.number(), rhs.number(), lhs.unit));
	}
	if lhs.is_plain() && lhs.is_zero() {
		return Ok((lhs.number(), rhs.number(), rhs.unit));
	}
	if rhs.is_plain() && rhs.is_zero() {
		return Ok((lhs.number(), rhs.number(), lhs.unit));
	}
	if lhs.is_plain() || rhs.is_plain() {
		let unit = if lhs.is_plain() { rhs.unit } else { lhs.unit };
		return Err(EvalError::from(UnitMismatch::PlainWithUnit { verb, unit: unit.to_string() }));
	}
	if lhs.dimension() != rhs.dimension() {
		return Err(unit_error(UnitError::Incompatible, lhs.unit, rhs.unit, verb));
	}

	let unit = common_unit(lhs.unit, rhs.unit, pixels_per_inch);
	let convert = |operand: Quantity| -> Result<Number, EvalError> {
		let factor = operand.unit.conversion(unit).map_err(|error| unit_error(error, operand.unit, unit, verb))?;
		scaled(operand.number(), factor, pixels_per_inch)
	};
	Ok((convert(lhs)?, convert(rhs)?, unit))
}

/// Brings any count of operands into the unit a sum of them would be measured in, as `min`, `max`, and the statistics take them.
pub(crate) fn all_in_common_unit(operands: &[Quantity], verb: &'static str, pixels_per_inch: f64) -> Result<(Vec<Value>, Unit), EvalError> {
	if operands.iter().all(|operand| operand.is_plain()) {
		return Ok((operands.iter().map(|operand| operand.value).collect(), Unit::NONE));
	}

	// A plain zero takes whatever unit the others share
	let mut common: Option<Unit> = None;
	for operand in operands.iter().filter(|operand| !(operand.is_plain() && operand.is_zero())) {
		common = Some(match common {
			None => operand.unit,
			Some(unit) if unit.is_none() != operand.is_plain() => {
				let unit = if operand.is_plain() { unit } else { operand.unit };
				return Err(EvalError::from(UnitMismatch::PlainWithUnit { verb, unit: unit.to_string() }));
			}
			Some(unit) if unit.dimension() != operand.dimension() => return Err(unit_error(UnitError::Incompatible, unit, operand.unit, verb)),
			Some(unit) => common_unit(unit, operand.unit, pixels_per_inch),
		});
	}
	let unit = common.unwrap_or(Unit::NONE);

	let values = operands
		.iter()
		.map(|operand| into_region_unit(*operand, unit, pixels_per_inch).map(Value::Number))
		.collect::<Result<Vec<Value>, EvalError>>()?;
	Ok((values, unit))
}

/// A binary operation over quantities. Sums and comparisons bring both into one unit, products multiply their units, and a
/// power raises its base's unit, while logic takes only plain truth values.
#[inline]
pub(crate) fn binary_op(lhs: Quantity, op: BinaryOp, rhs: Quantity, pixels_per_inch: f64) -> Result<Quantity, EvalError> {
	// Plain numbers skip the unit arithmetic, the common case
	if lhs.is_plain() && rhs.is_plain() {
		return plain_binary_op(lhs.number(), op, rhs.number()).map(|number| Quantity::of_number(number, Unit::NONE));
	}
	unit_binary_op(lhs, op, rhs, pixels_per_inch)
}

#[inline]
fn plain_binary_op(lhs: Number, op: BinaryOp, rhs: Number) -> Result<Number, EvalError> {
	// Logic rejects operands that aren't truth values, while the other operators reject operand types they don't support
	let rejected = if matches!(op, BinaryOp::And | BinaryOp::Or) {
		EvalError::NotATruthValue
	} else {
		EvalError::OperatorTypeError
	};
	lhs.binary_op(op, rhs).ok_or(rejected)
}

// Out of line, keeping the plain arithmetic that dominates compact
#[inline(never)]
pub(crate) fn unit_binary_op(lhs: Quantity, op: BinaryOp, rhs: Quantity, pixels_per_inch: f64) -> Result<Quantity, EvalError> {
	use BinaryOp as Op;
	match op {
		Op::Add | Op::Sub => {
			let verb = if op == Op::Add { "added to" } else { "subtracted from" };
			let (lhs, rhs, unit) = in_common_unit(lhs, rhs, verb, pixels_per_inch)?;
			Ok(Quantity::of_number(plain_binary_op(lhs, op, rhs)?, unit))
		}
		Op::Lt | Op::Leq | Op::Gt | Op::Geq | Op::Eq | Op::Neq => {
			let (lhs, rhs, _) = in_common_unit(lhs, rhs, "compared with", pixels_per_inch)?;
			Ok(Quantity::of_number(plain_binary_op(lhs, op, rhs)?, Unit::NONE))
		}
		Op::Mul | Op::Div => {
			let rhs_unit = if op == Op::Div { rhs.unit.inverse() } else { rhs.unit };
			let (unit, scale) = lhs.unit.product(rhs_unit, pixels_per_inch).map_err(|error| unit_error(error, lhs.unit, rhs.unit, "multiplied by"))?;

			// The left operand takes the conversion's numerator and a divisor its denominator, so `1in / 2.54cm` is exactly 1
			let (numerator, denominator) = scale.split();
			let (lhs, rhs) = (lhs.number(), rhs.number());
			if let Some(exact) = exact_integer_product(lhs, op, rhs, scale) {
				return Ok(Quantity::of_number(exact, unit));
			}
			let split = || -> Result<Number, EvalError> {
				let lhs = scaled(lhs, numerator, pixels_per_inch)?;
				if op == Op::Div {
					plain_binary_op(lhs, op, scaled(rhs, denominator, pixels_per_inch)?)
				} else {
					scaled(plain_binary_op(lhs, op, rhs)?, denominator.inverse_exact(), pixels_per_inch)
				}
			};
			let number = split()?;

			// Near the limits of f64, scaling first or last may keep a finite result that scaling one operand overflowed
			if !all_finite(number) && all_finite(lhs) && all_finite(rhs) {
				let first = plain_binary_op(scaled(lhs, scale, pixels_per_inch)?, op, rhs)?;
				let last = scaled(plain_binary_op(lhs, op, rhs)?, scale, pixels_per_inch)?;
				let number = [first, last].into_iter().find(|number| all_finite(*number)).unwrap_or(number);
				return Ok(Quantity::of_number(number, unit));
			}
			Ok(Quantity::of_number(number, unit))
		}
		Op::Pow => power(lhs, rhs),
		Op::And | Op::Or => Err(EvalError::NotATruthValue),
	}
}

/// A product or quotient of integers times a conversion's ratio, computed in 128 bits where it lands on an integer that fits.
fn exact_integer_product(lhs: Number, op: BinaryOp, rhs: Number, scale: Size) -> Option<Number> {
	let (Number::Integer(lhs), Number::Integer(rhs)) = (lhs, rhs) else { return None };
	let (numerator, denominator) = scale.rational(0.)?;

	let (dividend, divisor) = match op {
		BinaryOp::Mul => ((lhs as i128).checked_mul(rhs as i128)?.checked_mul(numerator)?, denominator),
		_ => ((lhs as i128).checked_mul(numerator)?, (rhs as i128).checked_mul(denominator)?),
	};
	if divisor == 0 || dividend % divisor != 0 {
		return None;
	}
	i64::try_from(dividend / divisor).ok().map(Number::Integer)
}

fn all_finite(number: Number) -> bool {
	number.to_quaternion().parts().iter().all(|part| part.is_finite())
}

/// A power: a plain exponent raises the base's unit, which must land every unit on an integer power, while an exponent in an
/// angle unit with no plain part is the rotation `e^(i θ)`, read in radians.
fn power(base: Quantity, exponent: Quantity) -> Result<Quantity, EvalError> {
	let (base, exponent) = (base.plain_if_dimensionless(), exponent.plain_if_dimensionless());

	if !exponent.is_plain() {
		let in_radians = exponent.convert(radians(), 0.).ok().map(|value| Quantity::plain(value).number());
		return match in_radians {
			Some(angle) if base.is_plain() && angle.to_quaternion().w == 0. => plain_binary_op(base.number(), BinaryOp::Pow, angle).map(|number| Quantity::of_number(number, Unit::NONE)),
			Some(_) => Err(EvalError::AngleExponentWithPlainPart),
			None => Err(EvalError::from(UnitMismatch::ExponentWithUnits(exponent.unit.to_string()))),
		};
	}

	let number = plain_binary_op(base.number(), BinaryOp::Pow, exponent.number())?;
	if base.is_plain() {
		return Ok(Quantity::of_number(number, Unit::NONE));
	}

	// The exponent of a value with units must be real, and raises each unit's power
	let real_exponent = exponent.number().as_real().ok_or_else(|| EvalError::from(UnitMismatch::FractionalUnitPower(base.unit.to_string())))?;
	let unit = base.unit.power(real_exponent).map_err(|error| unit_error(error, base.unit, base.unit, "raised to"))?;
	Ok(Quantity::of_number(number, unit))
}

/// A unary operation over a quantity: signs and the magnitude keep the unit, while the factorial and logic take plain numbers.
pub(crate) fn unary_op(operand: Quantity, op: UnaryOp) -> Result<Quantity, EvalError> {
	let operand = if matches!(op, UnaryOp::Not | UnaryOp::Fac) { operand.plain_if_dimensionless() } else { operand };
	let rejected = if op == UnaryOp::Not { EvalError::NotATruthValue } else { EvalError::OperatorTypeError };

	match op {
		UnaryOp::Not if !operand.is_plain() => Err(EvalError::NotATruthValue),
		UnaryOp::Fac if !operand.is_plain() => Err(EvalError::from(UnitMismatch::UnitsNotAccepted {
			function: "!".to_string(),
			unit: operand.unit.to_string(),
		})),
		_ => Ok(Quantity::of_number(operand.number().unary_op(op).ok_or(rejected)?, operand.unit)),
	}
}

/// Compares a chain of quantities, all brought into one unit first.
pub(crate) fn comparison_operands(first: Quantity, rest: &[(BinaryOp, Quantity)], pixels_per_inch: f64) -> Result<(Number, Vec<(BinaryOp, Number)>), EvalError> {
	if first.is_plain() && rest.iter().all(|(_, operand)| operand.is_plain()) {
		return Ok((first.number(), rest.iter().map(|(op, operand)| (*op, operand.number())).collect()));
	}

	let operands: Vec<Quantity> = std::iter::once(first).chain(rest.iter().map(|(_, operand)| *operand)).collect();
	let (values, _) = all_in_common_unit(&operands, "compared with", pixels_per_inch)?;
	let numbers: Vec<Number> = values.into_iter().map(|Value::Number(number)| number).collect();
	Ok((numbers[0], rest.iter().map(|(op, _)| *op).zip(numbers[1..].iter().copied()).collect()))
}

/// How a builtin taking values treats their units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitRule {
	/// Plain values only, like `ln` or `choose`.
	Plain,
	/// Plain values only, since the result would depend on the unit: `floor`, `ceil`, `round`, `trunc`, and `fract`.
	Rounding,
	/// Values of one dimension brought into one unit, with the result in it, like `min`, `abs`, `snap`, and `mean`.
	Preserving,
	/// Values of one dimension, with a plain result, like `sign`.
	PreservingToPlain,
	/// Values of one dimension, with the result in its square: `variance`.
	Squared,
	/// Anything, with a plain result: `count`.
	Count,
	/// An angle, or a plain value read as radians, with a plain result: the circular functions.
	Circular,
	/// Plain values with a result in radians: the inverse circular functions.
	InverseCircular,
	/// Coordinates of one dimension with a result in radians: `atan2`.
	TwoArgumentArctangent,
	/// A plain value, or an angle with no plain part read as radians: `exp`, the function of the rotation `e^(i θ)`.
	Exponential,
	/// The root of the given degree, which must land every unit on an integer power: `sqrt` and `cbrt`.
	Root(u8),
	/// A root whose degree is the plain second argument: `root`.
	RootOfDegree,
	/// The product of the two arguments' units: `dot` and `cross`.
	Product,
	/// Directions, whose units drop: `normalize` and `axis`.
	Direction,
	/// The angle between two directions of any units, in radians.
	AngleBetween,
	/// A value of any unit, which the result keeps, turned by an angle about a direction: `rotate`.
	Rotate,
	/// An angle about a direction, with a plain result: `rotor`.
	Rotor,
	/// Two endpoints of one dimension and a plain fraction between them: `lerp`.
	Interpolate,
	/// A value of any unit, which the result keeps, against a direction: `project`, `reject`, and `reflect`.
	FirstArgument,
}

impl UnitRule {
	/// The unit of the result for plain arguments, which is radians for a function giving an angle.
	const fn plain_result_unit(self) -> Unit {
		match self {
			Self::InverseCircular | Self::TwoArgumentArctangent | Self::AngleBetween => radians(),
			_ => Unit::NONE,
		}
	}
}

/// How a builtin taking values treats their units, or `None` for a name that is no builtin of values.
pub fn unit_rule(name: &str) -> Option<UnitRule> {
	use UnitRule::*;
	Some(match name {
		"sin" | "cos" | "tan" | "csc" | "sec" | "cot" => Circular,
		"asin" | "acos" | "atan" | "acsc" | "asec" | "acot" => InverseCircular,
		"atan2" => TwoArgumentArctangent,
		"sinh" | "cosh" | "tanh" | "csch" | "sech" | "coth" | "asinh" | "acosh" | "atanh" | "acsch" | "asech" | "acoth" => Plain,
		"ln" | "log" | "xor" | "slerp" | "gcd" | "lcm" | "choose" | "pick" => Plain,
		"exp" => Exponential,
		"sqrt" => Root(2),
		"cbrt" => Root(3),
		"root" => RootOfDegree,
		"floor" | "ceil" | "round" | "trunc" | "fract" => Rounding,
		"hypot" | "abs" | "snap" | "mod" | "min" | "max" | "mean" | "median" | "mode" | "geomean" | "harmmean" | "rms" | "stdev" | "stdevpop" | "distance" | "conj" | "perp" => Preserving,
		"sign" => PreservingToPlain,
		"variance" | "variancepop" => Squared,
		"count" => Count,
		"lerp" => Interpolate,
		"dot" | "cross" => Product,
		"normalize" | "axis" => Direction,
		"angle" => AngleBetween,
		"rotate" => Rotate,
		"rotor" => Rotor,
		"project" | "reject" | "reflect" => FirstArgument,
		_ => return None,
	})
}

/// Reads an angle argument in radians, taking a plain value as radians already.
fn angle_in_radians(function: &str, argument: Quantity) -> Result<Value, EvalError> {
	let argument = argument.plain_if_dimensionless();
	if argument.is_plain() {
		return Ok(argument.value);
	}
	argument.convert(radians(), 0.).map_err(|_| {
		EvalError::from(UnitMismatch::AngleExpected {
			function: function.to_string(),
			unit: argument.unit.to_string(),
		})
	})
}

/// Reads an argument that must be plain, converting a dimensionless unit like `%` out.
fn plain_argument(function: &str, argument: Quantity, rule: UnitRule) -> Result<Value, EvalError> {
	let argument = argument.plain_if_dimensionless();
	if argument.is_plain() {
		return Ok(argument.value);
	}

	let (function, unit) = (function.to_string(), argument.unit.to_string());
	Err(if rule == UnitRule::Rounding {
		EvalError::from(UnitMismatch::RoundingWithUnits { function, unit })
	} else {
		EvalError::from(UnitMismatch::UnitsNotAccepted { function, unit })
	})
}

/// Runs a builtin taking values by its rule for units: the arguments are converted into the units it computes in and stripped,
/// so it sees plain numbers, and its result takes the unit that follows from theirs.
#[inline]
pub(crate) fn run_builtin(name: &str, rule: UnitRule, function: BuiltinFunction, arguments: &[Quantity], pixels_per_inch: f64) -> Result<Quantity, EvalError> {
	// Plain arguments skip the unit rules, the common case, passing through a stack buffer when they fit
	if arguments.iter().all(|argument| argument.is_plain()) {
		let mut stack_values = [Value::from_i64(0); 5];
		let heap_values: Vec<Value>;
		let values: &[Value] = if arguments.len() <= stack_values.len() {
			for (slot, argument) in stack_values.iter_mut().zip(arguments) {
				*slot = argument.value;
			}
			&stack_values[..arguments.len()]
		} else {
			heap_values = arguments.iter().map(|argument| argument.value).collect();
			&heap_values
		};

		let value = function(values).ok_or(EvalError::TypeError)?;
		return Ok(Quantity::new(value, rule.plain_result_unit()));
	}
	run_builtin_with_units(name, rule, function, arguments, pixels_per_inch)
}

/// Runs a builtin over arguments gathered as values beside their units, which the evaluator keeps apart so plain ones need no copy.
#[inline]
pub(crate) fn run_builtin_on_parts(name: &str, rule: UnitRule, function: BuiltinFunction, values: &[Value], units: &[Unit], pixels_per_inch: f64) -> Result<Quantity, EvalError> {
	if units.iter().all(|unit| unit.is_none()) {
		let value = function(values).ok_or(EvalError::TypeError)?;
		return Ok(Quantity::new(value, rule.plain_result_unit()));
	}

	let arguments: Vec<Quantity> = values.iter().zip(units).map(|(value, unit)| Quantity::new(*value, *unit)).collect();
	run_builtin_with_units(name, rule, function, &arguments, pixels_per_inch)
}

#[inline(never)]
fn run_builtin_with_units(name: &str, rule: UnitRule, function: BuiltinFunction, arguments: &[Quantity], pixels_per_inch: f64) -> Result<Quantity, EvalError> {
	use UnitRule::*;

	let call = |values: &[Value]| function(values).ok_or(EvalError::TypeError);
	let result_in = |value: Value, unit: Unit| Ok(Quantity::new(value, unit));
	let plain_arguments = || arguments.iter().map(|argument| plain_argument(name, *argument, rule)).collect::<Result<Vec<Value>, EvalError>>();

	match rule {
		Plain | Rounding => result_in(call(&plain_arguments()?)?, Unit::NONE),
		Preserving | PreservingToPlain | Squared => {
			let (values, unit) = all_in_common_unit(arguments, "mixed with", pixels_per_inch)?;
			let value = call(&values)?;
			match rule {
				Preserving => result_in(value, unit),
				Squared => result_in(value, unit.power(2.).map_err(|error| unit_error(error, unit, unit, "squared"))?),
				_ => result_in(value, Unit::NONE),
			}
		}
		Count => result_in(call(&arguments.iter().map(|argument| argument.value).collect::<Vec<Value>>())?, Unit::NONE),
		Circular => {
			let angles = arguments.iter().map(|argument| angle_in_radians(name, *argument)).collect::<Result<Vec<Value>, EvalError>>()?;
			result_in(call(&angles)?, Unit::NONE)
		}
		InverseCircular => result_in(call(&plain_arguments()?)?, radians()),
		TwoArgumentArctangent => {
			let (values, _) = all_in_common_unit(arguments, "mixed with", pixels_per_inch)?;
			result_in(call(&values)?, radians())
		}
		Exponential => {
			let [argument] = arguments else { return Err(EvalError::TypeError) };
			let argument = argument.plain_if_dimensionless();
			if argument.is_plain() {
				return result_in(call(&[argument.value])?, Unit::NONE);
			}
			let angle = angle_in_radians(name, argument)?;
			let Value::Number(number) = angle;
			if number.to_quaternion().w != 0. {
				return Err(EvalError::AngleExponentWithPlainPart);
			}
			result_in(call(&[angle])?, Unit::NONE)
		}
		Root(degree) => {
			let [argument] = arguments else { return Err(EvalError::TypeError) };
			let argument = argument.plain_if_dimensionless();
			let unit = argument.unit.power(1. / degree as f64).map_err(|error| unit_error(error, argument.unit, argument.unit, "rooted"))?;
			result_in(call(&[argument.value])?, unit)
		}
		RootOfDegree => {
			let [argument, degree] = arguments else { return Err(EvalError::TypeError) };
			let argument = argument.plain_if_dimensionless();
			let degree = plain_argument(name, *degree, rule)?;
			let unit = match degree.as_real() {
				Some(degree) => argument.unit.power(1. / degree).map_err(|error| unit_error(error, argument.unit, argument.unit, "rooted"))?,
				None if argument.is_plain() => Unit::NONE,
				None => return Err(EvalError::from(UnitMismatch::FractionalUnitPower(argument.unit.to_string()))),
			};
			result_in(call(&[argument.value, degree])?, unit)
		}
		Product => {
			let [a, b] = arguments else { return Err(EvalError::TypeError) };
			let (unit, scale) = a.unit.product(b.unit, pixels_per_inch).map_err(|error| unit_error(error, a.unit, b.unit, "multiplied by"))?;
			let Value::Number(number) = call(&[a.value, b.value])?;
			Ok(Quantity::of_number(scaled(number, scale, pixels_per_inch)?, unit))
		}
		Direction => result_in(call(&arguments.iter().map(|argument| argument.value).collect::<Vec<Value>>())?, Unit::NONE),
		AngleBetween => result_in(call(&arguments.iter().map(|argument| argument.value).collect::<Vec<Value>>())?, radians()),
		Rotate => {
			let [value, angle, axis @ ..] = arguments else { return Err(EvalError::TypeError) };
			let mut values = vec![value.value, angle_in_radians(name, *angle)?];
			values.extend(axis.iter().map(|axis| axis.value));
			result_in(call(&values)?, value.unit)
		}
		Rotor => {
			let [angle, axis @ ..] = arguments else { return Err(EvalError::TypeError) };
			let mut values = vec![angle_in_radians(name, *angle)?];
			values.extend(axis.iter().map(|axis| axis.value));
			result_in(call(&values)?, Unit::NONE)
		}
		Interpolate => {
			let [a, b, t] = arguments else { return Err(EvalError::TypeError) };
			let (endpoints, unit) = all_in_common_unit(&[*a, *b], "mixed with", pixels_per_inch)?;
			let t = plain_argument(name, *t, rule)?;
			result_in(call(&[endpoints[0], endpoints[1], t])?, unit)
		}
		FirstArgument => {
			let [value, direction] = arguments else { return Err(EvalError::TypeError) };
			result_in(call(&[value.value, direction.value])?, value.unit)
		}
	}
}

/// A matrix with the units of its linear part and of its translation, so it maps a value in some unit to one in that unit times
/// the linear unit, plus the translation. A range from `0` to `10 mm` has both in `mm`, mapping a plain parameter to a length,
/// while a rotation is plain and leaves any unit as it was. A zero translation has no unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatrixQuantity {
	pub matrix: Matrix,
	pub linear: Unit,
	pub translation: Unit,
}

impl MatrixQuantity {
	pub const fn plain(matrix: Matrix) -> Self {
		Self {
			matrix,
			linear: Unit::NONE,
			translation: Unit::NONE,
		}
	}

	pub fn is_plain(&self) -> bool {
		self.linear.is_none() && self.translation.is_none()
	}

	/// Builds the matrix from its linear part and its translation, dropping the unit of a zero translation.
	fn from_parts(linear: Matrix, linear_unit: Unit, translation: Quantity) -> Self {
		let translation_quaternion = translation.number().to_quaternion();
		Self {
			matrix: Matrix {
				translation: translation_quaternion,
				..linear
			},
			linear: linear_unit,
			translation: if translation_quaternion == Quaternion::ZERO { Unit::NONE } else { translation.unit },
		}
	}

	fn linear_part(&self) -> Matrix {
		Matrix {
			translation: Quaternion::ZERO,
			..self.matrix
		}
	}

	fn translation_part(&self) -> Quantity {
		Quantity::of_number(Number::Quaternion(self.matrix.translation).canonical(), self.translation)
	}

	/// Multiplies the linear part's entries by a factor, as converting its unit does.
	fn scaled_linear(linear: Matrix, size: Size, pixels_per_inch: f64) -> Result<Matrix, EvalError> {
		if size.is_one() {
			return Ok(linear);
		}
		let mut rows = linear.rows;
		for (row, spanned) in rows.iter_mut().zip(linear.axes) {
			if spanned {
				*row = scaled(Number::Quaternion(*row), size, pixels_per_inch)?.to_quaternion();
			}
		}
		Ok(Matrix { rows, ..linear })
	}

	/// The image of a value, `M v + c`, whose linear part is in the value's unit times the linear unit, and which the translation's unit must match.
	pub(crate) fn apply(&self, value: Quantity, pixels_per_inch: f64) -> Result<Quantity, EvalError> {
		if self.is_plain() && value.is_plain() {
			return Ok(Quantity::of_number(Number::Quaternion(self.matrix.apply(value.number().to_quaternion())).canonical(), Unit::NONE));
		}
		// A plain zero is the same in every unit, so its image is the translation alone
		if value.is_plain() && value.is_zero() {
			return Ok(self.translation_part());
		}

		let linear = self.linear_part().apply(value.number().to_quaternion());
		let (unit, scale) = value
			.unit
			.product(self.linear, pixels_per_inch)
			.map_err(|error| unit_error(error, value.unit, self.linear, "multiplied by"))?;
		let image = Quantity::of_number(scaled(Number::Quaternion(linear).canonical(), scale, pixels_per_inch)?, unit);
		if self.matrix.translation == Quaternion::ZERO {
			return Ok(image);
		}
		binary_op(image, BinaryOp::Add, self.translation_part(), pixels_per_inch)
	}

	/// The composition `A B`, applying `B` first.
	pub(crate) fn compose(&self, inner: &Self, pixels_per_inch: f64) -> Result<Self, EvalError> {
		if self.is_plain() && inner.is_plain() {
			return Ok(Self::plain(self.matrix.compose(inner.matrix)));
		}

		let (linear_unit, scale) = self
			.linear
			.product(inner.linear, pixels_per_inch)
			.map_err(|error| unit_error(error, self.linear, inner.linear, "composed with"))?;
		let linear = Self::scaled_linear(self.linear_part().compose(inner.linear_part()), scale, pixels_per_inch)?;
		let translation = self.apply(inner.translation_part(), pixels_per_inch)?;
		Ok(Self::from_parts(linear, linear_unit, translation))
	}

	/// The inverse map, whose linear unit is the reciprocal and whose translation is in the translation's unit over the linear one.
	pub(crate) fn inverse(&self, pixels_per_inch: f64) -> Result<Self, EvalError> {
		let inverse = self.matrix.inverse().ok_or(EvalError::SingularMatrix)?;
		if self.is_plain() {
			return Ok(Self::plain(inverse));
		}

		let linear_unit = self.linear.inverse();
		let translation_part = Quantity::of_number(Number::Quaternion(inverse.translation).canonical(), self.translation);
		let (translation_unit, scale) = translation_part
			.unit
			.product(linear_unit, pixels_per_inch)
			.map_err(|error| unit_error(error, self.translation, linear_unit, "divided by"))?;
		let translation = Quantity::of_number(scaled(translation_part.number(), scale, pixels_per_inch)?, translation_unit);
		let linear = Matrix {
			translation: Quaternion::ZERO,
			..inverse
		};
		Ok(Self::from_parts(linear, linear_unit, translation))
	}

	/// A whole power by repeated composition, where a negative one is of the inverse.
	pub(crate) fn power(&self, exponent: i64, pixels_per_inch: f64) -> Result<Self, EvalError> {
		if self.is_plain() {
			return self.matrix.power(exponent).map(Self::plain).ok_or(EvalError::SingularMatrix);
		}

		// The zeroth power is the identity over the axes the matrix spans, as for a plain matrix
		let identity = self.matrix.power(0).ok_or(EvalError::SingularMatrix)?;
		let mut result = Self::plain(identity);

		// By squaring, in O(log n) compositions, squaring only while bits remain
		let (mut base, mut remaining) = (if exponent < 0 { self.inverse(pixels_per_inch)? } else { *self }, exponent.unsigned_abs());
		while remaining > 0 {
			if remaining & 1 == 1 {
				result = result.compose(&base, pixels_per_inch)?;
			}
			remaining >>= 1;
			if remaining > 0 {
				base = base.compose(&base, pixels_per_inch)?;
			}
		}
		Ok(result)
	}

	/// The pointwise sum or difference of two matrices, whose linear parts and translations must each match in dimension.
	pub(crate) fn sum(&self, op: BinaryOp, other: &Self, pixels_per_inch: f64) -> Result<Self, EvalError> {
		if self.is_plain() && other.is_plain() {
			let matrix = if op == BinaryOp::Sub { self.matrix - other.matrix } else { self.matrix + other.matrix };
			return Ok(Self::plain(matrix));
		}

		let verb = if op == BinaryOp::Sub { "subtracted from" } else { "added to" };
		let linear_unit = if self.linear == other.linear {
			self.linear
		} else {
			common_unit(self.linear, other.linear, pixels_per_inch)
		};
		let convert = |matrix: &Self| -> Result<Matrix, EvalError> {
			let factor = matrix.linear.conversion(linear_unit).map_err(|error| unit_error(error, matrix.linear, linear_unit, verb))?;
			Self::scaled_linear(matrix.linear_part(), factor, pixels_per_inch)
		};
		let (left, right) = (convert(self)?, convert(other)?);
		let linear = if op == BinaryOp::Sub { left - right } else { left + right };
		let translation = binary_op(self.translation_part(), op, other.translation_part(), pixels_per_inch)?;
		Ok(Self::from_parts(linear, linear_unit, translation))
	}

	/// The matrix with a value attached as translation, or subtracted from it.
	pub(crate) fn translated(&self, op: BinaryOp, by: Quantity, pixels_per_inch: f64) -> Result<Self, EvalError> {
		let translation = binary_op(self.translation_part(), op, by, pixels_per_inch)?;
		Ok(Self::from_parts(self.linear_part(), self.linear, translation))
	}

	pub(crate) fn negated(&self) -> Self {
		Self { matrix: -self.matrix, ..*self }
	}

	/// The matrix's entries converted into the given linear and translation units, which must measure the same dimensions as its own.
	pub(crate) fn in_units(&self, linear: Unit, translation: Unit, pixels_per_inch: f64) -> Result<Matrix, EvalError> {
		let linear_factor = self.linear.conversion(linear).map_err(|error| unit_error(error, self.linear, linear, "compared with"))?;
		let linear_part = Self::scaled_linear(self.linear_part(), linear_factor, pixels_per_inch)?;
		let translation = into_region_unit(self.translation_part(), translation, pixels_per_inch)?.to_quaternion();
		Ok(Matrix { translation, ..linear_part })
	}

	/// The matrix in plain numbers measured in `unit`, as a region of values in that unit: its linear part maps a plain parameter
	/// to that unit, and its translation must match it.
	pub(crate) fn as_region_in(&self, unit: Unit, pixels_per_inch: f64) -> Result<Matrix, EvalError> {
		let linear_factor = self.linear.conversion(unit).map_err(|error| unit_error(error, self.linear, unit, "mixed with"))?;
		let linear = Self::scaled_linear(self.linear_part(), linear_factor, pixels_per_inch)?;
		let translation = self.translation_part();
		let translation = if translation.is_zero() && translation.is_plain() {
			Quaternion::ZERO
		} else {
			let factor = translation.unit.conversion(unit).map_err(|error| unit_error(error, translation.unit, unit, "mixed with"))?;
			scaled(translation.number(), factor, pixels_per_inch)?.to_quaternion()
		};
		Ok(Matrix { translation, ..linear })
	}

	/// The unit of the values a region holds: its linear unit, or its translation's when the linear part is plain.
	pub(crate) fn region_unit(&self) -> Unit {
		if self.linear.is_none() { self.translation } else { self.linear }
	}
}

impl fmt::Display for MatrixQuantity {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.matrix.fmt(f)?;
		if !self.linear.is_none() {
			write!(f, " in {}", self.linear)?;
		}
		if !self.translation.is_none() && self.translation != self.linear {
			write!(f, " translated in {}", self.translation)?;
		}
		Ok(())
	}
}

/// The values of a matrix literal's entries brought into one unit, which becomes the matrix's linear unit.
pub(crate) fn literal_entries(entries: &[Quantity], pixels_per_inch: f64) -> Result<(Vec<Quaternion>, Unit), EvalError> {
	let (values, unit) = all_in_common_unit(entries, "mixed with", pixels_per_inch)?;
	Ok((values.into_iter().map(|Value::Number(number)| number.to_quaternion()).collect(), unit))
}

/// The corners of a range brought into one unit, with that unit.
pub(crate) fn range_corners(from: Quantity, to: Quantity, pixels_per_inch: f64) -> Result<(Number, Number, Unit), EvalError> {
	in_common_unit(from, to, "mixed with", pixels_per_inch)
}

/// Converts a value into the unit it's measured against, where a plain zero matches any unit.
pub(crate) fn into_region_unit(value: Quantity, unit: Unit, pixels_per_inch: f64) -> Result<Number, EvalError> {
	let value = if unit.is_none() { value.plain_if_dimensionless() } else { value };
	if value.unit == unit || (value.is_plain() && value.is_zero()) {
		return Ok(value.number());
	}
	if value.is_plain() || unit.is_none() {
		let unit = if value.is_plain() { unit } else { value.unit };
		return Err(EvalError::from(UnitMismatch::PlainWithUnit {
			verb: "mixed with",
			unit: unit.to_string(),
		}));
	}

	let factor = value.unit.conversion(unit).map_err(|error| unit_error(error, value.unit, unit, "mixed with"))?;
	scaled(value.number(), factor, pixels_per_inch)
}

/// The unit of a range function's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionResult {
	/// The unit of the region the value is measured against: `clamp`.
	FirstRegion,
	/// The unit of the region the value is mapped into: `remap`.
	LastRegion,
	/// No unit: `within` and `smoothstep`.
	Plain,
}

pub fn region_result(name: &str) -> RegionResult {
	match name {
		"clamp" => RegionResult::FirstRegion,
		"remap" => RegionResult::LastRegion,
		_ => RegionResult::Plain,
	}
}

/// The unit of a value taken from a matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatrixValueUnit {
	/// The translation's unit: `translation`.
	Translation,
	/// The linear unit to the power of the axes the matrix spans: `det`.
	Determinant,
}

pub fn matrix_value_unit(name: &str) -> MatrixValueUnit {
	match name {
		"translation" => MatrixValueUnit::Translation,
		_ => MatrixValueUnit::Determinant,
	}
}

/// How a matrix builder reads its arguments' units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuilderUnits {
	/// The value's unit becomes the linear unit: `matrix` and `scale`.
	Scaling,
	/// An angle read in radians, about a direction of any unit: `rotation`.
	Rotation,
	/// Directions of any unit and a plain factor: `shear`.
	Shear,
}

pub fn builder_units(name: &str) -> BuilderUnits {
	match name {
		"rotation" => BuilderUnits::Rotation,
		"shear" => BuilderUnits::Shear,
		_ => BuilderUnits::Scaling,
	}
}

/// The plain arguments of a matrix builder with the linear unit of the matrix it builds.
pub(crate) fn builder_arguments(units: BuilderUnits, arguments: &[Quantity]) -> Result<(Vec<Value>, Unit), EvalError> {
	match units {
		BuilderUnits::Scaling => {
			let [value] = arguments else { return Err(EvalError::TypeError) };
			let value = value.plain_if_dimensionless();
			Ok((vec![value.value], value.unit))
		}
		BuilderUnits::Rotation => {
			let [angle, axis @ ..] = arguments else { return Err(EvalError::TypeError) };
			let mut values = vec![angle_in_radians("rotation", *angle)?];
			values.extend(axis.iter().map(|axis| axis.value));
			Ok((values, Unit::NONE))
		}
		BuilderUnits::Shear => {
			let [along, by, factor] = arguments else { return Err(EvalError::TypeError) };
			let factor = plain_argument("shear", *factor, UnitRule::Plain)?;
			Ok((vec![direction(*along)?, direction(*by)?, factor], Unit::NONE))
		}
	}
}

/// A direction given as a value: a plain one as it is, and one with a unit as its unit vector, since its size in that unit means nothing.
fn direction(value: Quantity) -> Result<Value, EvalError> {
	let value = value.plain_if_dimensionless();
	if value.is_plain() {
		return Ok(value.value);
	}
	value.number().to_quaternion().normalized().map(Value::from).ok_or(EvalError::Indeterminate)
}

/// The unit of a value taken from a matrix by a function.
pub(crate) fn matrix_value_result_unit(unit: MatrixValueUnit, matrix: &MatrixQuantity) -> Result<Unit, EvalError> {
	match unit {
		MatrixValueUnit::Translation => Ok(matrix.translation),
		MatrixValueUnit::Determinant => {
			let spanned = matrix.matrix.axes.iter().filter(|spanned| **spanned).count();
			matrix.linear.power(spanned as f64).map_err(|_| EvalError::UnitOverflow)
		}
	}
}
