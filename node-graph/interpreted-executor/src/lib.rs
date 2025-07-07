pub mod dynamic_executor;
pub mod node_registry;
pub mod util;

#[cfg(test)]
mod tests {
	use futures::executor::block_on;
	use graphene_core::*;

	#[test]
	fn double_number() {
		use graph_craft::document::*;
		use graph_craft::*;

		let network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0)],
			nodes: [
				// Simple identity node taking a number as input from outside the graph
				(
					NodeId(0),
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(u32), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ops::identity::IDENTIFIER),
						..Default::default()
					},
				),
				// An add node adding the result of the id node to its self
				(
					NodeId(1),
					DocumentNode {
						inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::AddNode")),
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
