use std::{borrow::Borrow, marker::PhantomData};

use const_default::ConstDefault;

use crate::{Exec, Node};

pub struct IntNode<const N: u32>;
impl<const N: u32> Node for IntNode<N> {
    type Input<'o> = ();
    type Output<'i> = u32;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&self, _input: I) -> u32 {
        N
    }
}

#[derive(Default)]
pub struct ValueNode<T>(T);
impl<T> Node for ValueNode<T> {
    type Input<'i> = () where T: 'i;
    type Output<'o> = &'o T where T: 'o;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, _input: I) -> &T {
        &self.0
    }
}
impl<T> ValueNode<T> {
    pub const fn new(value: T) -> ValueNode<T> {
        ValueNode(value)
    }
}

#[derive(Default)]
pub struct DefaultNode<T>(PhantomData<T>);
impl<T: Default> Node for DefaultNode<T> {
    type Input<'i> = () where T: 'i;
    type Output<'o> = T where T: 'o;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, _input: I) -> T {
        T::default()
    }
}
impl<T> DefaultNode<T> {
    pub const fn new() -> DefaultNode<T> {
        DefaultNode(PhantomData)
    }
}

pub struct DefaultRefNode<T>(ValueNode<T>);
impl<T: 'static> Node for DefaultRefNode<T> {
    type Input<'i> = () where T: 'i;
    type Output<'o> = &'o T where T: 'o;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, _input: I) -> &'a T {
        self.0.exec()
    }
}
#[cfg(feature = "const_default")]
impl<T: ConstDefault> DefaultRefNode<T> {
    pub const fn new() -> DefaultRefNode<T> {
        DefaultRefNode(ValueNode::new(T::DEFAULT))
    }
}
#[cfg(not(feature = "const_default"))]
impl<T: Default> DefaultRefNode<T> {
    pub fn new() -> DefaultRefNode<T> {
        DefaultRefNode(ValueNode::new(T::default()))
    }
}
