use graphene_core::{Cache, Node};
use once_cell::sync::OnceCell;

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<T> {
	cache: OnceCell<T>,
}
impl<'n, T> Node<T> for &'n CacheNode<T> {
	type Output = &'n T;
	fn eval(self, input: T) -> Self::Output {
		self.cache.get_or_init(|| {
			trace!("Creating new cache node");
			input
		})
	}
}
impl<T> Node<T> for CacheNode<T> {
	type Output = T;
	fn eval(self, input: T) -> Self::Output {
		input
	}
}

impl<T> CacheNode<T> {
	pub fn new() -> CacheNode<T> {
		CacheNode { cache: OnceCell::new() }
	}
}
impl<T> Cache for CacheNode<T> {
	fn clear(&mut self) {
		self.cache = OnceCell::new();
	}
}
