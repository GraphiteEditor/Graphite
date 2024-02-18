use crate::Node;
use core::marker::PhantomData;
use core::ops::{Add, Div, Mul, Rem, Sub};
use num_traits::Pow;

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

// Add Pair
// TODO: Delete this redundant (two-argument version of the) add node. It's only used in tests.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AddPairNode;
impl<'i, L: Add<R, Output = O> + 'i, R: 'i, O: 'i> Node<'i, (L, R)> for AddPairNode {
	type Output = <L as Add<R>>::Output;
	fn eval(&'i self, input: (L, R)) -> Self::Output {
		input.0 + input.1
	}
}
impl AddPairNode {
	pub const fn new() -> Self {
		Self
	}
}

// Add
pub struct AddNode<Second> {
	second: Second,
}
#[node_macro::node_fn(AddNode)]
fn add_parameter<U, T>(first: U, second: T) -> <U as Add<T>>::Output
where
	U: Add<T>,
{
	first + second
}

// Subtract
pub struct SubtractNode<Second> {
	second: Second,
}
#[node_macro::node_fn(SubtractNode)]
fn sub<U, T>(first: U, second: T) -> <U as Sub<T>>::Output
where
	U: Sub<T>,
{
	first - second
}

// Divide
pub struct DivideNode<Second> {
	second: Second,
}
#[node_macro::node_fn(DivideNode)]
fn div<U, T>(first: U, second: T) -> <U as Div<T>>::Output
where
	U: Div<T>,
{
	first / second
}

// Multiply
pub struct MultiplyNode<Second> {
	second: Second,
}
#[node_macro::node_fn(MultiplyNode)]
fn mul<U, T>(first: U, second: T) -> <U as Mul<T>>::Output
where
	U: Mul<T>,
{
	first * second
}

// Exponent
pub struct ExponentNode<Second> {
	second: Second,
}
#[node_macro::node_fn(ExponentNode)]
fn exp<U, T>(first: U, second: T) -> <U as Pow<T>>::Output
where
	U: Pow<T>,
{
	first.pow(second)
}

// Floor
pub struct FloorNode;
#[node_macro::node_fn(FloorNode)]
fn floor(input: f64) -> f64 {
	input.floor()
}

// Ceil
pub struct CeilingNode;
#[node_macro::node_fn(CeilingNode)]
fn ceil(input: f64) -> f64 {
	input.ceil()
}

// Round
pub struct RoundNode;
#[node_macro::node_fn(RoundNode)]
fn round(input: f64) -> f64 {
	input.round()
}

// Absolute Value
pub struct AbsoluteValue;
#[node_macro::node_fn(AbsoluteValue)]
fn abs(input: f64) -> f64 {
	input.abs()
}

// Log
pub struct LogarithmNode<Second> {
	second: Second,
}
#[node_macro::node_fn(LogarithmNode)]
fn ln<U: num_traits::float::Float>(first: U, second: U) -> U {
	first.log(second)
}

// Natural Log
pub struct NaturalLogarithmNode;
#[node_macro::node_fn(NaturalLogarithmNode)]
fn ln(input: f64) -> f64 {
	input.ln()
}

// Sine
pub struct SineNode;
#[node_macro::node_fn(SineNode)]
fn ln(input: f64) -> f64 {
	input.sin()
}

// Cosine
pub struct CosineNode;
#[node_macro::node_fn(CosineNode)]
fn ln(input: f64) -> f64 {
	input.cos()
}

// Tangent
pub struct TangentNode;
#[node_macro::node_fn(TangentNode)]
fn ln(input: f64) -> f64 {
	input.tan()
}

// Min
pub struct MinimumNode<Second> {
	second: Second,
}
#[node_macro::node_fn(MinimumNode)]
fn min<T: core::cmp::PartialOrd>(first: T, second: T) -> T {
	match first < second {
		true => first,
		false => second,
	}
}

