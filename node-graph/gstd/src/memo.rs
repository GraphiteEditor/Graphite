use graphene_core::{Node, NodeIO};
use node_macro;
use once_cell::sync::OnceCell;

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<T> {
	cache: OnceCell<T>,
}
#[node_macro::node_fn(CacheNode)]
fn cache<T>(input: T) -> &'input T {
	self.cache.get_or_init(|| {
		trace!("Creating new cache node");
		input
	})
}
