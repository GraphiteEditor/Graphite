use crate::Node;

use core::marker::PhantomData;
use core::ops::{Add, Div, Mul, Rem, Sub};
use num_traits::Pow;
use rand::{Rng, SeedableRng};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

// Add
#[node_macro::new_node_fn(category("Math: Arithmetic"))]
fn add<U: Add<T>, T>(
	_: (),
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64, glam::DVec2)] augend: U,
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64, glam::DVec2)] addend: T,
) -> <U as Add<T>>::Output {
	augend + addend
}

// Subtract
#[node_macro::new_node_fn(category("Math: Arithmetic"))]
fn subtract<U: Sub<T>, T>(
	_: (),
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64, glam::DVec2)] minuend: U,
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64, glam::DVec2)] subtrahend: T,
) -> <U as Sub<T>>::Output {
	minuend - subtrahend
}

// Multiply
#[node_macro::new_node_fn(category("Math: Arithmetic"))]
fn multiply<U: Mul<T>, T>(
	_: (),
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64, glam::DVec2, f64)] multiplier: U,
	#[default(1.)]
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64, glam::DVec2, glam::DVec2)]
	multiplicand: T,
) -> <U as Mul<T>>::Output {
	multiplier * multiplicand
}

// Divide
#[node_macro::new_node_fn(category("Math: Arithmetic"))]
fn divide<U: Div<T>, T>(
	_: (),
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64, glam::DVec2, glam::DVec2)] numerator: U,
	#[default(1.)]
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64, glam::DVec2, f64)]
	denominator: T,
) -> <U as Div<T>>::Output {
	numerator / denominator
}

// Modulo
#[node_macro::new_node_fn(category("Math: Arithmetic"))]
fn modulo<U: Rem<T>, T>(
	_: (),
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64)] numerator: U,
	#[default(1.)]
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64)]
	modulus: T,
) -> <U as Rem<T>>::Output {
	numerator % modulus
}

// Exponent
#[node_macro::new_node_fn(category("Math: Arithmetic"))]
fn exponent<U: Pow<T>, T>(
	_: (),
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64)] base: U,
	#[default(2.)]
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64)]
	power: T,
) -> <U as num_traits::Pow<T>>::Output {
	base.pow(power)
}

