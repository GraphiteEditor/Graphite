use crate::list::Item;
use crate::registry::DynFuture;
use crate::{Node, WasmNotSend};

#[derive(Clone, Copy)]
pub struct ClonedNode<T: Clone>(pub T);

impl<'i, T: Clone + 'i, I> Node<'i, I> for ClonedNode<T> {
	type Output = T;
	#[inline(always)]
	fn eval(&'i self, _input: I) -> Self::Output {
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

/// Yields a precomputed `Item<T>` as a ready future, ignoring its input context.
/// Generated list-content variants feed each already-evaluated content slot through this so the kernel's own
/// context modifications become no-ops (the slot was evaluated once, up front, at the ambient footprint).
pub struct PrecomputedItemNode<T>(pub Item<T>);

impl<'i, T: Clone + WasmNotSend + 'i, I: 'i> Node<'i, I> for PrecomputedItemNode<T> {
	type Output = DynFuture<'i, Item<T>>;
	#[inline(always)]
	fn eval(&'i self, _input: I) -> Self::Output {
		let item = self.0.clone();
		Box::pin(async move { item })
	}
}

impl<T> PrecomputedItemNode<T> {
	pub const fn new(item: Item<T>) -> Self {
		Self(item)
	}
}
