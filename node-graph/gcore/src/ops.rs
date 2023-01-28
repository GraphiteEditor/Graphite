use core::marker::PhantomData;
use core::ops::Add;

use crate::Node;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AddNode;

impl<'i, L: Add<R, Output = O> + 'i, R: 'i, O: 'i> Node<'i, (L, R)> for AddNode {
	type Output = <L as Add<R>>::Output;
	fn eval<'s: 'i>(&'s self, input: (L, R)) -> Self::Output {
		input.0 + input.1
	}
}

impl AddNode {
	pub const fn new() -> Self {
		Self
	}
}
/*
#[cfg(feature = "std")]
pub mod dynamic {
	use super::*;

	// Unfortunatly we can't impl the AddNode as we get
	// `upstream crates may add a new impl of trait `core::ops::Add` for type `alloc::boxed::Box<(dyn dyn_any::DynAny<'_> + 'static)>` in future versions`
	pub struct DynamicAddNode;

	// Alias for a dynamic type
	pub type Dynamic<'a> = alloc::boxed::Box<dyn dyn_any::DynAny<'a> + 'a>;

	/// Resolves the dynamic types for a dynamic node.
	///
	/// Macro uses format `BaseNode => (arg1: u32) (arg1: i32)`
	macro_rules! resolve_dynamic_types {
	($node:ident => $(($($arg:ident : $t:ty),*))*) => {
		$(
			// Check for each possible set of arguments if their types match the arguments given
			if $(core::any::TypeId::of::<$t>() == $arg.type_id())&&* {
				// Cast the arguments and then call the inner node
				alloc::boxed::Box::new($node.eval(($(*dyn_any::downcast::<$t>($arg).unwrap()),*)) ) as Dynamic
			}
		)else*
		else {
			panic!("Unhandled type"); // TODO: Exit neatly (although this should probably not happen)
		}
	};
}

	impl<'i> Node<(Dynamic<'i>, Dynamic<'i>)> for DynamicAddNode {
		type Output = Dynamic<'i>;
		fn eval<'s: 'i>(self, (left, right): (Dynamic, Dynamic)) -> Self::Output {
			resolve_dynamic_types! { AddNode =>
			(left: usize, right: usize)
			(left: u8, right: u8)
			(left: u16, right: u16)
			(left: u32, right: u32)
			(left: u64, right: u64)
			(left: u128, right: u128)
			(left: isize, right: isize)
			(left: i8, right: i8)
			(left: i16, right: i16)
			(left: i32, right: i32)
			(left: i64, right: i64)
			(left: i128, right: i128)
			(left: f32, right: f32)
			(left: f64, right: f64) }
		}
	}
}*/

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CloneNode<O>(PhantomData<O>);
impl<'i, O: Clone + 'i> Node<'i, &'i O> for CloneNode<O> {
	type Output = O;
	fn eval<'s: 'i>(&'s self, input: &'i O) -> Self::Output {
		input.clone()
	}
}
impl<O> CloneNode<O> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FstNode;
impl<'i, L: 'i, R: 'i> Node<'i, (L, R)> for FstNode {
	type Output = L;
	fn eval<'s: 'i>(&'s self, input: (L, R)) -> Self::Output {
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
	fn eval<'s: 'i>(&'s self, input: (L, R)) -> Self::Output {
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
	fn eval<'s: 'i>(&'s self, input: (L, R)) -> Self::Output {
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
	fn eval<'s: 'i>(&'s self, input: O) -> Self::Output {
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
	fn eval<'s: 'i>(&'s self, input: O) -> Self::Output {
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
pub struct TypeNode<N, I, O>(pub N, pub PhantomData<(I, O)>);
impl<'i, N, I: 'i, O: 'i> Node<'i, I> for TypeNode<N, I, O>
where
	N: Node<'i, I, Output = O>,
{
	type Output = O;
	fn eval<'s: 'i>(&'s self, input: I) -> Self::Output {
		self.0.eval(input)
	}
}

impl<'i, N: Node<'i, I>, I: 'i> TypeNode<N, I, N::Output> {
	pub fn new(node: N) -> Self {
		Self(node, PhantomData)
	}
}

impl<'i, N: Node<'i, I> + Clone, I: 'i> Clone for TypeNode<N, I, N::Output> {
	fn clone(&self) -> Self {
		Self(self.0.clone(), self.1)
	}
}
impl<'i, N: Node<'i, I> + Copy, I: 'i> Copy for TypeNode<N, I, N::Output> {}

/// input.map(|x| self.0.eval(x))
pub struct MapResultNode<MN, I, E>(pub MN, pub PhantomData<(I, E)>);

impl<'i, MN: Node<'i, I>, I: 'i, E: 'i> Node<'i, Result<I, E>> for MapResultNode<MN, I, E> {
	type Output = Result<MN::Output, E>;
	fn eval<'s: 'i>(&'s self, input: Result<I, E>) -> Self::Output {
		input.map(|x| self.0.eval(x))
	}
}

impl<'i, MN: Node<'i, I>, I: 'i> MapResultNode<MN, I, MN::Output> {
	pub fn new(node: MN) -> Self {
		Self(node, PhantomData)
	}
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
	N: Node<'input, _I, Output = Result<_O, _E>>,
{
	match input.map(|x| node.eval(x)) {
		Ok(Ok(x)) => Ok(x),
		Ok(Err(e)) => Err(e),
		Err(e) => Err(e),
	}
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
		let fst = ValueNode(Ok(&4u32)).then(CloneNode::new()).then(MapResultNode::new(CloneNode::new()));
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
