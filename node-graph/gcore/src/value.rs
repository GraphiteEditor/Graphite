use core::marker::PhantomData;

use crate::Node;

#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct IntNode<const N: u32>;

impl<'i, const N: u32> Node<'i, ()> for IntNode<N> {
	type Output = u32;
	fn eval<'s: 'i>(&'s self, _input: ()) -> Self::Output {
		N
	}
}

#[derive(Default, Debug)]
pub struct ValueNode<T>(pub T);

impl<'i, T: 'i> Node<'i, ()> for ValueNode<T> {
	type Output = &'i T;
	fn eval<'s: 'i>(&'s self, _input: ()) -> Self::Output {
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

impl<'i, T: Default + 'i> Node<'i, ()> for DefaultNode<T> {
	type Output = T;
	fn eval<'s: 'i>(&self, _input: ()) -> Self::Output {
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

impl<'i> Node<'i, ()> for UnitNode {
	type Output = ();
	fn eval<'s: 'i>(&self, _input: ()) -> Self::Output {}
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
