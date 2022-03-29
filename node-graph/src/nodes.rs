use std::{
    any::Any, borrow::Borrow, collections::hash_map::DefaultHasher, hash::Hasher, iter, iter::Sum,
    marker::PhantomData,
};

use crate::{insert_after_nth, /*After,*/ Node};
use once_cell::sync::OnceCell;

pub struct IntNode<const N: u32>;
impl<const N: u32> Node for IntNode<N> {
    type Out<'a> = u32;
    type Input<'a> = ();
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&self, _input: I) -> u32 {
        N
    }
}

#[derive(Default)]
pub struct ValueNode<T>(T);
impl<T> Node for ValueNode<T> {
    type Out<'a> = &'a T where T: 'a;
    type Input<'a> = () where T: 'a;
    fn eval<'n, I: Borrow<Self::Input<'n>>>(&'n self, _input: I) -> &T {
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
    type Out<'a> = T::Output;
    type Input<'a> = (T, T);
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> T::Output {
        input.borrow().0 + input.borrow().1
    }
}

/// Caches the output of a given Node and acts as a proxy
pub struct CacheNode<'n, 'c, CachedNode: Node + 'c> {
    node: &'n CachedNode,
    cache: OnceCell<CachedNode::Out<'c>>,
}
impl<'n: 'c, 'c, CashedNode: Node> Node for CacheNode<'n, 'c, CashedNode> {
    type Out<'a> = &'a CashedNode::Out<'c> where 'c: 'a;
    type Input<'a> = CashedNode::Input<'c> where 'c: 'a;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Out<'a> {
        self.cache.get_or_init(|| self.node.eval(input))
    }
}

impl<'n, 'c, NODE: Node> CacheNode<'n, 'c, NODE> {
    pub fn clear(&'n mut self) {
        self.cache = OnceCell::new();
    }
    pub fn new(node: &'n NODE) -> CacheNode<'n, 'c, NODE> {
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
pub struct ComposeNode<'n, FIRST, SECOND>
where
    FIRST: Node,
{
    first: &'n FIRST,
    second: &'n SECOND,
    _phantom_data: PhantomData<INTERMEDIATE>,
}

impl<'n, FIRST, SECOND> Node for ComposeNode<'n, FIRST, SECOND>
where
    FIRST: Node,
    SECOND: Node,
{
    fn eval<'a, T: &Self::Input<'a>>(&'a self, input: T) -> &Self::Out<'a> {
        self.second.eval(self.first.eval(input))
        //let curry = CurryNthArgNode::<'_, _, _, _, _, 0>::new(self.second, self.first);
        //CurryNthArgNode::<'_, _, _, _, _, 0>::new(curry, ValueNode::new(input)).eval(input)
    }

    type Out<'a> = SECOND::Out<'a>
    where
        Self: 'a;

    type Input<'a> = FIRST::Input<'a>
    where
        Self: 'a;
}
*/
/*

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
*/
