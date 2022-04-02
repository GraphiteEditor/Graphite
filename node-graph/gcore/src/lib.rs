#![feature(generic_associated_types)]
pub mod generic;
pub mod ops;
pub mod structural;
pub mod value;

use std::{any::Any, borrow::Borrow, ops::Deref};

#[rustfmt::skip]
pub trait Node {
    // Self: 'a means that Self has to live at least as long as 'a (the input and output)
    // this ensures that the node does not spontaneously disappear during evaluation
    type Input<'i> where Self: 'i;
    type Output<'o> where Self: 'o;

    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a>;
}

pub trait SimpleNode<'n, I, O> {
    fn eval_simple(&self, input: &I) -> &O;
}

impl<T: for<'n> SimpleNode<'n, I, O>, I, O> Node for T {
    type Input<'i> = &'i I where Self: 'i;
    type Output<'o> = &'o O where Self: 'o;

    fn eval<'a, In: Borrow<Self::Input<'a>>>(&'a self, input: In) -> Self::Output<'a> {
        self.eval_simple(input.borrow())
    }
}

#[rustfmt::skip]
pub trait OutputNode<'a, T>: Node<Output<'a> = T> where Self: 'a {}
#[rustfmt::skip]
pub trait ArgNode<'a, T>: OutputNode<'a, T> + Node<Input<'a> = ()> where Self: 'a {}

pub trait AnyRef: Node {
    fn any<'a>(&'a self, input: &'a dyn Any) -> Self::Output<'a>
    where
        Self::Input<'a>: 'static + Copy;
}

impl<T: Node> AnyRef for T {
    fn any<'a>(&'a self, input: &'a dyn Any) -> Self::Output<'a>
    where
        Self::Input<'a>: 'static + Copy,
    {
        self.eval::<&Self::Input<'a>>(input.downcast_ref::<Self::Input<'a>>().unwrap_or_else(
            || {
                panic!(
                    "Node was evaluated with wrong input. The input has to be of type: {}",
                    std::any::type_name::<Self::Input<'a>>(),
                )
            },
        ))
    }
}

trait Ref<T>: Node {}
impl<'a, T: 'a, N: Node<Output<'a> = &'a T> + 'a> Ref<T> for N {}

pub trait ExecPtr<'n, T>: Node {
    fn fn_ptr(&self) -> &T;
}

impl<'n, T: 'n, N: Ref<T>> ExecPtr<'n, T> for N
where
    for<'a> &'a (): Borrow<<Self as Node>::Input<'a>>,
    for<'a> &'a T: From<N::Output<'a>>,
{
    fn fn_ptr(&self) -> &T {
        let value: &T = self.eval(&()).into();
        value
    }
}

pub trait Exec: Node
where
    for<'a> &'a (): Borrow<<Self as Node>::Input<'a>>,
{
    fn exec(&self) -> Self::Output<'_> {
        self.eval(&())
    }
}
impl<T: Node> Exec for T where for<'a> &'a (): Borrow<<T as Node>::Input<'a>> {}

pub trait DynamicInput {
    fn set_kwarg_by_name(&mut self, name: &str, value: &dyn Any);
    fn set_arg_by_index(&mut self, index: usize, value: &dyn Any);
}
