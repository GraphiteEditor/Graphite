use glam::{DAffine2, DVec2};
use graphene_core::gradient::GradientStops;
use graphene_core::registry::types::{Fraction, Percentage, PixelSize, TextArea};
use graphene_core::table::Table;
use graphene_core::transform::Footprint;
use graphene_core::{Color, Ctx, num_traits};
use log::warn;
use math_parser::ast;
use math_parser::context::{EvalContext, NothingMap, ValueProvider};
use math_parser::value::{Number, Value};
use num_traits::Pow;
use rand::{Rng, SeedableRng};
use std::ops::{Add, Div, Mul, Rem, Sub};

/// The struct that stores the context for the maths parser.
/// This is currently just limited to supplying `a` and `b` until we add better node graph support and UI for variadic inputs.
struct MathNodeContext {
	a: f64,
	b: f64,
}

impl ValueProvider for MathNodeContext {
	fn get_value(&self, name: &str) -> Option<Value> {
		if name.eq_ignore_ascii_case("a") {
			Some(Value::from_f64(self.a))
		} else if name.eq_ignore_ascii_case("b") {
			Some(Value::from_f64(self.b))
		} else {
			None
		}
	}
}

/// Calculates a mathematical expression with input values "A" and "B"
#[node_macro::node(category("Math: Arithmetic"), properties("math_properties"))]
fn math<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The value of "A" when calculating the expression
	#[implementations(f64, f32)]
	operand_a: T,
	/// A math expression that may incorporate "A" and/or "B", such as "sqrt(A + B) - B^2"
	#[default(A + B)]
	expression: String,
	/// The value of "B" when calculating the expression
	#[implementations(f64, f32)]
	#[default(1.)]
	operand_b: T,
) -> T {
	let (node, _unit) = match ast::Node::try_parse_from_str(&expression) {
		Ok(expr) => expr,
		Err(e) => {
			warn!("Invalid expression: `{expression}`\n{e:?}");
			return T::from(0.).unwrap();
		}
	};
	let context = EvalContext::new(
		MathNodeContext {
			a: operand_a.to_f64().unwrap(),
			b: operand_b.to_f64().unwrap(),
		},
		NothingMap,
	);

	let value = match node.eval(&context) {
		Ok(value) => value,
		Err(e) => {
			warn!("Expression evaluation error: {e:?}");
			return T::from(0.).unwrap();
		}
	};

	let Value::Number(num) = value;
	match num {
		Number::Real(val) => T::from(val).unwrap(),
		Number::Complex(c) => T::from(c.re).unwrap(),
	}
}

/// The addition operation (+) calculates the sum of two numbers.
#[node_macro::node(category("Math: Arithmetic"))]
fn add<U: Add<T>, T>(
	_: impl Ctx,
	/// The left-hand side of the addition operation.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	augend: U,
	/// The right-hand side of the addition operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	addend: T,
) -> <U as Add<T>>::Output {
	augend + addend
}

/// The subtraction operation (-) calculates the difference between two numbers.
#[node_macro::node(category("Math: Arithmetic"))]
fn subtract<U: Sub<T>, T>(
	_: impl Ctx,
	/// The left-hand side of the subtraction operation.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	minuend: U,
	/// The right-hand side of the subtraction operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	subtrahend: T,
) -> <U as Sub<T>>::Output {
	minuend - subtrahend
}

/// The multiplication operation (×) calculates the product of two numbers.
#[node_macro::node(category("Math: Arithmetic"))]
fn multiply<U: Mul<T>, T>(
	_: impl Ctx,
	/// The left-hand side of the multiplication operation.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2, DAffine2)]
	multiplier: U,
	/// The right-hand side of the multiplication operation.
	#[default(1.)]
	#[implementations(f64, f32, u32, DVec2, DVec2, f64, DAffine2)]
	multiplicand: T,
) -> <U as Mul<T>>::Output {
	multiplier * multiplicand
}

