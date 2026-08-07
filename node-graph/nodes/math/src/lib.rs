use core_types::attribute::Attr;
use core_types::gpoll::{GraphError, Interrupt};
use core_types::list::List;
use core_types::registry::types::{Fraction, Percentage, PixelSize};
use core_types::transform::Footprint;
use core_types::{Color, Ctx, ExtractIndex, InjectIndex, num_traits};
use glam::{DAffine2, DVec2};
use log::warn;
use math_parser::ast;
use math_parser::context::{EvalContext, NothingMap, ValueProvider};
use math_parser::value::{Number, Value};
use rand::{Rng, SeedableRng};
use std::ops::{Add, Mul, Rem, Sub};
use vector_types::Gradient;
use vector_types::markers::{
	GradientCyclic as GradientCyclicAttr, GradientForm as GradientFormAttr, GradientHueDirection as GradientHueDirectionAttr, GradientInterpolation as GradientInterpolationAttr,
	GradientSpace as GradientSpaceAttr, GradientSpread as GradientSpreadAttr,
};

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
	#[default("A + B")]
	expression: String,
	/// The value of "B" when calculating the expression.
	#[implementations(f64, f32)]
	#[default(1.)]
	operand_b: T,
) -> T {
	let node = match ast::Node::try_parse_from_str(&expression) {
		Ok(expr) => expr,
		Err(e) => {
			warn!("Invalid expression: `{expression}`\n{e}");
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

/// The addition operation (`+`) calculates the sum of two scalar numbers or vec2s.
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

/// The subtraction operation (`-`) calculates the difference between two scalar numbers or vec2s.
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

/// The multiplication operation (`×`) calculates the product of two scalar numbers, vec2s, or transforms.
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

pub trait SafeDivide<Rhs = Self> {
	type Output;
	fn safe_divide(self, denominator: Rhs) -> Self::Output;
}
impl SafeDivide for f64 {
	type Output = f64;
	fn safe_divide(self, denominator: f64) -> f64 {
		if denominator == 0. { 0. } else { self / denominator }
	}
}
impl SafeDivide for f32 {
	type Output = f32;
	fn safe_divide(self, denominator: f32) -> f32 {
		if denominator == 0. { 0. } else { self / denominator }
	}
}
impl SafeDivide for u32 {
	type Output = u32;
	fn safe_divide(self, denominator: u32) -> u32 {
		self.checked_div(denominator).unwrap_or(0)
	}
}
impl SafeDivide for DVec2 {
	type Output = DVec2;
	fn safe_divide(self, denominator: DVec2) -> DVec2 {
		DVec2::new(self.x.safe_divide(denominator.x), self.y.safe_divide(denominator.y))
	}
}
impl SafeDivide<f64> for DVec2 {
	type Output = DVec2;
	fn safe_divide(self, denominator: f64) -> DVec2 {
		DVec2::new(self.x.safe_divide(denominator), self.y.safe_divide(denominator))
	}
}
impl SafeDivide<DVec2> for f64 {
	type Output = DVec2;
	fn safe_divide(self, denominator: DVec2) -> DVec2 {
		DVec2::new(self.safe_divide(denominator.x), self.safe_divide(denominator.y))
	}
}

/// The division operation (`÷`) calculates the quotient of two scalar numbers or vec2s.
///
/// Produces 0 for any division by 0. With vec2 inputs, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Arithmetic"))]
fn divide<A: SafeDivide<B>, B>(
	_: impl Ctx,
	/// The left-hand side of the division operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	numerator: A,
	/// The right-hand side of the division operation.
	#[default(1.)]
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	denominator: B,
) -> <A as SafeDivide<B>>::Output {
	numerator.safe_divide(denominator)
}

trait Componentwise {
	fn componentwise(self, f: impl Fn(f64) -> f64) -> Self;
}
impl Componentwise for f64 {
	fn componentwise(self, f: impl Fn(f64) -> f64) -> Self {
		f(self)
	}
}
impl Componentwise for f32 {
	fn componentwise(self, f: impl Fn(f64) -> f64) -> Self {
		f(self as f64) as f32
	}
}
impl Componentwise for DVec2 {
	fn componentwise(self, f: impl Fn(f64) -> f64) -> Self {
		DVec2::new(f(self.x), f(self.y))
	}
}

/// The reciprocal operation (`1/x`) calculates the multiplicative inverse of a number.
///
/// Produces 0 if the input is 0. With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Arithmetic"))]
fn reciprocal<T: Componentwise>(
	_: impl Ctx,
	/// The number for which the reciprocal is calculated.
	#[implementations(f64, f32, DVec2)]
	value: T,
) -> T {
	value.componentwise(|value| if value == 0. { 0. } else { 1. / value })
}

/// The modulo operation (`%`) calculates the remainder from the division of two scalar numbers or vec2s.
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

pub trait Exponent<Rhs = Self> {
	type Output;
	fn power(self, power: Rhs) -> Self::Output;
}
impl Exponent for f64 {
	type Output = f64;
	fn power(self, power: f64) -> f64 {
		self.powf(power)
	}
}
impl Exponent for f32 {
	type Output = f32;
	fn power(self, power: f32) -> f32 {
		self.powf(power)
	}
}
impl Exponent for u32 {
	type Output = u32;
	fn power(self, power: u32) -> u32 {
		self.pow(power)
	}
}
impl Exponent for DVec2 {
	type Output = DVec2;
	fn power(self, power: DVec2) -> DVec2 {
		DVec2::new(self.x.powf(power.x), self.y.powf(power.y))
	}
}
impl Exponent<f64> for DVec2 {
	type Output = DVec2;
	fn power(self, power: f64) -> DVec2 {
		DVec2::new(self.x.powf(power), self.y.powf(power))
	}
}
impl Exponent<DVec2> for f64 {
	type Output = DVec2;
	fn power(self, power: DVec2) -> DVec2 {
		DVec2::new(self.powf(power.x), self.powf(power.y))
	}
}

/// The exponent operation (`^`) calculates the result of raising a number to a power.
///
/// With vec2 inputs, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Arithmetic"))]
fn exponent<A: Exponent<B>, B>(
	_: impl Ctx,
	/// The base number that is raised to the power.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	base: A,
	/// The power to which the base number is raised.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	#[default(2.)]
	power: B,
) -> <A as Exponent<B>>::Output {
	base.power(power)
}

fn scalar_nth_root(radicand: f64, degree: f64) -> f64 {
	if degree == 2. {
		radicand.sqrt()
	} else if degree == 3. {
		radicand.cbrt()
	} else if degree <= 0. {
		0.
	} else {
		radicand.powf(1. / degree)
	}
}

pub trait NthRoot<Degree = Self> {
	type Output;
	fn nth_root(self, degree: Degree) -> Self::Output;
}
impl NthRoot for f64 {
	type Output = f64;
	fn nth_root(self, degree: f64) -> f64 {
		scalar_nth_root(self, degree)
	}
}
impl NthRoot for f32 {
	type Output = f32;
	fn nth_root(self, degree: f32) -> f32 {
		scalar_nth_root(self as f64, degree as f64) as f32
	}
}
impl NthRoot for DVec2 {
	type Output = DVec2;
	fn nth_root(self, degree: DVec2) -> DVec2 {
		DVec2::new(scalar_nth_root(self.x, degree.x), scalar_nth_root(self.y, degree.y))
	}
}
impl NthRoot<f64> for DVec2 {
	type Output = DVec2;
	fn nth_root(self, degree: f64) -> DVec2 {
		DVec2::new(scalar_nth_root(self.x, degree), scalar_nth_root(self.y, degree))
	}
}
impl NthRoot<DVec2> for f64 {
	type Output = DVec2;
	fn nth_root(self, degree: DVec2) -> DVec2 {
		DVec2::new(scalar_nth_root(self, degree.x), scalar_nth_root(self, degree.y))
	}
}

/// The `n`th root operation (`√`) calculates the inverse of exponentiation. Square root inverts squaring, cube root inverts cubing, and so on.
///
/// This is equivalent to raising the number to the power of `1/n`. With vec2 inputs, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Arithmetic"))]
fn root<A: NthRoot<B>, B>(
	_: impl Ctx,
	/// The number inside the radical for which the `n`th root is calculated.
	#[default(2.)]
	#[implementations(f64, f32, DVec2, DVec2, f64)]
	radicand: A,
	/// The degree of the root to be calculated. Square root is 2, cube root is 3, and so on.
	/// Degrees 0 or less are invalid and will produce an output of 0.
	#[default(2.)]
	#[implementations(f64, f32, f64, DVec2, DVec2)]
	degree: B,
) -> <A as NthRoot<B>>::Output {
	radicand.nth_root(degree)
}

fn scalar_logarithm(value: f64, base: f64) -> f64 {
	if base == 2. {
		value.log2()
	} else if base == 10. {
		value.log10()
	} else if (base - std::f64::consts::E).abs() < f64::EPSILON * 1e6 {
		value.ln()
	} else {
		value.log(base)
	}
}

pub trait Logarithm<Base = Self> {
	type Output;
	fn logarithm(self, base: Base) -> Self::Output;
}
impl Logarithm for f64 {
	type Output = f64;
	fn logarithm(self, base: f64) -> f64 {
		scalar_logarithm(self, base)
	}
}
impl Logarithm for f32 {
	type Output = f32;
	fn logarithm(self, base: f32) -> f32 {
		// The f32 representation of e widens inexactly, so match it against e at f32 precision and substitute the exact f64 e
		let base = if (base - std::f32::consts::E).abs() < f32::EPSILON * 10. {
			std::f64::consts::E
		} else {
			base as f64
		};
		scalar_logarithm(self as f64, base) as f32
	}
}
impl Logarithm for DVec2 {
	type Output = DVec2;
	fn logarithm(self, base: DVec2) -> DVec2 {
		DVec2::new(scalar_logarithm(self.x, base.x), scalar_logarithm(self.y, base.y))
	}
}
impl Logarithm<f64> for DVec2 {
	type Output = DVec2;
	fn logarithm(self, base: f64) -> DVec2 {
		DVec2::new(scalar_logarithm(self.x, base), scalar_logarithm(self.y, base))
	}
}
impl Logarithm<DVec2> for f64 {
	type Output = DVec2;
	fn logarithm(self, base: DVec2) -> DVec2 {
		DVec2::new(scalar_logarithm(self, base.x), scalar_logarithm(self, base.y))
	}
}

/// The logarithmic function (`log`) calculates the logarithm of a number with a specified base. If the natural logarithm function (`ln`) is desired, set the base to "e".
///
/// With vec2 inputs, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Arithmetic"))]
fn logarithm<A: Logarithm<B>, B>(
	_: impl Ctx,
	/// The number for which the logarithm is calculated.
	#[implementations(f64, f32, DVec2, DVec2, f64)]
	value: A,
	/// The base of the logarithm, such as 2 (binary), 10 (decimal), and e (natural logarithm).
	#[default(2.)]
	#[implementations(f64, f32, f64, DVec2, DVec2)]
	base: B,
) -> <A as Logarithm<B>>::Output {
	value.logarithm(base)
}

/// The sine trigonometric function (`sin`) calculates the ratio of the angle's opposite side length to its hypotenuse length.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Trig"))]
fn sine<T: Componentwise>(
	_: impl Ctx,
	/// The given angle.
	#[implementations(f64, f32, DVec2)]
	theta: T,
	/// Whether the given angle should be interpreted as radians instead of degrees.
	radians: bool,
) -> T {
	theta.componentwise(|theta| if radians { theta.sin() } else { theta.to_radians().sin() })
}

/// The cosine trigonometric function (`cos`) calculates the ratio of the angle's adjacent side length to its hypotenuse length.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Trig"))]
fn cosine<T: Componentwise>(
	_: impl Ctx,
	/// The given angle.
	#[implementations(f64, f32, DVec2)]
	theta: T,
	/// Whether the given angle should be interpreted as radians instead of degrees.
	radians: bool,
) -> T {
	theta.componentwise(|theta| if radians { theta.cos() } else { theta.to_radians().cos() })
}

/// The tangent trigonometric function (`tan`) calculates the ratio of the angle's opposite side length to its adjacent side length.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Trig"))]
fn tangent<T: Componentwise>(
	_: impl Ctx,
	/// The given angle.
	#[implementations(f64, f32, DVec2)]
	theta: T,
	/// Whether the given angle should be interpreted as radians instead of degrees.
	radians: bool,
) -> T {
	theta.componentwise(|theta| if radians { theta.tan() } else { theta.to_radians().tan() })
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

trait Lerp {
	fn lerp(self, end: Self, factor: f64) -> Self;
}
impl Lerp for f64 {
	fn lerp(self, end: Self, factor: f64) -> Self {
		self * (1. - factor) + end * factor
	}
}
impl Lerp for f32 {
	fn lerp(self, end: Self, factor: f64) -> Self {
		(self as f64 * (1. - factor) + end as f64 * factor) as f32
	}
}
impl Lerp for DVec2 {
	fn lerp(self, end: Self, factor: f64) -> Self {
		self * (1. - factor) + end * factor
	}
}

/// Linearly interpolates between the start and end values, where a factor of 0 gives the start value, 1 gives the end value, and 0.5 gives their midpoint.
///
/// With vec2 inputs, this traces the straight line path between the two points.
#[node_macro::node(category("Math: Numeric"))]
fn lerp<T: Lerp>(
	_: impl Ctx,
	/// The value produced when the factor is 0.
	#[implementations(f64, f32, DVec2)]
	start: T,
	/// The value produced when the factor is 1.
	#[default(1.)]
	#[implementations(f64, f32, DVec2)]
	end: T,
	/// The mix between the start (at 0) and end (at 1) values.
	#[default(0.5)]
	factor: f64,
	/// Whether to constrain the factor within 0 to 1, preventing extrapolation beyond the start and end values.
	#[default(true)]
	clamped: bool,
) -> T {
	let factor = if clamped { factor.clamp(0., 1.) } else { factor };

	// Exact endpoint factors pass the endpoint through untouched, since the unused operand would otherwise contaminate the weighted sum (NaN or infinity times 0 is NaN)
	if factor == 0. {
		start
	} else if factor == 1. {
		end
	} else {
		start.lerp(end, factor)
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
#[node_macro::node(name("As u32"), category("Debug"))]
fn as_u32(_: impl Ctx, value: u32) -> u32 {
	value
}

// TODO: Test that these are no longer needed in all circumstances, then remove them and add a migration to convert these into Passthrough nodes. Note: these act more as type annotations than as identity functions.
/// Convert a number to an integer of the type u64, which may be the required type for certain node inputs.
#[node_macro::node(name("As u64"), category("Debug"))]
fn as_u64(_: impl Ctx, value: u64) -> u64 {
	value
}

// TODO: Test that these are no longer needed in all circumstances, then remove them and add a migration to convert these into Passthrough nodes. Note: these act more as type annotations than as identity functions.
/// Convert an integer to a decimal number of the type f64, which may be the required type for certain node inputs.
#[node_macro::node(name("As f64"), category("Debug"))]
fn as_f64(_: impl Ctx, value: f64) -> f64 {
	value
}

/// The rounding function (`round`) maps an input value to its nearest whole number. Halfway values are rounded away from zero.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn round<T: Componentwise>(
	_: impl Ctx,
	/// The number to be rounded to the nearest whole number.
	#[implementations(f64, f32, DVec2)]
	value: T,
) -> T {
	value.componentwise(f64::round)
}

/// The floor function (`floor`) rounds down an input value to the nearest whole number, unless the input number is already whole.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn floor<T: Componentwise>(
	_: impl Ctx,
	/// The number to be rounded down.
	#[implementations(f64, f32, DVec2)]
	value: T,
) -> T {
	value.componentwise(f64::floor)
}

/// The ceiling function (`ceil`) rounds up an input value to the nearest whole number, unless the input number is already whole.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn ceiling<T: Componentwise>(
	_: impl Ctx,
	/// The number to be rounded up.
	#[implementations(f64, f32, DVec2)]
	value: T,
) -> T {
	value.componentwise(f64::ceil)
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
///
/// With a vec2 input, this applies separately to the X and Y components. For the overall length of a vec2, see the "Magnitude" node instead.
#[node_macro::node(category("Math: Numeric"))]
fn absolute_value<T: AbsoluteValue>(
	_: impl Ctx,
	/// The number to be made positive.
	#[implementations(f64, f32, i32, i64, DVec2)]
	value: T,
) -> T {
	value.abs()
}

/// The sign function (`sign`) reports whether an input value is positive (1), negative (-1), or zero (0).
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn sign<T: Componentwise>(
	_: impl Ctx,
	/// The number whose sign is checked.
	#[implementations(f64, f32, DVec2)]
	value: T,
) -> T {
	value.componentwise(|value| {
		if value > 0. {
			1.
		} else if value < 0. {
			-1.
		} else {
			0.
		}
	})
}

pub trait MinMax<Rhs = Self> {
	type Output;
	fn minimum(self, other: Rhs) -> Self::Output;
	fn maximum(self, other: Rhs) -> Self::Output;
}
impl MinMax for f64 {
	type Output = f64;
	fn minimum(self, other: f64) -> f64 {
		if self < other { self } else { other }
	}
	fn maximum(self, other: f64) -> f64 {
		if self > other { self } else { other }
	}
}
impl MinMax for f32 {
	type Output = f32;
	fn minimum(self, other: f32) -> f32 {
		if self < other { self } else { other }
	}
	fn maximum(self, other: f32) -> f32 {
		if self > other { self } else { other }
	}
}
impl MinMax for u32 {
	type Output = u32;
	fn minimum(self, other: u32) -> u32 {
		if self < other { self } else { other }
	}
	fn maximum(self, other: u32) -> u32 {
		if self > other { self } else { other }
	}
}
impl MinMax for String {
	type Output = String;
	fn minimum(self, other: Self) -> String {
		if self < other { self } else { other }
	}
	fn maximum(self, other: Self) -> String {
		if self > other { self } else { other }
	}
}
impl MinMax for DVec2 {
	type Output = DVec2;
	fn minimum(self, other: DVec2) -> DVec2 {
		self.min(other)
	}
	fn maximum(self, other: DVec2) -> DVec2 {
		self.max(other)
	}
}
impl MinMax<f64> for DVec2 {
	type Output = DVec2;
	fn minimum(self, other: f64) -> DVec2 {
		self.min(DVec2::splat(other))
	}
	fn maximum(self, other: f64) -> DVec2 {
		self.max(DVec2::splat(other))
	}
}
impl MinMax<DVec2> for f64 {
	type Output = DVec2;
	fn minimum(self, other: DVec2) -> DVec2 {
		DVec2::splat(self).min(other)
	}
	fn maximum(self, other: DVec2) -> DVec2 {
		DVec2::splat(self).max(other)
	}
}

/// The minimum function (`min`) picks the smaller of two numbers.
///
/// With vec2 inputs, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn min<A: MinMax<B>, B>(
	_: impl Ctx,
	/// One of the two numbers, of which the lesser is returned.
	#[implementations(f64, f32, u32, String, DVec2, DVec2, f64)]
	value: A,
	/// The other of the two numbers, of which the lesser is returned.
	#[implementations(f64, f32, u32, String, DVec2, f64, DVec2)]
	other_value: B,
) -> <A as MinMax<B>>::Output {
	value.minimum(other_value)
}

/// The maximum function (`max`) picks the larger of two numbers.
///
/// With vec2 inputs, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn max<A: MinMax<B>, B>(
	_: impl Ctx,
	/// One of the two numbers, of which the greater is returned.
	#[implementations(f64, f32, u32, String, DVec2, DVec2, f64)]
	value: A,
	/// The other of the two numbers, of which the greater is returned.
	#[implementations(f64, f32, u32, String, DVec2, f64, DVec2)]
	other_value: B,
) -> <A as MinMax<B>>::Output {
	value.maximum(other_value)
}

/// The clamp function (`clamp`) restricts a number to a specified range between a minimum and maximum value. The minimum and maximum values are automatically swapped if they are reversed.
///
/// With vec2 inputs, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn clamp<A: MinMax<B>, B: MinMax<Output = B> + Clone>(
	_: impl Ctx,
	/// The number to be clamped, which is restricted to the range between the minimum and maximum values.
	#[implementations(f64, f32, u32, String, DVec2, DVec2, f64)]
	value: A,
	/// The left (smaller) side of the range. The output is never less than this number.
	#[implementations(f64, f32, u32, String, DVec2, f64, DVec2)]
	min: B,
	/// The right (greater) side of the range. The output is never greater than this number.
	#[implementations(f64, f32, u32, String, DVec2, f64, DVec2)]
	#[default(1)]
	max: B,
) -> <A as MinMax<B>>::Output
where
	<A as MinMax<B>>::Output: MinMax<B, Output = <A as MinMax<B>>::Output>,
{
	let (min, max) = (min.clone().minimum(max.clone()), min.maximum(max));
	value.maximum(min).minimum(max)
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

/// Adds together all the numbers in the input list, producing their total.
#[node_macro::node(category("Math: Numeric"))]
fn sum(_: impl Ctx, values: List<f64>) -> f64 {
	values.iter_element_values().sum()
}

/// Averages all the numbers in the input list. An empty list gives 0.
#[node_macro::node(category("Math: Numeric"))]
fn average(_: impl Ctx, values: List<f64>) -> f64 {
	let count = values.len();
	let average = if count == 0 { 0. } else { values.iter_element_values().sum::<f64>() / count as f64 };

	average
}

/// Gives the smallest number in the input list. An empty list gives 0.
#[node_macro::node(category("Math: Numeric"))]
fn minimum(_: impl Ctx, values: List<f64>) -> f64 {
	values.iter_element_values().copied().reduce(f64::min).unwrap_or_default()
}

/// Gives the largest number in the input list. An empty list gives 0.
#[node_macro::node(category("Math: Numeric"))]
fn maximum(_: impl Ctx, values: List<f64>) -> f64 {
	values.iter_element_values().copied().reduce(f64::max).unwrap_or_default()
}

/// Outputs true if at least one value in the input list is true. An empty list gives false.
#[node_macro::node(category("Math: Logic"))]
fn any(_: impl Ctx, values: List<bool>) -> bool {
	values.iter_element_values().any(|&value| value)
}

/// Outputs true only if every value in the input list is true. An empty list gives true.
#[node_macro::node(category("Math: Logic"))]
fn all(_: impl Ctx, values: List<bool>) -> bool {
	values.iter_element_values().all(|&value| value)
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
	#[implementations(f64, f32, u32, DVec2, bool, String)]
	value: T,
	/// The other of the two values to compare for equality.
	#[implementations(f64, f32, u32, DVec2, bool, String)]
	other_value: T,
) -> bool {
	other_value == value
}

/// The inequality operation (`!=`, `XOR`) compares two values and returns true if they are not equal, or false if they are.
#[node_macro::node(category("Math: Logic"))]
fn not_equals<T: std::cmp::PartialEq<T>>(
	_: impl Ctx,
	/// One of the two values to compare for inequality.
	#[implementations(f64, f32, u32, DVec2, bool, String)]
	value: T,
	/// The other of the two values to compare for inequality.
	#[implementations(f64, f32, u32, DVec2, bool, String)]
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

/// Evaluates either the "If True" or "If False" input branch based on whether the input condition is true or false.
#[node_macro::node(category("Math: Logic"))]
fn switch<T>(ctx: impl Ctx + Copy, condition: bool, #[expose] if_true: impl Node<Context<'_>, Output = T>, #[expose] if_false: impl Node<Context<'_>, Output = T>) -> Result<T, Interrupt> {
	if condition { if_true.eval(ctx) } else { if_false.eval(ctx) }
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

/// Constructs a vec2 value, a two-dimensional quantity which may be set to any XY pair.
#[node_macro::node(category("Value"), name("Vec2 Value"))]
fn vec2_value(_: impl Ctx, _primary: (), #[name("Vec2")] vec2: DVec2) -> DVec2 {
	vec2
}

/// Constructs a color value which may be set to any color.
#[node_macro::node(category("Value"))]
fn color_value(_: impl Ctx, _primary: (), #[default(Color::BLACK)] color: Color) -> Color {
	color
}

/// Constructs a color value from red, green, blue, and alpha components given as numbers from 0 to 1.
#[node_macro::node(category("Color"), name("RGBA to Color"))]
fn rgba_to_color(_: impl Ctx, _primary: (), red: Fraction, green: Fraction, blue: Fraction, #[default(1.)] alpha: Fraction) -> Color {
	let red = (red as f32).clamp(0., 1.);
	let green = (green as f32).clamp(0., 1.);
	let blue = (blue as f32).clamp(0., 1.);
	let alpha = (alpha as f32).clamp(0., 1.);

	// RGB user inputs are interpreted as sRGB display values; lift to linear-light for the internal `Color`
	Color::from_gamma_srgb_channels(red, green, blue, alpha)
}

/// Constructs a color value from hue, saturation, value, and alpha components given as numbers from 0 to 1.
#[node_macro::node(category("Color"), name("HSVA to Color"))]
fn hsva_to_color(_: impl Ctx, _primary: (), hue: Fraction, #[default(1.)] saturation: Fraction, #[default(1.)] value: Fraction, #[default(1.)] alpha: Fraction) -> Color {
	let hue = (hue as f32) - (hue as f32).floor();
	let saturation = (saturation as f32).clamp(0., 1.);
	let value = (value as f32).clamp(0., 1.);
	let alpha = (alpha as f32).clamp(0., 1.);

	Color::from_hsva(hue, saturation, value, alpha)
}

/// Constructs a color value from hue, saturation, lightness, and alpha components given as numbers from 0 to 1.
#[node_macro::node(category("Color"), name("HSLA to Color"))]
fn hsla_to_color(_: impl Ctx, _primary: (), hue: Fraction, #[default(1.)] saturation: Fraction, #[default(0.5)] lightness: Fraction, #[default(1.)] alpha: Fraction) -> Color {
	let hue = (hue as f32) - (hue as f32).floor();
	let saturation = (saturation as f32).clamp(0., 1.);
	let lightness = (lightness as f32).clamp(0., 1.);
	let alpha = (alpha as f32).clamp(0., 1.);

	Color::from_hsla(hue, saturation, lightness, alpha)
}

/// Constructs a color value from a CSS color string. Accepts hex (`#RRGGBB`, `#RRGGBBAA`, plus bare and shorthand variants), CSS named colors (like `red`), and functional notations (`rgb(...)`, `hsl(...)`, etc.). Invalid inputs produce no color.
#[node_macro::node(category("Color"), name("Hex to Color"))]
fn hex_to_color(ctx: impl Ctx + ExtractIndex + InjectIndex + Copy, hex_code: String) -> Result<IList<Color>, Interrupt> {
	// An invalid input serves an empty level: no color
	match (core_types::misc::parse_css_color(&hex_code), ctx.index()) {
		(Some(color), 0) => Ok(color),
		_ => Err(GraphError::past_end().into()),
	}
}

/// Constructs a gradient value which may be set to any sequence of color stops to represent the transition between colors.
#[node_macro::node(category("Value"))]
fn gradient_value(_: impl Ctx, _primary: (), #[default(Color::BLACK, Color::WHITE)] gradient: Gradient) -> Gradient {
	gradient
}

/// Sets the form (linear or radial) of each gradient in the input list.
#[node_macro::node(category("Gradient"))]
fn gradient_form(_: impl Ctx, gradient: Gradient, gradient_form: vector_types::GradientForm) -> (Gradient, Attr<GradientFormAttr>) {
	(gradient, Attr(gradient_form))
}

/// Sets how each gradient in the input list extends past its endpoints: Pad, Reflect, Repeat, or Clear.
#[node_macro::node(category("Gradient"))]
fn gradient_spread(_: impl Ctx, gradient: Gradient, gradient_spread: vector_types::GradientSpread) -> (Gradient, Attr<GradientSpreadAttr>) {
	(gradient, Attr(gradient_spread))
}

/// Sets the color space in which each gradient in the input list interpolates between its stops.
#[node_macro::node(category("Gradient"))]
fn gradient_space(_: impl Ctx, gradient: Gradient, gradient_space: vector_types::GradientSpace) -> (Gradient, Attr<GradientSpaceAttr>) {
	(gradient, Attr(gradient_space))
}

/// Sets the hue path each gradient in the input list interpolates along in polar color spaces.
#[node_macro::node(category("Gradient"))]
fn gradient_hue_direction(_: impl Ctx, gradient: Gradient, gradient_hue_direction: vector_types::GradientHueDirection) -> (Gradient, Attr<GradientHueDirectionAttr>) {
	(gradient, Attr(gradient_hue_direction))
}

/// Sets how the color progresses across each interval between gradient stops: stepped, linear, or smoothstep.
#[node_macro::node(category("Gradient"))]
fn gradient_interpolation(_: impl Ctx, gradient: Gradient, gradient_interpolation: vector_types::GradientInterpolation) -> (Gradient, Attr<GradientInterpolationAttr>) {
	(gradient, Attr(gradient_interpolation))
}

/// Sets the position of each of a gradient's stops, a factor from 0 to 1 along the gradient.
///
/// A list shorter than the stop count repeats its last value, a longer list is truncated, and an empty list sets each stop to its default evenly spaced position.
#[node_macro::node(category("Gradient"))]
fn gradient_positions(_: impl Ctx, mut gradient: Gradient, positions: List<f64>) -> Gradient {
	let positions: Vec<f64> = positions.iter_element_values().copied().collect();
	gradient.set_positions(&positions);
	gradient
}

/// Sets the interpolation midpoint for each interval between gradient stops, a factor from 0 to 1 where the 0.5 default means linear interpolation and another value skews the transition speed toward one stop or the other.
///
/// The final stop's midpoint controls the wrap back around to the first stop when the gradient is cyclic, and is otherwise ignored.
///
/// A list shorter than the stop count repeats its last value, a longer list is truncated, and an empty list sets each midpoint to its default of 0.5.
#[node_macro::node(category("Gradient"))]
fn gradient_midpoints(_: impl Ctx, mut gradient: Gradient, midpoints: List<f64>) -> Gradient {
	let midpoints: Vec<f64> = midpoints.iter_element_values().copied().collect();
	gradient.set_midpoints(&midpoints);
	gradient
}

/// Evaluates the color at the specified position along the gradient, given a position from 0 (left) to 1 (right). Positions beyond that range follow the gradient's `gradient_spread` attribute: Pad (default), Reflect, Repeat, or Clear. Colors between stops interpolate in the gradient's `gradient_space` color space.
#[node_macro::node(category("Color"))]
fn sample_gradient(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	_primary: (),
	#[default(Color::BLACK, Color::WHITE)] gradient: IList<Gradient>,
	position: Fraction,
) -> Result<IList<Color>, Interrupt> {
	// An unwired gradient serves an empty level: no color
	if gradient.is_empty() || ctx.index() != 0 {
		return Err(GraphError::past_end().into());
	}

	let settings = vector_types::GradientSettings {
		spread: gradient.lane(0).attr::<GradientSpreadAttr>(),
		cyclic: gradient.lane(0).attr::<GradientCyclicAttr>(),
		space: gradient.lane(0).attr::<GradientSpaceAttr>(),
		hue_direction: gradient.lane(0).attr::<GradientHueDirectionAttr>(),
		interpolation: gradient.lane(0).attr::<GradientInterpolationAttr>(),
	};
	Ok(gradient.element_ref(0).evaluate(position, settings))
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

/// Composes a vec2 from its X and Y components.
///
/// The inverse of this node is **Split Vec2**, which decomposes a vec2 back into its X and Y components.
#[node_macro::node(category("Math: Vec2"), name("Combine Vec2"))]
fn combine_vec2(
	_: impl Ctx,
	_primary: (),
	/// The X component of the vec2.
	#[expose]
	x: f64,
	/// The Y component of the vec2.
	#[expose]
	y: f64,
) -> DVec2 {
	DVec2::new(x, y)
}

/// The dot product operation (`·`) calculates the degree of similarity of a vec2 pair based on their angles and lengths.
///
/// Calculated as `‖a‖‖b‖cos(θ)`, it represents the product of their lengths (`‖a‖‖b‖`) scaled by the alignment of their directions (`cos(θ)`).
/// The output ranges from the positive to negative product of their lengths based on when they are pointing in the same or opposite directions.
/// If either vec2 has zero length, the output is 0.
#[node_macro::node(category("Math: Vec2"))]
fn dot_product(
	_: impl Ctx,
	/// An operand of the dot product operation.
	value: DVec2,
	/// The other operand of the dot product operation.
	#[default(1., 0.)]
	other_value: DVec2,
	/// Whether to normalize both input vec2s so the calculation ranges in `[-1, 1]` by considering only their degree of directional alignment.
	normalize: bool,
) -> f64 {
	if normalize {
		value.normalize_or_zero().dot(other_value.normalize_or_zero())
	} else {
		value.dot(other_value)
	}
}

/// The cross product operation (`×`) calculates the signed area of the parallelogram formed by a vec2 pair.
///
/// The sign gives the rotation direction from the first vec2 to the second: positive for clockwise, negative for counterclockwise, and 0 when both are parallel, as drawn in the viewport.
#[node_macro::node(category("Math: Vec2"))]
fn cross_product(
	_: impl Ctx,
	/// The vec2 on the left-hand side of the cross product operation.
	value: DVec2,
	/// The vec2 on the right-hand side of the cross product operation.
	#[default(1., 0.)]
	other_value: DVec2,
) -> f64 {
	value.perp_dot(other_value)
}

/// Calculates the angle swept between two vec2s.
///
/// The angle ranges from -180° to 180° (or -π to π radians) and its sign gives the sweep direction from the "Direction From" input to the "Direction To" input: positive for clockwise, negative for counterclockwise, as drawn in the viewport and matching the direction convention of the Transform node's rotation.
#[node_macro::node(category("Math: Vec2"))]
fn angle_between(
	_: impl Ctx,
	/// The direction the angle is measured from.
	direction_from: DVec2,
	/// The direction the angle is measured to.
	#[default(1., 0.)]
	direction_to: DVec2,
	/// Whether the resulting angle should be given in radians instead of degrees.
	radians: bool,
) -> f64 {
	if direction_from == DVec2::ZERO || direction_to == DVec2::ZERO {
		return 0.;
	}

	let angle = direction_from.angle_to(direction_to);
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

/// Calculates the angle needed for a rightward-facing object placed at the "Position From" point to turn so it points toward the "Position To" point.
#[node_macro::node(category("Math: Vec2"))]
fn angle_to<T: ToPosition, U: ToPosition>(
	_: impl Ctx,
	/// The position from which the angle is measured.
	#[implementations(DVec2, DAffine2, DVec2, DAffine2)]
	position_from: T,
	/// The position toward which the angle is measured.
	#[expose]
	#[implementations(DVec2, DVec2, DAffine2, DAffine2)]
	position_to: U,
	/// Whether the resulting angle should be given in radians instead of degrees.
	radians: bool,
) -> f64 {
	let from = position_from.to_position();
	let to = position_to.to_position();
	let delta = to - from;
	let angle = delta.y.atan2(delta.x);
	if radians { angle } else { angle.to_degrees() }
}

/// The magnitude operator (`‖x‖`) calculates the length of a vec2, which is the distance from the base to the tip of the arrow it represents.
#[node_macro::node(category("Math: Vec2"))]
fn magnitude(_: impl Ctx, vec2: DVec2) -> f64 {
	vec2.length()
}

/// Measures the distance between two points, which is the length of the straight line segment connecting them.
#[node_macro::node(category("Math: Vec2"))]
fn distance(
	_: impl Ctx,
	/// The point the distance is measured from.
	position_from: DVec2,
	/// The point the distance is measured to.
	position_to: DVec2,
) -> f64 {
	position_from.distance(position_to)
}

/// Scales the input vec2 to unit length while preserving its direction. This is equivalent to dividing the input vec2 by its own magnitude.
///
/// Returns 0 when the input vec2 has zero length.
#[node_macro::node(category("Math: Vec2"))]
fn normalize(_: impl Ctx, vec2: DVec2) -> DVec2 {
	vec2.normalize_or_zero()
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	pub fn dot_product_function() {
		let vector_a = DVec2::new(1., 2.);
		let vector_b = DVec2::new(3., 4.);
		assert_eq!(dot_product(&(), vector_a, vector_b, false), 11.);
	}

	#[test]
	pub fn magnitude_function() {
		let vector = DVec2::new(3., 4.);
		assert_eq!(magnitude(&(), vector), 5.);
	}

	#[test]
	pub fn distance_function() {
		let (position_from, position_to) = (DVec2::new(1., 2.), DVec2::new(4., 6.));
		assert_eq!(distance(&(), position_from, position_to), 5.);
	}

	#[test]
	pub fn cross_product_sign() {
		let vec2 = |x, y| DVec2::new(x, y);
		assert_eq!(cross_product(&(), vec2(1., 0.), vec2(0., 1.)), 1.);
		assert_eq!(cross_product(&(), vec2(0., 1.), vec2(1., 0.)), -1.);
		assert_eq!(cross_product(&(), vec2(2., 2.), vec2(1., 1.)), 0.);
	}

	#[test]
	pub fn sign_of_negative_zero_is_positive_zero() {
		let result = sign(&(), -0.0_f64);
		assert_eq!(result, 0.);
		assert!(result.is_sign_positive());
	}

	#[test]
	pub fn sign_componentwise() {
		assert_eq!(sign(&(), DVec2::new(-5., 3.)), DVec2::new(-1., 1.));
	}

	#[test]
	pub fn lerp_endpoints_are_exact() {
		let lerp_between = |factor, clamped| lerp(&(), 3., 7., factor, clamped);
		assert_eq!(lerp_between(0., true), 3.);
		assert_eq!(lerp_between(1., true), 7.);
		assert_eq!(lerp_between(0.5, true), 5.);
	}

	#[test]
	pub fn lerp_clamped_and_extrapolated() {
		let lerp_between = |factor, clamped| lerp(&(), 0., 10., factor, clamped);
		assert_eq!(lerp_between(2., true), 10.);
		assert_eq!(lerp_between(2., false), 20.);
	}

	#[test]
	pub fn lerp_endpoint_factors_pass_endpoints_through() {
		let lerp_between = |start: f64, end: f64, factor| lerp(&(), start, end, factor, true);
		assert_eq!(lerp_between(3., f64::INFINITY, 0.), 3.);
		assert_eq!(lerp_between(f64::NAN, 7., 1.), 7.);
		assert_eq!(lerp_between(3., f64::INFINITY, 1.), f64::INFINITY);
		assert!(lerp_between(-0., 7., 0.).is_sign_negative());
		assert!(lerp_between(5., -0., 1.).is_sign_negative());
	}

	#[test]
	pub fn angle_between_signed() {
		let right = DVec2::new(1., 0.);
		let down = DVec2::new(0., 1.);
		let angle = |a, b, radians| angle_between(&(), a, b, radians);
		assert_eq!(angle(right, down, false), 90.);
		assert_eq!(angle(down, right, false), -90.);
	}

	#[test]
	pub fn angle_between_zero_vector() {
		let (zero, right) = (DVec2::ZERO, DVec2::new(1., 0.));
		assert_eq!(angle_between(&(), zero, right, false), 0.);
	}

	#[test]
	pub fn clamp_vec2_within_swapped_bounds() {
		let vec2 = |x, y| DVec2::new(x, y);
		assert_eq!(clamp(&(), vec2(-5., 5.), vec2(1., 1.), vec2(0., 2.)), DVec2::new(0., 2.));
	}

	#[test]
	pub fn min_max_vec2_with_scalar() {
		let vec2 = |x, y| DVec2::new(x, y);
		assert_eq!(super::min(&(), vec2(-5., 5.), 0_f64), DVec2::new(-5., 0.));
		assert_eq!(super::max(&(), vec2(-5., 5.), 0_f64), DVec2::new(0., 5.));
	}

	#[test]
	pub fn scalar_with_vec2_operand_orders() {
		let vec2 = |x, y| DVec2::new(x, y);
		assert_eq!(super::min(&(), 0_f64, vec2(-5., 5.)), DVec2::new(-5., 0.));
		assert_eq!(super::max(&(), 0_f64, vec2(-5., 5.)), DVec2::new(0., 5.));
		assert_eq!(exponent(&(), 2_f64, vec2(2., 3.)), DVec2::new(4., 8.));
		assert_eq!(root(&(), 64_f64, vec2(2., 3.)), DVec2::new(8., 4.));
		assert_eq!(logarithm(&(), 8_f64, vec2(2., 10.)), DVec2::new(3., 8_f64.log10()));
		assert_eq!(clamp(&(), 5_f64, vec2(0., 6.), vec2(1., 10.)), DVec2::new(1., 6.));
	}

	#[test]
	pub fn vec2_degrees_and_bases() {
		let vec2 = |x, y| DVec2::new(x, y);
		assert_eq!(root(&(), vec2(64., 27.), vec2(2., 3.)), DVec2::new(8., 3.));
		assert_eq!(logarithm(&(), vec2(8., 100.), vec2(2., 10.)), DVec2::new(3., 2.));
	}

	#[test]
	pub fn logarithm_f32_base_e_and_near_e() {
		assert_eq!(logarithm(&(), 8_f32, std::f32::consts::E), 8_f64.ln() as f32);
		assert_eq!(logarithm(&(), 8_f32, 2.7_f32), 8_f64.log(2.7_f32 as f64) as f32);
	}

	#[test]
	pub fn round_floor_ceiling_vec2() {
		let vec2 = |x, y| DVec2::new(x, y);
		assert_eq!(round(&(), vec2(1.5, -1.4)), DVec2::new(2., -1.));
		assert_eq!(floor(&(), vec2(1.9, -1.1)), DVec2::new(1., -2.));
		assert_eq!(ceiling(&(), vec2(1.1, -1.9)), DVec2::new(2., -1.));
	}

	#[test]
	fn test_basic_expression() {
		let result = math(&(), 0., "2 + 2".to_string(), 0.);
		assert_eq!(result, 4.);
	}

	#[test]
	fn test_complex_expression() {
		let result = math(&(), 0., "(5 * 3) + (10 / 2)".to_string(), 0.);
		assert_eq!(result, 20.);
	}

	#[test]
	fn test_default_expression() {
		let result = math(&(), 0., "0".to_string(), 0.);
		assert_eq!(result, 0.);
	}

	#[test]
	fn test_invalid_expression() {
		let result = math(&(), 0., "invalid".to_string(), 0.);
		assert_eq!(result, 0.);
	}

	#[test]
	pub fn add_vectors() {
		assert_eq!(super::add(&(), DVec2::ONE, DVec2::ONE), DVec2::ONE * 2.);
	}

	#[test]
	pub fn subtract_f64() {
		assert_eq!(super::subtract(&(), 5_f64, 3_f64), 2.);
	}

	#[test]
	pub fn divide_vectors() {
		assert_eq!(super::divide(&(), DVec2::ONE, 2_f64), DVec2::ONE / 2.);
	}

	#[test]
	pub fn divide_vector_by_partially_zero_vector() {
		assert_eq!(super::divide(&(), DVec2::new(1., 2.), DVec2::new(2., 0.)), DVec2::new(0.5, 0.));
	}

	#[test]
	pub fn modulo_positive() {
		assert_eq!(super::modulo(&(), -5_f64, 2_f64, true), 1_f64);
	}

	#[test]
	pub fn modulo_negative() {
		assert_eq!(super::modulo(&(), -5_f64, 2_f64, false), -1_f64);
	}
}

#[cfg(test)]
mod graphene_test {
	use super::*;
	use core_types::arena::Arena;
	use core_types::context::{ContextImpl, EvalScope};
	use core_types::gpoll::{Finality, GPoll};
	use core_types::node::{BatchStatus, Node};
	use core_types::record::{Layout, LiftedSource, RecordValue, serve_input};
	use core_types::registry::{ErasedRecordNode, construct};
	use core_types::value::record_value_source;
	use std::mem::MaybeUninit;

	fn scope_fixture(arena: &Arena) -> EvalScope<'_> {
		EvalScope::new(None, None, None, &[], arena)
	}

	fn frames_for(layouts: &[&Layout]) -> core_types::record::Frames<'static> {
		core_types::record::test_frames(layouts.iter().map(|layout| layout.frame_bytes()).sum::<usize>().max(1 << 12))
	}

	/// Lifts a plain-element test source onto a record input, returned beside its
	/// element-only layout for the generated node's constructor.
	fn lifted<T, F>(kernel: F) -> (LiftedSource<T, F>, Layout)
	where
		T: Clone + Send + Sync + core_types::StaticTypeSized + 'static,
		<T as core_types::StaticTypeSized>::Static: Clone + Send + Sync,
		F: for<'c> Fn(&ContextImpl<'c>) -> GPoll<T>,
	{
		let lift = LiftedSource::<T, _>::new(kernel);
		let layout = Node::<ContextImpl>::layout(&lift).clone();
		(lift, layout)
	}

	fn element<T: Copy>(layout: &Layout, value: &RecordValue<'_>) -> T {
		unsafe { layout.rec(value).element::<T>() }
	}

	fn out_layout<T: Clone + Send + Sync + core_types::StaticTypeSized>() -> Layout
	where
		<T as core_types::StaticTypeSized>::Static: Clone + Send + Sync,
	{
		Layout::default().with_writes(0, core_types::record::element_write::<T>(), &[])
	}

	fn installed<N: Node<ContextImpl<'static>>>(mut node: N, layout: &Layout) -> N {
		node.set_layout(core_types::record::RecordLayout {
			named_writes: Vec::new(),
			named_reads: Vec::new(),
			named_read_defaults: Vec::new(),
			frame_bytes: layout.frame_bytes(),
			plan: Vec::new(),
			layout: layout.clone(),
			lane_invariant: u32::MAX,
		});
		node
	}

	#[test]
	fn generated_add_evaluates_through_the_node_path() {
		let arena = Arena::new(64).unwrap();
		let scope = scope_fixture(&arena);
		let ctx = ContextImpl::root(&scope);

		let (a, la) = lifted(|_: &ContextImpl| GPoll::Final(1.0f64));
		let (b, lb) = lifted(|_: &ContextImpl| GPoll::Final(2.0f64));
		let out = out_layout::<f64>();
		let graph = installed(AddNode::<_, _, f64, f64>::new(a, b, &la, &lb), &out);
		let frames = frames_for(&[&la, &lb, &out]);

		let GPoll::Final(value) = serve_input(&graph, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(element::<f64>(&out, &value), 3.0);
	}

	#[test]
	fn generated_add_batches_through_the_erased_edge() {
		let arena = Arena::new(64).unwrap();
		let scope = scope_fixture(&arena);
		let ctx = ContextImpl::root(&scope);

		let (index, li) = lifted(|input: &ContextImpl| GPoll::Final(core_types::ExtractIndex::<0>::index(input) as f64));
		let (src, ls) = lifted(|_: &ContextImpl| GPoll::Final(10.0f64));
		let out = out_layout::<f64>();
		let node = installed(AddNode::<_, _, f64, f64>::new(index, src, &li, &ls), &out);
		let frames = frames_for(&[&li, &ls, &out]);

		let erased: Box<ErasedRecordNode> = Box::new(node);
		// One u64 word per lane at the element-only layout.
		let mut scratch = [const { MaybeUninit::uninit() }; 4];
		let status = erased.eval_batch(&ctx, 2..6, Some(&mut scratch), &frames);
		let BatchStatus::Filled(batch, finality, _) = status else {
			panic!("expected filled, got {status:?}");
		};
		let mut got = Vec::new();
		batch.share().for_each(|_, lane| got.push(unsafe { lane.element::<f64>() }));
		assert_eq!(got, vec![12.0, 13.0, 14.0, 15.0]);
		assert_eq!(finality, Finality::AllFinal);
	}

	#[test]
	fn generated_wire_constructor_resolves_and_wires() {
		let arena = Arena::new(64).unwrap();
		let scope = scope_fixture(&arena);
		let ctx = ContextImpl::root(&scope);

		let entries = super::_logical_or_mod::logical_or_entries();
		let mut wired = construct(&entries[0], vec![record_value_source(true), record_value_source(false)]).unwrap();
		let layout = out_layout::<bool>();
		wired.set_layout(core_types::record::RecordLayout {
			named_writes: Vec::new(),
			named_reads: Vec::new(),
			named_read_defaults: Vec::new(),
			frame_bytes: layout.frame_bytes(),
			plan: Vec::new(),
			layout: layout.clone(),
			lane_invariant: u32::MAX,
		});
		let edge = wired.downcast_record::<bool>().unwrap();
		let frames = frames_for(&[&layout]);

		let GPoll::Final(value) = serve_input(&edge, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert!(element::<bool>(&layout, &value));
	}

	#[test]
	fn ctor_registration_populates_the_node_registry() {
		let registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
		let rows = registry
			.iter()
			.find_map(|(id, rows)| id.as_str().ends_with("::AddNode").then_some(rows))
			.expect("AddNode rows registered at startup");
		assert_eq!(rows.len(), 6);
	}

	#[test]
	fn generic_add_registers_one_entry_per_implementation() {
		let arena = Arena::new(64).unwrap();
		let scope = scope_fixture(&arena);
		let ctx = ContextImpl::root(&scope);

		let entries = super::_add_mod::add_entries();
		assert_eq!(entries.len(), 6);
		assert_eq!(
			entries[0].io.inputs,
			vec![core_types::registry::record_source_type::<f64>(), core_types::registry::record_source_type::<f64>()]
		);
		assert_eq!(entries[0].io.return_value, core_types::registry::record_type::<f64>());
		assert_eq!(
			entries[3].io.inputs,
			vec![core_types::registry::record_source_type::<DVec2>(), core_types::registry::record_source_type::<DVec2>()]
		);
		assert_eq!(entries[3].io.return_value, core_types::registry::record_type::<DVec2>());

		let mut wired = construct(&entries[0], vec![record_value_source(1.5f64), record_value_source(2.5f64)]).unwrap();
		let layout = out_layout::<f64>();
		wired.set_layout(core_types::record::RecordLayout {
			named_writes: Vec::new(),
			named_reads: Vec::new(),
			named_read_defaults: Vec::new(),
			frame_bytes: layout.frame_bytes(),
			plan: Vec::new(),
			layout: layout.clone(),
			lane_invariant: u32::MAX,
		});
		let edge = wired.downcast_record::<f64>().unwrap();
		let frames = frames_for(&[&layout]);

		let GPoll::Final(value) = serve_input(&edge, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(element::<f64>(&layout, &value), 4.0);
	}

	#[test]
	fn switch_registers_one_erased_row() {
		// Routing forwards the whole record, so the branch types need no rows.
		let entries = super::_switch_mod::switch_entries();
		assert_eq!(entries.len(), 1);
		assert_eq!(entries[0].io.inputs[0], core_types::registry::record_source_type::<bool>());
		assert!(matches!(&entries[0].io.return_value, core_types::Type::Record(element) if matches!(**element, core_types::Type::Generic(_))));
		assert_eq!(entries[0].io.inputs.len(), 3);
	}

	#[test]
	fn converted_switch_evaluates_only_the_taken_branch() {
		use std::sync::Arc;
		use std::sync::atomic::{AtomicU32, Ordering};

		let arena = Arena::new(64).unwrap();
		let scope = scope_fixture(&arena);
		let ctx = ContextImpl::root(&scope);

		let taken = Arc::new(AtomicU32::new(0));
		let untaken = Arc::new(AtomicU32::new(0));
		let (cond, lc) = lifted(|_: &ContextImpl| GPoll::Final(true));
		let (if_true, lt) = lifted({
			let runs = taken.clone();
			move |_: &ContextImpl| {
				runs.fetch_add(1, Ordering::Relaxed);
				GPoll::Final(1.0)
			}
		});
		let (if_false, lf) = lifted({
			let runs = untaken.clone();
			move |_: &ContextImpl| {
				runs.fetch_add(1, Ordering::Relaxed);
				GPoll::Final(2.0)
			}
		});
		let union = core_types::record::Layout::union(&[&lt, &lf]);
		let graph = SwitchNode::new(cond, if_true, if_false, &union, &lc);
		let out = Node::<ContextImpl>::layout(&graph).clone();
		let frames = frames_for(&[&lc, &lt, &lf, &out]);

		let GPoll::Final(value) = serve_input(&graph, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(element::<f64>(&out, &value), 1.0);
		assert_eq!(taken.load(Ordering::Relaxed), 1);
		assert_eq!(untaken.load(Ordering::Relaxed), 0);
	}

	#[test]
	fn converted_switch_passes_branch_status_through() {
		let arena = Arena::new(64).unwrap();
		let scope = scope_fixture(&arena);
		let ctx = ContextImpl::root(&scope);

		let (c1, lc1) = lifted(|_: &ContextImpl| GPoll::Final(true));
		let (p1, lp1) = lifted(|_: &ContextImpl| GPoll::<f64>::Pending);
		let (pa1, lpa1) = lifted(|_: &ContextImpl| GPoll::Partial(7.0f64));
		let pending = SwitchNode::new(c1, p1, pa1, &core_types::record::Layout::union(&[&lp1, &lpa1]), &lc1);

		let (c2, lc2) = lifted(|_: &ContextImpl| GPoll::Final(false));
		let (p2, lp2) = lifted(|_: &ContextImpl| GPoll::<f64>::Pending);
		let (pa2, lpa2) = lifted(|_: &ContextImpl| GPoll::Partial(7.0f64));
		let partial = SwitchNode::new(c2, p2, pa2, &core_types::record::Layout::union(&[&lp2, &lpa2]), &lc2);
		let out = Node::<ContextImpl>::layout(&partial).clone();
		let frames = frames_for(&[&lc1, &lp1, &lpa1, &lc2, &lp2, &lpa2, &out]);

		assert!(matches!(serve_input(&pending, &ctx, &frames), GPoll::Pending));
		let GPoll::Partial(value) = serve_input(&partial, &ctx, &frames) else {
			panic!("expected a partial record");
		};
		assert_eq!(element::<f64>(&out, &value), 7.0);
	}

	#[test]
	fn converted_switch_merges_condition_status_into_the_branch_result() {
		let arena = Arena::new(64).unwrap();
		let scope = scope_fixture(&arena);
		let ctx = ContextImpl::root(&scope);

		let (cond, lc) = lifted(|_: &ContextImpl| GPoll::Partial(true));
		let (if_true, lt) = lifted(|_: &ContextImpl| GPoll::Final(1.0f64));
		let (if_false, lf) = lifted(|_: &ContextImpl| GPoll::Final(2.0f64));
		let union = core_types::record::Layout::union(&[&lt, &lf]);
		let graph = SwitchNode::new(cond, if_true, if_false, &union, &lc);
		let out = Node::<ContextImpl>::layout(&graph).clone();
		let frames = frames_for(&[&lc, &lt, &lf, &out]);

		let GPoll::Partial(value) = serve_input(&graph, &ctx, &frames) else {
			panic!("expected a partial record");
		};
		assert_eq!(element::<f64>(&out, &value), 1.0);
	}

	#[test]
	fn generated_eval_computes_on_stand_in_and_traces_fallback() {
		let arena = Arena::new(64).unwrap();
		let scope = scope_fixture(&arena);
		let ctx = ContextImpl::root(&scope);

		let (fallback, lfb) = lifted(|_: &ContextImpl| GPoll::fallback(0.0f64, "upstream failed"));
		let (src, ls) = lifted(|_: &ContextImpl| GPoll::Final(5.0f64));
		let out = out_layout::<f64>();
		let graph = installed(AddNode::<_, _, f64, f64>::new(fallback, src, &lfb, &ls), &out);
		let frames = frames_for(&[&lfb, &ls, &out]);

		let GPoll::Fallback(boxed) = serve_input(&graph, &ctx, &frames) else {
			panic!("fallback must propagate with the computed stand-in");
		};
		assert_eq!(element::<f64>(&out, &boxed.0), 5.0);
		assert!(boxed.1.kind == "upstream failed");
		assert_eq!(boxed.1.trace, vec![0]);
	}
}
