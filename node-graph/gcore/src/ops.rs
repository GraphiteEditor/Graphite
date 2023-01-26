use core::marker::PhantomData;
use core::ops::Add;

use crate::{Node, NodeIO};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AddNode;

impl<'i, 's: 'i, L: Add<R> + 'i, R: 'i> NodeIO<'i, (L, R)> for AddNode {
	type Output = <L as Add<R>>::Output;
}

impl<'i, 's: 'i, L: Add<R, Output = O> + 'i, R: 'i, O: 'i> Node<'i, 's, (L, R)> for AddNode {
	fn eval(&'s self, input: (L, R)) -> <Self as NodeIO<'i, (L, R)>>::Output {
		input.0 + input.1
	}
}

impl AddNode {
	pub fn new() -> Self {
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
		fn eval(self, (left, right): (Dynamic, Dynamic)) -> Self::Output {
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
impl<'i, 's: 'i, O: Clone> NodeIO<'i, &'i O> for CloneNode<O> {
	type Output = O;
}
impl<'i, 's: 'i, O: Clone + 'i> Node<'i, 's, &'i O> for CloneNode<O> {
	fn eval(&'s self, input: &'i O) -> <Self as NodeIO<'i, &'i O>>::Output {
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
impl<'i, 's: 'i, L: 'i, R: 'i> NodeIO<'i, (L, R)> for FstNode {
	type Output = L;
}
impl<'i, 's: 'i, L: 'i, R: 'i> Node<'i, 's, (L, R)> for FstNode {
	fn eval(&'s self, input: (L, R)) -> <Self as NodeIO<'i, (L, R)>>::Output {
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
impl<'i, 's: 'i, L: 'i, R: 'i> NodeIO<'i, (L, R)> for SndNode {
	type Output = R;
}
impl<'i, 's: 'i, L: 'i, R: 'i> Node<'i, 's, (L, R)> for SndNode {
	fn eval(&'s self, input: (L, R)) -> <Self as NodeIO<'i, (L, R)>>::Output {
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
impl<'i, 's: 'i, L: 'i, R: 'i> NodeIO<'i, (L, R)> for SwapNode {
	type Output = (R, L);
}
impl<'i, 's: 'i, L: 'i, R: 'i> Node<'i, 's, (L, R)> for SwapNode {
	fn eval(&'s self, input: (L, R)) -> <Self as NodeIO<'i, (L, R)>>::Output {
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
impl<'i, 's: 'i, O: Clone + 'i> NodeIO<'i, O> for DupNode {
	type Output = (O, O);
}
impl<'i, 's: 'i, O: Clone + 'i> Node<'i, 's, O> for DupNode {
	fn eval(&'s self, input: O) -> <Self as NodeIO<'i, O>>::Output {
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
impl<'i, 's: 'i, O: 'i> NodeIO<'i, O> for IdNode {
	type Output = O;
}
impl<'i, 's: 'i, O: 'i> Node<'i, 's, O> for IdNode {
	fn eval(&'s self, input: O) -> <Self as NodeIO<'i, O>>::Output {
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
impl<'i, 's: 'i, N, I: 'i, O: 'i> NodeIO<'i, I> for TypeNode<N, I, O>
where
	N: NodeIO<'i, I, Output = O>,
{
	type Output = O;
}
impl<'i, 's: 'i, N, I: 'i, O: 'i> Node<'i, 's, I> for TypeNode<N, I, O>
where
	N: Node<'i, 's, I, Output = O>,
{
	fn eval(&'s self, input: I) -> <Self as NodeIO<'i, I>>::Output {
		self.0.eval(input)
	}
}

impl<'i, 's: 'i, N: Node<'i, 's, I>, I: 'i> TypeNode<N, I, <N as NodeIO<'i, I>>::Output> {
	pub fn new(node: N) -> Self {
		Self(node, PhantomData)
	}
}

impl<'i, 's: 'i, N: Node<'i, 's, I> + Clone, I: 'i> Clone for TypeNode<N, I, <N as NodeIO<'i, I>>::Output> {
	fn clone(&self) -> Self {
		Self(self.0.clone(), self.1)
	}
}
impl<'i, 's: 'i, N: Node<'i, 's, I> + Copy, I: 'i> Copy for TypeNode<N, I, <N as NodeIO<'i, I>>::Output> {}

/// input.map(|x| self.0.eval(x))
pub struct MapResultNode<MN, I, E>(pub MN, pub PhantomData<(I, E)>);
impl<'i, 's: 'i, MN, I: 'i, E: 'i> NodeIO<'i, Result<I, E>> for MapResultNode<MN, I, E>
where
	MN: NodeIO<'i, I>,
{
	type Output = Result<<MN as NodeIO<'i, I>>::Output, E>;
}

impl<'i, 's: 'i, MN: Node<'i, 's, I>, I: 'i, E: 'i> Node<'i, 's, Result<I, E>> for MapResultNode<MN, I, E> {
	fn eval(&'s self, input: Result<I, E>) -> <Self as NodeIO<'i, Result<I, E>>>::Output {
		input.map(|x| self.0.eval(x))
	}
}

impl<'i, 's: 'i, MN: Node<'i, 's, I>, I: 'i> MapResultNode<MN, I, <MN as NodeIO<'i, I>>::Output> {
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
fn flat_map<_I, _O, _E, N>(input: Result<_I, _E>, node: &'node N) -> Result<_O, _E>
where
	N: Node<'input, 'node, _I, Output = Result<_O, _E>> + 'node,
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
		//let flat_map = TypeNode::new(flat_map);
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
