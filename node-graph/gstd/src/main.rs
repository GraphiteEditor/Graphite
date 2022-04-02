use graphene_core::{Exec, Node};
use graphene_std::*;

fn main() {
    let int = value::IntNode::<32>;
    let _add: u32 = ops::AddNode::<u32>::default().eval((int.exec(), int.exec()));
    let fnode = generic::FnNode::new(|(a, b): &(i32, i32)| a - b);
    //let sub = fnode.any(&("a", 2));
    let cache = memo::CacheNode::new(&fnode);
    let cached_result = cache.eval(&(2, 3));

    println!("{}", cached_result)
}
