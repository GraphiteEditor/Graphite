#![feature(generic_associated_types)]
//#![deny(rust_2018_idioms)]
use std::{any::Any, borrow::Borrow};

mod iter;
pub mod nodes;
use iter::insert_after_nth;
use nodes::*;

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

pub trait DefaultNode: Default {
    fn default_node() -> ValueNode<Self> {
        ValueNode::new(Self::default())
    }
}
impl<T: std::default::Default> DefaultNode for T {}

pub trait After: Sized {
    fn after<'a, First: Node>(&'a self, first: &'a First) -> ComposeNode<'a, First, Self> {
        ComposeNode::new(first, self)
    }
}
impl<Second: Node> After for Second {}

pub trait DynamicInput {
    fn set_kwarg_by_name(&mut self, name: &str, value: &dyn Any);
    fn set_arg_by_index(&mut self, index: usize, value: &dyn Any);
}

fn main() {
    let int = IntNode::<32>;
    let add: u32 = AddNode::<u32>::default().eval((int.eval(&()), int.eval(&())));
    let fnode = FnNode::new(|(a, b): &(i32, i32)| a - b);
    //let sub = fnode.any(&("a", 2));
    let cache = CacheNode::new(&fnode);
    let foo = cache.eval(&(2, 3));

    /*
    let curry: CurryNthArgNode<'_, _, _, u32, u32, 0> = CurryNthArgNode::new(&AddNode, &int);
    let composition = curry.after(&curry);
    let n = ValueNode::new(10_u32);
    let curry: CurryNthArgNode<'_, _, _, u32, _, 0> = CurryNthArgNode::new(&composition, &n);
    */
    println!("{}", foo)
}
