use core::marker::PhantomData;
pub use graphene_core::value::*;
use graphene_core::Node;

use dyn_any::{DynAny, StaticType, StaticTypeSized};

pub struct AnyRefNode<'n, N: Node<'n, I, Output = O>, I, O>(
    &'n N,
    PhantomData<&'n I>,
    PhantomData<&'n O>,
);

impl<'n, N: Node<'n, I, Output = &'n O>, I, O: DynAny<'n>> Node<'n, I>
    for AnyRefNode<'n, N, I, &'n O>
{
    type Output = &'n (dyn DynAny<'n>);
    fn eval(&'n self, input: &'n I) -> Self::Output {
        let value: &O = self.0.eval(input);
        value
    }
}
impl<'n, N: Node<'n, I, Output = &'n O>, I, O: 'n + ?Sized> AnyRefNode<'n, N, I, &'n O> {
    pub fn new(n: &'n N) -> AnyRefNode<'n, N, I, &'n O> {
        AnyRefNode(n, PhantomData, PhantomData)
    }
}

pub struct StorageNode<'n>(&'n dyn Node<'n, (), Output = &'n dyn DynAny<'n>>);

impl<'n> Node<'n, ()> for StorageNode<'n> {
    type Output = &'n (dyn DynAny<'n>);
    fn eval(&'n self, input: &'n ()) -> Self::Output {
        let value = self.0.eval(input);
        value
    }
}
impl<'n> StorageNode<'n> {
    pub fn new<N: Node<'n, (), Output = &'n dyn DynAny<'n>>>(n: &'n N) -> StorageNode<'n> {
        StorageNode(n)
    }
}

#[derive(Default)]
pub struct AnyValueNode<'n, T>(T, PhantomData<&'n ()>);
impl<'n, T: 'n + DynAny<'n>> Node<'n, ()> for AnyValueNode<'n, T> {
    type Output = &'n dyn DynAny<'n>;
    fn eval(&'n self, _input: &()) -> &'n dyn DynAny<'n> {
        &self.0
    }
}

impl<'n, T> AnyValueNode<'n, T> {
    pub const fn new(value: T) -> AnyValueNode<'n, T> {
        AnyValueNode(value, PhantomData)
    }
}
