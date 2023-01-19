use core::marker::PhantomData;
use core::ops::Add;

use crate::{Node, NodeIO};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AddNode<L: Add<R>, R>(PhantomData<(L, R)>);

impl<'i, 's: 'i, L: Add<R>, R> NodeIO<'i> for AddNode<L, R> {
	type Output = <L as Add<R>>::Output;
	type Input = (L, R);
}

impl<'i, 's: 'i, L: Add<R, Output = O> + 'i, R: 'i, O: 'i> Node<'i, 's> for AddNode<L, R> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		input.0 + input.1
	}
}

impl<L: Add<R>, R> AddNode<L, R> {
	pub fn new() -> Self {
		Self(PhantomData)
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
impl<'i, 's: 'i, O: Clone> NodeIO<'i> for CloneNode<O> {
	type Input = &'i O;
	type Output = O;
}
impl<'i, 's: 'i, O: Clone + 'i> Node<'i, 's> for CloneNode<O> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		input.clone()
	}
}
impl<O> CloneNode<O> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FstNode<L, R>(PhantomData<(L, R)>);
impl<'i, 's: 'i, L, R> NodeIO<'i> for FstNode<L, R> {
	type Input = (L, R);
	type Output = L;
}
impl<'i, 's: 'i, L: 'i, R: 'i> Node<'i, 's> for FstNode<L, R> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		input.0
	}
}
impl<L, R> FstNode<L, R> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

/// Destructures a Tuple of two values and returns the first one
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SndNode<L, R>(PhantomData<(L, R)>);
impl<'i, 's: 'i, L, R> NodeIO<'i> for SndNode<L, R> {
	type Input = (L, R);
	type Output = R;
}
impl<'i, 's: 'i, L: 'i, R: 'i> Node<'i, 's> for SndNode<L, R> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		input.1
	}
}
impl<L, R> SndNode<L, R> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

/// Destructures a Tuple of two values and returns them in reverse order
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SwapNode<L, R>(PhantomData<(L, R)>);
impl<'i, 's: 'i, L, R> NodeIO<'i> for SwapNode<L, R> {
	type Input = (L, R);
	type Output = (R, L);
}
impl<'i, 's: 'i, L: 'i, R: 'i> Node<'i, 's> for SwapNode<L, R> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		let (a, b) = input;
		(b, a)
	}
}
impl<L, R> SwapNode<L, R> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

/// Return a tuple with two instances of the input argument
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DupNode<O>(PhantomData<O>);
impl<'i, 's: 'i, O: Clone> NodeIO<'i> for DupNode<O> {
	type Input = O;
	type Output = (O, O);
}
impl<'i, 's: 'i, O: Clone + 'i> Node<'i, 's> for DupNode<O> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		(input.clone(), input)
	}
}
impl<'i, 's: 'i, O: Clone> DupNode<O> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

/// Return the Input Argument
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct IdNode<O>(PhantomData<O>);
impl<'i, 's: 'i, O> NodeIO<'i> for IdNode<O> {
	type Input = O;
	type Output = O;
}
impl<'i, 's: 'i, O: 'i> Node<'i, 's> for IdNode<O> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		input
	}
}

impl<O> IdNode<O> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

/// Ascribe the node types
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TypeNode<N, I, O>(pub N, pub PhantomData<(I, O)>);
impl<'i, 's: 'i, N, I, O> NodeIO<'i> for TypeNode<N, I, O>
where
	N: NodeIO<'i, Input = I, Output = O>,
{
	type Input = I;
	type Output = O;
}

impl<'i, 's: 'i, N: Node<'i, 's>> TypeNode<N, <N as NodeIO<'i>>::Input, <N as NodeIO<'i>>::Output> {
	pub fn new(node: N) -> Self {
		Self(node, PhantomData)
	}
}

impl<'i, 's: 'i, N: Node<'i, 's> + Clone> Clone for TypeNode<N, <N as NodeIO<'i>>::Input, <N as NodeIO<'i>>::Output> {
	fn clone(&self) -> Self {
		Self(self.0.clone(), self.1)
	}
}
impl<'i, 's: 'i, N: Node<'i, 's> + Copy> Copy for TypeNode<N, <N as NodeIO<'i>>::Input, <N as NodeIO<'i>>::Output> {}

/// input.map(|x| self.0.eval(x))
pub struct MapResultNode<MN, I, E>(pub MN, pub PhantomData<(I, E)>);
impl<'i, 's: 'i, MN, I, E> NodeIO<'i> for MapResultNode<MN, I, E>
where
	MN: NodeIO<'i, Input = I>,
{
	type Input = Result<I, E>;
	type Output = Result<<MN as NodeIO<'i>>::Output, E>;
}

impl<'i, 's: 'i, MN: Node<'i, 's, Input = I>, I: 'i, E: 'i> Node<'i, 's> for MapResultNode<MN, I, E> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		input.map(|x| self.0.eval(x))
	}
}

impl<'i, 's: 'i, MN: Node<'i, 's>> MapResultNode<MN, <MN as NodeIO<'i>>::Input, <MN as NodeIO<'i>>::Output> {
	pub fn new(node: MN) -> Self {
		Self(node, PhantomData)
	}
}

pub struct FlatMapResultNode<'i, 's: 'i, MN: Node<'i, 's, Input = I>, I, E>(pub MN, pub PhantomData<&'s (&'i I, E)>);
impl<'i, 's: 'i, MN: Node<'i, 's, Input = I, Output = Result<O, E>>, O, I, E> NodeIO<'i> for FlatMapResultNode<'i, 's, MN, I, E> {
	type Input = Result<I, E>;
	type Output = <MN as NodeIO<'i>>::Output;
}

impl<'i, 's: 'i, MN: Node<'i, 's, Input = I, Output = Result<O, E>>, I, O, E> Node<'i, 's> for FlatMapResultNode<'i, 's, MN, I, E> {
	fn eval(&'s self, input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		match input.map(|x| self.0.eval(x)) {
			Ok(Ok(x)) => Ok(x),
			Ok(Err(e)) => Err(e),
			Err(e) => Err(e),
		}
	}
}
impl<'i, 's: 'i, MN: Node<'i, 's, Input = I>, I> FlatMapResultNode<'i, 's, MN, <MN as NodeIO<'i>>::Input, <MN as NodeIO<'i>>::Output> {
	pub fn new(node: MN) -> Self {
		Self(node, PhantomData)
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
		let cloned = (&ValueNode(4u32)).then(CloneNode::new());
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
