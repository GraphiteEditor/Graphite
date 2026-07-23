use core_types::Context;
use core_types::context::{CloneVarArgs, ExtractAll};
use core_types::list::{Bundle, Item, List};
use core_types::registry::types::{Fraction, Percentage, PixelSize};
use core_types::transform::Footprint;
use core_types::{Color, Ctx, OwnedContextImpl, num_traits};
use glam::{DAffine2, DVec2};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use log::warn;
use math_parser::ast;
use math_parser::context::{EvalContext, NothingMap, ValueProvider};
use math_parser::value::{Number, Value};
use rand::{Rng, SeedableRng};
use std::ops::{Add, Mul, Rem, Sub};
use vector_types::Gradient;

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
	operand_a: Item<T>,
	/// A math expression that may incorporate "A" and/or "B", such as `sqrt(A + B) - B^2`.
	#[default("A + B")]
	expression: Item<String>,
	/// The value of "B" when calculating the expression.
	#[implementations(f64, f32)]
	#[default(1.)]
	operand_b: Item<T>,
) -> Item<T> {
	let (operand_a, attributes) = operand_a.into_parts();
	let (expression, operand_b) = (expression.element(), *operand_b.element());

	let (node, _unit) = match ast::Node::try_parse_from_str(expression) {
		Ok(expr) => expr,
		Err(e) => {
			warn!("Invalid expression: `{expression}`\n{e:?}");
			return Item::from_parts(T::from(0.).unwrap(), attributes);
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
			return Item::from_parts(T::from(0.).unwrap(), attributes);
		}
	};

	let Value::Number(num) = value;
	let result = match num {
		Number::Real(val) => T::from(val).unwrap(),
		Number::Complex(c) => T::from(c.re).unwrap(),
	};

	Item::from_parts(result, attributes)
}

/// The addition operation (`+`) calculates the sum of two scalar numbers or vec2s.
#[node_macro::node(category("Math: Arithmetic"))]
fn add<A: Add<B>, B>(
	_: impl Ctx,
	/// The left-hand side of the addition operation.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	augend: Item<A>,
	/// The right-hand side of the addition operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	addend: Item<B>,
) -> Item<<A as Add<B>>::Output> {
	let (augend, attributes) = augend.into_parts();

	Item::from_parts(augend + addend.into_element(), attributes)
}

/// The subtraction operation (`-`) calculates the difference between two scalar numbers or vec2s.
#[node_macro::node(category("Math: Arithmetic"))]
fn subtract<A: Sub<B>, B>(
	_: impl Ctx,
	/// The left-hand side of the subtraction operation.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	minuend: Item<A>,
	/// The right-hand side of the subtraction operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	subtrahend: Item<B>,
) -> Item<<A as Sub<B>>::Output> {
	let (minuend, attributes) = minuend.into_parts();

	Item::from_parts(minuend - subtrahend.into_element(), attributes)
}

/// The multiplication operation (`×`) calculates the product of two scalar numbers, vec2s, or transforms.
#[node_macro::node(category("Math: Arithmetic"))]
fn multiply<A: Mul<B>, B>(
	_: impl Ctx,
	/// The left-hand side of the multiplication operation.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2, DAffine2)]
	multiplier: Item<A>,
	/// The right-hand side of the multiplication operation.
	#[default(1.)]
	#[implementations(f64, f32, u32, DVec2, DVec2, f64, DAffine2)]
	multiplicand: Item<B>,
) -> Item<<A as Mul<B>>::Output> {
	let (multiplier, attributes) = multiplier.into_parts();

	Item::from_parts(multiplier * multiplicand.into_element(), attributes)
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
	numerator: Item<A>,
	/// The right-hand side of the division operation.
	#[default(1.)]
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	denominator: Item<B>,
) -> Item<<A as SafeDivide<B>>::Output> {
	let (numerator, attributes) = numerator.into_parts();

	Item::from_parts(numerator.safe_divide(denominator.into_element()), attributes)
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
	value: Item<T>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.componentwise(|value| if value == 0. { 0. } else { 1. / value }), attributes)
}

/// The modulo operation (`%`) calculates the remainder from the division of two scalar numbers or vec2s.
///
/// The sign of the result shares the sign of the numerator unless *Always Positive* is enabled.
#[node_macro::node(category("Math: Arithmetic"))]
fn modulo<A: Rem<B, Output: Add<B, Output: Rem<B, Output = A::Output>>>, B: Copy>(
	_: impl Ctx,
	/// The left-hand side of the modulo operation.
	#[implementations(f64, f32, u32, DVec2, DVec2, f64)]
	numerator: Item<A>,
	/// The right-hand side of the modulo operation.
	#[default(2.)]
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	modulus: Item<B>,
	/// Ensures the result is always positive, even if the numerator is negative.
	#[default(true)]
	always_positive: Item<bool>,
) -> Item<<A as Rem<B>>::Output> {
	let (numerator, attributes) = numerator.into_parts();
	let (modulus, always_positive) = (*modulus.element(), *always_positive.element());

	let result = if always_positive { (numerator % modulus + modulus) % modulus } else { numerator % modulus };
	Item::from_parts(result, attributes)
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
	base: Item<A>,
	/// The power to which the base number is raised.
	#[implementations(f64, f32, u32, DVec2, f64, DVec2)]
	#[default(2.)]
	power: Item<B>,
) -> Item<<A as Exponent<B>>::Output> {
	let (base, attributes) = base.into_parts();

	Item::from_parts(base.power(power.into_element()), attributes)
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
	radicand: Item<A>,
	/// The degree of the root to be calculated. Square root is 2, cube root is 3, and so on.
	/// Degrees 0 or less are invalid and will produce an output of 0.
	#[default(2.)]
	#[implementations(f64, f32, f64, DVec2, DVec2)]
	degree: Item<B>,
) -> Item<<A as NthRoot<B>>::Output> {
	let (radicand, attributes) = radicand.into_parts();

	Item::from_parts(radicand.nth_root(degree.into_element()), attributes)
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
	value: Item<A>,
	/// The base of the logarithm, such as 2 (binary), 10 (decimal), and e (natural logarithm).
	#[default(2.)]
	#[implementations(f64, f32, f64, DVec2, DVec2)]
	base: Item<B>,
) -> Item<<A as Logarithm<B>>::Output> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.logarithm(base.into_element()), attributes)
}

