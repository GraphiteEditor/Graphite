use core_types::registry::types::{Fraction, Percentage, PixelSize, TextArea};
use core_types::table::Table;
use core_types::transform::Footprint;
use core_types::{Color, Ctx, num_traits};
use glam::{DAffine2, DVec2};
use log::warn;
use math_parser::ast;
use math_parser::context::{EvalContext, NothingMap, ValueProvider};
use math_parser::value::{Number, Value};
use num_traits::Pow;
use rand::{Rng, SeedableRng};
use std::ops::{Add, Div, Mul, Rem, Sub};
use vector_types::GradientStops;

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

/// Calculates a mathematical expression with input values "A" and "B".
#[node_macro::node(category("Math: Arithmetic"), properties("math_properties"))]
fn math<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The value of "A" when calculating the expression.
	#[implementations(f64, f32)]
	operand_a: T,
	/// A math expression that may incorporate "A" and/or "B", such as `sqrt(A + B) - B^2`.
	#[default(A + B)]
	expression: String,
	/// The value of "B" when calculating the expression.
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

/// The addition operation (`+`) calculates the sum of two scalar numbers or vectors.
#[node_macro::node(category("Math: Arithmetic"))]
fn add<A: Add<B>, B>(
	_: impl Ctx,
	/// The left-hand side of the addition operation.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	augend: A,
	/// The right-hand side of the addition operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	addend: B,
) -> <A as Add<B>>::Output {
	augend + addend
}

/// The subtraction operation (`-`) calculates the difference between two scalar numbers or vectors.
#[node_macro::node(category("Math: Arithmetic"))]
fn subtract<A: Sub<B>, B>(
	_: impl Ctx,
	/// The left-hand side of the subtraction operation.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	minuend: A,
	/// The right-hand side of the subtraction operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	subtrahend: B,
) -> <A as Sub<B>>::Output {
	minuend - subtrahend
}

/// The multiplication operation (`×`) calculates the product of two scalar numbers, vectors, or transforms.
#[node_macro::node(category("Math: Arithmetic"))]
fn multiply<A: Mul<B>, B>(
	_: impl Ctx,
	/// The left-hand side of the multiplication operation.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2, DAffine2)]
	multiplier: A,
	/// The right-hand side of the multiplication operation.
	#[default(1.)]
	#[implementations(f64, f32, u32, DVec2, DVec2, f64, DAffine2)]
	multiplicand: B,
) -> <A as Mul<B>>::Output {
	multiplier * multiplicand
}

/// The division operation (`÷`) calculates the quotient of two scalar numbers or vectors.
///
/// Produces 0 if the denominator is 0.
#[node_macro::node(category("Math: Arithmetic"))]
fn divide<A: Div<B> + Default + PartialEq, B: Default + PartialEq>(
	_: impl Ctx,
	/// The left-hand side of the division operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	numerator: A,
	/// The right-hand side of the division operation.
	#[default(1.)]
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	denominator: B,
) -> <A as Div<B>>::Output
where
	<A as Div<B>>::Output: Default,
{
	if denominator == B::default() {
		return <A as Div<B>>::Output::default();
	}
	numerator / denominator
}

/// The reciprocal operation (`1/x`) calculates the multiplicative inverse of a number.
///
/// Produces 0 if the input is 0.
#[node_macro::node(category("Math: Arithmetic"))]
fn reciprocal<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number for which the reciprocal is calculated.
	#[implementations(f64, f32)]
	value: T,
) -> T {
	if value == T::from(0.).unwrap() { T::from(0.).unwrap() } else { T::from(1.).unwrap() / value }
}

