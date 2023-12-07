use crate::Node;

use core::{
	cell::{Cell, RefCell, RefMut},
	marker::PhantomData,
};

#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct IntNode<const N: u32>;

impl<'i, const N: u32> Node<'i, ()> for IntNode<N> {
	type Output = u32;
	#[inline(always)]
	fn eval(&'i self, _input: ()) -> Self::Output {
		N
	}
}

#[derive(Default, Debug, Clone, Copy)]
pub struct ValueNode<T>(pub T);

impl<'i, T: 'i> Node<'i, ()> for ValueNode<T> {
	type Output = &'i T;
	#[inline(always)]
	fn eval(&'i self, _input: ()) -> Self::Output {
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

#[derive(Default, Debug, Clone)]
pub struct RefCellMutNode<T>(pub RefCell<T>);

impl<'i, T: 'i> Node<'i, ()> for RefCellMutNode<T> {
	type Output = RefMut<'i, T>;
	#[inline(always)]
	fn eval(&'i self, _input: ()) -> Self::Output {
		let a = self.0.borrow_mut();
		a
	}
}

impl<T> RefCellMutNode<T> {
	pub const fn new(value: T) -> RefCellMutNode<T> {
		RefCellMutNode(RefCell::new(value))
	}
}

#[derive(Default)]
pub struct OnceCellNode<T>(pub Cell<T>);

impl<'i, T: Default + 'i> Node<'i, ()> for OnceCellNode<T> {
	type Output = T;
	#[inline(always)]
	fn eval(&'i self, _input: ()) -> Self::Output {
		self.0.replace(T::default())
	}
}

impl<T> OnceCellNode<T> {
	pub const fn new(value: T) -> OnceCellNode<T> {
		OnceCellNode(Cell::new(value))
	}
}

#[derive(Clone, Copy)]
pub struct ClonedNode<T: Clone>(pub T);

impl<'i, T: Clone + 'i> Node<'i, ()> for ClonedNode<T> {
	type Output = T;
	#[inline(always)]
	fn eval(&'i self, _input: ()) -> Self::Output {
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

#[derive(Clone, Copy)]
/// The DebugClonedNode logs every time it is evaluated.
/// This is useful for debugging.
pub struct DebugClonedNode<T: Clone>(pub T);

impl<'i, T: Clone + 'i> Node<'i, ()> for DebugClonedNode<T> {
	type Output = T;
	#[inline(always)]
	fn eval(&'i self, _input: ()) -> Self::Output {
		#[cfg(not(target_arch = "spirv"))]
		// KEEP THIS `debug!()` - It acts as the output for the debug node itself
		log::debug!("DebugClonedNode::eval");

		self.0.clone()
	}
}

impl<T: Clone> DebugClonedNode<T> {
	pub const fn new(value: T) -> DebugClonedNode<T> {
		DebugClonedNode(value)
	}
}

#[derive(Clone, Copy)]
pub struct CopiedNode<T: Copy>(pub T);

impl<'i, T: Copy + 'i> Node<'i, ()> for CopiedNode<T> {
	type Output = T;
	#[inline(always)]
	fn eval(&'i self, _input: ()) -> Self::Output {
		self.0
	}
}

impl<T: Copy> CopiedNode<T> {
	pub const fn new(value: T) -> CopiedNode<T> {
		CopiedNode(value)
	}
}

#[derive(Default)]
pub struct DefaultNode<T>(PhantomData<T>);

impl<'i, T: Default + 'i> Node<'i, ()> for DefaultNode<T> {
	type Output = T;
	fn eval(&'i self, _input: ()) -> Self::Output {
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
	fn eval(&'i self, _input: T) -> Self::Output {}
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
	#[allow(clippy::unit_cmp)]
	fn test_unit_node() {
		let node = ForgetNode::new();
		assert_eq!(node.eval(()), ());
	}
}