/// The division operation (÷) calculates the quotient of two numbers.
///
/// Produces 0 if the denominator is 0.
#[node_macro::node(category("Math: Arithmetic"))]
fn divide<U: Div<T> + Default + PartialEq, T: Default + PartialEq>(
	_: impl Ctx,
	/// The left-hand side of the division operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	numerator: U,
	/// The right-hand side of the division operation.
	#[default(1.)]
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	denominator: T,
) -> <U as Div<T>>::Output
where
	<U as Div<T>>::Output: Default,
{
	if denominator == T::default() {
		return <U as Div<T>>::Output::default();
	}
	numerator / denominator
}

/// The modulo operation (%) calculates the remainder from the division of two numbers. The sign of the result shares the sign of the numerator unless "Always Positive" is enabled.
#[node_macro::node(category("Math: Arithmetic"))]
fn modulo<U: Rem<T, Output: Add<T, Output: Rem<T, Output = U::Output>>>, T: Copy>(
	_: impl Ctx,
	/// The left-hand side of the modulo operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	numerator: U,
	/// The right-hand side of the modulo operation.
	#[default(2.)]
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	modulus: T,
	/// Ensures the result will always be positive, even if the numerator is negative.
	#[default(true)]
	always_positive: bool,
) -> <U as Rem<T>>::Output {
	if always_positive { (numerator % modulus + modulus) % modulus } else { numerator % modulus }
}

/// The exponent operation (^) calculates the result of raising a number to a power.
#[node_macro::node(category("Math: Arithmetic"))]
fn exponent<T: Pow<T>>(
	_: impl Ctx,
	/// The base number that will be raised to the power.
	#[implementations(f64, f32, u32)]
	base: T,
	/// The power to which the base number will be raised.
	#[implementations(f64, f32, u32)]
	#[default(2.)]
	power: T,
) -> <T as num_traits::Pow<T>>::Output {
	base.pow(power)
}

/// The square root operation (√) calculates the nth root of a number, equivalent to raising the number to the power of 1/n.
#[node_macro::node(category("Math: Arithmetic"))]
fn root<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number for which the nth root will be calculated.
	#[default(2.)]
	#[implementations(f64, f32)]
	radicand: T,
	/// The degree of the root to be calculated. Square root is 2, cube root is 3, and so on.
	#[default(2.)]
	#[implementations(f64, f32)]
	degree: T,
) -> T {
	if degree == T::from(2.).unwrap() {
		radicand.sqrt()
	} else if degree == T::from(3.).unwrap() {
		radicand.cbrt()
	} else {
		radicand.powf(T::from(1.).unwrap() / degree)
	}
}

/// The logarithmic function (log) calculates the logarithm of a number with a specified base. If the natural logarithm function (ln) is desired, set the base to "e".
#[node_macro::node(category("Math: Arithmetic"))]
fn logarithm<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number for which the logarithm will be calculated.
	#[implementations(f64, f32)]
	value: T,
	/// The base of the logarithm, such as 2 (binary), 10 (decimal), and e (natural logarithm).
	#[default(2.)]
	#[implementations(f64, f32)]
	base: T,
) -> T {
	if base == T::from(2.).unwrap() {
		value.log2()
	} else if base == T::from(10.).unwrap() {
		value.log10()
	} else if base - T::from(std::f64::consts::E).unwrap() < T::epsilon() * T::from(1e6).unwrap() {
		value.ln()
	} else {
		value.log(base)
	}
}

/// The sine trigonometric function (sin) calculates the ratio of the angle's opposite side length to its hypotenuse length.
#[node_macro::node(category("Math: Trig"))]
fn sine<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The given angle.
	#[implementations(f64, f32)]
	theta: T,
	/// Whether the given angle should be interpreted as radians instead of degrees.
	radians: bool,
) -> T {
	if radians { theta.sin() } else { theta.to_radians().sin() }
}

/// The cosine trigonometric function (cos) calculates the ratio of the angle's adjacent side length to its hypotenuse length.
#[node_macro::node(category("Math: Trig"))]
fn cosine<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The given angle.
	#[implementations(f64, f32)]
	theta: T,
	/// Whether the given angle should be interpreted as radians instead of degrees.
	radians: bool,
) -> T {
	if radians { theta.cos() } else { theta.to_radians().cos() }
}