/// The modulo operation (`%`) calculates the remainder from the division of two scalar numbers or vectors.
///
/// The sign of the result shares the sign of the numerator unless *Always Positive* is enabled.
#[node_macro::node(category("Math: Arithmetic"))]
fn modulo<A: Rem<B, Output: Add<B, Output: Rem<B, Output = A::Output>>>, B: Copy>(
	_: impl Ctx,
	/// The left-hand side of the modulo operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	numerator: A,
	/// The right-hand side of the modulo operation.
	#[default(2.)]
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	modulus: B,
	/// Ensures the result is always positive, even if the numerator is negative.
	#[default(true)]
	always_positive: bool,
) -> <A as Rem<B>>::Output {
	if always_positive { (numerator % modulus + modulus) % modulus } else { numerator % modulus }
}

/// The exponent operation (`^`) calculates the result of raising a number to a power.
#[node_macro::node(category("Math: Arithmetic"))]
fn exponent<T: Pow<T>>(
	_: impl Ctx,
	/// The base number that is raised to the power.
	#[implementations(f64, f32, u32)]
	base: T,
	/// The power to which the base number is raised.
	#[implementations(f64, f32, u32)]
	#[default(2.)]
	power: T,
) -> <T as num_traits::Pow<T>>::Output {
	base.pow(power)
}

/// The `n`th root operation (`√`) calculates the inverse of exponentiation. Square root inverts squaring, cube root inverts cubing, and so on.
///
/// This is equivalent to raising the number to the power of `1/n`.
#[node_macro::node(category("Math: Arithmetic"))]
fn root<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number inside the radical for which the `n`th root is calculated.
	#[default(2.)]
	#[implementations(f64, f32)]
	radicand: T,
	/// The degree of the root to be calculated. Square root is 2, cube root is 3, and so on.
	/// Degrees 0 or less are invalid and will produce an output of 0.
	#[default(2.)]
	#[implementations(f64, f32)]
	degree: T,
) -> T {
	if degree == T::from(2.).unwrap() {
		radicand.sqrt()
	} else if degree == T::from(3.).unwrap() {
		radicand.cbrt()
	} else if degree <= T::from(0.).unwrap() {
		T::from(0.).unwrap()
	} else {
		radicand.powf(T::from(1.).unwrap() / degree)
	}
}

/// The logarithmic function (`log`) calculates the logarithm of a number with a specified base. If the natural logarithm function (`ln`) is desired, set the base to "e".
#[node_macro::node(category("Math: Arithmetic"))]
fn logarithm<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number for which the logarithm is calculated.
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

/// The sine trigonometric function (`sin`) calculates the ratio of the angle's opposite side length to its hypotenuse length.
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

/// The cosine trigonometric function (`cos`) calculates the ratio of the angle's adjacent side length to its hypotenuse length.
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

/// The tangent trigonometric function (`tan`) calculates the ratio of the angle's opposite side length to its adjacent side length.
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

/// The inverse sine trigonometric function (`asin`) calculates the angle whose sine is the input value.
#[node_macro::node(category("Math: Trig"))]
fn sine_inverse<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The given value for which the angle is calculated. Must be in the domain `[-1, 1]` (it will be clamped to -1 or 1 otherwise).
	#[implementations(f64, f32)]
	value: T,
	/// Whether the resulting angle should be given in as radians instead of degrees.
	radians: bool,
) -> T {
	let angle = value.clamp(T::from(-1.).unwrap(), T::from(1.).unwrap()).asin();
	if radians { angle } else { angle.to_degrees() }
}

/// The inverse cosine trigonometric function (`acos`) calculates the angle whose cosine is the input value.
#[node_macro::node(category("Math: Trig"))]
fn cosine_inverse<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The given value for which the angle is calculated. Must be in the domain `[-1, 1]` (it will be clamped to -1 or 1 otherwise).
	#[implementations(f64, f32)]
	value: T,
	/// Whether the resulting angle should be given in as radians instead of degrees.
	radians: bool,
) -> T {
	let angle = value.clamp(T::from(-1.).unwrap(), T::from(1.).unwrap()).acos();
	if radians { angle } else { angle.to_degrees() }
}

