use gpu_compiler_bin_wrapper::CompileRequest;
use gpu_executor::{ShaderIO, ShaderInput};
use graph_craft::concrete;
use graph_craft::document::*;
use graph_craft::*;

use std::borrow::Cow;
use std::time::Duration;

fn main() {
	let client = reqwest::blocking::Client::new();

	// let network = NodeNetwork {
	// 	inputs: vec![0],
	// 	outputs: vec![NodeOutput::new(0, 0)],
	// 	disabled: vec![],
	// 	previous_outputs: None,
	// 	nodes: [(
	// 		0,
	// 		DocumentNode {
	// 			name: "Inc".into(),
	// 			inputs: vec![NodeInput::Network(concrete!(u32))],
	// 			implementation: DocumentNodeImplementation::Network(add_network()),
	// 			metadata: DocumentNodeMetadata::default(),
	// 		},
	// 	)]
	// 	.into_iter()
	// 	.collect(),
	// };
	let network = add_network();
	let compiler = graph_craft::executor::Compiler {};
	let proto_network = compiler.compile_single(network, true).unwrap();

	let io = ShaderIO {
		inputs: vec![ShaderInput::StorageBuffer((), concrete!(u32))],
		output: ShaderInput::OutputBuffer((), concrete!(&mut [u32])),
	};

	let compile_request = CompileRequest::new(vec![proto_network], vec![concrete!(u32)], vec![concrete!(u32)], io);
	let response = client
		.post("http://localhost:3000/compile/spirv")
		.timeout(Duration::from_secs(30))
		.json(&compile_request)
		.send()
		.unwrap();
	println!("response: {:?}", response);
}

fn add_network() -> NodeNetwork {
	NodeNetwork {
		inputs: vec![],
		outputs: vec![NodeOutput::new(0, 0)],
		disabled: vec![],
		previous_outputs: None,
		nodes: [
			(
				0,
				DocumentNode {
					name: "Dup".into(),
					inputs: vec![NodeInput::value(value::TaggedValue::U32(5u32), false)],
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode")),
					..Default::default()
				},
			),
			// (
			// 	1,
			// 	DocumentNode {
			// 		name: "Add".into(),
			// 		inputs: vec![NodeInput::node(0, 0)],
			// 		metadata: DocumentNodeMetadata::default(),
			// 		implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::AddNode")),
			// 	},
			// ),
		]
		.into_iter()
		.collect(),
	}
}
