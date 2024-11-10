use crate::raster::BlendMode;
use crate::raster::ImageFrame;
use crate::registry::types::Percentage;
use crate::vector::style::GradientStops;
use crate::{Color, Node};

use core::marker::PhantomData;
use core::ops::{Add, Div, Mul, Rem, Sub};
use glam::DVec2;
use num_traits::Pow;
use rand::{Rng, SeedableRng};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

/// The addition operation (+) calculates the sum of two numbers.
#[node_macro::node(category("Math: Arithmetic"))]
fn add<U: Add<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, DVec2, f64, DVec2)] augend: U,
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32, DVec2, DVec2, f64)] addend: T,
) -> <U as Add<T>>::Output {
	augend + addend
}

/// The subtraction operation (-) calculates the difference between two numbers.
#[node_macro::node(category("Math: Arithmetic"))]
fn subtract<U: Sub<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, DVec2, f64, DVec2)] minuend: U,
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32, DVec2, DVec2, f64)] subtrahend: T,
) -> <U as Sub<T>>::Output {
	minuend - subtrahend
}

/// The multiplication operation (×) calculates the product of two numbers.
#[node_macro::node(category("Math: Arithmetic"))]
fn multiply<U: Mul<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, DVec2, f64, DVec2)] multiplier: U,
	#[default(1.)]
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32, DVec2, DVec2, f64)]
	multiplicand: T,
) -> <U as Mul<T>>::Output {
	multiplier * multiplicand
}

/// The division operation (÷) calculates the quotient of two numbers.
#[node_macro::node(category("Math: Arithmetic"))]
fn divide<U: Div<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, DVec2, DVec2, f64)] numerator: U,
	#[default(1.)]
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32, DVec2, f64, DVec2)]
	denominator: T,
) -> <U as Div<T>>::Output {
	numerator / denominator
}

/// The modulo operation (%) calculates the remainder from the division of two numbers. The sign of the result shares the sign of the numerator unless "Always Positive" is enabled.
#[node_macro::node(category("Math: Arithmetic"))]
fn modulo<U: Rem<T, Output: Add<T, Output: Rem<T, Output = U::Output>>>, T: Copy>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, DVec2, DVec2, f64)] numerator: U,
	#[default(2.)]
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32, DVec2, f64, DVec2)]
	modulus: T,
	always_positive: bool,
) -> <U as Rem<T>>::Output {
	if always_positive {
		(numerator % modulus + modulus) % modulus
	} else {
		numerator % modulus
	}
}

/// The exponent operation (^) calculates the result of raising a number to a power.
#[node_macro::node(category("Math: Arithmetic"))]
fn exponent<U: Pow<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32)] base: U,
	#[default(2.)]
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32)]
	power: T,
) -> <U as num_traits::Pow<T>>::Output {
	base.pow(power)
}

/// The square root operation (√) calculates the nth root of a number, equivalent to raising the number to the power of 1/n.
#[node_macro::node(category("Math: Arithmetic"))]
fn root<U: num_traits::float::Float>(
	_: (),
	#[default(2.)]
	#[implementations(f64, f32)]
	radicand: U,
	#[default(2.)]
	#[implementations(f64, f32)]
	degree: U,
) -> U {
	if degree == U::from(2.).unwrap() {
		radicand.sqrt()
	} else if degree == U::from(3.).unwrap() {
		radicand.cbrt()
	} else {
		radicand.powf(U::from(1.).unwrap() / degree)
	}
}

/// The logarithmic function (log) calculates the logarithm of a number with a specified base. If the natural logarithm function (ln) is desired, set the base to "e".
#[node_macro::node(category("Math: Arithmetic"))]
fn logarithm<U: num_traits::float::Float>(
	_: (),
	#[implementations(f64, f32)] value: U,
	#[default(2.)]
	#[implementations(f64, f32)]
	base: U,
) -> U {
	if base == U::from(2.).unwrap() {
		value.log2()
	} else if base == U::from(10.).unwrap() {
		value.log10()
	} else if base - U::from(std::f64::consts::E).unwrap() < U::epsilon() * U::from(1e6).unwrap() {
		value.ln()
	} else {
		value.log(base)
	}
}