/// The tangent trigonometric function (tan) calculates the ratio of the angle's opposite side length to its adjacent side length.
#[node_macro::node(category("Math: Trig"))]
fn tangent<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The given angle.
	#[implementations(f64, f32)]
	theta: T,
	/// Whether the given angle should be interpreted as radians instead of degrees.
	radians: bool,
) -> T {
	if radians { theta.tan() } else { theta.to_radians().tan() }
}

/// The inverse sine trigonometric function (asin) calculates the angle whose sine is the specified value.
#[node_macro::node(category("Math: Trig"))]
fn sine_inverse<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The given value for which the angle will be calculated. Must be in the range [-1, 1] or else the result will be NaN.
	#[implementations(f64, f32)]
	value: T,
	/// Whether the resulting angle should be given in as radians instead of degrees.
	radians: bool,
) -> T {
	if radians { value.asin() } else { value.asin().to_degrees() }
}

/// The inverse cosine trigonometric function (acos) calculates the angle whose cosine is the specified value.
#[node_macro::node(category("Math: Trig"))]
fn cosine_inverse<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The given value for which the angle will be calculated. Must be in the range [-1, 1] or else the result will be NaN.
	#[implementations(f64, f32)]
	value: T,
	/// Whether the resulting angle should be given in as radians instead of degrees.
	radians: bool,
) -> T {
	if radians { value.acos() } else { value.acos().to_degrees() }
}

/// The inverse tangent trigonometric function (atan or atan2, depending on input type) calculates:
/// atan: the angle whose tangent is the specified scalar number.
/// atan2: the angle of a ray from the origin to the specified vec2.
///
/// The resulting angle is always in the range [-90°, 90°] or, in radians, [-π/2, π/2].
#[node_macro::node(category("Math: Trig"))]
fn tangent_inverse<T: TangentInverse>(
	_: impl Ctx,
	/// The given value for which the angle will be calculated.
	#[implementations(f64, f32, DVec2)]
	value: T,
	/// Whether the resulting angle should be given in as radians instead of degrees.
	radians: bool,
) -> T::Output {
	value.atan(radians)
}

pub trait TangentInverse {
	type Output: num_traits::float::Float;
	fn atan(self, radians: bool) -> Self::Output;
}
impl TangentInverse for f32 {
	type Output = f32;
	fn atan(self, radians: bool) -> Self::Output {
		if radians { self.atan() } else { self.atan().to_degrees() }
	}
}
impl TangentInverse for f64 {
	type Output = f64;
	fn atan(self, radians: bool) -> Self::Output {
		if radians { self.atan() } else { self.atan().to_degrees() }
	}
}
impl TangentInverse for DVec2 {
	type Output = f64;
	fn atan(self, radians: bool) -> Self::Output {
		if radians { self.y.atan2(self.x) } else { self.y.atan2(self.x).to_degrees() }
	}
}

#[node_macro::node(category("Math: Numeric"))]
fn remap<U: num_traits::float::Float>(
	_: impl Ctx,
	#[implementations(f64, f32)] value: U,
	#[implementations(f64, f32)] input_min: U,
	#[implementations(f64, f32)]
	#[default(1.)]
	input_max: U,
	#[implementations(f64, f32)] output_min: U,
	#[implementations(f64, f32)]
	#[default(1.)]
	output_max: U,
	clamped: bool,
) -> U {
	let input_range = input_max - input_min;

	// Handle division by zero
	if input_range.abs() < U::epsilon() {
		return output_min;
	}

	let normalized = (value - input_min) / input_range;
	let output_range = output_max - output_min;

	let result = output_min + normalized * output_range;

	if clamped {
		// Handle both normal and inverted ranges, since we want to allow the user to use this node to also reverse a range.
		if output_min <= output_max {
			result.clamp(output_min, output_max)
		} else {
			result.clamp(output_max, output_min)
		}
	} else {
		result
	}
}