/// The sine trigonometric function (`sin`) calculates the ratio of the angle's opposite side length to its hypotenuse length.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Trig"))]
fn sine<T: Componentwise>(
	_: impl Ctx,
	/// The given angle.
	#[implementations(f64, f32, DVec2)]
	theta: Item<T>,
	/// Whether the given angle should be interpreted as radians instead of degrees.
	radians: Item<bool>,
) -> Item<T> {
	let (theta, attributes) = theta.into_parts();
	let radians = *radians.element();

	let result = theta.componentwise(|theta| if radians { theta.sin() } else { theta.to_radians().sin() });
	Item::from_parts(result, attributes)
}

/// The cosine trigonometric function (`cos`) calculates the ratio of the angle's adjacent side length to its hypotenuse length.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Trig"))]
fn cosine<T: Componentwise>(
	_: impl Ctx,
	/// The given angle.
	#[implementations(f64, f32, DVec2)]
	theta: Item<T>,
	/// Whether the given angle should be interpreted as radians instead of degrees.
	radians: Item<bool>,
) -> Item<T> {
	let (theta, attributes) = theta.into_parts();
	let radians = *radians.element();

	let result = theta.componentwise(|theta| if radians { theta.cos() } else { theta.to_radians().cos() });
	Item::from_parts(result, attributes)
}

/// The tangent trigonometric function (`tan`) calculates the ratio of the angle's opposite side length to its adjacent side length.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Trig"))]
fn tangent<T: Componentwise>(
	_: impl Ctx,
	/// The given angle.
	#[implementations(f64, f32, DVec2)]
	theta: Item<T>,
	/// Whether the given angle should be interpreted as radians instead of degrees.
	radians: Item<bool>,
) -> Item<T> {
	let (theta, attributes) = theta.into_parts();
	let radians = *radians.element();

	let result = theta.componentwise(|theta| if radians { theta.tan() } else { theta.to_radians().tan() });
	Item::from_parts(result, attributes)
}

/// The inverse sine trigonometric function (`asin`) calculates the angle whose sine is the input value.
#[node_macro::node(category("Math: Trig"))]
fn sine_inverse<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The given value for which the angle is calculated. Must be in the domain `[-1, 1]` (it will be clamped to -1 or 1 otherwise).
	#[implementations(f64, f32)]
	value: Item<T>,
	/// Whether the resulting angle should be given in as radians instead of degrees.
	radians: Item<bool>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();

	let angle = value.clamp(T::from(-1.).unwrap(), T::from(1.).unwrap()).asin();
	let result = if *radians.element() { angle } else { angle.to_degrees() };
	Item::from_parts(result, attributes)
}

/// The inverse cosine trigonometric function (`acos`) calculates the angle whose cosine is the input value.
#[node_macro::node(category("Math: Trig"))]
fn cosine_inverse<T: num_traits::float::Float>(
	_: impl Ctx,
	/// The given value for which the angle is calculated. Must be in the domain `[-1, 1]` (it will be clamped to -1 or 1 otherwise).
	#[implementations(f64, f32)]
	value: Item<T>,
	/// Whether the resulting angle should be given in as radians instead of degrees.
	radians: Item<bool>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();

	let angle = value.clamp(T::from(-1.).unwrap(), T::from(1.).unwrap()).acos();
	let result = if *radians.element() { angle } else { angle.to_degrees() };
	Item::from_parts(result, attributes)
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
	value: Item<T>,
	/// Whether the resulting angle should be given in as radians instead of degrees.
	radians: Item<bool>,
) -> Item<T::Output> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.atan(*radians.element()), attributes)
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
	value: Item<U>,
	/// The lower bound of the input range.
	#[implementations(f64, f32)]
	input_min: Item<U>,
	/// The upper bound of the input range.
	#[implementations(f64, f32)]
	#[default(1.)]
	input_max: Item<U>,
	/// The lower bound of the output range.
	#[implementations(f64, f32)]
	output_min: Item<U>,
	/// The upper bound of the output range.
	#[implementations(f64, f32)]
	#[default(1.)]
	output_max: Item<U>,
	/// Whether to constrain the result within the output range instead of extrapolating beyond its bounds.
	clamped: Item<bool>,
) -> Item<U> {
	let (value, attributes) = value.into_parts();
	let (input_min, input_max, output_min, output_max) = (*input_min.element(), *input_max.element(), *output_min.element(), *output_max.element());

	let input_range = input_max - input_min;

	// Handle division by zero
	if input_range.abs() < U::epsilon() {
		return Item::from_parts(output_min, attributes);
	}

	let normalized = (value - input_min) / input_range;
	let output_range = output_max - output_min;

	let result = output_min + normalized * output_range;

	let result = if *clamped.element() {
		// Handle both normal and inverted ranges, since we want to allow the user to use this node to also reverse a range.
		if output_min <= output_max {
			result.clamp(output_min, output_max)
		} else {
			result.clamp(output_max, output_min)
		}
	} else {
		result
	};

	Item::from_parts(result, attributes)
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
	start: Item<T>,
	/// The value produced when the factor is 1.
	#[default(1.)]
	#[implementations(f64, f32, DVec2)]
	end: Item<T>,
	/// The mix between the start (at 0) and end (at 1) values.
	#[default(0.5)]
	factor: Item<f64>,
	/// Whether to constrain the factor within 0 to 1, preventing extrapolation beyond the start and end values.
	#[default(true)]
	clamped: Item<bool>,
) -> Item<T> {
	let (start, attributes) = start.into_parts();
	let factor = if *clamped.element() { factor.element().clamp(0., 1.) } else { *factor.element() };

	// Exact endpoint factors pass the endpoint through untouched, since the unused operand would otherwise contaminate the weighted sum (NaN or infinity times 0 is NaN)
	let result = if factor == 0. {
		start
	} else if factor == 1. {
		end.into_element()
	} else {
		start.lerp(end.into_element(), factor)
	};
	Item::from_parts(result, attributes)
}

