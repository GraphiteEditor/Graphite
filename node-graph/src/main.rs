use std::any::Any;

type Function = Box<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>;

struct Node {
    func: Function,
    code: String,
    return_type: String,
    args: String,
}

impl Node {
    fn eval<T: 'static, U: 'static>(&self, t: T) -> U {
        *(self.func)(Box::new(t)).downcast::<U>().unwrap()
    }
    #[allow(unused)]
    fn id(self) -> Self {
        self
    }
}

impl std::ops::Mul<Self> for Node {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        node_compose(self, other)
    }
}

pub fn compose<F: 'static, G: 'static, Fv, Gv, V>(g: G, f: F) -> Box<dyn Fn(Fv) -> V>
where
    F: Fn(Fv) -> Gv,
    G: Fn(Gv) -> V,
{
    Box::new(move |x| g(f(x)))
}

fn node_compose(g: Node, f: Node) -> Node {
    #[rustfmt::skip]
    let Node { func: ff, code: fc, args: fa, return_type: fr} = f;
    #[rustfmt::skip]
    let Node { func, code, args, return_type } = g;
    assert_eq!(args, fr);
    Node {
        func: Box::new(move |x| func(ff(x))),
        code: fc + code.as_str(), // temporary TODO: replace
        return_type,
        args: fa,
    }
}
#[graph_proc_macros::to_node]
fn id<T:'static>(t: T) -> T {
    t
}

#[graph_proc_macros::to_node]
fn gen_int() -> (u32, u32) {
    (42, 43)
}
#[graph_proc_macros::to_node]
fn format_int(x: u32, y: u32) -> String {
    x.to_string() + &y.to_string()
}

#[graph_proc_macros::to_node]
fn curry_first_u32(x: u32, node: Node) -> Node {
    assert_eq!(node.args[1..].split(",").next(), Some("u32"));
    curry_first_arg_node::<u32>().eval((x, node))
}

#[graph_proc_macros::to_node]
fn curry_first_arg<T: 'static + Clone>(x: T, node: Node) -> Node {
    node_after_fn_node().eval::<(Node, Function), Node>((
        node,
        Box::new(move |y: Box<dyn Any>| {
            Box::new((x.clone(), *y.downcast::<T>().unwrap())) as Box<dyn Any>
        }) ,
    ))
}

#[graph_proc_macros::to_node]
fn compose_node(g: Node, f: Node) -> Node {
    node_compose(g, f)
}

#[graph_proc_macros::to_node]
fn node_after_fn(g: Node, f: Box<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>) -> Node {
    let Node {
        func, return_type, ..
    } = g;
    Node {
        func: compose(func, f),
        code: "unimplemented".to_string(),
        return_type,
        args: "".to_string(),
    }
}

#[graph_proc_macros::to_node]
fn node_from_fn(f: Box<dyn Fn(Box<dyn Any>) -> Box<dyn Any>>) -> Node {
    node_after_fn_node().eval((id_node::<Box<dyn Any>>, f))
}

fn main() {
    println!("{:?}",(format_int_node() * gen_int_node()).eval::<_, String>(()));
    println!(
        "{:?}",
        curry_first_u32_node()
            .eval::<(u32, Node), Node>((3, format_int_node()))
            .eval::<u32, String>(43)
    );
    println!(
        "{:?}",
        curry_first_arg_node::<u32>()
            .eval::<(u32, Node), Node>((3, format_int_node()))
            .eval::<u32, String>(43)
    );
}
