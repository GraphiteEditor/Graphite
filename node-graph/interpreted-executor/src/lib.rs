pub mod dynamic_executor;
pub mod node_registry;

#[cfg(test)]
mod tests {
	use dyn_any::IntoDynAny;
	use graph_craft::document::value::TaggedValue;
	use graphene_core::*;
	use std::borrow::Cow;

	use futures::executor::block_on;

	#[test]
	fn execute_add() {
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

		use crate::dynamic_executor::DynamicExecutor;
		use graph_craft::graphene_compiler::{Compiler, Executor};

		let compiler = Compiler {};
		let protograph = compiler.compile_single(network, true).expect("Graph should be generated");

		let exec = block_on(DynamicExecutor::new(protograph)).unwrap_or_else(|e| panic!("Failed to create executor: {}", e));

		let result = block_on((&exec).execute(32_u32)).unwrap();
		assert_eq!(result, TaggedValue::U32(33));
	}

	#[test]
	fn double_number() {
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
						inputs: vec![NodeInput::ShortCircut(concrete!(u32))],
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

		use crate::dynamic_executor::DynamicExecutor;
		use graph_craft::graphene_compiler::Compiler;

		let compiler = Compiler {};
		let protograph = compiler.compile_single(network, true).expect("Graph should be generated");

		let _exec = block_on(DynamicExecutor::new(protograph)).map(|e| panic!("The network should not type check ")).unwrap_err();
	}
}
