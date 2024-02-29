pub mod dynamic_executor;
pub mod node_registry;

#[cfg(test)]
mod tests {
	use graph_craft::document::value::TaggedValue;
	use graphene_core::*;

	use futures::executor::block_on;

	#[test]
	fn execute_add() {
		use graph_craft::document::*;

		use graph_craft::*;

		fn add_network() -> NodeNetwork {
			NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: [
					(
						NodeId(0),
						DocumentNode {
							name: "Cons".into(),
							inputs: vec![NodeInput::Network(concrete!(u32)), NodeInput::Network(concrete!(&u32))],
							implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::structural::ConsNode<_, _>")),
							..Default::default()
						},
					),
					(
						NodeId(1),
						DocumentNode {
							name: "Add".into(),
							inputs: vec![NodeInput::node(NodeId(0), 0)],
							implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::AddPairNode")),
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
			inputs: vec![NodeId(0)],
			outputs: vec![NodeOutput::new(NodeId(0), 0)],
			nodes: [(
				NodeId(0),
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
		let protograph = compiler.compile_single(network).expect("Graph should be generated");

		let exec = block_on(DynamicExecutor::new(protograph)).unwrap_or_else(|e| panic!("Failed to create executor: {e:?}"));

		let result = block_on((&exec).execute(32_u32)).unwrap();
		assert_eq!(result, TaggedValue::U32(33));
	}

	#[test]
	fn double_number() {
		use graph_craft::document::*;

		use graph_craft::*;

		let network = NodeNetwork {
			inputs: vec![NodeId(0)],
			outputs: vec![NodeOutput::new(NodeId(1), 0)],
			nodes: [
				// Simple identity node taking a number as input from outside the graph
				(
					NodeId(0),
					DocumentNode {
						name: "id".into(),
						inputs: vec![NodeInput::Network(concrete!(u32))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
						..Default::default()
					},
				),
				// An add node adding the result of the id node to its self
				(
					NodeId(1),
					DocumentNode {
						name: "Add".into(),
						inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::AddNode<_>")),
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
		let protograph = compiler.compile_single(network).expect("Graph should be generated");

		let _exec = block_on(DynamicExecutor::new(protograph)).map(|_e| panic!("The network should not type check ")).unwrap_err();
	}
}
