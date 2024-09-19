use crate::registry::types::Percentage;
use crate::Node;

use core::marker::PhantomData;
use core::ops::{Add, Div, Mul, Rem, Sub};
use num_traits::Pow;
use rand::{Rng, SeedableRng};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

// Add
#[node_macro::node(category("Math: Arithmetic"))]
fn add<U: Add<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, glam::DVec2)] augend: U,
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32, glam::DVec2)] addend: T,
) -> <U as Add<T>>::Output {
	augend + addend
}

// Subtract
#[node_macro::node(category("Math: Arithmetic"))]
fn subtract<U: Sub<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, glam::DVec2)] minuend: U,
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32, glam::DVec2)] subtrahend: T,
) -> <U as Sub<T>>::Output {
	minuend - subtrahend
}

// Multiply
#[node_macro::node(category("Math: Arithmetic"))]
fn multiply<U: Mul<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, glam::DVec2, f64)] multiplier: U,
	#[default(1.)]
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32, glam::DVec2, glam::DVec2)]
	multiplicand: T,
) -> <U as Mul<T>>::Output {
	multiplier * multiplicand
}

// Divide
#[node_macro::node(category("Math: Arithmetic"))]
fn divide<U: Div<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, glam::DVec2, glam::DVec2)] numerator: U,
	#[default(1.)]
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32, glam::DVec2, f64)]
	denominator: T,
) -> <U as Div<T>>::Output {
	numerator / denominator
}

// Modulo
#[node_macro::node(category("Math: Arithmetic"))]
fn modulo<U: Rem<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32)] numerator: U,
	#[default(2.)]
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32)]
	modulus: T,
) -> <U as Rem<T>>::Output {
	numerator % modulus
}

// Exponent
#[node_macro::node(category("Math: Arithmetic"))]
fn exponent<U: Pow<T>, T>(
	_: (),
	#[implementations(f64, &f64, f64, &f64, f32, &f32, f32, &f32, u32, &u32, u32, &u32, )] base: U,
	#[default(2.)]
	#[implementations(f64, f64, &f64, &f64, f32, f32, &f32, &f32, u32, u32, &u32, &u32)]
	power: T,
) -> <U as num_traits::Pow<T>>::Output {
	base.pow(power)
}

// Root
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

// Logarithm
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

// Sine
#[node_macro::node(category("Math: Trig"))]
fn sine(_: (), theta: f64) -> f64 {
	theta.sin()
}

// Cosine
#[node_macro::node(category("Math: Trig"))]
fn cosine(_: (), theta: f64) -> f64 {
	theta.cos()
}

// Tangent
#[node_macro::node(category("Math: Trig"))]
fn tangent(_: (), theta: f64) -> f64 {
	theta.tan()
}

// Random
#[node_macro::node(category("Math: Numeric"))]
fn random(_: (), _primary: (), seed: u64, min: f64, #[default(1.)] max: f64) -> f64 {
	let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
	let result = rng.gen::<f64>();
	let (min, max) = if min < max { (min, max) } else { (max, min) };
	result * (max - min) + min
}

// Round
#[node_macro::node(category("Math: Numeric"))]
fn round(_: (), value: f64) -> f64 {
	value.round()
}

// Floor
#[node_macro::node(category("Math: Numeric"))]
fn floor(_: (), value: f64) -> f64 {
	value.floor()
}

// Ceiling
#[node_macro::node(category("Math: Numeric"))]
fn ceiling(_: (), value: f64) -> f64 {
	value.ceil()
}

// Absolute Value
#[node_macro::node(category("Math: Numeric"))]
fn absolute_value(_: (), value: f64) -> f64 {
	value.abs()
}

// Min
#[node_macro::node(category("Math: Numeric"))]
fn min<T: core::cmp::PartialOrd>(_: (), #[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] value: T, #[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] other_value: T) -> T {
	match value < other_value {
		true => value,
		false => other_value,
	}
}

// Max
#[node_macro::node(category("Math: Numeric"))]
fn max<T: core::cmp::PartialOrd>(_: (), #[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] value: T, #[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] other_value: T) -> T {
	match value > other_value {
		true => value,
		false => other_value,
	}
}

