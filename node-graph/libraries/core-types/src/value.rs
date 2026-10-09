use crate::Node;
use crate::list::Item;
use crate::poll::Poll;

#[derive(Clone, Copy)]
pub struct ClonedNode<T: Clone>(pub T);

impl<T: Clone, I: ?Sized> Node<I> for ClonedNode<T> {
	type Output = T;
	#[inline(always)]
	fn eval(&self, _input: &I) -> Poll<T> {
		Poll::Final(self.0.clone())
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

/// Yields a precomputed `Item<T>`, ignoring its input context.
/// Generated list-content variants feed each already-evaluated content slot through this so the kernel's own
/// context modifications become no-ops (the slot was evaluated once, up front, at the ambient footprint).
pub struct PrecomputedItemNode<T>(pub Item<T>);

impl<T: Clone, I: ?Sized> Node<I> for PrecomputedItemNode<T> {
	type Output = Item<T>;
	#[inline(always)]
	fn eval(&self, _input: &I) -> Poll<Item<T>> {
		Poll::Final(self.0.clone())
	}
}

impl<T> PrecomputedItemNode<T> {
	pub const fn new(item: Item<T>) -> Self {
		Self(item)
	}
}
