use graphene_core::{Cache, Node};
use once_cell::sync::OnceCell;

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<CachedNode: Node<I>, I> {
	node: CachedNode,
	cache: OnceCell<CachedNode::Output>,
}
impl<'n, CashedNode: Node<I> + Copy, I> Node<I> for &'n CacheNode<CashedNode, I> {
	type Output = &'n CashedNode::Output;
	fn eval(self, input: I) -> Self::Output {
		self.cache.get_or_init(|| self.node.eval(input))
	}
}

impl<'n, CachedNode: Node<I>, I> CacheNode<CachedNode, I> {
	pub fn clear(&'n mut self) {
		self.cache = OnceCell::new();
	}
	pub fn new(node: CachedNode) -> CacheNode<CachedNode, I> {
		CacheNode { node, cache: OnceCell::new() }
	}
}
impl<CachedNode: Node<I>, I> Cache for CacheNode<CachedNode, I> {
	fn clear(&mut self) {
		self.cache = OnceCell::new();
	}
}

/*use dyn_any::{DynAny, StaticType};
#[derive(DynAny)]
struct Boo<'a>(&'a u8);
*/