/// The inverse tangent trigonometric function (`atan` or `atan2`, depending on input type) calculates:
/// `atan`: the angle whose tangent is the input scalar number.
/// `atan2`: the angle of a ray from the origin to the input vec2.
///
/// The resulting angle is always in the range `[-90°, 90°]` or, in radians, `[-π/2, π/2]`.
#[node_macro::node(category("Math: Trig"))]
fn tangent_inverse<T: TangentInverse>(
	_: impl Ctx,
	/// The given value for which the angle is calculated.
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

/// Linearly maps an input value from one range to another. The ranges may be reversed.
///
/// For example, 0.5 in the input range `[0, 1]` would map to 0 in the output range `[-180, 180]`.
#[node_macro::node(category("Math: Numeric"))]
fn remap<U: num_traits::float::Float>(
	_: impl Ctx,
	/// The value to be mapped between ranges.
	#[implementations(f64, f32)]
	value: U,
	/// The lower bound of the input range.
	#[implementations(f64, f32)]
	input_min: U,
	/// The upper bound of the input range.
	#[implementations(f64, f32)]
	#[default(1.)]
	input_max: U,
	/// The lower bound of the output range.
	#[implementations(f64, f32)]
	output_min: U,
	/// The upper bound of the output range.
	#[implementations(f64, f32)]
	#[default(1.)]
	output_max: U,
	/// Whether to constrain the result within the output range instead of extrapolating beyond its bounds.
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

/// The random function (`rand`) converts a seed into a random number within the specified range, inclusive of the minimum and exclusive of the maximum. The minimum and maximum values are automatically swapped if they are reversed.
#[node_macro::node(category("Math: Numeric"))]
fn random(
	_: impl Ctx,
	_primary: (),
	/// Seed to determine the unique variation of which number is generated.
	seed: u64,
	/// The smaller end of the range within which the random number is generated.
	min: f64,
	/// The larger end of the range within which the random number is generated.
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

/// The rounding function (`round`) maps an input value to its nearest whole number. Halfway values are rounded away from zero.
#[node_macro::node(category("Math: Numeric"))]
fn round<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number to be rounded to the nearest whole number.
	#[implementations(f64, f32)]
	value: T,
) -> T {
	value.round()
}

/// The floor function (`floor`) rounds down an input value to the nearest whole number, unless the input number is already whole.
#[node_macro::node(category("Math: Numeric"))]
fn floor<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number to be rounded down.
	#[implementations(f64, f32)]
	value: T,
) -> T {
	value.floor()
}

/// The ceiling function (`ceil`) rounds up an input value to the nearest whole number, unless the input number is already whole.
#[node_macro::node(category("Math: Numeric"))]
fn ceiling<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The number to be rounded up.
	#[implementations(f64, f32)]
	value: T,
) -> T {
	value.ceil()
}

trait AbsoluteValue {
	fn abs(self) -> Self;
}
impl AbsoluteValue for DVec2 {
	fn abs(self) -> Self {
		DVec2::new(self.x.abs(), self.y.abs())
	}
}
impl AbsoluteValue for f32 {
	fn abs(self) -> Self {
		self.abs()
	}
}
impl AbsoluteValue for f64 {
	fn abs(self) -> Self {
		self.abs()
	}
}
impl AbsoluteValue for i32 {
	fn abs(self) -> Self {
		self.abs()
	}
}
impl AbsoluteValue for i64 {
	fn abs(self) -> Self {
		self.abs()
	}
}

/// The absolute value function (`abs`) removes the negative sign from an input value, if present.
#[node_macro::node(category("Math: Numeric"))]
fn absolute_value<T: AbsoluteValue>(
	_: impl Ctx,
	/// The number to be made positive.
	#[implementations(f64, f32, i32, i64, DVec2)]
	value: T,
) -> T {
	value.abs()
}

/// The minimum function (`min`) picks the smaller of two numbers.
#[node_macro::node(category("Math: Numeric"))]
fn min<T: std::cmp::PartialOrd>(
	_: impl Ctx,
	/// One of the two numbers, of which the lesser is returned.
	#[implementations(f64, f32, u32, &str)]
	value: T,
	/// The other of the two numbers, of which the lesser is returned.
	#[implementations(f64, f32, u32, &str)]
	other_value: T,
) -> T {
	if value < other_value { value } else { other_value }
}

