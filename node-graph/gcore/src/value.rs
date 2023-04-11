use core::marker::PhantomData;
use dyn_any::{DynAny, StaticType, StaticTypeSized};

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

impl<T: StaticTypeSized> StaticType for ValueNode<T> {
	type Static = ValueNode<T::Static>;
}

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

#[derive(Clone)]
pub struct ClonedNode<T: Clone>(pub T);

impl<T: Clone + StaticTypeSized> StaticType for ClonedNode<T>
where
	T::Static: Clone,
{
	type Static = ClonedNode<T::Static>;
}

impl<'i, T: Clone + 'i> Node<'i, ()> for ClonedNode<T> {
	type Output = T;
	fn eval<'s: 'i>(&'s self, _input: ()) -> Self::Output {
		self.0.clone()
	}
}

impl<T: Clone> ClonedNode<T> {
	pub const fn new(value: T) -> ClonedNode<T> {
		ClonedNode(value)
	}
}

impl<T: Clone> From<T> for ClonedNode<T> {
	fn from(value: T) -> Self {
		ClonedNode::new(value)
	}
}
impl<T: Clone + Copy> Copy for ClonedNode<T> {}

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
pub struct ForgetNode;

impl<'i, T: 'i> Node<'i, T> for ForgetNode {
	type Output = ();
	fn eval<'s: 'i>(&self, _input: T) -> Self::Output {}
}

impl ForgetNode {
	pub const fn new() -> Self {
		ForgetNode
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
		assert_eq!(node.eval(()), &5);
		let type_erased = &node as &dyn for<'a> Node<'a, (), Output = &'a i32>;
		assert_eq!(type_erased.eval(()), &5);
	}
	#[test]
	fn test_default_node() {
		let node = DefaultNode::<u32>::new();
		assert_eq!(node.eval(()), 0);
	}
	#[test]
	fn test_unit_node() {
		let node = ForgetNode::new();
		assert_eq!(node.eval(()), ());
	}
}