// Maxi
pub struct MaximumNode<Second> {
	second: Second,
}
#[node_macro::node_fn(MaximumNode)]
fn max<T: core::cmp::PartialOrd>(first: T, second: T) -> T {
	match first > second {
		true => first,
		false => second,
	}
}

// Equals
pub struct EqualsNode<Second> {
	second: Second,
}
#[node_macro::node_fn(EqualsNode)]
fn eq<T: core::cmp::PartialEq>(first: T, second: T) -> bool {
	first == second
}

// Modulo
pub struct ModuloNode<Second> {
	second: Second,
}
#[node_macro::node_fn(ModuloNode)]
fn modulo<U, T>(first: U, second: T) -> <U as Rem<T>>::Output
where
	U: Rem<T>,
{
	first % second
}

pub struct ConstructVector2<X, Y> {
	x: X,
	y: Y,
}
#[node_macro::node_fn(ConstructVector2)]
fn construct_vector2(_primary: (), x: f64, y: f64) -> glam::DVec2 {
	glam::DVec2::new(x, y)
}

// Size Of
#[cfg(feature = "std")]
struct SizeOfNode;
#[cfg(feature = "std")]
#[node_macro::node_fn(SizeOfNode)]
fn flat_map(ty: crate::Type) -> Option<usize> {
	ty.size()
}

// Some
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SomeNode;
#[node_macro::node_fn(SomeNode)]
fn some<T>(input: T) -> Option<T> {
	Some(input)
}

// Clone
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CloneNode<O>(PhantomData<O>);
impl<'i, 'n: 'i, O: Clone + 'i> Node<'i, &'n O> for CloneNode<O> {
	type Output = O;
	fn eval(&'i self, input: &'i O) -> Self::Output {
		input.clone()
	}
}
impl<O> CloneNode<O> {
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

// First of Pair
/// Return the first element of a 2-tuple
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FirstOfPairNode;
impl<'i, L: 'i, R: 'i> Node<'i, (L, R)> for FirstOfPairNode {
	type Output = L;
	fn eval(&'i self, input: (L, R)) -> Self::Output {
		input.0
	}
}
impl FirstOfPairNode {
	pub fn new() -> Self {
		Self
	}
}

// Second of Pair
/// Return the second element of a 2-tuple
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SecondOfPairNode;
impl<'i, L: 'i, R: 'i> Node<'i, (L, R)> for SecondOfPairNode {
	type Output = R;
	fn eval(&'i self, input: (L, R)) -> Self::Output {
		input.1
	}
}
impl SecondOfPairNode {
	pub fn new() -> Self {
		Self
	}
}

// Swap Pair
/// Return a new 2-tuple with the elements reversed
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SwapPairNode;
impl<'i, L: 'i, R: 'i> Node<'i, (L, R)> for SwapPairNode {
	type Output = (R, L);
	fn eval(&'i self, input: (L, R)) -> Self::Output {
		(input.1, input.0)
	}
}
impl SwapPairNode {
	pub fn new() -> Self {
		Self
	}
}

// Make Pair
/// Return a 2-tuple with two duplicates of the input argument
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MakePairNode;
impl<'i, O: Clone + 'i> Node<'i, O> for MakePairNode {
	type Output = (O, O);
	fn eval(&'i self, input: O) -> Self::Output {
		(input.clone(), input)
	}
}
impl MakePairNode {
	pub fn new() -> Self {
		Self
	}
}

