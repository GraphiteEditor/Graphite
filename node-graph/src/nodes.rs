use std::{
    any::Any,
    borrow::Borrow,
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    iter,
    iter::Sum,
    marker::PhantomData,
};

use crate::{insert_after_nth, After, DynamicInput, Node};
use once_cell::sync::OnceCell;
use parking_lot::RawRwLock;
use storage_map::{StorageMap, StorageMapGuard};

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

#[derive(Default)]
/// Destructures a Tuple of two values and returns the first one
pub struct FstNode<T, U>(PhantomData<T>, PhantomData<U>);
impl<T: Copy, U> Node for FstNode<T, U> {
    type Output<'a> = &'a T where Self: 'a;
    type Input<'a> = &'a (T, U) where Self: 'a;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        let &(ref a, _) = input.borrow();
        a
    }
}

#[derive(Default)]
/// Destructures a Tuple of two values and returns the first one
pub struct SndNode<T, U>(PhantomData<T>, PhantomData<U>);
impl<T, U: Copy> Node for SndNode<T, U> {
    type Output<'a> = &'a U where Self: 'a;
    type Input<'a> = &'a (T, U) where Self: 'a;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        let &(_, ref b) = input.borrow();
        b
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

pub struct FnNode<T: Fn(&In) -> O, In, O>(T, PhantomData<In>, PhantomData<O>);
impl<T: Fn(&In) -> O, In, O> Node for FnNode<T, In, O> {
    type Output<'a> = O where Self: 'a;
    type Input<'a> = In where Self: 'a;

    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        self.0(input.borrow())
    }
}

impl<T: Fn(&In) -> O, In, O> FnNode<T, In, O> {
    pub fn new(f: T) -> Self {
        FnNode(f, PhantomData::default(), PhantomData::default())
    }
}

pub struct FnNodeWithState<T: Fn(&In, &State) -> O, In, O, State>(
    T,
    State,
    PhantomData<In>,
    PhantomData<O>,
);
impl<T: Fn(&In, &State) -> O, In, O, State> Node for FnNodeWithState<T, In, O, State> {
    type Output<'a> = O where Self: 'a;
    type Input<'a> = In where Self: 'a;

    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        self.0(input.borrow(), &self.1)
    }
}

impl<T: Fn(&In, &State) -> O, In, O, State> FnNodeWithState<T, In, O, State> {
    pub fn new(f: T, state: State) -> Self {
        FnNodeWithState(f, state, PhantomData::default(), PhantomData::default())
    }
}

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

pub struct ProxyNode<T: DynamicInput>(T);
impl<T: DynamicInput> Node for ProxyNode<T> {
    type Output<'a> = T where Self: 'a;

    type Input<'a> = &'a () where Self: 'a;

    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        todo!()
    }
}
impl<T: DynamicInput> DynamicInput for ProxyNode<T> {
    fn set_kwarg_by_name(&mut self, name: &str, value: &dyn Any) {
        self.0.set_kwarg_by_name(name, value)
    }

    fn set_arg_by_index(&mut self, index: usize, value: &dyn Any) {
        self.0.set_arg_by_index(index, value)
    }
}

/// Caches the output of a given Node and acts as a proxy
/// Automatically resets if it receives different input
pub struct SmartCacheNode<'n, 'c, NODE: Node + 'c> {
    node: &'n NODE,
    map: StorageMap<RawRwLock, HashMap<u64, CacheNode<'n, 'c, NODE>>>,
}
impl<'n: 'c, 'c, NODE: Node + 'c> Node for SmartCacheNode<'n, 'c, NODE>
where
    for<'a> NODE::Input<'a>: Hash,
{
    type Input<'a> = NODE::Input<'a> where Self: 'a, 'c : 'a;
    type Output<'a> = StorageMapGuard<'a, RawRwLock,  CacheNode<'n, 'c, NODE>> where Self: 'a, 'c: 'a;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        let mut hasher = DefaultHasher::new();
        input.borrow().hash(&mut hasher);
        let hash = hasher.finish();

        self.map
            .get_or_create_with(&hash, || CacheNode::new(self.node))
    }
}

impl<'n, 'c, NODE: Node> SmartCacheNode<'n, 'c, NODE> {
    pub fn clear(&'n mut self) {
        self.map = StorageMap::default();
    }
    pub fn new(node: &'n NODE) -> SmartCacheNode<'n, 'c, NODE> {
        SmartCacheNode {
            node,
            map: StorageMap::default(),
        }
    }
}

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