/// The sine trigonometric function (sin) calculates the ratio of the angle's opposite side length to its hypotenuse length.
#[node_macro::node(category("Math: Trig"))]
fn sine<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] theta: U, radians: bool) -> U {
	if radians {
		theta.sin()
	} else {
		theta.to_radians().sin()
	}
}

/// The cosine trigonometric function (cos) calculates the ratio of the angle's adjacent side length to its hypotenuse length.
#[node_macro::node(category("Math: Trig"))]
fn cosine<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] theta: U, radians: bool) -> U {
	if radians {
		theta.cos()
	} else {
		theta.to_radians().cos()
	}
}

/// The tangent trigonometric function (tan) calculates the ratio of the angle's opposite side length to its adjacent side length.
#[node_macro::node(category("Math: Trig"))]
fn tangent<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] theta: U, radians: bool) -> U {
	if radians {
		theta.tan()
	} else {
		theta.to_radians().tan()
	}
}

/// The inverse sine trigonometric function (asin) calculates the angle whose sine is the specified value.
#[node_macro::node(category("Math: Trig"))]
fn sine_inverse<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] value: U, radians: bool) -> U {
	if radians {
		value.asin()
	} else {
		value.asin().to_degrees()
	}
}

/// The inverse cosine trigonometric function (acos) calculates the angle whose cosine is the specified value.
#[node_macro::node(category("Math: Trig"))]
fn cosine_inverse<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] value: U, radians: bool) -> U {
	if radians {
		value.acos()
	} else {
		value.acos().to_degrees()
	}
}

/// The inverse tangent trigonometric function (atan) calculates the angle whose tangent is the specified value.
#[node_macro::node(category("Math: Trig"))]
fn tangent_inverse<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] value: U, radians: bool) -> U {
	if radians {
		value.atan()
	} else {
		value.atan().to_degrees()
	}
}

/// The inverse tangent trigonometric function (atan2) calculates the angle whose tangent is the ratio of the two specified values.
#[node_macro::node(name("Tangent Inverse 2-Argument"), category("Math: Trig"))]
fn tangent_inverse_2_argument<U: num_traits::float::Float>(
	_: (),
	#[implementations(f64, f32)] y: U,
	#[expose]
	#[implementations(f64, f32)]
	x: U,
	radians: bool,
) -> U {
	if radians {
		y.atan2(x)
	} else {
		y.atan2(x).to_degrees()
	}
}

/// The random function (rand) converts a seed into a random number within the specified range, inclusive of the minimum and exclusive of the maximum. The minimum and maximum values are automatically swapped if they are reversed.
#[node_macro::node(category("Math: Numeric"))]
fn random<U: num_traits::float::Float>(
	_: (),
	_primary: (),
	seed: u64,
	#[implementations(f64, f32)]
	#[default(0.)]
	min: U,
	#[implementations(f64, f32)]
	#[default(1.)]
	max: U,
) -> f64 {
	let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
	let result = rng.gen::<f64>();
	let (min, max) = if min < max { (min, max) } else { (max, min) };
	let (min, max) = (min.to_f64().unwrap(), max.to_f64().unwrap());
	result * (max - min) + min
}

/// Convert a number to an integer of the type u32, which may be the required type for certain node inputs. This will be removed in the future when automatic type conversion is implemented.
#[node_macro::node(name("To u32"), category("Math: Numeric"))]
fn to_u32<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] value: U) -> u32 {
	let value = U::clamp(value, U::from(0.).unwrap(), U::from(u32::MAX as f64).unwrap());
	value.to_u32().unwrap()
}

/// Convert a number to an integer of the type u64, which may be the required type for certain node inputs. This will be removed in the future when automatic type conversion is implemented.
#[node_macro::node(name("To u64"), category("Math: Numeric"))]
fn to_u64<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] value: U) -> u64 {
	let value = U::clamp(value, U::from(0.).unwrap(), U::from(u64::MAX as f64).unwrap());
	value.to_u64().unwrap()
}

/// The rounding function (round) maps an input value to its nearest whole number. Halfway values are rounded away from zero.
#[node_macro::node(category("Math: Numeric"))]
fn round<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] value: U) -> U {
	value.round()
}