/// The random function (`rand`) converts a seed into a random number within the specified range, inclusive of the minimum and exclusive of the maximum. The minimum and maximum values are automatically swapped if they are reversed.
#[node_macro::node(category("Math: Numeric"))]
fn random(
	_: impl Ctx,
	_primary: (),
	/// Seed to determine the unique variation of which number is generated.
	seed: Item<u64>,
	/// The smaller end of the range within which the random number is generated.
	min: Item<f64>,
	/// The larger end of the range within which the random number is generated.
	#[default(1.)]
	max: Item<f64>,
) -> Item<f64> {
	let mut rng = rand::rngs::StdRng::seed_from_u64(*seed.element());
	let result = rng.random::<f64>();
	let (min, max) = (*min.element(), *max.element());
	let (min, max) = if min < max { (min, max) } else { (max, min) };
	Item::new_from_element(result * (max - min) + min)
}

// TODO: Test that these are no longer needed in all circumstances, then remove them and add a migration to convert these into Passthrough nodes. Note: these act more as type annotations than as identity functions.
/// Convert a number to an integer of the type u32, which may be the required type for certain node inputs.
#[node_macro::node(name("As u32"), category("Debug"))]
fn as_u32(_: impl Ctx, value: Item<u32>) -> Item<u32> {
	value
}

// TODO: Test that these are no longer needed in all circumstances, then remove them and add a migration to convert these into Passthrough nodes. Note: these act more as type annotations than as identity functions.
/// Convert a number to an integer of the type u64, which may be the required type for certain node inputs.
#[node_macro::node(name("As u64"), category("Debug"))]
fn as_u64(_: impl Ctx, value: Item<u64>) -> Item<u64> {
	value
}

// TODO: Test that these are no longer needed in all circumstances, then remove them and add a migration to convert these into Passthrough nodes. Note: these act more as type annotations than as identity functions.
/// Convert an integer to a decimal number of the type f64, which may be the required type for certain node inputs.
#[node_macro::node(name("As f64"), category("Debug"))]
fn as_f64(_: impl Ctx, value: Item<f64>) -> Item<f64> {
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
	value: Item<T>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.componentwise(f64::round), attributes)
}

/// The floor function (`floor`) rounds down an input value to the nearest whole number, unless the input number is already whole.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn floor<T: Componentwise>(
	_: impl Ctx,
	/// The number to be rounded down.
	#[implementations(f64, f32, DVec2)]
	value: Item<T>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.componentwise(f64::floor), attributes)
}

/// The ceiling function (`ceil`) rounds up an input value to the nearest whole number, unless the input number is already whole.
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn ceiling<T: Componentwise>(
	_: impl Ctx,
	/// The number to be rounded up.
	#[implementations(f64, f32, DVec2)]
	value: Item<T>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.componentwise(f64::ceil), attributes)
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
	value: Item<T>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.abs(), attributes)
}

/// The sign function (`sign`) reports whether an input value is positive (1), negative (-1), or zero (0).
///
/// With a vec2 input, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn sign<T: Componentwise>(
	_: impl Ctx,
	/// The number whose sign is checked.
	#[implementations(f64, f32, DVec2)]
	value: Item<T>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();

	let result = value.componentwise(|value| {
		if value > 0. {
			1.
		} else if value < 0. {
			-1.
		} else {
			0.
		}
	});
	Item::from_parts(result, attributes)
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
	value: Item<A>,
	/// The other of the two numbers, of which the lesser is returned.
	#[implementations(f64, f32, u32, String, DVec2, f64, DVec2)]
	other_value: Item<B>,
) -> Item<<A as MinMax<B>>::Output> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.minimum(other_value.into_element()), attributes)
}

/// The maximum function (`max`) picks the larger of two numbers.
///
/// With vec2 inputs, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn max<A: MinMax<B>, B>(
	_: impl Ctx,
	/// One of the two numbers, of which the greater is returned.
	#[implementations(f64, f32, u32, String, DVec2, DVec2, f64)]
	value: Item<A>,
	/// The other of the two numbers, of which the greater is returned.
	#[implementations(f64, f32, u32, String, DVec2, f64, DVec2)]
	other_value: Item<B>,
) -> Item<<A as MinMax<B>>::Output> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.maximum(other_value.into_element()), attributes)
}

/// The clamp function (`clamp`) restricts a number to a specified range between a minimum and maximum value. The minimum and maximum values are automatically swapped if they are reversed.
///
/// With vec2 inputs, this applies separately to the X and Y components.
#[node_macro::node(category("Math: Numeric"))]
fn clamp<A: MinMax<B>, B: MinMax<Output = B> + Clone>(
	_: impl Ctx,
	/// The number to be clamped, which is restricted to the range between the minimum and maximum values.
	#[implementations(f64, f32, u32, String, DVec2, DVec2, f64)]
	value: Item<A>,
	/// The left (smaller) side of the range. The output is never less than this number.
	#[implementations(f64, f32, u32, String, DVec2, f64, DVec2)]
	min: Item<B>,
	/// The right (greater) side of the range. The output is never greater than this number.
	#[implementations(f64, f32, u32, String, DVec2, f64, DVec2)]
	#[default(1)]
	max: Item<B>,
) -> Item<<A as MinMax<B>>::Output>
where
	<A as MinMax<B>>::Output: MinMax<B, Output = <A as MinMax<B>>::Output>,
{
	let (value, attributes) = value.into_parts();
	let (min, max) = (min.into_element(), max.into_element());

	let (min, max) = (min.clone().minimum(max.clone()), min.maximum(max));
	Item::from_parts(value.maximum(min).minimum(max), attributes)
}

