use core::marker::PhantomData;
use core::ops::Add;

use crate::Node;

pub struct AddNode;
impl<'n, L: Add<R, Output = O> + 'n, R, O: 'n> Node<'n, (L, R)> for AddNode {
	type Output = <L as Add<R>>::Output;
	fn eval(&'n self, input: (L, R)) -> Self::Output {
		input.0 + input.1
	}
}

pub struct CloneNode;
impl<'n, O: Clone> Node<'n, &'n O> for CloneNode {
	type Output = O;
	fn eval(&'n self, input: &'n O) -> Self::Output {
		input.clone()
	}
}

pub struct FstNode;
impl<'n, T: 'n, U> Node<'n, (T, U)> for FstNode {
	type Output = T;
	fn eval(&'n self, input: (T, U)) -> Self::Output {
		let (a, _) = input;
		a
	}
}
impl<'n, T: 'n, U> Node<'n, &'n (T, U)> for FstNode {
	type Output = &'n T;
	fn eval(&'n self, input: &'n (T, U)) -> Self::Output {
		let (a, _) = input;
		a
	}
}

/// Destructures a Tuple of two values and returns the first one
pub struct SndNode;
impl<'n, T, U: 'n> Node<'n, (T, U)> for SndNode {
	type Output = U;
	fn eval(&'n self, input: (T, U)) -> Self::Output {
		let (_, b) = input;
		b
	}
}

impl<'n, T, U: 'n> Node<'n, &'n (T, U)> for SndNode {
	type Output = &'n U;
	fn eval(&'n self, input: &'n (T, U)) -> Self::Output {
		let (_, b) = input;
		b
	}
}

/// Destructures a Tuple of two values and returns them in reverse order
pub struct SwapNode;
impl<'n, T: 'n, U: 'n> Node<'n, (T, U)> for SwapNode {
	type Output = (U, T);
	fn eval(&'n self, input: (T, U)) -> Self::Output {
		let (a, b) = input;
		(b, a)
	}
}

impl<'n, T, U: 'n> Node<'n, &'n (T, U)> for SwapNode {
	type Output = (&'n U, &'n T);
	fn eval(&'n self, input: &'n (T, U)) -> Self::Output {
		let (a, b) = input;
		(b, a)
	}
}

/// Return a tuple with two instances of the input argument
pub struct DupNode;
impl<'n, T: Clone + 'n> Node<'n, T> for DupNode {
	type Output = (T, T);
	fn eval(&'n self, input: T) -> Self::Output {
		(input.clone(), input) //TODO: use Copy/Clone implementation
	}
}

/// Return the Input Argument
pub struct IdNode;
impl<'n, T: 'n> Node<'n, T> for IdNode {
	type Output = T;
	fn eval(&'n self, input: T) -> Self::Output {
		input
	}
}

pub struct MapResultNode<'n, MN: Node<'n, I>, I, E>(pub MN, pub PhantomData<&'n (I, E)>);

impl<'n, MN: Node<'n, I>, I, E> Node<'n, Result<I, E>> for MapResultNode<'n, MN, I, E> {
	type Output = Result<MN::Output, E>;
	fn eval(&'n self, input: Result<I, E>) -> Self::Output {
		input.map(|x| self.0.eval(x))
	}
}

impl<'n, MN: Node<'n, I>, I, E> MapResultNode<'n, MN, I, E> {
	pub const fn new(mn: MN) -> Self {
		Self(mn, PhantomData)
	}
}
pub struct FlatMapResultNode<'n, MN: Node<'n, I>, I, E>(pub MN, pub PhantomData<&'n (I, E)>);

impl<'n, MN: Node<'n, I, Output = Result<O, E>>, I, O: 'n, E: 'n> Node<'n, Result<I, E>> for FlatMapResultNode<'n, MN, I, E> {
	type Output = Result<O, E>;
	fn eval(&'n self, input: Result<I, E>) -> Self::Output {
		match input.map(|x| self.0.eval(x)) {
			Ok(Ok(x)) => Ok(x),
			Ok(Err(e)) => Err(e),
			Err(e) => Err(e),
		}
	}
}

impl<'n, MN: Node<'n, I>, I, E> FlatMapResultNode<'n, MN, I, E> {
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
		let dup = DupNode.after(value);
		assert_eq!(dup.eval(()), (&4, &4));
	}
	#[test]
	pub fn id_node() {
		let value = IdNode.after(ValueNode(4u32));
		assert_eq!(value.eval(()), &4);
	}
	#[test]
	pub fn clone_node() {
		let cloned = CloneNode.after(ValueNode(4u32));
		assert_eq!(cloned.eval(()), 4);
	}
	#[test]
	pub fn fst_node() {
		let fst = FstNode.after(ValueNode((4u32, "a")).clone());
		assert_eq!(fst.eval(()), 4);
	}
	#[test]
	pub fn snd_node() {
		let fst = SndNode.after(ValueNode((4u32, "a")).clone());
		assert_eq!(fst.eval(()), "a");
	}
	#[test]
	pub fn add_node() {
		let a = ValueNode(42u32);
		let b = ValueNode(6u32);
		let cons_a = ConsNode(a);
		let sum = AddNode.after(cons_a).after(b);

		assert_eq!(sum.eval(()), 48);
	}
	#[test]
	pub fn foo() {
		fn int(_: (), state: &u32) -> &u32 {
			state
		}
		fn swap(input: (u32, u32)) -> (u32, u32) {
			(input.1, input.0)
		}
		let fnn = FnNode::new(&swap);
		let fns = FnNodeWithState::new(int, 42u32);
		assert_eq!(fnn.eval((1u32, 2u32)), (2, 1));
		assert_eq!(fns.eval(()), &42);
	}
}
