use crate::Node;

use core::ops::{Deref, DerefMut, Index, IndexMut};

pub struct SetNode<Storage> {
	storage: Storage,
}
impl<'input, T: 'input, I: 'input, A: 'input + 'input, S0: 'input> Node<'input, (T, I)> for SetNode<S0>
where
	A: DerefMut,
	A::Target: IndexMut<I, Output = T>,
	S0: for<'any_input> Node<'input, (), Output = A>,
{
	type Output = ();
	#[inline]
	fn eval(&'input self, input: (T, I)) -> Self::Output {
		let mut storage = self.storage.eval(());
		let (value, index) = input;
		*storage.deref_mut().index_mut(index).deref_mut() = value;
	}
}
impl<'input, S0: 'input> SetNode<S0> {
	pub const fn new(storage: S0) -> Self {
		Self { storage }
	}
}

pub struct ExtractXNode {}

#[node_macro::node_fn(ExtractXNode)]
fn extract_x_node(input: glam::UVec3) -> usize {
	input.x as usize
}

pub struct SetOwnedNode<Storage> {
	storage: core::cell::RefCell<Storage>,
}

impl<Storage> SetOwnedNode<Storage> {
	pub fn new(storage: Storage) -> Self {
		Self {
			storage: core::cell::RefCell::new(storage),
		}
	}
}

impl<'input, I: 'input, T: 'input, Storage, A: ?Sized> Node<'input, (T, I)> for SetOwnedNode<Storage>
where
	Storage: DerefMut<Target = A> + 'input,
	A: IndexMut<I, Output = T> + 'input,
{
	type Output = ();
	fn eval(&'input self, input: (T, I)) -> Self::Output {
		let (value, index) = input;
		*self.storage.borrow_mut().index_mut(index) = value;
	}
}

pub struct GetNode<Storage> {
	storage: Storage,
}

impl<Storage> GetNode<Storage> {
	pub fn new(storage: Storage) -> Self {
		Self { storage }
	}
}

impl<'input, I: 'input, T: 'input, Storage, SNode, A: ?Sized> Node<'input, I> for GetNode<SNode>
where
	SNode: Node<'input, (), Output = Storage>,
	Storage: Deref<Target = A> + 'input,
	A: Index<I, Output = T> + 'input,
	T: Clone,
{
	type Output = T;
	fn eval(&'input self, index: I) -> Self::Output {
		let storage = self.storage.eval(());
		storage.deref().index(index).clone()
	}
}

#[cfg(test)]
mod test {
	use crate::value::{CopiedNode, OnceCellNode};
	use crate::Node;

	use super::*;
	#[test]
	fn get_node_array() {
		let storage = [1, 2, 3];
		let node = GetNode::new(CopiedNode::new(&storage));
		assert_eq!((&node as &dyn Node<'_, usize, Output = i32>).eval(1), 2);
	}

	#[test]
	fn get_node_vec() {
		let storage = vec![1, 2, 3];
		let node = GetNode::new(CopiedNode::new(&storage));
		assert_eq!(node.eval(1), 2);
	}

	#[test]
	fn get_node_slice() {
		let storage: &[i32] = &[1, 2, 3];
		let node = GetNode::new(CopiedNode::new(storage));
		let _ = &node as &dyn Node<'_, usize, Output = i32>;
		assert_eq!(node.eval(1), 2);
	}

	#[test]
	fn set_node_slice() {
		let mut backing_storage = [1, 2, 3];
		let storage: &mut [i32] = &mut backing_storage;
		let storage_node = OnceCellNode::new(storage);
		let node = SetNode::new(storage_node);
		node.eval((4, 1));
		assert_eq!(backing_storage, [1, 4, 3]);
	}

	#[test]
	fn set_owned_node_array() {
		let mut storage = [1, 2, 3];
		let node = SetOwnedNode::new(&mut storage);
		node.eval((4, 1));
		assert_eq!(storage, [1, 4, 3]);
	}

	#[test]
	fn set_owned_node_vec() {
		let mut storage = vec![1, 2, 3];
		let node = SetOwnedNode::new(&mut storage);
		node.eval((4, 1));
		assert_eq!(storage, [1, 4, 3]);
	}

	#[test]
	fn set_owned_node_slice() {
		let mut backing_storage = [1, 2, 3];
		let storage: &mut [i32] = &mut backing_storage;
		let node = SetOwnedNode::new(storage);
		let node = &node as &dyn Node<'_, (i32, usize), Output = ()>;
		node.eval((4, 1));
		assert_eq!(backing_storage, [1, 4, 3]);
	}
}
