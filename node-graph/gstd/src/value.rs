use std::borrow::Borrow;

use graphene_core::Node;

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

#[rustfmt::skip]
pub trait OutputNode<'a, T>: Node<Output<'a> = T> where Self: 'a {}
impl<T: std::default::Default> DefaultNode for T {}

impl<T> ValueNode<T> {
    pub fn new(value: T) -> ValueNode<T> {
        ValueNode(value)
    }
}

pub trait DefaultNode: Default {
    fn default_node() -> ValueNode<Self> {
        ValueNode::new(Self::default())
    }
}
