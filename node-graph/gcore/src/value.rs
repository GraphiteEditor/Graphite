use std::{any::Any, marker::PhantomData};

use crate::{Exec, Node};

pub struct IntNode<const N: u32>;
impl<'n, const N: u32> Exec<'n> for IntNode<N> {
    type Output = u32;
    fn exec(&self) -> u32 {
        N
    }
}

#[derive(Default)]
pub struct ValueNode<'n, T>(T, PhantomData<&'n ()>);
impl<'n, T: 'n> Exec<'n> for ValueNode<'n, T> {
    type Output = &'n T;
    fn exec(&'n self) -> &'n T {
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
impl<'n, T: Default + 'n> Exec<'n> for DefaultNode<T> {
    type Output = T;
    fn exec(&self) -> T {
        T::default()
    }
}
impl<T> DefaultNode<T> {
    pub const fn new() -> DefaultNode<T> {
        DefaultNode(PhantomData)
    }
}

pub struct AnyRefNode<'n, N: Node<'n, I, Output = &'n O>, I, O>(
    &'n N,
    PhantomData<&'n I>,
    PhantomData<&'n O>,
);
impl<'n, N: Node<'n, I, Output = &'n O>, I, O: 'static> Node<'n, I> for AnyRefNode<'n, N, I, O> {
    type Output = &'n (dyn Any + 'static);
    fn eval(&'n self, input: &'n I) -> Self::Output {
        let value: &O = self.0.eval(input);
        value
    }
}
impl<'n, N: Node<'n, I, Output = &'n O>, I, O: 'static> AnyRefNode<'n, N, I, O> {
    pub fn new(n: &'n N) -> AnyRefNode<'n, N, I, O> {
        AnyRefNode(n, PhantomData, PhantomData)
    }
}

pub struct DefaultRefNode<'n, T>(ValueNode<'n, T>);
impl<'n, T: 'n> Exec<'n> for DefaultRefNode<'n, T> {
    type Output = &'n T;
    fn exec(&'n self) -> &'n T {
        self.0.exec()
    }
}
impl<'n, T: Default> Default for DefaultRefNode<'n, T> {
    fn default() -> DefaultRefNode<'n, T> {
        DefaultRefNode(ValueNode::new(T::default()))
    }
}
