//#![feature(generic_associated_types)]
use graphene_std::value::{AnyRefNode, ValueNode};
use graphene_std::*;

/*fn mul(a: f32, b: f32) -> f32 {
    a * b
}*/

mod mul {
    use graphene_std::{DynamicInput, Node};
    use std::any::Any;
    type F32Node<'n> = &'n (dyn Node<'n, (), Output = &'n (dyn Any + 'static)> + 'n);
    #[derive(Copy, Clone)]
    pub struct MulNode<'n> {
        pub a: Option<F32Node<'n>>,
        pub b: Option<F32Node<'n>>,
    }
    impl<'n> Node<'n, ()> for MulNode<'n> {
        type Output = f32;
        fn eval(&'n self, _input: &'n ()) -> <Self as graphene_std::Node<'n, ()>>::Output {
            let a: &f32 = self
                .a
                .map(|v| v.eval(&()).downcast_ref().unwrap())
                .unwrap_or(&2.);
            let b: &f32 = self
                .b
                .map(|v| v.eval(&()).downcast_ref().unwrap())
                .unwrap_or(&1.);
            a * b
        }
    }
    macro_rules! new {
        () => {
            mul::MulNode { a: None, b: None }
        };
    }
    pub(crate) use new;

    impl<'i: 'f, 'f> DynamicInput<'f> for MulNode<'f> {
        fn set_kwarg_by_name(
            &mut self,
            name: &str,
            value: &'f dyn Node<'f, (), Output = &'f (dyn Any + 'static)>,
        ) {
            todo!()
        }
        fn set_arg_by_index(
            &mut self,
            index: usize,
            value: &'f dyn Node<'f, (), Output = &'f (dyn Any + 'static)>,
        ) {
            match index {
                0 => self.a = Some(value),
                _ => todo!(),
            }
        }
    }
}

fn main() {
    //let mut mul = mul::MulNode::new();
    let a = ValueNode::new(3.4f32);
    let any_a = AnyRefNode::new(&a);
    let _mul2 = mul::MulNode {
        a: None,
        b: Some(&any_a),
    };
    let mut mul2 = mul::new!();
    //let cached = memo::CacheNode::new(&mul1);
    //let foo = value::AnyRefNode::new(&cached);
    mul2.set_arg_by_index(0, &any_a);

    let int = value::IntNode::<32>;
    int.eval(&());
    println!("{}", mul2.eval(&()));
    //let _add: u32 = ops::AddNode::<u32>::default().eval((int.exec(), int.exec()));
    //let fnode = generic::FnNode::new(|(a, b): &(i32, i32)| a - b);
    //let sub = fnode.any(&("a", 2));
    //let cache = memo::CacheNode::new(&fnode);
    //let cached_result = cache.eval(&(2, 3));

    //println!("{}", cached_result)
}
