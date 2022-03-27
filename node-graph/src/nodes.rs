use std::{
    any::Any, collections::hash_map::DefaultHasher, hash::Hasher, iter, iter::Sum,
    marker::PhantomData,
};

use crate::{insert_after_nth, After, Node};
use once_cell::sync::OnceCell;

pub struct IntNode<const N: u32>;
impl<'n, const N: u32> Node<'n, u32> for IntNode<N> {
    fn eval(&'n self, _input: impl Iterator<Item = &'n dyn Any>) -> u32 {
        N
    }
}

#[derive(Default)]
pub struct ValueNode<T>(T);
impl<'n, T> Node<'n, &'n T> for ValueNode<T> {
    fn eval(&'n self, _input: impl Iterator<Item = &'n dyn Any>) -> &T {
        &self.0
    }
}
impl<'n, T: Copy> Node<'n, T> for ValueNode<T> {
    fn eval(&'n self, _input: impl Iterator<Item = &'n dyn Any>) -> T {
        self.0
    }
}

impl<T> ValueNode<T> {
    pub fn new(value: T) -> ValueNode<T> {
        ValueNode(value)
    }
}

pub struct AddNode;
impl<'n, T: Sum + 'static + Copy> Node<'n, T> for AddNode {
    fn eval(&'n self, input: impl Iterator<Item = &'n dyn Any>) -> T {
        input.map(|x| *(x.downcast_ref::<T>().unwrap())).sum::<T>()
    }
}

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<'n, NODE: Node<'n, OUT>, OUT: Clone> {
    node: &'n NODE,
    cache: OnceCell<OUT>,
}
impl<'n, NODE: Node<'n, OUT>, OUT: Clone> Node<'n, &'n OUT> for CacheNode<'n, NODE, OUT> {
    fn eval(&'n self, input: impl Iterator<Item = &'n dyn Any> + Clone) -> &'n OUT {
        self.cache.get_or_init(|| self.node.eval(input))
    }
}

impl<'n, NODE: Node<'n, OUT>, OUT: Clone> CacheNode<'n, NODE, OUT> {
    fn clear(&'n mut self) {
        self.cache = OnceCell::new();
    }
    fn new(node: &'n NODE) -> CacheNode<'n, NODE, OUT> {
        CacheNode {
            node,
            cache: OnceCell::new(),
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

pub struct ComposeNode<'n, FIRST, SECOND, INTERMEDIATE>
where
    FIRST: Node<'n, INTERMEDIATE>,
{
    first: &'n FIRST,
    second: &'n SECOND,
    _phantom_data: PhantomData<INTERMEDIATE>,
}

impl<'n, FIRST, SECOND, OUT: 'n, INTERMEDIATE: 'static + Clone> Node<'n, OUT>
    for ComposeNode<'n, FIRST, SECOND, INTERMEDIATE>
where
    FIRST: Node<'n, INTERMEDIATE>,
    SECOND: Node<'n, OUT>,
{
    fn eval(&'n self, input: impl Iterator<Item = &'n dyn Any> + Clone) -> OUT {
        let curry = CurryNthArgNode::<'_, _, _, _, _, 0>::new(self.second, self.first);
        CurryNthArgNode::<'_, _, _, _, _, 0>::new(curry, ValueNode::new(input)).eval(input)
    }
}

impl<'n, FIRST, SECOND, INTERMEDIATE: 'static> ComposeNode<'n, FIRST, SECOND, INTERMEDIATE>
where
    FIRST: Node<'n, INTERMEDIATE>,
{
    pub fn new(first: &'n FIRST, second: &'n SECOND) -> Self {
        ComposeNode::<'n, FIRST, SECOND, INTERMEDIATE> {
            first,
            second,
            _phantom_data: PhantomData::default(),
        }
    }
}

impl<'n, OUT, SECOND: Node<'n, OUT>> After<'n, OUT, SECOND> for SECOND {
    fn after<INTERMEDIATE, FIRST: Node<'n, INTERMEDIATE>>(
        &'n self,
        first: &'n FIRST,
    ) -> ComposeNode<'n, FIRST, SECOND, INTERMEDIATE> {
        ComposeNode::<'n, FIRST, SECOND, INTERMEDIATE> {
            first,
            second: self,
            _phantom_data: PhantomData::default(),
        }
    }
}