/// The maximum function (`max`) picks the larger of two numbers.
#[node_macro::node(category("Math: Numeric"))]
fn max<T: std::cmp::PartialOrd>(
	_: impl Ctx,
	/// One of the two numbers, of which the greater is returned.
	#[implementations(f64, f32, u32, &str)]
	value: T,
	/// The other of the two numbers, of which the greater is returned.
	#[implementations(f64, f32, u32, &str)]
	other_value: T,
) -> T {
	if value > other_value { value } else { other_value }
}

/// The clamp function (`clamp`) restricts a number to a specified range between a minimum and maximum value. The minimum and maximum values are automatically swapped if they are reversed.
#[node_macro::node(category("Math: Numeric"))]
fn clamp<T: std::cmp::PartialOrd>(
	_: impl Ctx,
	/// The number to be clamped, which is restricted to the range between the minimum and maximum values.
	#[implementations(f64, f32, u32, &str)]
	value: T,
	/// The left (smaller) side of the range. The output is never less than this number.
	#[implementations(f64, f32, u32, &str)]
	min: T,
	/// The right (greater) side of the range. The output is never greater than this number.
	#[implementations(f64, f32, u32, &str)]
	#[default(1)]
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
	/// One of the two numbers for which the GCD is calculated.
	#[implementations(u32, u64, i32)]
	value: T,
	/// The other of the two numbers for which the GCD is calculated.
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
	/// One of the two numbers for which the LCM is calculated.
	#[implementations(u32, u64, i32)]
	value: T,
	/// The other of the two numbers for which the LCM is calculated.
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

/// The less-than operation (`<`) compares two values and returns true if the first value is less than the second, or false if it is not.
/// If enabled with *Or Equal*, the less-than-or-equal operation (`<=`) is used instead.
#[node_macro::node(category("Math: Logic"))]
fn less_than<T: std::cmp::PartialOrd<T>>(
	_: impl Ctx,
	/// The number on the left-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	value: T,
	/// The number on the right-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	other_value: T,
	/// Uses the less-than-or-equal operation (`<=`) instead of the less-than operation (`<`).
	or_equal: bool,
) -> bool {
	if or_equal { value <= other_value } else { value < other_value }
}

/// The greater-than operation (`>`) compares two values and returns true if the first value is greater than the second, or false if it is not.
/// If enabled with *Or Equal*, the greater-than-or-equal operation (`>=`) is used instead.
#[node_macro::node(category("Math: Logic"))]
fn greater_than<T: std::cmp::PartialOrd<T>>(
	_: impl Ctx,
	/// The number on the left-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	value: T,
	/// The number on the right-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	other_value: T,
	/// Uses the greater-than-or-equal operation (`>=`) instead of the greater-than operation (`>`).
	or_equal: bool,
) -> bool {
	if or_equal { value >= other_value } else { value > other_value }
}

/// The equality operation (`==`, `XNOR`) compares two values and returns true if they are equal, or false if they are not.
#[node_macro::node(category("Math: Logic"))]
fn equals<T: std::cmp::PartialEq<T>>(
	_: impl Ctx,
	/// One of the two values to compare for equality.
	#[implementations(f64, f32, u32, DVec2, bool, &str, String)]
	value: T,
	/// The other of the two values to compare for equality.
	#[implementations(f64, f32, u32, DVec2, bool, &str, String)]
	other_value: T,
) -> bool {
	other_value == value
}

/// The inequality operation (`!=`, `XOR`) compares two values and returns true if they are not equal, or false if they are.
#[node_macro::node(category("Math: Logic"))]
fn not_equals<T: std::cmp::PartialEq<T>>(
	_: impl Ctx,
	/// One of the two values to compare for inequality.
	#[implementations(f64, f32, u32, DVec2, bool, &str)]
	value: T,
	/// The other of the two values to compare for inequality.
	#[implementations(f64, f32, u32, DVec2, bool, &str)]
	other_value: T,
) -> bool {
	other_value != value
}

/// The logical OR operation (`||`) returns true if either of the two inputs are true, or false if both are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_or(
	_: impl Ctx,
	/// One of the two boolean values, either of which may be true for the node to output true.
	value: bool,
	/// The other of the two boolean values, either of which may be true for the node to output true.
	#[expose]
	other_value: bool,
) -> bool {
	value || other_value
}