/// The greatest common divisor (GCD) calculates the largest positive integer that divides both of the two input numbers without leaving a remainder.
#[node_macro::node(category("Math: Numeric"))]
fn greatest_common_divisor<T: num_traits::int::PrimInt + std::ops::ShrAssign<i32> + std::ops::SubAssign>(
	_: impl Ctx,
	/// One of the two numbers for which the GCD is calculated.
	#[implementations(u32, u64, i32)]
	value: Item<T>,
	/// The other of the two numbers for which the GCD is calculated.
	#[implementations(u32, u64, i32)]
	other_value: Item<T>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();
	let other_value = *other_value.element();

	let result = if value == T::zero() {
		other_value
	} else if other_value == T::zero() {
		value
	} else {
		binary_gcd(value, other_value)
	};

	Item::from_parts(result, attributes)
}

/// The least common multiple (LCM) calculates the smallest positive integer that is a multiple of both of the two input numbers.
#[node_macro::node(category("Math: Numeric"))]
fn least_common_multiple<T: num_traits::ToPrimitive + num_traits::FromPrimitive + num_traits::identities::Zero>(
	_: impl Ctx,
	/// One of the two numbers for which the LCM is calculated.
	#[implementations(u32, u64, i32)]
	value: Item<T>,
	/// The other of the two numbers for which the LCM is calculated.
	#[implementations(u32, u64, i32)]
	other_value: Item<T>,
) -> Item<T> {
	let (value, attributes) = value.into_parts();

	let value = value.to_i128().unwrap();
	let other_value = other_value.element().to_i128().unwrap();

	if value == 0 || other_value == 0 {
		return Item::from_parts(T::zero(), attributes);
	}
	let gcd = binary_gcd(value, other_value);

	Item::from_parts(T::from_i128((value * other_value).abs() / gcd).unwrap(), attributes)
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
fn sum(_: impl Ctx, values: List<f64>) -> Item<f64> {
	Item::new_from_element(values.iter_element_values().sum())
}

/// Averages all the numbers in the input list. An empty list gives 0.
#[node_macro::node(category("Math: Numeric"))]
fn average(_: impl Ctx, values: List<f64>) -> Item<f64> {
	let count = values.len();
	let average = if count == 0 { 0. } else { values.iter_element_values().sum::<f64>() / count as f64 };

	Item::new_from_element(average)
}

/// Gives the smallest number in the input list. An empty list gives 0.
#[node_macro::node(category("Math: Numeric"))]
fn minimum(_: impl Ctx, values: List<f64>) -> Item<f64> {
	Item::new_from_element(values.iter_element_values().copied().reduce(f64::min).unwrap_or_default())
}

/// Gives the largest number in the input list. An empty list gives 0.
#[node_macro::node(category("Math: Numeric"))]
fn maximum(_: impl Ctx, values: List<f64>) -> Item<f64> {
	Item::new_from_element(values.iter_element_values().copied().reduce(f64::max).unwrap_or_default())
}

/// Outputs true if at least one value in the input list is true. An empty list gives false.
#[node_macro::node(category("Math: Logic"))]
fn any(_: impl Ctx, values: List<bool>) -> Item<bool> {
	Item::new_from_element(values.iter_element_values().any(|&value| value))
}

/// Outputs true only if every value in the input list is true. An empty list gives true.
#[node_macro::node(category("Math: Logic"))]
fn all(_: impl Ctx, values: List<bool>) -> Item<bool> {
	Item::new_from_element(values.iter_element_values().all(|&value| value))
}

/// The less-than operation (`<`) compares two values and returns true if the first value is less than the second, or false if it is not.
/// If enabled with *Or Equal*, the less-than-or-equal operation (`<=`) is used instead.
#[node_macro::node(category("Math: Logic"))]
fn less_than<T: std::cmp::PartialOrd<T>>(
	_: impl Ctx,
	/// The number on the left-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	value: Item<T>,
	/// The number on the right-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	other_value: Item<T>,
	/// Uses the less-than-or-equal operation (`<=`) instead of the less-than operation (`<`).
	or_equal: Item<bool>,
) -> Item<bool> {
	let (value, attributes) = value.into_parts();
	let other_value = other_value.into_element();

	let result = if *or_equal.element() { value <= other_value } else { value < other_value };
	Item::from_parts(result, attributes)
}

/// The greater-than operation (`>`) compares two values and returns true if the first value is greater than the second, or false if it is not.
/// If enabled with *Or Equal*, the greater-than-or-equal operation (`>=`) is used instead.
#[node_macro::node(category("Math: Logic"))]
fn greater_than<T: std::cmp::PartialOrd<T>>(
	_: impl Ctx,
	/// The number on the left-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	value: Item<T>,
	/// The number on the right-hand side of the comparison.
	#[implementations(f64, f32, u32)]
	other_value: Item<T>,
	/// Uses the greater-than-or-equal operation (`>=`) instead of the greater-than operation (`>`).
	or_equal: Item<bool>,
) -> Item<bool> {
	let (value, attributes) = value.into_parts();
	let other_value = other_value.into_element();

	let result = if *or_equal.element() { value >= other_value } else { value > other_value };
	Item::from_parts(result, attributes)
}

/// The equality operation (`==`, `XNOR`) compares two values and returns true if they are equal, or false if they are not.
#[node_macro::node(category("Math: Logic"))]
fn equals<T: std::cmp::PartialEq<T>>(
	_: impl Ctx,
	/// One of the two values to compare for equality.
	#[implementations(f64, f32, u32, DVec2, bool, String)]
	value: Item<T>,
	/// The other of the two values to compare for equality.
	#[implementations(f64, f32, u32, DVec2, bool, String)]
	other_value: Item<T>,
) -> Item<bool> {
	let value = value.into_element();

	Item::new_from_element(other_value.into_element() == value)
}

/// The inequality operation (`!=`, `XOR`) compares two values and returns true if they are not equal, or false if they are.
#[node_macro::node(category("Math: Logic"))]
fn not_equals<T: std::cmp::PartialEq<T>>(
	_: impl Ctx,
	/// One of the two values to compare for inequality.
	#[implementations(f64, f32, u32, DVec2, bool, String)]
	value: Item<T>,
	/// The other of the two values to compare for inequality.
	#[implementations(f64, f32, u32, DVec2, bool, String)]
	other_value: Item<T>,
) -> Item<bool> {
	let value = value.into_element();

	Item::new_from_element(other_value.into_element() != value)
}

/// The logical OR operation (`||`) returns true if either of the two inputs are true, or false if both are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_or(
	_: impl Ctx,
	/// One of the two boolean values, either of which may be true for the node to output true.
	value: Item<bool>,
	/// The other of the two boolean values, either of which may be true for the node to output true.
	#[expose]
	other_value: Item<bool>,
) -> Item<bool> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value || *other_value.element(), attributes)
}

