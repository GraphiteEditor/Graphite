use graphene_core::Node;
use once_cell::sync::OnceCell;

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<'n, CachedNode: Node<'n, Input>, Input> {
    node: &'n CachedNode,
    cache: OnceCell<CachedNode::Output>,
}
impl<'n, CashedNode: Node<'n, Input>, Input> Node<'n, Input> for CacheNode<'n, CashedNode, Input> {
    type Output = &'n CashedNode::Output;
    fn eval(&'n self, input: &'n Input) -> Self::Output {
        self.cache.get_or_init(|| self.node.eval(input))
    }
}

impl<'n, CachedNode: Node<'n, Input>, Input> CacheNode<'n, CachedNode, Input> {
    pub fn clear(&'n mut self) {
        self.cache = OnceCell::new();
    }
    pub fn new(node: &'n CachedNode) -> CacheNode<'n, CachedNode, Input> {
        CacheNode {
            node,
            cache: OnceCell::new(),
        }
    }
}