/// The floor function (floor) reduces an input value to its nearest larger whole number, unless the input number is already whole.
#[node_macro::node(category("Math: Numeric"))]
fn floor<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] value: U) -> U {
	value.floor()
}

/// The ceiling function (ceil) increases an input value to its nearest smaller whole number, unless the input number is already whole.
#[node_macro::node(category("Math: Numeric"))]
fn ceiling<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] value: U) -> U {
	value.ceil()
}

/// The absolute value function (abs) removes the negative sign from an input value, if present.
#[node_macro::node(category("Math: Numeric"))]
fn absolute_value<U: num_traits::float::Float>(_: (), #[implementations(f64, f32)] value: U) -> U {
	value.abs()
}

/// The minimum function (min) picks the smaller of two numbers.
#[node_macro::node(category("Math: Numeric"))]
fn min<T: core::cmp::PartialOrd>(_: (), #[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] value: T, #[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] other_value: T) -> T {
	match value < other_value {
		true => value,
		false => other_value,
	}
}

/// The maximum function (max) picks the larger of two numbers.
#[node_macro::node(category("Math: Numeric"))]
fn max<T: core::cmp::PartialOrd>(_: (), #[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] value: T, #[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] other_value: T) -> T {
	match value > other_value {
		true => value,
		false => other_value,
	}
}

/// The clamp function (clamp) restricts a number to a specified range between a minimum and maximum value. The minimum and maximum values are automatically swapped if they are reversed.
#[node_macro::node(category("Math: Numeric"))]
fn clamp<T: core::cmp::PartialOrd>(
	_: (),
	#[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] value: T,
	#[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] min: T,
	#[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] max: T,
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

/// The equality operation (==) compares two values and returns true if they are equal, or false if they are not.
#[node_macro::node(category("Math: Logic"))]
fn equals<U: core::cmp::PartialEq<T>, T>(
	_: (),
	#[implementations(f64, &f64, f32, &f32, u32, &u32, DVec2, &DVec2, &str)] value: T,
	#[implementations(f64, &f64, f32, &f32, u32, &u32, DVec2, &DVec2, &str)]
	#[min(100.)]
	#[max(200.)]
	other_value: U,
) -> bool {
	other_value == value
}

/// The inequality operation (!=) compares two values and returns true if they are not equal, or false if they are.
#[node_macro::node(category("Math: Logic"))]
fn not_equals<U: core::cmp::PartialEq<T>, T>(
	_: (),
	#[implementations(f64, &f64, f32, &f32, u32, &u32, DVec2, &DVec2, &str)] value: T,
	#[implementations(f64, &f64, f32, &f32, u32, &u32, DVec2, &DVec2, &str)]
	#[min(100.)]
	#[max(200.)]
	other_value: U,
) -> bool {
	other_value != value
}

/// The logical or operation (||) returns true if either of the two inputs are true, or false if both are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_or(_: (), value: bool, other_value: bool) -> bool {
	value || other_value
}

/// The logical and operation (&&) returns true if both of the two inputs are true, or false if any are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_and(_: (), value: bool, other_value: bool) -> bool {
	value && other_value
}

/// The logical not operation (!) reverses true and false value of the input.
#[node_macro::node(category("Math: Logic"))]
fn logical_not(_: (), input: bool) -> bool {
	!input
}

/// Constructs a bool value which may be set to true or false.
#[node_macro::node(category("Value"))]
fn bool_value(_: (), _primary: (), #[name("Bool")] bool_value: bool) -> bool {
	bool_value
}

/// Constructs a number value which may be set to any real number.
#[node_macro::node(category("Value"))]
fn number_value(_: (), _primary: (), number: f64) -> f64 {
	number
}

/// Constructs a number value which may be set to any value from 0% to 100% by dragging the slider.
#[node_macro::node(category("Value"))]
fn percentage_value(_: (), _primary: (), percentage: Percentage) -> f64 {
	percentage
}

/// Constructs a two-dimensional vector value which may be set to any XY coordinate.
#[node_macro::node(category("Value"))]
fn vector2_value(_: (), _primary: (), x: f64, y: f64) -> DVec2 {
	DVec2::new(x, y)
}