/// The random function (rand) converts a seed into a random number within the specified range, inclusive of the minimum and exclusive of the maximum. The minimum and maximum values are automatically swapped if they are reversed.
#[node_macro::node(category("Math: Numeric"))]
fn random(
	_: impl Ctx,
	_primary: (),
	/// Seed to determine the unique variation of which number will be generated.
	seed: u64,
	/// The smaller end of the range within which the random number will be generated.
	#[default(0.)]
	min: f64,
	/// The larger end of the range within which the random number will be generated.
	#[default(1.)]
	max: f64,
) -> f64 {
	let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
	let result = rng.random::<f64>();
	let (min, max) = if min < max { (min, max) } else { (max, min) };
	result * (max - min) + min
}

// TODO: Test that these are no longer needed in all circumstances, then remove them and add a migration to convert these into Passthrough nodes. Note: these act more as type annotations than as identity functions.
/// Convert a number to an integer of the type u32, which may be the required type for certain node inputs.
#[node_macro::node(name("To u32"), category("Debug"))]
fn to_u32(_: impl Ctx, value: u32) -> u32 {
	value
}

// TODO: Test that these are no longer needed in all circumstances, then remove them and add a migration to convert these into Passthrough nodes. Note: these act more as type annotations than as identity functions.
/// Convert a number to an integer of the type u64, which may be the required type for certain node inputs.
#[node_macro::node(name("To u64"), category("Debug"))]
fn to_u64(_: impl Ctx, value: u64) -> u64 {
	value
}

// TODO: Test that these are no longer needed in all circumstances, then remove them and add a migration to convert these into Passthrough nodes. Note: these act more as type annotations than as identity functions.
/// Convert an integer to a decimal number of the type f64, which may be the required type for certain node inputs.
#[node_macro::node(name("To f64"), category("Debug"))]
fn to_f64(_: impl Ctx, value: f64) -> f64 {
	value
}

/// The rounding function (round) maps an input value to its nearest whole number. Halfway values are rounded away from zero.
#[node_macro::node(category("Math: Numeric"))]
fn round<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number which will be rounded.
	#[implementations(f64, f32)]
	value: T,
) -> T {
	value.round()
}

/// The floor function (floor) rounds down an input value to the nearest whole number, unless the input number is already whole.
#[node_macro::node(category("Math: Numeric"))]
fn floor<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number which will be rounded down.
	#[implementations(f64, f32)]
	value: T,
) -> T {
	value.floor()
}

/// The ceiling function (ceil) rounds up an input value to the nearest whole number, unless the input number is already whole.
#[node_macro::node(category("Math: Numeric"))]
fn ceiling<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number which will be rounded up.
	#[implementations(f64, f32)]
	value: T,
) -> T {
	value.ceil()
}

/// The absolute value function (abs) removes the negative sign from an input value, if present.
#[node_macro::node(category("Math: Numeric"))]
fn absolute_value<T: num_traits::sign::Signed>(
	_: impl Ctx,
	/// The number which will be made positive.
	#[implementations(f64, f32, i32, i64)]
	value: T,
) -> T {
	value.abs()
}

/// The minimum function (min) picks the smaller of two numbers.
#[node_macro::node(category("Math: Numeric"))]
fn min<T: std::cmp::PartialOrd>(
	_: impl Ctx,
	/// One of the two numbers, of which the lesser will be returned.
	#[implementations(f64, f32, u32, &str)]
	value: T,
	/// The other of the two numbers, of which the lesser will be returned.
	#[implementations(f64, f32, u32, &str)]
	other_value: T,
) -> T {
	if value < other_value { value } else { other_value }
}

/// The maximum function (max) picks the larger of two numbers.
#[node_macro::node(category("Math: Numeric"))]
fn max<T: std::cmp::PartialOrd>(
	_: impl Ctx,
	/// One of the two numbers, of which the greater will be returned.
	#[implementations(f64, f32, u32, &str)]
	value: T,
	/// The other of the two numbers, of which the greater will be returned.
	#[implementations(f64, f32, u32, &str)]
	other_value: T,
) -> T {
	if value > other_value { value } else { other_value }
}

