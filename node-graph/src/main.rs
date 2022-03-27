#![deny(rust_2018_idioms)]
use std::any::Any;

mod iter;
mod nodes;
use iter::insert_after_nth;
use nodes::*;

pub trait Node<'n, OUT> {
    fn eval(&'n self, input: impl Iterator<Item = &'n dyn Any> + Clone) -> OUT;
    // fn source code
    // positon
}
trait After<'n, OUT, SECOND: Node<'n, OUT>> {
    fn after<INTERMEDIATE, FIRST: Node<'n, INTERMEDIATE>>(
        &'n self,
        first: &'n FIRST,
    ) -> ComposeNode<'n, FIRST, SECOND, INTERMEDIATE>;
}

fn main() {
    use std::iter;
    let int = IntNode::<32>;
    let curry: CurryNthArgNode<'_, _, _, u32, u32, 0> = CurryNthArgNode::new(&AddNode, &int);
    let composition = curry.after(&curry);
    let n = ValueNode::new(10_u32);
    let curry: CurryNthArgNode<'_, _, _, u32, _, 0> = CurryNthArgNode::new(&composition, &n);
    println!("{}", curry.eval(iter::empty()))
}
