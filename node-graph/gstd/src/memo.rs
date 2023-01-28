use graphene_core::Node;
use once_cell::sync::OnceCell;

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<T> {
	cache: OnceCell<T>,
}
impl<'i, T: 'i> Node<'i, T> for CacheNode<T> {
	type Output = &'i T;
	fn eval<'s: 'i>(&'s self, input: T) -> &'i T {
		self.cache.get_or_init(|| {
			trace!("Creating new cache node");
			input
		})
	}
}

impl<T> CacheNode<T> {
	pub fn new() -> CacheNode<T> {
		CacheNode { cache: OnceCell::new() }
	}
}
