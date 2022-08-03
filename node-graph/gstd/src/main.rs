use borrow_stack::BorrowStack;
//#![feature(generic_associated_types)]
use dyn_any::{DynAny, StaticType};
use graphene_std::value::{AnyRefNode, AnyValueNode, StorageNode, ValueNode};
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
    impl<'n> Node<'n> for MulNodeAnyProxy<'n> {
        type Output = MulNodeInput<'n>;
        fn eval(&'n self) -> <Self as graphene_std::Node<'n>>::Output {
            let a = self.a.unwrap().eval();
            let a: &f32 = self
                .a
                .map(|v| downcast_ref(v.eval()).unwrap())
                .unwrap_or(&1.);
            /*let b: &f32 = self
                .b
                .map(|v| v.eval(&()).downcast_ref::<&'n f32, &'n f32>().unwrap())
                .unwrap_or(&&2.);
            a * b*/
            MulNodeInput { a, b: a }
        }
    }
    impl<'n> Node<'n> for MulNodeTypedProxy<'n> {
        type Output = MulNodeInput<'n>;
        fn eval(&'n self) -> <Self as graphene_std::Node<'n>>::Output {
            let a = self.a.unwrap().eval();
            let b = self.b.unwrap().eval();
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
type SNode<'n> = dyn Node<'n, Output = &'n dyn DynAny<'n>>;

struct NodeStore<'n>(borrow_stack::FixedSizeStack<'n, Box<SNode<'n>>>);

impl<'n> NodeStore<'n> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn push(&'n mut self, f: fn(&'n [Box<SNode>]) -> Box<SNode<'n>>) {
        unsafe { self.0.push(f(self.0.get())) };
    }

    /*fn get_index(&'n self, index: usize) -> &'n SNode<'n> {
        assert!(index < self.0.len());
        &unsafe { self.0.get()[index] }
    }*/
}

fn main() {
    use graphene_std::*;
    use quote::quote;
    use syn::parse::Parse;
    let nodes = vec![
        NodeKind::Input,
        NodeKind::Value(syn::parse_quote!(1u32)),
        NodeKind::Node(syn::parse_quote!(graphene_core::ops::AddNode), vec![0, 0]),
    ];

    //println!("{}", node_graph(1));

    let nodegraph = NodeGraph {
        nodes,
        input: syn::Type::Verbatim(quote! {u32}),
        output: syn::Type::Verbatim(quote! {u32}),
    };

    //let pretty = pretty_token_stream::Pretty::new(nodegraph.serialize_gpu("add"));
    //pretty.print();
    /*
        use dyn_any::{downcast_ref, DynAny, StaticType};
        //let mut mul = mul::MulNode::new();
        let mut stack: borrow_stack::FixedSizeStack<Box<dyn Node<'_, Output = &dyn DynAny>>> =
            borrow_stack::FixedSizeStack::new(42);
        unsafe { stack.push(Box::new(AnyValueNode::new(1f32))) };
        //let node = unsafe { stack.get(0) };
        //let boxed = Box::new(StorageNode::new(node));
        //unsafe { stack.push(boxed) };
        let result = unsafe { &stack.get()[0] }.eval();
        dbg!(downcast_ref::<f32>(result));
        /*unsafe {
            stack
                .push(Box::new(AnyRefNode::new(stack.get(0).as_ref()))
                    as Box<dyn Node<(), Output = &dyn DynAny>>)
        };*/
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
        Node::eval(&int);
        println!("{}", Node::eval(&int));
        //let _add: u32 = ops::AddNode::<u32>::default().eval((int.exec(), int.exec()));
        //let fnode = generic::FnNode::new(|(a, b): &(i32, i32)| a - b);
        //let sub = fnode.any(&("a", 2));
        //let cache = memo::CacheNode::new(&fnode);
        //let cached_result = cache.eval(&(2, 3));
    */
    //println!("{}", cached_result)
}