/// The clamp function (clamp) restricts a number to a specified range between a minimum and maximum value. The minimum and maximum values are automatically swapped if they are reversed.
#[node_macro::node(category("Math: Numeric"))]
fn clamp<T: std::cmp::PartialOrd>(
	_: impl Ctx,
	/// The number to be clamped, which will be restricted to the range between the minimum and maximum values.
	#[implementations(f64, f32, u32, &str)]
	value: T,
	/// The left (smaller) side of the range. The output will never be less than this number.
	#[implementations(f64, f32, u32, &str)]
	min: T,
	/// The right (greater) side of the range. The output will never be greater than this number.
	#[implementations(f64, f32, u32, &str)]
	max: T,
) -> T {
	let (min, max) = if min < max { (min, max) } else { (max, min) };
	if value < min {
		min
	} else if value > max {
		max
	} else {
		value
	}
}

/// The greatest common divisor (GCD) calculates the largest positive integer that divides both of the two input numbers without leaving a remainder.
#[node_macro::node(category("Math: Numeric"))]
fn greatest_common_divisor<T: num_traits::int::PrimInt + std::ops::ShrAssign<i32> + std::ops::SubAssign>(
	_: impl Ctx,
	/// One of the two numbers for which the GCD will be calculated.
	#[implementations(u32, u64, i32)]
	value: T,
	/// The other of the two numbers for which the GCD will be calculated.
	#[implementations(u32, u64, i32)]
	other_value: T,
) -> T {
	if value == T::zero() {
		return other_value;
	}
	if other_value == T::zero() {
		return value;
	}
	binary_gcd(value, other_value)
}

/// The least common multiple (LCM) calculates the smallest positive integer that is a multiple of both of the two input numbers.
#[node_macro::node(category("Math: Numeric"))]
fn least_common_multiple<T: num_traits::ToPrimitive + num_traits::FromPrimitive + num_traits::identities::Zero>(
	_: impl Ctx,
	/// One of the two numbers for which the LCM will be calculated.
	#[implementations(u32, u64, i32)]
	value: T,
	/// The other of the two numbers for which the LCM will be calculated.
	#[implementations(u32, u64, i32)]
	other_value: T,
) -> T {
	let value = value.to_i128().unwrap();
	let other_value = other_value.to_i128().unwrap();

	if value == 0 || other_value == 0 {
		return T::zero();
	}
	let gcd = binary_gcd(value, other_value);

	T::from_i128((value * other_value).abs() / gcd).unwrap()
}

fn binary_gcd<T: num_traits::int::PrimInt + std::ops::ShrAssign<i32> + std::ops::SubAssign>(mut a: T, mut b: T) -> T {
	if a == T::zero() {
		return b;
	}
	if b == T::zero() {
		return a;
	}

	let mut shift = 0;
	while (a | b) & T::one() == T::zero() {
		a >>= 1;
		b >>= 1;
		shift += 1;
	}

	while a & T::one() == T::zero() {
		a >>= 1;
	}

	while b != T::zero() {
		while b & T::one() == T::zero() {
			b >>= 1;
		}
		if a > b {
			std::mem::swap(&mut a, &mut b);
		}
		b -= a;
	}

	a << shift
}

/// The equality operation (==) compares two values and returns true if they are equal, or false if they are not.
#[node_macro::node(category("Math: Logic"))]
fn equals<T: std::cmp::PartialEq<T>>(
	_: impl Ctx,
	/// One of the two numbers to compare for equality.
	#[implementations(f64, f32, u32, DVec2, &str, String)]
	value: T,
	/// The other of the two numbers to compare for equality.
	#[implementations(f64, f32, u32, DVec2, &str, String)]
	other_value: T,
) -> bool {
	other_value == value
}

