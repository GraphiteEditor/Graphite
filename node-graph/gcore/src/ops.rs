use crate::Node;
use core::marker::PhantomData;
use core::ops::{Add, Div, Mul, Rem, Sub};
use num_traits::Pow;

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

// Add
#[node_macro::new_node_fn(category("Math"))]
fn add<U: Add<T>, T>(
	_: (),
	#[expose]
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64, glam::DVec2)]
	primary: U,
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64, glam::DVec2)] addend: T,
) -> <U as Add<T>>::Output {
	primary + addend
}

// Subtract
#[node_macro::new_node_fn(category("Math"))]
fn subtract<U: Sub<T>, T>(
	_: (),
	#[expose]
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64, glam::DVec2)]
	primary: U,
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64, glam::DVec2)] subtrahend: T,
) -> <U as Sub<T>>::Output {
	primary - subtrahend
}

// Divide
#[node_macro::new_node_fn(category("Math"))]
fn divide<U: Div<T>, T>(
	_: (),
	#[expose]
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64, glam::DVec2, glam::DVec2)]
	primary: U,
	#[default(1.)]
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64, glam::DVec2, f64)]
	divisor: T,
) -> <U as Div<T>>::Output {
	primary / divisor
}

// Multiply
#[node_macro::new_node_fn(category("Math"))]
fn multiply<U: Mul<T>, T>(
	_: (),
	#[expose]
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64, glam::DVec2, f64)]
	primary: U,
	#[default(1.)]
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64, glam::DVec2, glam::DVec2)]
	multiplicant: T,
) -> <U as Mul<T>>::Output {
	primary * multiplicant
}

// Exponent
#[node_macro::new_node_fn(category("Math"))]
fn exponent<U: Pow<T>, T>(
	_: (),
	#[expose]
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64)]
	primary: U,
	#[default(2.)]
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64)]
	power: T,
) -> <U as num_traits::Pow<T>>::Output {
	primary.pow(power)
}

// Floor
#[node_macro::new_node_fn(category("Math"))]
fn floor(_: (), #[expose] primary: f64) -> f64 {
	primary.floor()
}

// Ceil
#[node_macro::new_node_fn(category("Math"))]
fn ceiling(_: (), #[expose] primary: f64) -> f64 {
	primary.ceil()
}

// Round
#[node_macro::new_node_fn(category("Math"))]
fn round(_: (), #[expose] primary: f64) -> f64 {
	primary.round()
}

// Absolute Value
#[node_macro::new_node_fn(category("Math"))]
fn absolute_value(_: (), #[expose] primary: f64) -> f64 {
	primary.abs()
}

// Log
#[node_macro::new_node_fn(category("Math"))]
fn logarithm<U: num_traits::float::Float>(
	_: (),
	#[expose]
	#[implementations(f32, f64)]
	first: U,
	#[default(2.)]
	#[implementations(f32, f64)]
	base: U,
) -> U {
	first.log(base)
}

// Natural Log
#[node_macro::new_node_fn(category("Math"))]
fn natural_logarithm<U: num_traits::float::Float>(
	_: (),
	#[expose]
	#[default(1.)]
	#[implementations(f32, f64)]
	first: U,
) -> U {
	first.ln()
}

// Sine
#[node_macro::new_node_fn(category("Math"))]
fn sine(_: (), #[expose] primary: f64) -> f64 {
	primary.sin()
}

// Cosine
#[node_macro::new_node_fn(category("Math"))]
fn cosine(_: (), #[expose] primary: f64) -> f64 {
	primary.cos()
}

// Tangent
#[node_macro::new_node_fn(category("Math"))]
fn tangent(_: (), #[expose] primary: f64) -> f64 {
	primary.tan()
}

// Min
#[node_macro::new_node_fn(category("Math"))]
fn min<T: core::cmp::PartialOrd>(
	_: (),
	#[expose]
	#[implementations(u32, &u32, f32, &f32, f64, &f64, &str)]
	operand_a: T,
	#[expose]
	#[implementations(u32, &u32, f32, &f32, f64, &f64, &str)]
	operand_b: T,
) -> T {
	match operand_a < operand_b {
		true => operand_a,
		false => operand_b,
	}
}

// Maxi
#[node_macro::new_node_fn(category("Math"))]
fn max<T: core::cmp::PartialOrd>(
	_: (),
	#[expose]
	#[implementations(u32, &u32, f32, &f32, f64, &f64, &str)]
	operand_a: T,
	#[expose]
	#[implementations(u32, &u32, f32, &f32, f64, &f64, &str)]
	operand_b: T,
) -> T {
	match operand_a > operand_b {
		true => operand_a,
		false => operand_b,
	}
}

// Equals
#[node_macro::new_node_fn(category("Math"))]
fn equals<U: core::cmp::PartialEq<T>, T>(
	_: (),
	#[expose]
	#[implementations(u32, &u32, f32, &f32, f64, &f64, &str)]
	operand_a: T,
	#[expose]
	#[implementations(u32, &u32, f32, &f32, f64, &f64, &str)]
	operand_b: U,
) -> bool {
	operand_b == operand_a
}

// Modulo
#[node_macro::new_node_fn(category("Math"))]
fn modulo<U: Rem<T>, T>(
	_: (),
	#[expose]
	#[implementations(u32, &u32, u32, &u32, f32, &f32, f32, &f32, f64, &f64, f64, &f64)]
	primary: U,
	#[expose]
	#[default(1.)]
	#[implementations(u32, u32, &u32, &u32, f32, f32, &f32, &f32, f64, f64, &f64, &f64)]
	modulus: T,
) -> <U as Rem<T>>::Output {
	primary % modulus
}

#[node_macro::new_node_fn(category("Value"))]
fn construct_vector2(_: (), #[expose] x: f64, #[expose] y: f64) -> glam::DVec2 {
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
fn some<T>(
	_: (),
	#[expose]
	#[implementations(f64, f32, u32, u64, String, crate::Color)]
	input: T,
) -> Option<T> {
	Some(input)
}

// Unwrap
#[node_macro::new_node_fn]
fn unwrap<T: Default>(
	_: (),
	#[expose]
	#[implementations(Option<f64>, Option<f32>, Option<u32>, Option<u64>, Option<String>, Option<crate::Color>)]
	input: Option<T>,
) -> T {
	input.unwrap_or_default()
}

// Clone
#[node_macro::new_node_fn]
fn clone<'i, T: Clone + 'i>(
	_: (),
	#[expose]
	#[implementations(&crate::raster::ImageFrame<crate::Color>)]
	value: &'i T,
) -> T {
	value.clone()
}

// Identity
/// Return the input argument unchanged
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
