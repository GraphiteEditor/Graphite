pub mod node_registry;

#[cfg(test)]
mod tests {

	use std::marker::PhantomData;

	use graphene_core::value::ValueNode;
	use graphene_core::{structural::*, RefNode};

	use borrow_stack::BorrowStack;
	use dyn_any::{downcast, IntoDynAny};
	use graphene_std::any::{Any, DowncastNode, DynAnyNode, TypeErasedNode};
	use graphene_std::ops::AddNode;

	#[test]
	fn borrow_stack() {
		let stack = borrow_stack::FixedSizeStack::new(256);
		unsafe {
			let dynanynode: DynAnyNode<_, (), _, _> = DynAnyNode::new(ValueNode(2_u32));
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

		let result = unsafe { &stack.get()[1] }.eval_ref(4_u32.into_dyn());
		assert_eq!(*downcast::<(u32, &u32)>(result).unwrap(), (4_u32, &2_u32));
		let result = unsafe { &stack.get()[1] }.eval_ref(4_u32.into_dyn());
		let add = unsafe { &stack.get()[2] }.eval_ref(result);
		assert_eq!(*downcast::<u32>(add).unwrap(), 6_u32);
		let add = unsafe { &stack.get()[3] }.eval_ref(4_u32.into_dyn());
		assert_eq!(*downcast::<u32>(add).unwrap(), 6_u32);
	}

	#[test]
	fn craft_from_flattened() {
		use graphene_std::document::*;
		// This is input and evaluated
		let construction_network = NodeNetwork {
			inputs: vec![10],
			output: 1,
			nodes: [
				(
					1,
					DocumentNode {
						name: "Inc".into(),
						inputs: vec![],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode {
							name: "id".into(),
							input: ProtoNodeInput::Node(11),
							construction_args: ConstructionArgs::None,
						}),
					},
				),
				(
					10,
					DocumentNode {
						name: "cons".into(),
						inputs: vec![],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode {
							name: "cons".into(),
							input: ProtoNodeInput::Network,
							construction_args: ConstructionArgs::Nodes(vec![14]),
						}),
					},
				),
				(
					11,
					DocumentNode {
						name: "add".into(),
						inputs: vec![],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode {
							name: "add".into(),
							input: ProtoNodeInput::Node(10),
							construction_args: ConstructionArgs::None,
						}),
					},
				),
				(
					14,
					DocumentNode {
						name: "Value: 2".into(),
						inputs: vec![],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode {
							name: "value".into(),
							input: ProtoNodeInput::None,
							construction_args: ConstructionArgs::Value(2_u32.into_any()),
						}),
					},
				),
			]
			.into_iter()
			.collect(),
		};
	}
}