/// The inequality operation (!=) compares two values and returns true if they are not equal, or false if they are.
#[node_macro::node(category("Math: Logic"))]
fn not_equals<T: std::cmp::PartialEq<T>>(
	_: impl Ctx,
	/// One of the two numbers to compare for inequality.
	#[implementations(f64, f32, u32, DVec2, &str)]
	value: T,
	/// The other of the two numbers to compare for inequality.
	#[implementations(f64, f32, u32, DVec2, &str)]
	other_value: T,
) -> bool {
	other_value != value
}

/// The less-than operation (<) compares two values and returns true if the first value is less than the second, or false if it is not.
/// If enabled with "Or Equal", the less-than-or-equal operation (<=) will be used instead.
#[node_macro::node(category("Math: Logic"))]
fn less_than<T: std::cmp::PartialOrd<T>>(
	_: impl Ctx,
	/// The number on the left-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	value: T,
	/// The number on the right-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	other_value: T,
	/// Uses the less-than-or-equal operation (<=) instead of the less-than operation (<).
	or_equal: bool,
) -> bool {
	if or_equal { value <= other_value } else { value < other_value }
}

/// The greater-than operation (>) compares two values and returns true if the first value is greater than the second, or false if it is not.
/// If enabled with "Or Equal", the greater-than-or-equal operation (>=) will be used instead.
#[node_macro::node(category("Math: Logic"))]
fn greater_than<T: std::cmp::PartialOrd<T>>(
	_: impl Ctx,
	/// The number on the left-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	value: T,
	/// The number on the right-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	other_value: T,
	/// Uses the greater-than-or-equal operation (>=) instead of the greater-than operation (>).
	or_equal: bool,
) -> bool {
	if or_equal { value >= other_value } else { value > other_value }
}

/// The logical or operation (||) returns true if either of the two inputs are true, or false if both are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_or(
	_: impl Ctx,
	/// One of the two boolean values, either of which may be true for the node to output true.
	value: bool,
	/// The other of the two boolean values, either of which may be true for the node to output true.
	other_value: bool,
) -> bool {
	value || other_value
}

/// The logical and operation (&&) returns true if both of the two inputs are true, or false if any are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_and(
	_: impl Ctx,
	/// One of the two boolean values, both of which must be true for the node to output true.
	value: bool,
	/// The other of the two boolean values, both of which must be true for the node to output true.
	other_value: bool,
) -> bool {
	value && other_value
}

/// The logical not operation (!) reverses true and false value of the input.
#[node_macro::node(category("Math: Logic"))]
fn logical_not(
	_: impl Ctx,
	/// The boolean value to be reversed.
	input: bool,
) -> bool {
	!input
}

/// Constructs a bool value which may be set to true or false.
#[node_macro::node(category("Value"))]
fn bool_value(_: impl Ctx, _primary: (), #[name("Bool")] bool_value: bool) -> bool {
	bool_value
}

/// Constructs a number value which may be set to any real number.
#[node_macro::node(category("Value"))]
fn number_value(_: impl Ctx, _primary: (), number: f64) -> f64 {
	number
}

/// Constructs a number value which may be set to any value from 0% to 100% by dragging the slider.
#[node_macro::node(category("Value"))]
fn percentage_value(_: impl Ctx, _primary: (), percentage: Percentage) -> f64 {
	percentage
}

/// Constructs a two-dimensional vector value which may be set to any XY pair.
#[node_macro::node(category("Value"), name("Vec2 Value"))]
fn vec2_value(_: impl Ctx, _primary: (), x: f64, y: f64) -> DVec2 {
	DVec2::new(x, y)
}

/// Constructs a color value which may be set to any color, or no color.
#[node_macro::node(category("Value"))]
fn color_value(_: impl Ctx, _primary: (), #[default(Color::RED)] color: Table<Color>) -> Table<Color> {
	color
}

/// Constructs a gradient value which may be set to any sequence of color stops to represent the transition between colors.
#[node_macro::node(category("Value"))]
fn gradient_value(_: impl Ctx, _primary: (), gradient: GradientStops) -> GradientStops {
	gradient
}

