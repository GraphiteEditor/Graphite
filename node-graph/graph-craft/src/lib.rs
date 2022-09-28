#[cfg(test)]
mod tests {

	use graphene_core::structural::*;
	use graphene_core::value::ValueNode;

	use borrow_stack::BorrowStack;
	use graphene_std::any::{Any, DynAnyNode, DynAnyNodeTrait};
	use graphene_std::ops::AddNode;

	#[test]
	fn borrow_stack() {
		let stack = borrow_stack::FixedSizeStack::new(256);
		unsafe {
			let dynanynode: DynAnyNode<'_, _, ()> = DynAnyNode::new(&ValueNode(2_u32));
			let refn = Box::new(dynanynode) as Box<dyn DynAnyNodeTrait>;
			stack.push(refn);
		}
		unsafe {
			let dynanynode: DynAnyNode<'_, _, &u32> = DynAnyNode::new(&ConsNode(ValueNode(2_u32)));
			let refn = Box::new(dynanynode) as Box<dyn DynAnyNodeTrait>;
			stack.push(refn);
		}
		unsafe {
			let dynanynode: DynAnyNode<'_, _, (&u32, u32)> = DynAnyNode::new(&AddNode);
			let refn = Box::new(dynanynode) as Box<dyn DynAnyNodeTrait>;
			stack.push(refn);
		}

		let mut input = Box::new(()) as Any;
		for i in 0..3 {
			let value = unsafe { &stack.get()[i] };
			input = value.eval_ref_dispatch(input);
		}

		assert_eq!(*dyn_any::downcast::<u32>(input).unwrap(), 4)

		//assert_eq!(4, *dyn_any::downcast::<u32>(DynamicAddNode.eval((Box::new(2_u32) as Dynamic, Box::new(2_u32) as Dynamic))).unwrap());
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
