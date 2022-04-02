#![feature(generic_associated_types)]
use graphene_std::*;

/*fn mul(a: f32, b: f32) -> f32 {
    a * b
}*/

mod mul {
    use graphene_std::{
        value::DefaultNode, value::DefaultRefNode, ArgNode, DynamicInput, ExecPtr, Node,
    };
    use std::{any::Any, ops::Deref};
    const A: DefaultRefNode<f32> = DefaultRefNode::new();
    const B: DefaultRefNode<f32> = DefaultRefNode::new();
    type F32Node<'n> = &'n dyn ExecPtr<'n, f32, Output<'n> = &'n f32, Input<'n> = ()>;
    pub struct MulNode<'n> {
        a: F32Node<'n>,
        b: F32Node<'n>,
    }
    impl<'n> Node for MulNode<'n> {
        type Input<'i> = () where Self: 'i;
        type Output<'o> = f32 where Self: 'o;
        fn eval<'a, I>(&'a self, input: I) -> <Self as graphene_std::Node>::Output<'a>
        where
            I: std::borrow::Borrow<Self::Input<'a>>,
        {
            let a = self.a.fn_ptr();
            let b = self.b.fn_ptr();
            a * b
        }
    }
    impl<'n> MulNode<'n> {
        pub const fn new() -> Self {
            Self { a: &A, b: &B }
        }
    }
    impl DynamicInput for MulNode<'_> {
        fn set_kwarg_by_name(&mut self, _: &str, _: &(dyn std::any::Any + 'static)) {
            todo!()
        }
        fn set_arg_by_index(&mut self, index: usize, input: &(dyn std::any::Any + 'static)) {
            match index {
                0 => self.a = input.downcast_ref::<&dyn ExecPtr<'_, f32>>().unwrap(),
                _ => todo!(),
            }
        }
    }
}

fn main() {
    let mut mul = mul::MulNode::new();

    let int = value::IntNode::<32>;
    let _add: u32 = ops::AddNode::<u32>::default().eval((int.exec(), int.exec()));
    let fnode = generic::FnNode::new(|(a, b): &(i32, i32)| a - b);
    //let sub = fnode.any(&("a", 2));
    let cache = memo::CacheNode::new(&fnode);
    let cached_result = cache.eval(&(2, 3));

    println!("{}", cached_result)
}