// Root
#[node_macro::new_node_fn(category("Math: Arithmetic"))]
fn root<U: num_traits::float::Float>(
	_: (),
	#[default(2.)]
	#[implementations(f32, f64)]
	radicand: U,
	#[default(2.)]
	#[implementations(f32, f64)]
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

// Log
#[node_macro::new_node_fn(category("Math: Arithmetic"))]
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
#[node_macro::new_node_fn(category("Math: Trig"))]
fn sine(_: (), theta: f64) -> f64 {
	theta.sin()
}

// Cosine
#[node_macro::new_node_fn(category("Math: Trig"))]
fn cosine(_: (), theta: f64) -> f64 {
	theta.cos()
}

// Tangent
#[node_macro::new_node_fn(category("Math: Trig"))]
fn tangent(_: (), theta: f64) -> f64 {
	theta.tan()
}

// Random
#[node_macro::new_node_fn(category("Math: Numeric"))]
fn random(_: (), _primary: (), seed: u64, #[default(0.)] min: f64, #[default(1.)] max: f64) -> f64 {
	let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
	let result = rng.gen::<f64>();
	let (min, max) = if min < max { (min, max) } else { (max, min) };
	result * (max - min) + min
}

// Round
#[node_macro::new_node_fn(category("Math: Numeric"))]
fn round(_: (), value: f64) -> f64 {
	value.round()
}

// Floor
#[node_macro::new_node_fn(category("Math: Numeric"))]
fn floor(_: (), value: f64) -> f64 {
	value.floor()
}

// Ceiling
#[node_macro::new_node_fn(category("Math: Numeric"))]
fn ceiling(_: (), value: f64) -> f64 {
	value.ceil()
}

// Absolute Value
#[node_macro::new_node_fn(category("Math: Numeric"))]
fn absolute_value(_: (), value: f64) -> f64 {
	value.abs()
}

// Min
#[node_macro::new_node_fn(category("Math: Numeric"))]
fn min<T: core::cmp::PartialOrd>(_: (), #[implementations(u32, &u32, f32, &f32, f64, &f64, &str)] value: T, #[implementations(u32, &u32, f32, &f32, f64, &f64, &str)] other_value: T) -> T {
	match value < other_value {
		true => value,
		false => other_value,
	}
}

// Max
#[node_macro::new_node_fn(category("Math: Numeric"))]
fn max<T: core::cmp::PartialOrd>(_: (), #[implementations(u32, &u32, f32, &f32, f64, &f64, &str)] value: T, #[implementations(u32, &u32, f32, &f32, f64, &f64, &str)] other_value: T) -> T {
	match value > other_value {
		true => value,
		false => other_value,
	}
}

// Equals
#[node_macro::new_node_fn(category("Math: Logic"))]
fn equals<U: core::cmp::PartialEq<T>, T>(_: (), #[implementations(u32, &u32, f32, &f32, f64, &f64, &str)] value: T, #[implementations(u32, &u32, f32, &f32, f64, &f64, &str)] other_value: U) -> bool {
	other_value == value
}

// Logical Or
#[node_macro::new_node_fn(category("Math: Logic"))]
fn logical_or(_: (), value: bool, other_value: bool) -> bool {
	value || other_value
}

// Logical And
#[node_macro::new_node_fn(category("Math: Logic"))]
fn logical_and(_: (), value: bool, other_value: bool) -> bool {
	value && other_value
}

// Logical Xor
#[node_macro::new_node_fn(category("Math: Logic"))]
fn logical_xor(_: (), value: bool, other_value: bool) -> bool {
	value ^ other_value
}

// Logical Not
#[node_macro::new_node_fn(category("Math: Logic"))]
fn logical_not(_: (), input: bool) -> bool {
	!input
}

// Vector2 Value
#[node_macro::new_node_fn(category("Value"), name("Vector2 Value"))]
fn vector2_value(_: (), _primary: (), x: f64, y: f64) -> glam::DVec2 {
	glam::DVec2::new(x, y)
}

// Size Of
#[cfg(feature = "std")]
#[node_macro::new_node_fn]
fn size_of(_: (), ty: crate::Type) -> Option<usize> {
	ty.size()
}

// Some
#[node_macro::new_node_fn]
fn some<T>(_: (), #[implementations(f64, f32, u32, u64, String, crate::Color)] input: T) -> Option<T> {
	Some(input)
}

// Unwrap
#[node_macro::new_node_fn]
fn unwrap<T: Default>(_: (), #[implementations(Option<f64>, Option<f32>, Option<u32>, Option<u64>, Option<String>, Option<crate::Color>)] input: Option<T>) -> T {
	input.unwrap_or_default()
}

// Clone
#[node_macro::new_node_fn]
fn clone<'i, T: Clone + 'i>(_: (), #[implementations(&crate::raster::ImageFrame<crate::Color>)] value: &'i T) -> T {
	value.clone()
}

// Identity
/// The identity function returns the input argument unchanged.
#[node_macro::new_node_fn]
fn identity<'i, T: 'i>(#[implementations()] value: T) -> T {
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

// Map Option
pub struct MapOptionNode<I, Mn> {
	node: Mn,
	_i: PhantomData<I>,
}
#[node_macro::node_fn(MapOptionNode<_I>)]
fn map_option_node<_I, N>(input: Option<_I>, node: &'input N) -> Option<<N as Node<'input, _I>>::Output>
where
	N: for<'a> Node<'a, _I>,
{
	input.map(|x| node.eval(x))
}

// Map Result
pub struct MapResultNode<I, E, Mn> {
	node: Mn,
	_i: PhantomData<I>,
	_e: PhantomData<E>,
}
#[node_macro::node_fn(MapResultNode<_I, _E>)]
fn map_result_node<_I, _E, N>(input: Result<_I, _E>, node: &'input N) -> Result<<N as Node<'input, _I>>::Output, _E>
where
	N: for<'a> Node<'a, _I>,
{
	input.map(|x| node.eval(x))
}

// Flat Map Result
pub struct FlatMapResultNode<I, O, E, Mn> {
	node: Mn,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
	_e: PhantomData<E>,
}
#[node_macro::node_fn(FlatMapResultNode<_I, _O, _E>)]
fn flat_map_node<_I, _O, _E, N>(input: Result<_I, _E>, node: &'input N) -> Result<_O, _E>
where
	N: for<'a> Node<'a, _I, Output = Result<_O, _E>>,
{
	match input.map(|x| node.eval(x)) {
		Ok(Ok(x)) => Ok(x),
		Ok(Err(e)) => Err(e),
		Err(e) => Err(e),
	}
}

// Into
pub struct IntoNode<I, O> {
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}
#[cfg(feature = "alloc")]
#[node_macro::node_fn(IntoNode<_I, _O>)]
async fn into<_I, _O>(input: _I) -> _O
where
	_I: Into<_O> + Sync + Send,
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
	pub fn map_result() {
		let value: ClonedNode<Result<&u32, ()>> = ClonedNode(Ok(&4u32));
		assert_eq!(value.eval(()), Ok(&4u32));
		// let type_erased_clone = clone as &dyn for<'a> Node<'a, &'a u32, Output = u32>;
		let map_result = MapResultNode::new(ValueNode::new(FnNode::new(|x: &u32| *x)));
		// let type_erased = &map_result as &dyn for<'a> Node<'a, Result<&'a u32, ()>, Output = Result<u32, ()>>;
		assert_eq!(map_result.eval(Ok(&4u32)), Ok(4u32));
		let fst = value.then(map_result);
		// let type_erased = &fst as &dyn for<'a> Node<'a, (), Output = Result<u32, ()>>;
		assert_eq!(fst.eval(()), Ok(4u32));
	}
	#[test]
	pub fn flat_map_result() {
		let fst = CloneNode::new(ValueNode(Ok(&4u32)));
		let fn_node: FnNode<_, &u32, Result<&u32, _>> = FnNode::new(|_| Err(8u32));
		assert_eq!(fn_node.eval(&4u32), Err(8u32));
		let flat_map = FlatMapResultNode::new(ValueNode::new(fn_node));
		let fst = fst.then(flat_map);
		assert_eq!(fst.eval(()), Err(8u32));
	}
	#[test]
	pub fn foo() {
		fn int(_: (), state: &u32) -> u32 {
			*state
		}
		fn swap(input: (u32, u32)) -> (u32, u32) {
			(input.1, input.0)
		}
		let fnn = FnNode::new(&swap);
		let fns = FnNodeWithState::new(int, 42u32);
		assert_eq!(fnn.eval((1u32, 2u32)), (2, 1));
		let result: u32 = fns.eval(());
		assert_eq!(result, 42);
	}
}
