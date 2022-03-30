use std::{
    any::Any, borrow::Borrow, collections::hash_map::DefaultHasher, hash::Hasher, iter, iter::Sum,
    marker::PhantomData,
};

use crate::{insert_after_nth, After, Node};
use once_cell::sync::OnceCell;

pub struct IntNode<const N: u32>;
impl<const N: u32> Node for IntNode<N> {
    type Output<'a> = u32;
    type Input<'a> = ();
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&self, _input: I) -> u32 {
        N
    }
}

#[derive(Default)]
pub struct ValueNode<T>(T);
impl<T> Node for ValueNode<T> {
    type Output<'o> = &'o T where T: 'o;
    type Input<'i> = () where T: 'i;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, _input: I) -> &T {
        &self.0
    }
}

impl<T> ValueNode<T> {
    pub fn new(value: T) -> ValueNode<T> {
        ValueNode(value)
    }
}

#[derive(Default)]
pub struct AddNode<T>(PhantomData<T>);
impl<T: std::ops::Add + 'static + Copy> Node for AddNode<T> {
    type Output<'a> = <T as std::ops::Add>::Output;
    type Input<'a> = (T, T);
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> T::Output {
        input.borrow().0 + input.borrow().1
    }
}

/// Caches the output of a given Node and acts as a proxy
pub struct CachingNode<'n, 'c, CachedNode: Node + 'c> {
    node: &'n CachedNode,
    cache: OnceCell<CachedNode::Output<'c>>,
}
impl<'n: 'c, 'c, CashedNode: Node> Node for CachingNode<'n, 'c, CashedNode> {
    type Output<'a> = &'a CashedNode::Output<'c> where 'c: 'a;
    type Input<'a> = CashedNode::Input<'c> where 'c: 'a;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        self.cache.get_or_init(|| self.node.eval(input))
    }
}

impl<'n, 'c, CachedNode: Node> CachingNode<'n, 'c, CachedNode> {
    pub fn clear(&'n mut self) {
        self.cache = OnceCell::new();
    }
    pub fn new(node: &'n CachedNode) -> CachingNode<'n, 'c, CachedNode> {
        CachingNode {
            node,
            cache: OnceCell::new(),
        }
    }
}

pub struct ComposeNode<'n, FIRST, SECOND> {
    first: &'n FIRST,
    second: &'n SECOND,
}

impl<'n, FIRST, SECOND> Node for ComposeNode<'n, FIRST, SECOND>
where
    FIRST: Node,
    SECOND: Node,
    for<'a> FIRST::Output<'a>: Borrow<SECOND::Input<'a>>,
{
    type Input<'a> = FIRST::Input<'a> where Self: 'a;
    type Output<'a> = SECOND::Output<'a> where Self: 'a;

    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        // evaluate the first node with the given input
        // and then pipe the result from the first computation
        // into the second node
        let arg = self.first.eval(input);
        self.second.eval(arg)
    }
}

impl<'n, FIRST, SECOND> ComposeNode<'n, FIRST, SECOND>
where
    FIRST: Node,
{
    pub fn new(first: &'n FIRST, second: &'n SECOND) -> Self {
        ComposeNode::<'n, FIRST, SECOND> { first, second }
    }
}

impl<'n, SECOND: Node> After<SECOND> for SECOND {
    fn after<'a, FIRST: Node>(&'a self, first: &'a FIRST) -> ComposeNode<'a, FIRST, SECOND> {
        ComposeNode::<'a, FIRST, SECOND> {
            first,
            second: self,
        }
    }
}

/*
/// Caches the output of a given Node and acts as a proxy
/// Automatically resets if it receives different input
pub struct SmartCacheNode<'n, NODE: Node<'n, OUT>, OUT: Clone> {
    node: &'n NODE,
    map: dashmap::DashMap<u64, CacheNode<'n, NODE, OUT>>,
}
impl<'n, NODE: for<'a> Node<'a, OUT>, OUT: Clone> Node<'n, &'n CacheNode<'n, NODE, OUT>>
    for SmartCacheNode<'n, NODE, OUT>
{
    fn eval(
        &'n self,
        input: impl Iterator<Item = &'n dyn Any> + Clone,
    ) -> &'n CacheNode<'n, NODE, OUT> {
        let mut hasher = DefaultHasher::new();
        input.clone().for_each(|value| unsafe {
            hasher.write(std::slice::from_raw_parts(
                value as *const dyn Any as *const u8,
                std::mem::size_of_val(value),
            ))
        });
        let hash = hasher.finish();
        self.map.entry(hash).or_insert(CacheNode::new(self.node));
        fn map<'a, 'c, 'd, N, OUT: Clone>(
            _key: &'a u64,
            node: &'c CacheNode<'d, N, OUT>,
        ) -> &'c CacheNode<'b, N, OUT>
        where
            N: for<'b> Node<'b, OUT>,
        {
            node
        }
        let foo: Option<&CacheNode<'n, NODE, OUT>> = self.map.view(&hash, map);
        foo.unwrap()
    }
}

impl<'n, NODE: Node<'n, OUT>, OUT: Clone> SmartCacheNode<'n, NODE, OUT> {
    fn clear(&'n mut self) {
        self.map.clear();
    }
    fn new(node: &'n NODE) -> SmartCacheNode<'n, NODE, OUT> {
        SmartCacheNode {
            node,
            map: dashmap::DashMap::new(),
        }
    }
}*/

/*

pub struct CurryNthArgNode<
    'n,
    CurryNode: Node<'n, OUT>,
    ArgNode: Node<'n, ARG>,
    ARG: Clone,
    OUT,
    const NTH: usize,
> {
    node: &'n CurryNode,
    arg: CacheNode<'n, ArgNode, ARG>,
    _phantom_out: std::marker::PhantomData<OUT>,
    _phantom_arg: std::marker::PhantomData<ARG>,
}
impl<
        'n,
        CurryNode: Node<'n, OUT>,
        ArgNode: Node<'n, ARG>,
        ARG: 'static + Clone,
        OUT,
        const NTH: usize,
    > Node<'n, OUT> for CurryNthArgNode<'n, CurryNode, ArgNode, ARG, OUT, NTH>
{
    fn eval(&'n self, input: impl Iterator<Item = &'n dyn Any> + Clone) -> OUT {
        let arg = self.arg.eval(iter::empty());
        let arg: &dyn Any = arg as &dyn Any;
        self.node.eval(insert_after_nth(NTH, input, arg))
    }
}

impl<'n, CurryNode: Node<'n, Out>, ArgNode: Node<'n, Arg>, Arg: Clone, Out, const Nth: usize>
    CurryNthArgNode<'n, CurryNode, ArgNode, Arg, Out, Nth>
{
    pub fn new(node: &'n CurryNode, arg: &'n ArgNode) -> Self {
        CurryNthArgNode::<'n, CurryNode, ArgNode, Arg, Out, Nth> {
            node,
            arg: CacheNode::new(arg),
            _phantom_out: PhantomData::default(),
            _phantom_arg: PhantomData::default(),
        }
    }
}
*/
/*
*/
