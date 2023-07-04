use core::marker::PhantomData;
use core::ops::{Add, Mul};

use crate::Node;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AddNode;

impl<'i, L: Add<R, Output = O> + 'i, R: 'i, O: 'i> Node<'i, (L, R)> for AddNode {
	type Output = <L as Add<R>>::Output;
	fn eval(&'i self, input: (L, R)) -> Self::Output {
		input.0 + input.1
	}
}

impl AddNode {
	pub const fn new() -> Self {
		Self
	}
}
pub struct AddParameterNode<Second> {
	second: Second,
}

#[node_macro::node_fn(AddParameterNode)]
fn add_parameter<U, T>(first: U, second: T) -> <U as Add<T>>::Output
where
	U: Add<T>,
{
	first + second
}

pub struct MulParameterNode<Second> {
	second: Second,
}

#[node_macro::node_fn(MulParameterNode)]
fn flat_map<U, T>(first: U, second: T) -> <U as Mul<T>>::Output
where
	U: Mul<T>,
{
	first * second
}

#[cfg(feature = "std")]
struct SizeOfNode {}

#[cfg(feature = "std")]
#[node_macro::node_fn(SizeOfNode)]
fn flat_map(ty: crate::Type) -> Option<usize> {
	ty.size()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SomeNode;
#[node_macro::node_fn(SomeNode)]
fn some<T>(input: T) -> Option<T> {
	Some(input)
}

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FstNode;
impl<'i, L: 'i, R: 'i> Node<'i, (L, R)> for FstNode {
	type Output = L;
	fn eval(&'i self, input: (L, R)) -> Self::Output {
		input.0
	}
}
impl FstNode {
	pub fn new() -> Self {
		Self
	}
}

/// Destructures a Tuple of two values and returns the first one
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SndNode;
impl<'i, L: 'i, R: 'i> Node<'i, (L, R)> for SndNode {
	type Output = R;
	fn eval(&'i self, input: (L, R)) -> Self::Output {
		input.1
	}
}
impl SndNode {
	pub fn new() -> Self {
		Self
	}
}

/// Destructures a Tuple of two values and returns them in reverse order
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SwapNode;
impl<'i, L: 'i, R: 'i> Node<'i, (L, R)> for SwapNode {
	type Output = (R, L);
	fn eval(&'i self, input: (L, R)) -> Self::Output {
		(input.1, input.0)
	}
}
impl SwapNode {
	pub fn new() -> Self {
		Self
	}
}

/// Return a tuple with two instances of the input argument
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DupNode;
impl<'i, O: Clone + 'i> Node<'i, O> for DupNode {
	type Output = (O, O);
	fn eval(&'i self, input: O) -> Self::Output {
		(input.clone(), input)
	}
}
impl DupNode {
	pub fn new() -> Self {
		Self
	}
}

/// Return the Input Argument
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct IdNode;
impl<'i, O: 'i> Node<'i, O> for IdNode {
	type Output = O;
	fn eval(&'i self, input: O) -> Self::Output {
		input
	}
}

impl IdNode {
	pub fn new() -> Self {
		Self
	}
}

/// Ascribe the node types
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

/// input.map(|x| self.0.eval(x))
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
	pub fn dup_node() {
		let value = ValueNode(4u32);
		let dup = ComposeNode::new(value, DupNode::new());
		assert_eq!(dup.eval(()), (&4, &4));
	}
	#[test]
	pub fn id_node() {
		let value = ValueNode(4u32).then(IdNode::new());
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
	pub fn fst_node() {
		let fst = ValueNode((4u32, "a")).then(CloneNode::new()).then(FstNode::new());
		assert_eq!(fst.eval(()), 4);
	}
	#[test]
	pub fn snd_node() {
		let fst = ValueNode((4u32, "a")).then(CloneNode::new()).then(SndNode::new());
		assert_eq!(fst.eval(()), "a");
	}
	#[test]
	pub fn object_safe() {
		let fst = ValueNode((4u32, "a")).then(CloneNode::new()).then(SndNode::new());
		let foo = &fst as &dyn Node<(), Output = &str>;
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

		let sum = tuple.then(AddNode::new());

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