/// Constructs a gradient value which may be set to any sequence of color stops to represent the transition between colors.
#[node_macro::node(category("Value"))]
fn gradient_table_value(_: impl Ctx, _primary: (), gradient: GradientStops) -> Table<GradientStops> {
	Table::new_from_element(gradient)
}

/// Gets the color at the specified position along the gradient, given a position from 0 (left) to 1 (right).
#[node_macro::node(category("Color"))]
fn sample_gradient(_: impl Ctx, _primary: (), gradient: GradientStops, position: Fraction) -> Table<Color> {
	let position = position.clamp(0., 1.);
	let color = gradient.evaluate(position);
	Table::new_from_element(color)
}

/// Constructs a string value which may be set to any plain text.
#[node_macro::node(category("Value"))]
fn string_value(_: impl Ctx, _primary: (), string: TextArea) -> String {
	string
}

/// Constructs a footprint value which may be set to any transformation of a unit square describing a render area, and a render resolution at least 1x1 integer pixels.
#[node_macro::node(category("Value"))]
fn footprint_value(_: impl Ctx, _primary: (), transform: DAffine2, #[default(100., 100.)] resolution: PixelSize) -> Footprint {
	Footprint {
		transform,
		resolution: resolution.max(DVec2::ONE).as_uvec2(),
		..Default::default()
	}
}

#[node_macro::node(category("Math: Vector"))]
fn dot_product(_: impl Ctx, vector_a: DVec2, vector_b: DVec2) -> f64 {
	vector_a.dot(vector_b)
}

/// Gets the length or magnitude of a vector.
#[node_macro::node(category("Math: Vector"))]
fn length(_: impl Ctx, vector: DVec2) -> f64 {
	vector.length()
}

/// Scales the input vector to unit length while preserving it's direction. This is equivalent to dividing the input vector by it's own magnitude.
///
/// Returns zero when the input vector is zero.
#[node_macro::node(category("Math: Vector"))]
fn normalize(_: impl Ctx, vector: DVec2) -> DVec2 {
	vector.normalize_or_zero()
}

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::Node;
	use graphene_core::generic::FnNode;

	#[test]
	pub fn dot_product_function() {
		let vector_a = DVec2::new(1., 2.);
		let vector_b = DVec2::new(3., 4.);
		assert_eq!(dot_product((), vector_a, vector_b), 11.);
	}

	#[test]
	pub fn length_function() {
		let vector = DVec2::new(3., 4.);
		assert_eq!(length((), vector), 5.);
	}

	#[test]
	fn test_basic_expression() {
		let result = math((), 0., "2 + 2".to_string(), 0.);
		assert_eq!(result, 4.);
	}

	#[test]
	fn test_complex_expression() {
		let result = math((), 0., "(5 * 3) + (10 / 2)".to_string(), 0.);
		assert_eq!(result, 20.);
	}

	#[test]
	fn test_default_expression() {
		let result = math((), 0., "0".to_string(), 0.);
		assert_eq!(result, 0.);
	}

	#[test]
	fn test_invalid_expression() {
		let result = math((), 0., "invalid".to_string(), 0.);
		assert_eq!(result, 0.);
	}

	#[test]
	pub fn foo() {
		let fnn = FnNode::new(|(a, b)| (b, a));
		assert_eq!(fnn.eval((1u32, 2u32)), (2, 1));
	}

	#[test]
	pub fn add_vectors() {
		assert_eq!(super::add((), DVec2::ONE, DVec2::ONE), DVec2::ONE * 2.);
	}

	#[test]
	pub fn subtract_f64() {
		assert_eq!(super::subtract((), 5_f64, 3_f64), 2.);
	}

	#[test]
	pub fn divide_vectors() {
		assert_eq!(super::divide((), DVec2::ONE, 2_f64), DVec2::ONE / 2.);
	}

	#[test]
	pub fn modulo_positive() {
		assert_eq!(super::modulo((), -5_f64, 2_f64, true), 1_f64);
	}

	#[test]
	pub fn modulo_negative() {
		assert_eq!(super::modulo((), -5_f64, 2_f64, false), -1_f64);
	}
}
