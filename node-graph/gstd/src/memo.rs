use graphene_core::{Cache, Node};
use once_cell::sync::OnceCell;
use std::marker::PhantomData;

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<'n, CachedNode: Node<'n, I>, I> {
	node: CachedNode,
	cache: OnceCell<CachedNode::Output>,
	_phantom: PhantomData<&'n ()>,
}
impl<'n, CashedNode: Node<'n, I>, I> Node<'n, I> for CacheNode<'n, CashedNode, I>
where
	CashedNode::Output: 'n,
{
	type Output = &'n CashedNode::Output;
	fn eval(&'n self, input: I) -> Self::Output {
		self.cache.get_or_init(|| self.node.eval(input))
	}
}

impl<'n, CachedNode: Node<'n, I>, I> CacheNode<'n, CachedNode, I> {
	pub fn clear(&'n mut self) {
		self.cache = OnceCell::new();
	}
	pub fn new(node: CachedNode) -> CacheNode<'n, CachedNode, I> {
		CacheNode {
			node,
			cache: OnceCell::new(),
			_phantom: PhantomData,
		}
	}
}
impl<'n, CachedNode: Node<'n, I>, I> Cache for CacheNode<'n, CachedNode, I> {
	fn clear(&mut self) {
		self.cache = OnceCell::new();
	}
}

/*use dyn_any::{DynAny, StaticType};
#[derive(DynAny)]
struct Boo<'a>(&'a u8);
*/
