use gpu_compiler_bin_wrapper::CompileRequest;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::*;

use graph_craft::*;
use std::borrow::Cow;

fn main() {
	let client = reqwest::blocking::Client::new();

	let network = NodeNetwork {
		inputs: vec![0u64.into()],
		outputs: vec![NodeOutput::new(0u64.into(), 0)],
		disabled: vec![],
		previous_outputs: None,
		nodes: [(
			0u64.into(),
			DocumentNode {
				name: "Inc Node".into(),
				inputs: vec![
					NodeInput::Network(concrete!(u32)),
					NodeInput::Value {
						tagged_value: TaggedValue::U32(1),
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

	let compile_request = CompileRequest::new(network, "u32".to_owned(), "u32".to_owned());
	let response = client.post("http://localhost:3000/compile/spriv").json(&compile_request).send().unwrap();
	println!("response: {:?}", response);
}

fn add_network() -> NodeNetwork {
	NodeNetwork {
		inputs: vec![0u64.into(), 0u64.into()],
		outputs: vec![NodeOutput::new(1u64.into(), 0)],
		disabled: vec![],
		previous_outputs: None,
		nodes: [
			(
				0u64.into(),
				DocumentNode {
					name: "Cons".into(),
					inputs: vec![NodeInput::Network(concrete!(u32)), NodeInput::Network(concrete!(u32))],
					metadata: DocumentNodeMetadata::default(),
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::structural::ConsNode")),
				},
			),
			(
				1u64.into(),
				DocumentNode {
					name: "Add".into(),
					inputs: vec![NodeInput::node(0u64.into(), 0)],
					metadata: DocumentNodeMetadata::default(),
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::AddNode")),
				},
			),
		]
		.into_iter()
		.collect(),
	}
}
