#[macro_use]
extern crate log;

pub mod executor;
pub mod node_registry;

#[cfg(test)]
mod tests {
	use dyn_any::IntoDynAny;
	use graphene_core::*;
	use std::borrow::Cow;

	/*
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
			TypeErasedNode(Box::pin(compose_node))
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
	}*/

	#[tokio::test]
	async fn execute_add() {
		use graph_craft::document::*;

		use graph_craft::*;

		fn add_network() -> NodeNetwork {
			NodeNetwork {
				inputs: vec![0, 0],
				outputs: vec![NodeOutput::new(1, 0)],
				nodes: [
					(
						0,
						DocumentNode {
							name: "Cons".into(),
							inputs: vec![NodeInput::Network(concrete!(u32)), NodeInput::Network(concrete!(&u32))],
							implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::structural::ConsNode<_, _>")),
							..Default::default()
						},
					),
					(
						1,
						DocumentNode {
							name: "Add".into(),
							inputs: vec![NodeInput::node(0, 0)],
							implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::AddNode")),
							..Default::default()
						},
					),
				]
				.into_iter()
				.collect(),
				..Default::default()
			}
		}

		let network = NodeNetwork {
			inputs: vec![0],
			outputs: vec![NodeOutput::new(0, 0)],
			nodes: [(
				0,
				DocumentNode {
					name: "Inc".into(),
					inputs: vec![
						NodeInput::Network(concrete!(u32)),
						NodeInput::Value {
							tagged_value: graph_craft::document::value::TaggedValue::U32(1u32),
							exposed: false,
						},
					],
					implementation: DocumentNodeImplementation::Network(add_network()),
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		};

		use crate::executor::DynamicExecutor;
		use graph_craft::executor::{Compiler, Executor};

		let compiler = Compiler {};
		let protograph = compiler.compile_single(network, true).expect("Graph should be generated");

		let exec = DynamicExecutor::new(protograph).await.unwrap_or_else(|e| panic!("Failed to create executor: {}", e));

		let result = exec.execute(32_u32.into_dyn()).await.unwrap();
		let val = *dyn_any::downcast::<u32>(result).unwrap();
		assert_eq!(val, 33_u32);
	}

	#[tokio::test]
	async fn double_number() {
		use graph_craft::document::*;

		use graph_craft::*;

		let network = NodeNetwork {
			inputs: vec![0],
			outputs: vec![NodeOutput::new(1, 0)],
			nodes: [
				// Simple identity node taking a number as input from ouside the graph
				(
					0,
					DocumentNode {
						name: "id".into(),
						inputs: vec![NodeInput::Network(concrete!(u32))],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode")),
						..Default::default()
					},
				),
				// An add node adding the result of the id node to its self
				(
					1,
					DocumentNode {
						name: "Add".into(),
						inputs: vec![NodeInput::node(0, 0), NodeInput::node(0, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::AddParameterNode<_>")),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};

		use crate::executor::DynamicExecutor;
		use graph_craft::executor::Compiler;

		let compiler = Compiler {};
		let protograph = compiler.compile_single(network, true).expect("Graph should be generated");

		let _exec = DynamicExecutor::new(protograph).await.map(|e| panic!("The network should not type check: {:#?}", e)).unwrap_err();
	}
}
