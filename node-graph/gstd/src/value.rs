use core::marker::PhantomData;
pub use graphene_core::value::*;
use graphene_core::Node;

use dyn_any::{DynAny, StaticType};
pub struct AnyRefNode<'n, N: Node<'n, I, Output = &'n O>, I, O>(
    &'n N,
    PhantomData<&'n I>,
    PhantomData<&'n O>,
);
impl<'n, N: Node<'n, I, Output = &'n O>, I, O: DynAny<'n>> Node<'n, I> for AnyRefNode<'n, N, I, O> {
    type Output = &'n (dyn DynAny<'n>);
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
