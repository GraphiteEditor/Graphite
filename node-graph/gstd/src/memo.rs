use graphene_core::Node;
use once_cell::sync::OnceCell;
use std::borrow::Borrow;

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<'n, 'c, CachedNode: Node + 'c> {
    node: &'n CachedNode,
    cache: OnceCell<CachedNode::Output<'c>>,
}
impl<'n: 'c, 'c, CashedNode: Node> Node for CacheNode<'n, 'c, CashedNode> {
    type Output<'a> = &'a CashedNode::Output<'c> where 'c: 'a;
    type Input<'a> = CashedNode::Input<'c> where 'c: 'a;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        self.cache.get_or_init(|| self.node.eval(input))
    }
}

impl<'n, 'c, CachedNode: Node> CacheNode<'n, 'c, CachedNode> {
    pub fn clear(&'n mut self) {
        self.cache = OnceCell::new();
    }
    pub fn new(node: &'n CachedNode) -> CacheNode<'n, 'c, CachedNode> {
        CacheNode {
            node,
            cache: OnceCell::new(),
        }
    }
}
