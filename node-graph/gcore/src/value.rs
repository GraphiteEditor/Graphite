use core::marker::PhantomData;

use crate::{Node, NodeIO};

#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct IntNode<const N: u32>;

impl<'n, const N: u32> NodeIO<'n> for IntNode<N> {
	type Output = u32;
	type Input = ();
}

impl<const N: u32> Node for IntNode<N> {
	fn eval<'i, 's: 'i>(&'s self, _input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		N
	}
}

#[derive(Default, Debug)]
pub struct ValueNode<T>(pub T);

impl<'n, T> NodeIO<'n> for ValueNode<T> {
	type Output = &'n T;
	type Input = ();
}

impl<'n, T: 'n> Node for ValueNode<T> {
	fn eval<'i, 's: 'i>(&'s self, _input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		&self.0
	}
}

impl<T> ValueNode<T> {
	pub const fn new(value: T) -> ValueNode<T> {
		ValueNode(value)
	}
}

impl<T> From<T> for ValueNode<T> {
	fn from(value: T) -> Self {
		ValueNode::new(value)
	}
}
impl<T: Clone> Clone for ValueNode<T> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}
impl<T: Clone + Copy> Copy for ValueNode<T> {}

#[derive(Default)]
pub struct DefaultNode<T>(PhantomData<T>);

impl<'n, T: Default> NodeIO<'n> for DefaultNode<T> {
	type Output = T;
	type Input = ();
}

impl<'n, T: Default + 'n> Node for DefaultNode<T> {
	fn eval<'i, 's: 'i>(&'s self, _input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {
		T::default()
	}
}

impl<T> DefaultNode<T> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

#[repr(C)]
/// Return the unit value
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct UnitNode;

impl<'n> NodeIO<'n> for UnitNode {
	type Output = ();
	type Input = ();
}

impl Node for UnitNode {
	fn eval<'i, 's: 'i>(&'s self, _input: <Self as NodeIO<'i>>::Input) -> <Self as NodeIO<'i>>::Output {}
}

impl UnitNode {
	pub const fn new() -> Self {
		UnitNode
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_int_node() {
		let node = IntNode::<5>;
		assert_eq!(node.eval(()), 5);
	}
	#[test]
	fn test_value_node() {
		let node = ValueNode::new(5);
		assert_eq!(*node.eval(()), 5);
	}
	#[test]
	fn test_default_node() {
		let node = DefaultNode::<u32>::new();
		assert_eq!(node.eval(()), 0);
	}
	#[test]
	fn test_unit_node() {
		let node = UnitNode::new();
		assert_eq!(node.eval(()), ());
	}
}