/// The logical AND operation (`&&`) returns true if both of the two inputs are true, or false if any are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_and(
	_: impl Ctx,
	/// One of the two boolean values, both of which must be true for the node to output true.
	value: Item<bool>,
	/// The other of the two boolean values, both of which must be true for the node to output true.
	#[expose]
	other_value: Item<bool>,
) -> Item<bool> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value && *other_value.element(), attributes)
}

/// The logical NOT operation (`!`) reverses true and false value of the input.
#[node_macro::node(category("Math: Logic"))]
fn logical_not(
	_: impl Ctx,
	/// The boolean value to be reversed.
	input: Item<bool>,
) -> Item<bool> {
	let (input, attributes) = input.into_parts();

	Item::from_parts(!input, attributes)
}

/// Evaluates either the "If True" or "If False" input branch based on whether the input condition is true or false.
#[node_macro::node(category("Math: Logic"))]
async fn switch<T: 'n + Send>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	condition: Item<bool>,
	#[expose]
	#[implementations(
		Context -> Item<String>,
		Context -> Item<bool>,
		Context -> Item<f32>,
		Context -> Item<f64>,
		Context -> Item<u32>,
		Context -> Item<u64>,
		Context -> Item<DVec2>,
		Context -> Item<DAffine2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Artboard>,
		Context -> Item<Bundle<String>>,
		Context -> Item<Bundle<bool>>,
		Context -> Item<Bundle<f32>>,
		Context -> Item<Bundle<f64>>,
		Context -> Item<Bundle<u32>>,
		Context -> Item<Bundle<u64>>,
		Context -> Item<Bundle<DVec2>>,
		Context -> Item<Bundle<DAffine2>>,
		Context -> Item<Bundle<Vector>>,
		Context -> Item<Bundle<Graphic>>,
		Context -> Item<Bundle<Raster<CPU>>>,
		Context -> Item<Bundle<Raster<GPU>>>,
		Context -> Item<Bundle<Color>>,
		Context -> Item<Bundle<Gradient>>,
		Context -> Item<Bundle<Artboard>>,
	)]
	if_true: impl Node<Context<'static>, Output = Item<T>>,
	#[expose]
	#[implementations(
		Context -> Item<String>,
		Context -> Item<bool>,
		Context -> Item<f32>,
		Context -> Item<f64>,
		Context -> Item<u32>,
		Context -> Item<u64>,
		Context -> Item<DVec2>,
		Context -> Item<DAffine2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Artboard>,
		Context -> Item<Bundle<String>>,
		Context -> Item<Bundle<bool>>,
		Context -> Item<Bundle<f32>>,
		Context -> Item<Bundle<f64>>,
		Context -> Item<Bundle<u32>>,
		Context -> Item<Bundle<u64>>,
		Context -> Item<Bundle<DVec2>>,
		Context -> Item<Bundle<DAffine2>>,
		Context -> Item<Bundle<Vector>>,
		Context -> Item<Bundle<Graphic>>,
		Context -> Item<Bundle<Raster<CPU>>>,
		Context -> Item<Bundle<Raster<GPU>>>,
		Context -> Item<Bundle<Color>>,
		Context -> Item<Bundle<Gradient>>,
		Context -> Item<Bundle<Artboard>>,
	)]
	if_false: impl Node<Context<'static>, Output = Item<T>>,
) -> Item<T> {
	let ctx = OwnedContextImpl::from(ctx).into_context();

	if *condition.element() { if_true.eval(ctx).await } else { if_false.eval(ctx).await }
}

/// Constructs a bool value which may be set to true or false.
#[node_macro::node(category("Value"))]
fn bool_value(_: impl Ctx, _primary: (), #[name("Bool")] bool_value: Item<bool>) -> Item<bool> {
	bool_value
}

/// Constructs a number value which may be set to any real number.
#[node_macro::node(category("Value"))]
fn number_value(_: impl Ctx, _primary: (), number: Item<f64>) -> Item<f64> {
	number
}

/// Constructs a number value which may be set to any value from 0% to 100% by dragging the slider.
#[node_macro::node(category("Value"))]
fn percentage_value(_: impl Ctx, _primary: (), percentage: Item<Percentage>) -> Item<f64> {
	percentage
}

/// Constructs a vec2 value, a two-dimensional quantity which may be set to any XY pair.
#[node_macro::node(category("Value"), name("Vec2 Value"))]
fn vec2_value(_: impl Ctx, _primary: (), #[name("Vec2")] vec2: Item<DVec2>) -> Item<DVec2> {
	vec2
}

/// Constructs a color value which may be set to any color.
#[node_macro::node(category("Value"))]
fn color_value(_: impl Ctx, _primary: (), #[default(Color::BLACK)] color: Item<Color>) -> Item<Color> {
	color
}

/// Constructs a color value from red, green, blue, and alpha components given as numbers from 0 to 1.
#[node_macro::node(category("Color"), name("RGBA to Color"))]
fn rgba_to_color(_: impl Ctx, _primary: (), red: Item<Fraction>, green: Item<Fraction>, blue: Item<Fraction>, #[default(1.)] alpha: Item<Fraction>) -> Item<Color> {
	let red = (*red.element() as f32).clamp(0., 1.);
	let green = (*green.element() as f32).clamp(0., 1.);
	let blue = (*blue.element() as f32).clamp(0., 1.);
	let alpha = (*alpha.element() as f32).clamp(0., 1.);

	// RGB user inputs are interpreted as sRGB display values; lift to linear-light for the internal `Color`
	Item::new_from_element(Color::from_gamma_srgb_channels(red, green, blue, alpha))
}