// Equals
#[node_macro::node(category("Math: Logic"))]
fn equals<U: core::cmp::PartialEq<T>, T>(
	_: (),
	#[implementations(f64, &f64, f32, &f32, u32, &u32, &str)] value: T,
	#[implementations(f64, &f64, f32, &f32, u32, &u32, &str)]
	#[min(100.)]
	#[max(200.)]
	other_value: U,
) -> bool {
	other_value == value
}

// Logical Or
#[node_macro::node(category("Math: Logic"))]
fn logical_or(_: (), value: bool, other_value: bool) -> bool {
	value || other_value
}

// Logical And
#[node_macro::node(category("Math: Logic"))]
fn logical_and(_: (), value: bool, other_value: bool) -> bool {
	value && other_value
}

// Logical Xor
#[node_macro::node(category("Math: Logic"))]
fn logical_xor(_: (), value: bool, other_value: bool) -> bool {
	value ^ other_value
}

// Logical Not
#[node_macro::node(category("Math: Logic"))]
fn logical_not(_: (), input: bool) -> bool {
	!input
}

// Bool Value
#[node_macro::node(category("Value"), name("Bool Value"))]
fn bool_value(_: (), _primary: (), #[name("Bool")] bool_value: bool) -> bool {
	bool_value
}

// Number Value
#[node_macro::node(category("Value"), name("Number Value"))]
fn number_value(_: (), _primary: (), number: f64) -> f64 {
	number
}

// Percentage Value
#[node_macro::node(category("Value"), name("Percentage Value"))]
fn percentage_value(_: (), _primary: (), percentage: Percentage) -> f64 {
	percentage
}

// Vector2 Value
#[node_macro::node(category("Value"), name("Vector2 Value"))]
fn vector2_value(_: (), _primary: (), x: f64, y: f64) -> glam::DVec2 {
	glam::DVec2::new(x, y)
}

// TODO: Make it possible to give Color::BLACK instead of 000000ff as the default
// Color Value
#[node_macro::node(category("Value"), name("Color Value"))]
fn color_value(_: (), _primary: (), #[default(000000ff)] color: crate::Color) -> crate::Color {
	color
}

// Gradient Value
#[node_macro::node(category("Value"), name("Gradient Value"))]
fn gradient_value(_: (), _primary: (), gradient: crate::vector::style::GradientStops) -> crate::vector::style::GradientStops {
	gradient
}

// Color Channel Value
#[node_macro::node(category("Value"), name("Color Channel Value"))]
fn color_channel_value(_: (), _primary: (), color_channel: crate::raster::adjustments::RedGreenBlue) -> crate::raster::adjustments::RedGreenBlue {
	color_channel
}

// Blend Mode Value
#[node_macro::node(category("Value"), name("Blend Mode Value"))]
fn blend_mode_value(_: (), _primary: (), blend_mode: crate::raster::BlendMode) -> crate::raster::BlendMode {
	blend_mode
}

// Size Of
#[cfg(feature = "std")]
#[node_macro::node]
fn size_of(_: (), ty: crate::Type) -> Option<usize> {
	ty.size()
}

// Some
#[node_macro::node]
fn some<T>(_: (), #[implementations(f64, f32, u32, u64, String, crate::Color)] input: T) -> Option<T> {
	Some(input)
}

// Unwrap
#[node_macro::node]
fn unwrap<T: Default>(_: (), #[implementations(Option<f64>, Option<f32>, Option<u32>, Option<u64>, Option<String>, Option<crate::Color>)] input: Option<T>) -> T {
	input.unwrap_or_default()
}

// Clone
#[node_macro::node]
fn clone<'i, T: Clone + 'i>(_: (), #[implementations(&crate::raster::ImageFrame<crate::Color>)] value: &'i T) -> T {
	value.clone()
}

// Identity
// TODO: Rename to "Passthrough"
/// The identity function returns the input argument unchanged.
#[node_macro::node(skip_impl)]
fn identity<'i, T: 'i>(value: T) -> T {
	value
}

// Type
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
