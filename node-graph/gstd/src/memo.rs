use graphene_core::{Cache, Node};
use once_cell::sync::OnceCell;
use std::marker::PhantomData;

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<'n, CachedNode: Node<'n>> {
    node: CachedNode,
    cache: OnceCell<CachedNode::Output>,
    _phantom: PhantomData<&'n ()>,
}
impl<'n, CashedNode: Node<'n>> Node<'n> for CacheNode<'n, CashedNode> {
    type Output = &'n CashedNode::Output;
    fn eval(&'n self) -> Self::Output {
        self.cache.get_or_init(|| self.node.eval())
    }
}

impl<'n, CachedNode: Node<'n>> CacheNode<'n, CachedNode> {
    pub fn clear(&'n mut self) {
        self.cache = OnceCell::new();
    }
    pub fn new(node: CachedNode) -> CacheNode<'n, CachedNode> {
        CacheNode {
            node,
            cache: OnceCell::new(),
            _phantom: PhantomData,
        }
    }
}
impl<'n, CachedNode: Node<'n>> Cache for CacheNode<'n, CachedNode> {
    fn clear(&mut self) {
        self.cache = OnceCell::new();
    }
}

/*use dyn_any::{DynAny, StaticType};
#[derive(DynAny)]
struct Boo<'a>(&'a u8);
*/
