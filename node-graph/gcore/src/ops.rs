use core::marker::PhantomData;
use core::ops::Add;

use crate::Node;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddNode;
impl<'n, L: Add<R, Output = O> + 'n, R, O: 'n> Node<(L, R)> for AddNode {
	type Output = <L as Add<R>>::Output;
	fn eval(self, input: (L, R)) -> Self::Output {
		input.0 + input.1
	}
}
impl<'n, L: Add<R, Output = O> + 'n, R, O: 'n> Node<(L, R)> for &'n AddNode {
	type Output = <L as Add<R>>::Output;
	fn eval(self, input: (L, R)) -> Self::Output {
		input.0 + input.1
	}
}
impl<'n, L: Add<R, Output = O> + 'n + Copy, R: Copy, O: 'n> Node<&'n (L, R)> for AddNode {
	type Output = <L as Add<R>>::Output;
	fn eval(self, input: &'n (L, R)) -> Self::Output {
		input.0 + input.1
	}
}
impl<'n, L: Add<R, Output = O> + 'n + Copy, R: Copy, O: 'n> Node<&'n (L, R)> for &'n AddNode {
	type Output = <L as Add<R>>::Output;
	fn eval(self, input: &'n (L, R)) -> Self::Output {
		input.0 + input.1
	}
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CloneNode;
impl<'n, O: Clone> Node<&'n O> for CloneNode {
	type Output = O;
	fn eval(self, input: &'n O) -> Self::Output {
		input.clone()
	}
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FstNode;
impl<'n, T: 'n, U> Node<(T, U)> for FstNode {
	type Output = T;
	fn eval(self, input: (T, U)) -> Self::Output {
		let (a, _) = input;
		a
	}
}
impl<'n, T: 'n, U> Node<&'n (T, U)> for FstNode {
	type Output = &'n T;
	fn eval(self, input: &'n (T, U)) -> Self::Output {
		let (a, _) = input;
		a
	}
}

/// Destructures a Tuple of two values and returns the first one
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SndNode;
impl<'n, T, U: 'n> Node<(T, U)> for SndNode {
	type Output = U;
	fn eval(self, input: (T, U)) -> Self::Output {
		let (_, b) = input;
		b
	}
}

impl<'n, T, U: 'n> Node<&'n (T, U)> for SndNode {
	type Output = &'n U;
	fn eval(self, input: &'n (T, U)) -> Self::Output {
		let (_, b) = input;
		b
	}
}

/// Destructures a Tuple of two values and returns them in reverse order
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SwapNode;
impl<'n, T: 'n, U: 'n> Node<(T, U)> for SwapNode {
	type Output = (U, T);
	fn eval(self, input: (T, U)) -> Self::Output {
		let (a, b) = input;
		(b, a)
	}
}

impl<'n, T, U: 'n> Node<&'n (T, U)> for SwapNode {
	type Output = (&'n U, &'n T);
	fn eval(self, input: &'n (T, U)) -> Self::Output {
		let (a, b) = input;
		(b, a)
	}
}

/// Return a tuple with two instances of the input argument
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DupNode;
impl<'n, T: Clone + 'n> Node<T> for DupNode {
	type Output = (T, T);
	fn eval(self, input: T) -> Self::Output {
		(input.clone(), input)
	}
}

/// Return the Input Argument
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdNode;
impl<'n, T: 'n> Node<T> for IdNode {
	type Output = T;
	fn eval(self, input: T) -> Self::Output {
		input
	}
}
impl<'n, T: 'n> Node<T> for &'n IdNode {
	type Output = T;
	fn eval(self, input: T) -> Self::Output {
		input
	}
}

pub struct MapResultNode<MN, I, E>(pub MN, pub PhantomData<(I, E)>);

impl<MN: Node<I>, I, E> Node<Result<I, E>> for MapResultNode<MN, I, E> {
	type Output = Result<MN::Output, E>;
	fn eval(self, input: Result<I, E>) -> Self::Output {
		input.map(|x| self.0.eval(x))
	}
}
impl<'n, MN: Node<I> + Copy, I, E> Node<Result<I, E>> for &'n MapResultNode<MN, I, E> {
	type Output = Result<MN::Output, E>;
	fn eval(self, input: Result<I, E>) -> Self::Output {
		input.map(|x| (&self.0).eval(x))
	}
}

impl<MN, I, E> MapResultNode<MN, I, E> {
	pub const fn new(mn: MN) -> Self {
		Self(mn, PhantomData)
	}
}
pub struct FlatMapResultNode<MN: Node<I>, I, E>(pub MN, pub PhantomData<(I, E)>);

impl<'n, MN: Node<I, Output = Result<O, E>>, I, O: 'n, E: 'n> Node<Result<I, E>> for FlatMapResultNode<MN, I, E> {
	type Output = Result<O, E>;
	fn eval(self, input: Result<I, E>) -> Self::Output {
		match input.map(|x| self.0.eval(x)) {
			Ok(Ok(x)) => Ok(x),
			Ok(Err(e)) => Err(e),
			Err(e) => Err(e),
		}
	}
}

impl<MN: Node<I>, I, E> FlatMapResultNode<MN, I, E> {
	pub const fn new(mn: MN) -> Self {
		Self(mn, PhantomData)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{generic::*, structural::*, value::*};

	#[test]
	pub fn dup_node() {
		let value = ValueNode(4u32);
		let dup = value.then(DupNode);
		assert_eq!(dup.eval(()), (4, 4));
	}
	#[test]
	pub fn id_node() {
		let value = ValueNode(4u32).then(IdNode);
		assert_eq!(value.eval(()), 4);
	}
	#[test]
	pub fn clone_node() {
		let cloned = (&ValueNode(4u32)).then(CloneNode);
		assert_eq!(cloned.eval(()), 4);
	}
	#[test]
	pub fn fst_node() {
		let fst = ValueNode((4u32, "a")).then(FstNode);
		assert_eq!(fst.eval(()), 4);
	}
	#[test]
	pub fn snd_node() {
		let fst = ValueNode((4u32, "a")).then(SndNode);
		assert_eq!(fst.eval(()), "a");
	}
	#[test]
	pub fn add_node() {
		let a = ValueNode(42u32);
		let b = ValueNode(6u32);
		let cons_a = ConsNode(a);

		let sum = b.then(cons_a).then(AddNode);

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
		let result: u32 = (&fns).eval(());
		assert_eq!(result, 42);
	}
}
