use std::{any::Any, borrow::Borrow};

use crate::{DynamicInput, Node};
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

pub trait After: Sized {
    fn after<'a, First: Node>(&'a self, first: &'a First) -> ComposeNode<'a, First, Self> {
        ComposeNode::new(first, self)
    }
}
impl<Second: Node> After for Second {}

pub struct ProxyNode<T: DynamicInput>(T);
impl<T: DynamicInput> Node for ProxyNode<T> {
    type Output<'a> = &'a T where Self: 'a;
    type Input<'a> = &'a () where Self: 'a;

    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, _input: I) -> Self::Output<'a> {
        &self.0
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