/// Constructs a color value from hue, saturation, value, and alpha components given as numbers from 0 to 1.
#[node_macro::node(category("Color"), name("HSVA to Color"))]
fn hsva_to_color(_: impl Ctx, _primary: (), hue: Item<Fraction>, #[default(1.)] saturation: Item<Fraction>, #[default(1.)] value: Item<Fraction>, #[default(1.)] alpha: Item<Fraction>) -> Item<Color> {
	let hue = (*hue.element() as f32) - (*hue.element() as f32).floor();
	let saturation = (*saturation.element() as f32).clamp(0., 1.);
	let value = (*value.element() as f32).clamp(0., 1.);
	let alpha = (*alpha.element() as f32).clamp(0., 1.);

	Item::new_from_element(Color::from_hsva(hue, saturation, value, alpha))
}

/// Constructs a color value from hue, saturation, lightness, and alpha components given as numbers from 0 to 1.
#[node_macro::node(category("Color"), name("HSLA to Color"))]
fn hsla_to_color(
	_: impl Ctx,
	_primary: (),
	hue: Item<Fraction>,
	#[default(1.)] saturation: Item<Fraction>,
	#[default(0.5)] lightness: Item<Fraction>,
	#[default(1.)] alpha: Item<Fraction>,
) -> Item<Color> {
	let hue = (*hue.element() as f32) - (*hue.element() as f32).floor();
	let saturation = (*saturation.element() as f32).clamp(0., 1.);
	let lightness = (*lightness.element() as f32).clamp(0., 1.);
	let alpha = (*alpha.element() as f32).clamp(0., 1.);

	Item::new_from_element(Color::from_hsla(hue, saturation, lightness, alpha))
}

/// Constructs a color value from a CSS color string. Accepts hex (`#RRGGBB`, `#RRGGBBAA`, plus bare and shorthand variants), CSS named colors (like `red`), and functional notations (`rgb(...)`, `hsl(...)`, etc.). Invalid inputs produce a transparent color.
#[node_macro::node(category("Color"), name("Hex to Color"))]
fn hex_to_color(_: impl Ctx, hex_code: Item<String>) -> Item<Color> {
	let color = core_types::misc::parse_css_color(hex_code.element()).unwrap_or_default();
	Item::new_from_element(color)
}

/// Constructs a gradient value which may be set to any sequence of color stops to represent the transition between colors.
#[node_macro::node(category("Value"))]
fn gradient_value(_: impl Ctx, _primary: (), gradient: Item<Gradient>) -> Item<Gradient> {
	gradient
}

/// Sets the type (linear or radial) of each gradient in the input list.
#[node_macro::node(category("Color"))]
fn gradient_type(_: impl Ctx, gradient: Item<Gradient>, gradient_type: Item<vector_types::GradientType>) -> Item<Gradient> {
	let mut gradient = gradient;
	gradient.set_attribute(core_types::ATTR_GRADIENT_TYPE, *gradient_type.element());
	gradient
}

/// Sets how each gradient in the input list extends past its endpoints: Pad, Reflect, or Repeat.
#[node_macro::node(category("Color"))]
fn spread_method(_: impl Ctx, gradient: Item<Gradient>, spread_method: Item<vector_types::GradientSpreadMethod>) -> Item<Gradient> {
	let mut gradient = gradient;
	gradient.set_attribute(core_types::ATTR_SPREAD_METHOD, *spread_method.element());
	gradient
}

/// Gets the color at the specified position along the gradient, given a position from 0 (left) to 1 (right).
#[node_macro::node(category("Color"))]
fn sample_gradient(_: impl Ctx, _primary: (), gradient: Item<Gradient>, position: Item<Fraction>) -> Item<Color> {
	let position = position.element().clamp(0., 1.);
	let color = gradient.element().evaluate(position);
	Item::new_from_element(color)
}

/// Constructs a footprint value which may be set to any transformation of a unit square describing a render area, and a render resolution at least 1x1 integer pixels.
#[node_macro::node(category("Value"))]
fn footprint_value(_: impl Ctx, _primary: (), transform: Item<DAffine2>, #[default(100., 100.)] resolution: Item<PixelSize>) -> Item<Footprint> {
	Item::new_from_element(Footprint {
		transform: *transform.element(),
		resolution: resolution.element().max(DVec2::ONE).as_uvec2(),
		..Default::default()
	})
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
	x: Item<f64>,
	/// The Y component of the vec2.
	#[expose]
	y: Item<f64>,
) -> Item<DVec2> {
	Item::new_from_element(DVec2::new(*x.element(), *y.element()))
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
	value: Item<DVec2>,
	/// The other operand of the dot product operation.
	#[default(1., 0.)]
	other_value: Item<DVec2>,
	/// Whether to normalize both input vec2s so the calculation ranges in `[-1, 1]` by considering only their degree of directional alignment.
	normalize: Item<bool>,
) -> Item<f64> {
	let (value, attributes) = value.into_parts();
	let other_value = *other_value.element();

	let result = if *normalize.element() {
		value.normalize_or_zero().dot(other_value.normalize_or_zero())
	} else {
		value.dot(other_value)
	};

	Item::from_parts(result, attributes)
}

/// The cross product operation (`×`) calculates the signed area of the parallelogram formed by a vec2 pair.
///
/// The sign gives the rotation direction from the first vec2 to the second: positive for clockwise, negative for counterclockwise, and 0 when both are parallel, as drawn in the viewport.
#[node_macro::node(category("Math: Vec2"))]
fn cross_product(
	_: impl Ctx,
	/// The vec2 on the left-hand side of the cross product operation.
	value: Item<DVec2>,
	/// The vec2 on the right-hand side of the cross product operation.
	#[default(1., 0.)]
	other_value: Item<DVec2>,
) -> Item<f64> {
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.perp_dot(*other_value.element()), attributes)
}

