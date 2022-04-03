//#![feature(generic_associated_types)]
use dyn_any::StaticType;
use graphene_std::value::{AnyRefNode, ValueNode};
use graphene_std::*;

/*fn mul(#[dyn_any(default)] a: f32, b: f32) -> f32 {
    a * b
}*/

mod mul {
    use dyn_any::{downcast_ref, DynAny, StaticType};
    use graphene_std::{DynAnyNode, DynNode, DynamicInput, Node};
    pub struct MulNodeInput<'n> {
        pub a: &'n f32,
        pub b: &'n f32,
    }
    #[derive(Copy, Clone)]
    pub struct MulNodeAnyProxy<'n> {
        pub a: Option<DynAnyNode<'n>>,
        pub b: Option<DynAnyNode<'n>>,
    }
    #[derive(Copy, Clone)]
    pub struct MulNodeTypedProxy<'n> {
        pub a: Option<DynNode<'n, &'n f32>>,
        pub b: Option<DynNode<'n, &'n f32>>,
    }
    impl<'n> Node<'n, ()> for MulNodeAnyProxy<'n> {
        type Output = MulNodeInput<'n>;
        fn eval(&'n self, _input: &'n ()) -> <Self as graphene_std::Node<'n, ()>>::Output {
            let a = self.a.unwrap().eval(&());
            let a: &f32 = self
                .a
                .map(|v| downcast_ref(v.eval(&())).unwrap())
                .unwrap_or(&1.);
            /*let b: &f32 = self
                .b
                .map(|v| v.eval(&()).downcast_ref::<&'n f32, &'n f32>().unwrap())
                .unwrap_or(&&2.);
            a * b*/
            MulNodeInput { a, b: a }
        }
    }
    impl<'n> Node<'n, ()> for MulNodeTypedProxy<'n> {
        type Output = MulNodeInput<'n>;
        fn eval(&'n self, _input: &'n ()) -> <Self as graphene_std::Node<'n, ()>>::Output {
            let a = self.a.unwrap().eval(&());
            let b = self.b.unwrap().eval(&());
            MulNodeInput { a, b }
        }
    }

    /*macro_rules! new {
        () => {
            mul::MulNode { a: None, b: None }
        };
    }*/
    //pub(crate) use new;

    impl<'n> DynamicInput<'n> for MulNodeAnyProxy<'n> {
        fn set_kwarg_by_name(&mut self, name: &str, value: DynAnyNode<'n>) {
            todo!()
        }
        fn set_arg_by_index(&mut self, index: usize, value: DynAnyNode<'n>) {
            match index {
                0 => {
                    self.a = Some(value);
                }
                _ => todo!(),
            }
        }
    }
}

fn main() {
    //let mut mul = mul::MulNode::new();
    let f = (3.2f32, 3.1f32);
    let a = ValueNode::new(1.);
    let id = std::any::TypeId::of::<&f32>();
    let any_a = AnyRefNode::new(&a);
    /*let _mul2 = mul::MulNodeInput {
        a: None,
        b: Some(&any_a),
    };
    let mut mul2 = mul::new!();
    //let cached = memo::CacheNode::new(&mul1);
    //let foo = value::AnyRefNode::new(&cached);
    mul2.set_arg_by_index(0, &any_a);*/
    let int = value::IntNode::<32>;
    int.exec();
    println!("{}", int.exec());
    //let _add: u32 = ops::AddNode::<u32>::default().eval((int.exec(), int.exec()));
    //let fnode = generic::FnNode::new(|(a, b): &(i32, i32)| a - b);
    //let sub = fnode.any(&("a", 2));
    //let cache = memo::CacheNode::new(&fnode);
    //let cached_result = cache.eval(&(2, 3));

    //println!("{}", cached_result)
}