/// The logical AND operation (`&&`) returns true if both of the two inputs are true, or false if any are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_and(
	_: impl Ctx,
	/// One of the two boolean values, both of which must be true for the node to output true.
	value: bool,
	/// The other of the two boolean values, both of which must be true for the node to output true.
	#[expose]
	other_value: bool,
) -> bool {
	value && other_value
}

/// The logical NOT operation (`!`) reverses true and false value of the input.
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
fn color_value(_: impl Ctx, _primary: (), #[default(Color::BLACK)] color: Table<Color>) -> Table<Color> {
	color
}

/// Constructs a color value from red, green, blue, and alpha components given as numbers from 0 to 1.
#[node_macro::node(category("Color"), name("RGBA to Color"))]
fn rgba_to_color(_: impl Ctx, _primary: (), red: Fraction, green: Fraction, blue: Fraction, #[default(1.)] alpha: Fraction) -> Table<Color> {
	let red = (red as f32).clamp(0., 1.);
	let green = (green as f32).clamp(0., 1.);
	let blue = (blue as f32).clamp(0., 1.);
	let alpha = (alpha as f32).clamp(0., 1.);

	Table::new_from_element(Color::from_rgbaf32_unchecked(red, green, blue, alpha))
}

/// Constructs a color value from hue, saturation, value, and alpha components given as numbers from 0 to 1.
#[node_macro::node(category("Color"), name("HSVA to Color"))]
fn hsva_to_color(_: impl Ctx, _primary: (), hue: Fraction, #[default(1.)] saturation: Fraction, #[default(1.)] value: Fraction, #[default(1.)] alpha: Fraction) -> Table<Color> {
	let hue = (hue as f32) - (hue as f32).floor();
	let saturation = (saturation as f32).clamp(0., 1.);
	let value = (value as f32).clamp(0., 1.);
	let alpha = (alpha as f32).clamp(0., 1.);

	Table::new_from_element(Color::from_hsva(hue, saturation, value, alpha))
}

/// Constructs a color value from hue, saturation, lightness, and alpha components given as numbers from 0 to 1.
#[node_macro::node(category("Color"), name("HSLA to Color"))]
fn hsla_to_color(_: impl Ctx, _primary: (), hue: Fraction, #[default(1.)] saturation: Fraction, #[default(0.5)] lightness: Fraction, #[default(1.)] alpha: Fraction) -> Table<Color> {
	let hue = (hue as f32) - (hue as f32).floor();
	let saturation = (saturation as f32).clamp(0., 1.);
	let lightness = (lightness as f32).clamp(0., 1.);
	let alpha = (alpha as f32).clamp(0., 1.);

	Table::new_from_element(Color::from_hsla(hue, saturation, lightness, alpha))
}

/// Constructs a color value from an sRGB color code string, such as `#RRGGBB` or `#RRGGBBAA`. Invalid hex code strings produce no color.
#[node_macro::node(category("Color"), name("Hex to Color"))]
fn hex_to_color(_: impl Ctx, hex_code: String) -> Table<Color> {
	match Color::from_hex_str(&hex_code) {
		Some(c) => Table::new_from_element(c),
		None => Table::new(),
	}
}

/// Constructs a gradient value which may be set to any sequence of color stops to represent the transition between colors.
#[node_macro::node(category("Value"))]
fn gradient_value(_: impl Ctx, _primary: (), gradient: Table<GradientStops>) -> Table<GradientStops> {
	gradient
}

