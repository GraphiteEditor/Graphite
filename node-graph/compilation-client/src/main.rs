use gpu_compiler_bin_wrapper::CompileRequest;
use graph_craft::document::*;
use graph_craft::generic;
use graph_craft::proto::*;

fn main() {
	let client = reqwest::blocking::Client::new();
	let compile_request = CompileRequest::new(add_network(), "u32".to_owned(), "u32".to_owned());
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
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::structural::ConsNode", &[generic!("T"), generic!("U")])),
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