/// Calculates the angle swept between two vectors.
///
/// The value is always positive and ranges from 0° (both vectors point the same direction) to 180° (both vectors point opposite directions).
#[node_macro::node(category("Math: Vec2"))]
fn angle_between(_: impl Ctx, vector_a: Item<DVec2>, vector_b: Item<DVec2>, radians: Item<bool>) -> Item<f64> {
	let (vector_a, attributes) = vector_a.into_parts();

	let dot_product = vector_a.normalize_or_zero().dot(vector_b.element().normalize_or_zero());
	let angle = dot_product.acos();
	let result = if *radians.element() { angle } else { angle.to_degrees() };
	Item::from_parts(result, attributes)
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
	position_from: Item<T>,
	/// The position toward which the angle is measured.
	#[expose]
	#[implementations(DVec2, DVec2, DAffine2, DAffine2)]
	position_to: Item<U>,
	/// Whether the resulting angle should be given in radians instead of degrees.
	radians: Item<bool>,
) -> Item<f64> {
	let (position_from, attributes) = position_from.into_parts();

	let from = position_from.to_position();
	let to = position_to.into_element().to_position();
	let delta = to - from;
	let angle = delta.y.atan2(delta.x);
	let result = if *radians.element() { angle } else { angle.to_degrees() };
	Item::from_parts(result, attributes)
}

/// The magnitude operator (`‖x‖`) calculates the length of a vec2, which is the distance from the base to the tip of the arrow it represents.
#[node_macro::node(category("Math: Vec2"))]
fn magnitude(_: impl Ctx, vec2: Item<DVec2>) -> Item<f64> {
	let (vec2, attributes) = vec2.into_parts();

	Item::from_parts(vec2.length(), attributes)
}

/// Measures the distance between two points, which is the length of the straight line segment connecting them.
#[node_macro::node(category("Math: Vec2"))]
fn distance(
	_: impl Ctx,
	/// The point the distance is measured from.
	position_from: Item<DVec2>,
	/// The point the distance is measured to.
	position_to: Item<DVec2>,
) -> Item<f64> {
	let (position_from, attributes) = position_from.into_parts();

	Item::from_parts(position_from.distance(*position_to.element()), attributes)
}

