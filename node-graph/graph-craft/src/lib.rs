#[macro_use]
extern crate log;

pub mod node_registry;

pub mod document;
pub mod proto;

#[cfg(test)]
mod tests {

	use std::marker::PhantomData;

	use graphene_core::value::ValueNode;
	use graphene_core::{structural::*, RefNode};

	use borrow_stack::BorrowStack;
	use borrow_stack::FixedSizeStack;
	use dyn_any::{downcast, IntoDynAny};
	use graphene_std::any::{Any, DowncastNode, DynAnyNode, TypeErasedNode};
	use graphene_std::ops::AddNode;

	#[test]
	fn borrow_stack() {
		let stack = borrow_stack::FixedSizeStack::new(256);
		unsafe {
			let dynanynode: DynAnyNode<ValueNode<u32>, (), _, _> = DynAnyNode::new(ValueNode(2_u32));
			stack.push(dynanynode.into_box());
		}
		stack.push_fn(|nodes| {
			let pre_node = nodes.get(0).unwrap();
			let downcast: DowncastNode<&TypeErasedNode, &u32> = DowncastNode::new(pre_node);
			let dynanynode: DynAnyNode<ConsNode<_, Any<'_>>, u32, _, _> = DynAnyNode::new(ConsNode(downcast, PhantomData));
			dynanynode.into_box()
		});
		stack.push_fn(|_| {
			let dynanynode: DynAnyNode<_, (u32, &u32), _, _> = DynAnyNode::new(AddNode);
			dynanynode.into_box()
		});
		stack.push_fn(|nodes| {
			let compose_node = nodes[1].after(&nodes[2]);
			TypeErasedNode(Box::new(compose_node))
		});

		let result = unsafe { &stack.get()[0] }.eval_ref(().into_dyn());
		assert_eq!(*downcast::<&u32>(result).unwrap(), &2_u32);
		let result = unsafe { &stack.get()[1] }.eval_ref(4_u32.into_dyn());
		assert_eq!(*downcast::<(u32, &u32)>(result).unwrap(), (4_u32, &2_u32));
		let result = unsafe { &stack.get()[1] }.eval_ref(4_u32.into_dyn());
		let add = unsafe { &stack.get()[2] }.eval_ref(result);
		assert_eq!(*downcast::<u32>(add).unwrap(), 6_u32);
		let add = unsafe { &stack.get()[3] }.eval_ref(4_u32.into_dyn());
		assert_eq!(*downcast::<u32>(add).unwrap(), 6_u32);
	}

	#[test]
	fn execute_add() {
		use crate::document::*;
		use crate::node_registry::push_node;
		use crate::proto::*;
		use graphene_core::Node;

		fn add_network() -> NodeNetwork {
			NodeNetwork {
				inputs: vec![0, 0],
				output: 1,
				nodes: [
					(
						0,
						DocumentNode {
							name: "Cons".into(),
							inputs: vec![NodeInput::Network, NodeInput::Network],
							implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new(
								"graphene_core::structural::ConsNode",
								&[Type::Concrete(std::borrow::Cow::Borrowed("u32")), Type::Concrete(std::borrow::Cow::Borrowed("u32"))],
							)),
							metadata: DocumentNodeMetadata::default(),
						},
					),
					(
						1,
						DocumentNode {
							name: "Add".into(),
							inputs: vec![NodeInput::Node(0)],
							implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new(
								"graphene_core::ops::AddNode",
								&[Type::Concrete(std::borrow::Cow::Borrowed("u32")), Type::Concrete(std::borrow::Cow::Borrowed("u32"))],
							)),
							metadata: DocumentNodeMetadata::default(),
						},
					),
				]
				.into_iter()
				.collect(),
			}
		}

		let mut network = NodeNetwork {
			inputs: vec![0],
			output: 0,
			nodes: [(
				0,
				DocumentNode {
					name: "Inc".into(),
					inputs: vec![
						NodeInput::Network,
						NodeInput::Value {
							tagged_value: value::TaggedValue::U32(1),
							exposed: false,
						},
					],
					implementation: DocumentNodeImplementation::Network(add_network()),
					metadata: DocumentNodeMetadata::default(),
				},
			)]
			.into_iter()
			.collect(),
		};

		let stack = FixedSizeStack::new(256);
		println!("flattening");
		network.flatten(0);
		//println!("flat_network: {:#?}", network);
		let mut proto_network = network.into_proto_network();
		proto_network.reorder_ids();
		//println!("reordered_ides: {:#?}", proto_network);
		for (_id, node) in proto_network.nodes {
			push_node(node, &stack);
		}

		let result = unsafe { stack.get().last().unwrap().eval(32_u32.into_dyn()) };
		let val = *dyn_any::downcast::<u32>(result).unwrap();
		assert_eq!(val, 33_u32);
	}
}
