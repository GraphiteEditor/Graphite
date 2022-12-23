use gpu_compiler_bin_wrapper::CompileRequest;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::*;
use graph_craft::proto::*;
use graph_craft::{concrete, generic};

fn main() {
	let client = reqwest::blocking::Client::new();

	let network = NodeNetwork {
		inputs: vec![0],
		output: 0,
		nodes: [(
			0,
			DocumentNode {
				name: "Inc Node".into(),
				inputs: vec![
					NodeInput::Network,
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
		inputs: vec![0, 0],
		output: 1,
		nodes: [
			(
				0,
				DocumentNode {
					name: "Cons".into(),
					inputs: vec![NodeInput::Network, NodeInput::Network],
					metadata: DocumentNodeMetadata::default(),
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::structural::ConsNode", &[generic!("T"), concrete!("u32")])),
				},
			),
			(
				1,
				DocumentNode {
					name: "Add".into(),
					inputs: vec![NodeInput::Node(0)],
					metadata: DocumentNodeMetadata::default(),
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::AddNode", &[generic!("T"), generic!("U")])),
				},
			),
		]
		.into_iter()
		.collect(),
	}
}