/// Scales the input vec2 to unit length while preserving its direction. This is equivalent to dividing the input vec2 by its own magnitude.
///
/// Returns 0 when the input vec2 has zero length.
#[node_macro::node(category("Math: Vec2"))]
fn normalize(_: impl Ctx, vec2: Item<DVec2>) -> Item<DVec2> {
	let (vec2, attributes) = vec2.into_parts();

	Item::from_parts(vec2.normalize_or_zero(), attributes)
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::Node;
	use core_types::generic::FnNode;

	#[test]
	pub fn dot_product_function() {
		let vector_a = Item::new_from_element(DVec2::new(1., 2.));
		let vector_b = Item::new_from_element(DVec2::new(3., 4.));
		assert_eq!(dot_product((), vector_a, vector_b, Item::new_from_element(false)).into_element(), 11.);
	}

	#[test]
	pub fn magnitude_function() {
		let vector = Item::new_from_element(DVec2::new(3., 4.));
		assert_eq!(magnitude((), vector).into_element(), 5.);
	}

	#[test]
	pub fn distance_function() {
		let (position_from, position_to) = (Item::new_from_element(DVec2::new(1., 2.)), Item::new_from_element(DVec2::new(4., 6.)));
		assert_eq!(distance((), position_from, position_to).into_element(), 5.);
	}

	#[test]
	pub fn cross_product_sign() {
		let vec2 = |x, y| Item::new_from_element(DVec2::new(x, y));
		assert_eq!(cross_product((), vec2(1., 0.), vec2(0., 1.)).into_element(), 1.);
		assert_eq!(cross_product((), vec2(0., 1.), vec2(1., 0.)).into_element(), -1.);
		assert_eq!(cross_product((), vec2(2., 2.), vec2(1., 1.)).into_element(), 0.);
	}

	#[test]
	pub fn sign_of_negative_zero_is_positive_zero() {
		let result = sign((), Item::new_from_element(-0.0_f64)).into_element();
		assert_eq!(result, 0.);
		assert!(result.is_sign_positive());
	}

	#[test]
	pub fn sign_componentwise() {
		assert_eq!(sign((), Item::new_from_element(DVec2::new(-5., 3.))).into_element(), DVec2::new(-1., 1.));
	}

	#[test]
	pub fn lerp_endpoints_are_exact() {
		let lerp_between = |factor, clamped| {
			lerp(
				(),
				Item::new_from_element(3.),
				Item::new_from_element(7.),
				Item::new_from_element(factor),
				Item::new_from_element(clamped),
			)
			.into_element()
		};
		assert_eq!(lerp_between(0., true), 3.);
		assert_eq!(lerp_between(1., true), 7.);
		assert_eq!(lerp_between(0.5, true), 5.);
	}

	#[test]
	pub fn lerp_clamped_and_extrapolated() {
		let lerp_between = |factor, clamped| {
			lerp(
				(),
				Item::new_from_element(0.),
				Item::new_from_element(10.),
				Item::new_from_element(factor),
				Item::new_from_element(clamped),
			)
			.into_element()
		};
		assert_eq!(lerp_between(2., true), 10.);
		assert_eq!(lerp_between(2., false), 20.);
	}

	#[test]
	pub fn lerp_endpoint_factors_pass_endpoints_through() {
		let lerp_between = |start: f64, end: f64, factor| {
			lerp(
				(),
				Item::new_from_element(start),
				Item::new_from_element(end),
				Item::new_from_element(factor),
				Item::new_from_element(true),
			)
			.into_element()
		};
		assert_eq!(lerp_between(3., f64::INFINITY, 0.), 3.);
		assert_eq!(lerp_between(f64::NAN, 7., 1.), 7.);
		assert_eq!(lerp_between(3., f64::INFINITY, 1.), f64::INFINITY);
		assert!(lerp_between(-0., 7., 0.).is_sign_negative());
		assert!(lerp_between(5., -0., 1.).is_sign_negative());
	}

	#[test]
	pub fn clamp_vec2_within_swapped_bounds() {
		let vec2 = |x, y| Item::new_from_element(DVec2::new(x, y));
		assert_eq!(clamp((), vec2(-5., 5.), vec2(1., 1.), vec2(0., 2.)).into_element(), DVec2::new(0., 2.));
	}

	#[test]
	pub fn min_max_vec2_with_scalar() {
		let vec2 = |x, y| Item::new_from_element(DVec2::new(x, y));
		assert_eq!(super::min((), vec2(-5., 5.), Item::new_from_element(0_f64)).into_element(), DVec2::new(-5., 0.));
		assert_eq!(super::max((), vec2(-5., 5.), Item::new_from_element(0_f64)).into_element(), DVec2::new(0., 5.));
	}

	#[test]
	pub fn scalar_with_vec2_operand_orders() {
		let vec2 = |x, y| Item::new_from_element(DVec2::new(x, y));
		assert_eq!(super::min((), Item::new_from_element(0_f64), vec2(-5., 5.)).into_element(), DVec2::new(-5., 0.));
		assert_eq!(super::max((), Item::new_from_element(0_f64), vec2(-5., 5.)).into_element(), DVec2::new(0., 5.));
		assert_eq!(exponent((), Item::new_from_element(2_f64), vec2(2., 3.)).into_element(), DVec2::new(4., 8.));
		assert_eq!(root((), Item::new_from_element(64_f64), vec2(2., 3.)).into_element(), DVec2::new(8., 4.));
		assert_eq!(logarithm((), Item::new_from_element(8_f64), vec2(2., 10.)).into_element(), DVec2::new(3., 8_f64.log10()));
		assert_eq!(clamp((), Item::new_from_element(5_f64), vec2(0., 6.), vec2(1., 10.)).into_element(), DVec2::new(1., 6.));
	}

	#[test]
	pub fn vec2_degrees_and_bases() {
		let vec2 = |x, y| Item::new_from_element(DVec2::new(x, y));
		assert_eq!(root((), vec2(64., 27.), vec2(2., 3.)).into_element(), DVec2::new(8., 3.));
		assert_eq!(logarithm((), vec2(8., 100.), vec2(2., 10.)).into_element(), DVec2::new(3., 2.));
	}

	#[test]
	pub fn logarithm_f32_base_e_and_near_e() {
		assert_eq!(
			logarithm((), Item::new_from_element(8_f32), Item::new_from_element(std::f32::consts::E)).into_element(),
			8_f64.ln() as f32
		);
		assert_eq!(
			logarithm((), Item::new_from_element(8_f32), Item::new_from_element(2.7_f32)).into_element(),
			8_f64.log(2.7_f32 as f64) as f32
		);
	}

	#[test]
	pub fn round_floor_ceiling_vec2() {
		let vec2 = |x, y| Item::new_from_element(DVec2::new(x, y));
		assert_eq!(round((), vec2(1.5, -1.4)).into_element(), DVec2::new(2., -1.));
		assert_eq!(floor((), vec2(1.9, -1.1)).into_element(), DVec2::new(1., -2.));
		assert_eq!(ceiling((), vec2(1.1, -1.9)).into_element(), DVec2::new(2., -1.));
	}

	#[test]
	fn test_basic_expression() {
		let result = math((), Item::new_from_element(0.), Item::new_from_element("2 + 2".to_string()), Item::new_from_element(0.));
		assert_eq!(result.into_element(), 4.);
	}

	#[test]
	fn test_complex_expression() {
		let result = math((), Item::new_from_element(0.), Item::new_from_element("(5 * 3) + (10 / 2)".to_string()), Item::new_from_element(0.));
		assert_eq!(result.into_element(), 20.);
	}

	#[test]
	fn test_default_expression() {
		let result = math((), Item::new_from_element(0.), Item::new_from_element("0".to_string()), Item::new_from_element(0.));
		assert_eq!(result.into_element(), 0.);
	}

	#[test]
	fn test_invalid_expression() {
		let result = math((), Item::new_from_element(0.), Item::new_from_element("invalid".to_string()), Item::new_from_element(0.));
		assert_eq!(result.into_element(), 0.);
	}

	#[test]
	pub fn foo() {
		let fnn = FnNode::new(|(a, b)| (b, a));
		assert_eq!(fnn.eval((1u32, 2u32)), (2, 1));
	}

	#[test]
	pub fn add_vectors() {
		assert_eq!(super::add((), Item::new_from_element(DVec2::ONE), Item::new_from_element(DVec2::ONE)).into_element(), DVec2::ONE * 2.);
	}

	#[test]
	pub fn subtract_f64() {
		assert_eq!(super::subtract((), Item::new_from_element(5_f64), Item::new_from_element(3_f64)).into_element(), 2.);
	}

	#[test]
	pub fn divide_vectors() {
		assert_eq!(super::divide((), Item::new_from_element(DVec2::ONE), Item::new_from_element(2_f64)).into_element(), DVec2::ONE / 2.);
	}

	#[test]
	pub fn divide_vector_by_partially_zero_vector() {
		let (numerator, denominator) = (Item::new_from_element(DVec2::new(1., 2.)), Item::new_from_element(DVec2::new(2., 0.)));
		assert_eq!(super::divide((), numerator, denominator).into_element(), DVec2::new(0.5, 0.));
	}

	#[test]
	pub fn modulo_positive() {
		assert_eq!(
			super::modulo((), Item::new_from_element(-5_f64), Item::new_from_element(2_f64), Item::new_from_element(true)).into_element(),
			1_f64
		);
	}

	#[test]
	pub fn modulo_negative() {
		assert_eq!(
			super::modulo((), Item::new_from_element(-5_f64), Item::new_from_element(2_f64), Item::new_from_element(false)).into_element(),
			-1_f64
		);
	}
}
