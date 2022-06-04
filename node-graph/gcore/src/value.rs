use core::marker::PhantomData;

use crate::Node;

pub struct IntNode<const N: u32>;
impl<'n, const N: u32> Node<'n> for IntNode<N> {
    type Output = u32;
    fn eval(&self) -> u32 {
        N
    }
}

#[derive(Default)]
pub struct ValueNode<T>(pub T);
impl<'n, T: 'n> Node<'n> for ValueNode<T> {
    type Output = &'n T;
    fn eval(&'n self) -> Self::Output {
        &self.0
    }
}

impl<'n, T> ValueNode<T> {
    pub const fn new(value: T) -> ValueNode<T> {
        ValueNode(value)
    }
}

#[derive(Default)]
pub struct DefaultNode<T>(PhantomData<T>);
impl<'n, T: Default + 'n> Node<'n> for DefaultNode<T> {
    type Output = T;
    fn eval(&self) -> T {
        T::default()
    }
}
impl<T> DefaultNode<T> {
    pub const fn new() -> DefaultNode<T> {
        DefaultNode(PhantomData)
    }
}