// Identity
/// Return the input argument unchanged
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct IdentityNode;
impl<'i, O: 'i> Node<'i, O> for IdentityNode {
	type Output = O;
	fn eval(&'i self, input: O) -> Self::Output {
		input
	}
}
impl IdentityNode {
	pub fn new() -> Self {
		Self
	}
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

// Map Result
pub struct MapResultNode<I, E, Mn> {
	node: Mn,
	_i: PhantomData<I>,
	_e: PhantomData<E>,
}
#[node_macro::node_fn(MapResultNode<_I,  _E>)]
fn flat_map<_I, _E, N>(input: Result<_I, _E>, node: &'input N) -> Result<<N as Node<'input, _I>>::Output, _E>
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
fn flat_map<_I, _O, _E, N>(input: Result<_I, _E>, node: &'input N) -> Result<_O, _E>
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
	_I: Into<_O>,
{
	input.into()
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{generic::*, structural::*, value::*};

	#[test]
	pub fn duplicate_node() {
		let value = ValueNode(4u32);
		let pair = ComposeNode::new(value, MakePairNode::new());
		assert_eq!(pair.eval(()), (&4, &4));
	}
	#[test]
	pub fn identity_node() {
		let value = ValueNode(4u32).then(IdentityNode::new());
		assert_eq!(value.eval(()), &4);
	}
	#[test]
	pub fn clone_node() {
		let cloned = ValueNode(4u32).then(CloneNode::new());
		assert_eq!(cloned.eval(()), 4);
		let type_erased = &CloneNode::new() as &dyn for<'a> Node<'a, &'a u32, Output = u32>;
		assert_eq!(type_erased.eval(&4), 4);
		let type_erased = &cloned as &dyn for<'a> Node<'a, (), Output = u32>;
		assert_eq!(type_erased.eval(()), 4);
	}
	#[test]
	pub fn first_node() {
		let first_of_pair = ValueNode((4u32, "a")).then(CloneNode::new()).then(FirstOfPairNode::new());
		assert_eq!(first_of_pair.eval(()), 4);
	}
	#[test]
	pub fn second_node() {
		let second_of_pair = ValueNode((4u32, "a")).then(CloneNode::new()).then(SecondOfPairNode::new());
		assert_eq!(second_of_pair.eval(()), "a");
	}
	#[test]
	pub fn object_safe() {
		let second_of_pair = ValueNode((4u32, "a")).then(CloneNode::new()).then(SecondOfPairNode::new());
		let foo = &second_of_pair as &dyn Node<(), Output = &str>;
		assert_eq!(foo.eval(()), "a");
	}
	#[test]
	pub fn map_result() {
		let value: ClonedNode<Result<&u32, ()>> = ClonedNode(Ok(&4u32));
		assert_eq!(value.eval(()), Ok(&4u32));
		//let type_erased_clone = clone as &dyn for<'a> Node<'a, &'a u32, Output = u32>;
		let map_result = MapResultNode::new(ValueNode::new(FnNode::new(|x: &u32| *x)));
		//et type_erased = &map_result as &dyn for<'a> Node<'a, Result<&'a u32, ()>, Output = Result<u32, ()>>;
		assert_eq!(map_result.eval(Ok(&4u32)), Ok(4u32));
		let fst = value.then(map_result);
		//let type_erased = &fst as &dyn for<'a> Node<'a, (), Output = Result<u32, ()>>;
		assert_eq!(fst.eval(()), Ok(4u32));
	}
	#[test]
	pub fn flat_map_result() {
		let fst = ValueNode(Ok(&4u32)).then(CloneNode::new()); //.then(FlatMapResultNode::new(FnNode::new(|x| Ok(x))));
		let fn_node: FnNode<_, &u32, Result<&u32, _>> = FnNode::new(|_| Err(8u32));
		assert_eq!(fn_node.eval(&4u32), Err(8u32));
		let flat_map = FlatMapResultNode::new(ValueNode::new(fn_node));
		let fst = fst.then(flat_map);
		assert_eq!(fst.eval(()), Err(8u32));
	}
	#[test]
	pub fn add_node() {
		let a = ValueNode(42u32);
		let b = ValueNode(6u32);
		let cons_a = ConsNode::new(a);
		let tuple = b.then(cons_a);

		let sum = tuple.then(AddPairNode::new());

		assert_eq!(sum.eval(()), 48);
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
