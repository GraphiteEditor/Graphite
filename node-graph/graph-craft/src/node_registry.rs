use std::marker::PhantomData;

use borrow_stack::FixedSizeStack;
use graphene_core::ops::{AddNode, IdNode};
use graphene_core::structural::{ConsNode, Then};
use graphene_core::{AsRefNode, Node};
use graphene_std::{
	any::{Any, DowncastNode, DynAnyNode, IntoTypeErasedNode, TypeErasedNode},
	document::{ConstructionArgs, ProtoNode, ProtoNodeInput},
};

struct NodeIdentifier {
	name: &'static str,
	types: &'static [&'static str],
}

const fn annotate<'n, 's: 'n, F>(f: F) -> F
where
	F: Fn(ProtoNode, FixedSizeStack<TypeErasedNode<'n>>),
{
	f
}

use borrow_stack::BorrowStack;
unsafe fn foo<'n>(proto_node: ProtoNode, stack: &'n FixedSizeStack<TypeErasedNode<'n>>) {
	let node_id = proto_node.input.unwrap_node() as usize;
	let nodes = stack.get();
	let pre_node = nodes.get(node_id).unwrap();
	let downcast: DowncastNode<_, &u32> = DowncastNode::new(pre_node);
	let dynanynode: DynAnyNode<ConsNode<_, Any<'_>>, u32, _, _> = DynAnyNode::new(ConsNode(downcast, PhantomData));
	stack.push(dynanynode.into_box());
}
fn borrow_stack() {
	let stack = borrow_stack::FixedSizeStack::new(256);
	unsafe {
		{
			let proto_node = ProtoNode::id();
			foo(proto_node, &stack);
			let proto_node = ProtoNode::id();
			let stack = &stack;
			let node_id = proto_node.input.unwrap_node() as usize;
			let nodes = stack.get();
			let pre_node = nodes.get(node_id).unwrap();
			let downcast: DowncastNode<&TypeErasedNode, &u32> = DowncastNode::new(pre_node);
			let dynanynode: DynAnyNode<ConsNode<_, Any<'_>>, u32, _, _> = DynAnyNode::new(ConsNode(downcast, PhantomData));
			stack.push(dynanynode.into_box());
		}
	};
}

static NODE_REGISTRY: &[(NodeIdentifier, fn(ProtoNode, &FixedSizeStack<TypeErasedNode<'static>>))] = &[
	(
		NodeIdentifier {
			name: "graphene_core::ops::IdNode",
			types: &["Any<'n>"],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();
				let node = pre_node.then(graphene_core::ops::IdNode);
				node.into_type_erased()
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::ops::AddNode",
			types: &["u32", "u32"],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();
				let node: DynAnyNode<AddNode, (u32, u32), _, _> = DynAnyNode::new(graphene_core::ops::AddNode);
				let node = (pre_node).then(node);

				node.into_type_erased()
			})
		},
	),
	/*(
		NodeIdentifier {
			name: "graphene_core::structural::ConsNode",
			types: &["&TypeErasedNode", "&u32", "u32"],
		},
		|proto_node, stack| {
			let node_id = proto_node.input.unwrap_node() as usize;
			stack.push_fn(move |nodes| {
				let pre_node = nodes.get(node_id).unwrap();
				let downcast: DowncastNode<_, &u32> = DowncastNode::new(pre_node);
				let dynanynode: DynAnyNode<ConsNode<_, Any<'_>>, u32, _, _> = DynAnyNode::new(ConsNode(downcast, PhantomData));
				dynanynode.into_box()
			})
		},
	),*/
	(
		NodeIdentifier {
			name: "graphene_core::any::DowncastNode",
			types: &["&TypeErasedNode", "&u32"],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();
				let node = pre_node.then(graphene_core::ops::IdNode);
				node.into_type_erased()
			})
		},
	),
];

#[cfg(test)]
mod test {
	use super::*;

	/*#[test]
	fn test() {
		let nodes = [TypeErasedNode(Box::new(42u32))];
		let node = NODE_REGISTRY[0].1(node, &nodes);
		assert_eq!(node.eval(()), 42);
	}*/
}
