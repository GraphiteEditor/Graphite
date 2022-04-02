#![feature(generic_associated_types)]
use std::{any::Any, borrow::Borrow};

#[rustfmt::skip]
pub trait Node {
    // Self: 'a means that Self has to live at least as long as 'a (the input and output)
    // this ensures that the node does not spontaneously disappear during evaluation
    type Output<'a> where Self: 'a;
    type Input<'a> where Self: 'a;

    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a>;
}

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