/// Constructs a color value which may be set to any color, or no color.
#[node_macro::node(category("Value"))]
fn color_value(_: (), _primary: (), #[default(Color::BLACK)] color: Option<Color>) -> Option<Color> {
	color
}

/// Constructs a gradient value which may be set to any sequence of color stops to represent the transition between colors.
#[node_macro::node(category("Value"))]
fn gradient_value(_: (), _primary: (), gradient: GradientStops) -> GradientStops {
	gradient
}

/// Constructs a blend mode choice value which may be set to any of the available blend modes in order to tell another node which blending operation to use.
#[node_macro::node(category("Value"))]
fn blend_mode_value(_: (), _primary: (), blend_mode: BlendMode) -> BlendMode {
	blend_mode
}

/// Meant for debugging purposes, not general use. Returns the size of the input type in bytes.
#[cfg(feature = "std")]
#[node_macro::node(category("Debug"))]
fn size_of(_: (), ty: crate::Type) -> Option<usize> {
	ty.size()
}

/// Meant for debugging purposes, not general use. Wraps the input value in the Some variant of an Option.
#[node_macro::node(category("Debug"))]
fn some<T>(_: (), #[implementations(f64, f32, u32, u64, String, Color)] input: T) -> Option<T> {
	Some(input)
}

/// Meant for debugging purposes, not general use. Unwraps the input value from an Option, returning the default value if the input is None.
#[node_macro::node(category("Debug"))]
fn unwrap<T: Default>(_: (), #[implementations(Option<f64>, Option<f32>, Option<u32>, Option<u64>, Option<String>, Option<Color>)] input: Option<T>) -> T {
	input.unwrap_or_default()
}

/// Meant for debugging purposes, not general use. Clones the input value.
#[node_macro::node(category("Debug"))]
fn clone<'i, T: Clone + 'i>(_: (), #[implementations(&ImageFrame<Color>)] value: &'i T) -> T {
	value.clone()
}

// TODO: Rename to "Passthrough"
/// Passes-through the input value without changing it. This is useful for rerouting wires for organization purposes.
#[node_macro::node(skip_impl)]
fn identity<'i, T: 'i>(value: T) -> T {
	value
}

// Type
// TODO: Document this
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TypeNode<N: for<'a> Node<'a, I>, I, O>(pub N, pub PhantomData<(I, O)>);
impl<'i, N, I: 'i, O: 'i> Node<'i, I> for TypeNode<N, I, O>
where
	N: for<'n> Node<'n, I, Output = O>,
{
	type Output = O;
	fn eval(&'i self, input: I) -> Self::Output {
		self.0.eval(input)
	}

	fn reset(&self) {
		self.0.reset();
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any>> {
		self.0.serialize()
	}
}
impl<'i, N: for<'a> Node<'a, I>, I: 'i> TypeNode<N, I, <N as Node<'i, I>>::Output> {
	pub fn new(node: N) -> Self {
		Self(node, PhantomData)
	}
}
impl<'i, N: for<'a> Node<'a, I> + Clone, I: 'i> Clone for TypeNode<N, I, <N as Node<'i, I>>::Output> {
	fn clone(&self) -> Self {
		Self(self.0.clone(), self.1)
	}
}
impl<'i, N: for<'a> Node<'a, I> + Copy, I: 'i> Copy for TypeNode<N, I, <N as Node<'i, I>>::Output> {}

// Into
pub struct IntoNode<O> {
	_o: PhantomData<O>,
}
#[cfg(feature = "alloc")]
#[node_macro::old_node_fn(IntoNode<_O>)]
async fn into<I, _O>(input: I) -> _O
where
	I: Into<_O> + Sync + Send,
{
	input.into()
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{generic::*, structural::*, value::*};

	#[test]
	pub fn identity_node() {
		let value = ValueNode(4u32).then(IdentityNode::new());
		assert_eq!(value.eval(()), &4);
	}
	#[test]
	pub fn foo() {
		let fnn = FnNode::new(|(a, b)| (b, a));
		assert_eq!(fnn.eval((1u32, 2u32)), (2, 1));
	}
}