/// Gets the color at the specified position along the gradient, given a position from 0 (left) to 1 (right).
#[node_macro::node(category("Color"))]
fn sample_gradient(_: impl Ctx, _primary: (), gradient: Table<GradientStops>, position: Fraction) -> Table<Color> {
	let Some(row) = gradient.get(0) else { return Table::new() };

	let position = position.clamp(0., 1.);
	let color = row.element.evaluate(position);
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

/// The dot product operation (`·`) calculates the degree of similarity of a vec2 pair based on their angles and lengths.
///
/// Calculated as `‖a‖‖b‖cos(θ)`, it represents the product of their lengths (`‖a‖‖b‖`) scaled by the alignment of their directions (`cos(θ)`).
/// The output ranges from the positive to negative product of their lengths based on when they are pointing in the same or opposite directions.
/// If any vector has zero length, the output is 0.
#[node_macro::node(category("Math: Vector"))]
fn dot_product(
	_: impl Ctx,
	/// An operand of the dot product operation.
	vector_a: DVec2,
	/// The other operand of the dot product operation.
	#[default(1., 0.)]
	vector_b: DVec2,
	/// Whether to normalize both input vectors so the calculation ranges in `[-1, 1]` by considering only their degree of directional alignment.
	normalize: bool,
) -> f64 {
	if normalize {
		vector_a.normalize_or_zero().dot(vector_b.normalize_or_zero())
	} else {
		vector_a.dot(vector_b)
	}
}

/// Calculates the angle swept between two vectors.
///
/// The value is always positive and ranges from 0° (both vectors point the same direction) to 180° (both vectors point opposite directions).
#[node_macro::node(category("Math: Vector"))]
fn angle_between(_: impl Ctx, vector_a: DVec2, vector_b: DVec2, radians: bool) -> f64 {
	let dot_product = vector_a.normalize_or_zero().dot(vector_b.normalize_or_zero());
	let angle = dot_product.acos();
	if radians { angle } else { angle.to_degrees() }
}

pub trait ToPosition {
	fn to_position(self) -> DVec2;
}
impl ToPosition for DVec2 {
	fn to_position(self) -> DVec2 {
		self
	}
}
impl ToPosition for DAffine2 {
	fn to_position(self) -> DVec2 {
		self.translation
	}
}

/// Calculates the angle needed for a rightward-facing object placed at the observer position to turn so it points toward the target position.
#[node_macro::node(category("Math: Vector"))]
fn angle_to<T: ToPosition, U: ToPosition>(
	_: impl Ctx,
	/// The position from which the angle is measured.
	#[implementations(DVec2, DAffine2, DVec2, DAffine2)]
	observer: T,
	/// The position toward which the angle is measured.
	#[expose]
	#[implementations(DVec2, DVec2, DAffine2, DAffine2)]
	target: U,
	/// Whether the resulting angle should be given in radians instead of degrees.
	radians: bool,
) -> f64 {
	let from = observer.to_position();
	let to = target.to_position();
	let delta = to - from;
	let angle = delta.y.atan2(delta.x);
	if radians { angle } else { angle.to_degrees() }
}

// TODO: Rename to "Magnitude"
/// The magnitude operator (`‖x‖`) calculates the length of a vec2, which is the distance from the base to the tip of the arrow represented by the vector.
#[node_macro::node(category("Math: Vector"))]
fn length(_: impl Ctx, vector: DVec2) -> f64 {
	vector.length()
}

/// Scales the input vector to unit length while preserving its direction. This is equivalent to dividing the input vector by its own magnitude.
///
/// Returns 0 when the input vector has zero length.
#[node_macro::node(category("Math: Vector"))]
fn normalize(_: impl Ctx, vector: DVec2) -> DVec2 {
	vector.normalize_or_zero()
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::Node;
	use core_types::generic::FnNode;

	#[test]
	pub fn dot_product_function() {
		let vector_a = DVec2::new(1., 2.);
		let vector_b = DVec2::new(3., 4.);
		assert_eq!(dot_product((), vector_a, vector_b, false), 11.);
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
