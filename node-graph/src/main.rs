#![feature(generic_associated_types)]
//#![deny(rust_2018_idioms)]
use std::{any::Any, borrow::Borrow};

mod iter;
pub mod nodes;
use iter::insert_after_nth;
use nodes::*;

pub trait Node {
    type Out<'a>
    where
        Self: 'a;
    type Input<'a>
    where
        Self: 'a;
    fn eval<'a, T: Borrow<Self::Input<'a>>>(&'a self, input: T) -> Self::Out<'a>;
}
pub trait AnyRef: Node {
    fn any<'a>(&'a self, input: &'a dyn Any) -> Self::Out<'a>
    where
        Self::Input<'a>: 'static + Copy;
}
impl<T: Node> AnyRef for T {
    fn any<'a>(&'a self, input: &'a dyn Any) -> Self::Out<'a>
    where
        Self::Input<'a>: 'static + Copy,
    {
        self.eval::<&Self::Input<'a>>(input.downcast_ref::<Self::Input<'a>>().unwrap())
    }
}

/*
trait After<SECOND: Node> {
    type Out<'a>
    where
        Self: 'a;
    fn after<'a><FIRST: Node>(
        &'a self,
        first: &'a FIRST,
    ) -> ComposeNode<'a, FIRST, SECOND, INTERMEDIATE>;
}*/

fn main() {
    let int = IntNode::<32>;
    let add: u32 = AddNode::<u32>::default().any(&(int.eval(&()), int.eval(&())) as &dyn Any);
    /*
    let curry: CurryNthArgNode<'_, _, _, u32, u32, 0> = CurryNthArgNode::new(&AddNode, &int);
    let composition = curry.after(&curry);
    let n = ValueNode::new(10_u32);
    let curry: CurryNthArgNode<'_, _, _, u32, _, 0> = CurryNthArgNode::new(&composition, &n);
    */
    println!("{}", add)
}
