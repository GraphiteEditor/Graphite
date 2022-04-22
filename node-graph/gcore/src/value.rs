use core::marker::PhantomData;

use crate::Node;

pub struct IntNode<const N: u32>;
impl<'n, const N: u32> Node<'n, ()> for IntNode<N> {
    type Output = u32;
    fn eval(&self, _input: &()) -> u32 {
        N
    }
}

#[derive(Default)]
pub struct ValueNode<'n, T>(T, PhantomData<&'n ()>);
impl<'n, T: 'n> Node<'n, ()> for ValueNode<'n, T> {
    type Output = &'n T;
    fn eval(&self, _input: &()) -> &T {
        &self.0
    }
}
impl<'n, T> ValueNode<'n, T> {
    pub const fn new(value: T) -> ValueNode<'n, T> {
        ValueNode(value, PhantomData)
    }
}

#[derive(Default)]
pub struct DefaultNode<T>(PhantomData<T>);
impl<'n, T: Default + 'n> Node<'n, ()> for DefaultNode<T> {
    type Output = T;
    fn eval(&self, _input: &()) -> T {
        T::default()
    }
}
impl<T> DefaultNode<T> {
    pub const fn new() -> DefaultNode<T> {
        DefaultNode(PhantomData)
    }
}

pub struct DefaultRefNode<'n, T>(ValueNode<'n, T>);
impl<'n, T: 'n> Node<'n, ()> for DefaultRefNode<'n, T> {
    type Output = &'n T;
    fn eval(&'n self, _input: &'n ()) -> &'n T {
        self.0.eval(&())
    }
}
impl<'n, T: Default> Default for DefaultRefNode<'n, T> {
    fn default() -> DefaultRefNode<'n, T> {
        DefaultRefNode(ValueNode::new(T::default()))
    }
}
