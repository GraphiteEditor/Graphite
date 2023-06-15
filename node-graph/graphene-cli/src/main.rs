use std::{collections::HashMap, error::Error};

use document_legacy::{
	document::Document,
	layers::layer_info::{LayerData, LayerDataType},
};
use futures::executor::block_on;
use graph_craft::{
	concrete,
	document::{value::TaggedValue, *},
	graphene_compiler::{Compiler, Executor},
	imaginate_input::ImaginatePreferences,
	NodeIdentifier, Type, TypeDescriptor,
};
use graphene_core::{application_io::NodeGraphUpdateSender, raster::ImageFrame, text::FontCache, Cow};
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::DynamicExecutor;

struct UpdateLogger {}

impl NodeGraphUpdateSender for UpdateLogger {
	fn send(&self, message: graphene_core::application_io::NodeGraphUpdateMessage) {
		println!("{:?}", message);
	}
}

fn main() -> Result<(), Box<dyn Error>> {
	let document_path = std::env::args().nth(1).expect("No document path provided");

	let document_string = std::fs::read_to_string(&document_path).expect("Failed to read document");

	let document: serde_json::Value = serde_json::from_str(&document_string).expect("Failed to parse document");
	let document = serde_json::from_value::<Document>(document["document_legacy"].clone()).expect("Failed to parse document");

	let Some(LayerDataType::Layer(ref node_graph)) = document.root.iter().find(|layer| matches!(layer.data, LayerDataType::Layer(_))).map(|x|&x.data) else { panic!("failed to extract node graph from docmuent") };
	let network = &node_graph.network;

	let wrapped_network = wrap_network_in_scope(network.clone());

	let compiler = Compiler {};
	let protograph = compiler.compile_single(wrapped_network, true)?;

	let mut executor = block_on(DynamicExecutor::new(protograph))?;

	let editor_api = WasmEditorApi {
		image_frame: None,
		font_cache: &FontCache::default(),
		application_io: &WasmApplicationIo::default(),
		node_graph_message_sender: &UpdateLogger {},
		imaginate_preferences: &ImaginatePreferences::default(),
	};

	let result = block_on((&executor).execute(editor_api))?;
	println!("result: {:?}", result);
	Ok(())
}

pub fn wrap_network_in_scope(mut network: NodeNetwork) -> NodeNetwork {
	let node_ids = network.nodes.keys().copied().collect::<Vec<_>>();

	network.generate_node_paths(&[]);
	for id in node_ids {
		network.flatten(id);
	}

	let mut network_inputs = Vec::new();
	let mut input_type = None;
	for (id, node) in network.nodes.iter() {
		for input in node.inputs.iter() {
			if let NodeInput::Network(_) = input {
				if input_type.is_none() {
					input_type = Some(input.clone());
				}
				assert_eq!(input, input_type.as_ref().unwrap(), "Networks wrapped in scope must have the same input type");
				network_inputs.push(*id);
			}
		}
	}
	let len = network_inputs.len();
	network.inputs = network_inputs;

	// if the network has no inputs, it doesn't need to be wrapped in a scope
	if len == 0 {
		return network;
	}

	let inner_network = DocumentNode {
		name: "Scope".to_string(),
		implementation: DocumentNodeImplementation::Network(network),
		inputs: core::iter::repeat(NodeInput::node(0, 1)).take(len).collect(),
		..Default::default()
	};

	// wrap the inner network in a scope
	let nodes = vec![begin_scope(), inner_network, end_scope()];
	NodeNetwork {
		inputs: vec![0],
		outputs: vec![NodeOutput::new(2, 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (id as NodeId, node)).collect(),
		..Default::default()
	}
}

fn begin_scope() -> DocumentNode {
	DocumentNode {
		name: "Begin Scope".to_string(),
		implementation: DocumentNodeImplementation::Network(NodeNetwork {
			inputs: vec![0],
			outputs: vec![NodeOutput::new(1, 0), NodeOutput::new(2, 0)],
			nodes: [
				DocumentNode {
					name: "SetNode".to_string(),
					inputs: vec![NodeInput::ShortCircut(concrete!(WasmEditorApi))],
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::SomeNode")),
					..Default::default()
				},
				DocumentNode {
					name: "LetNode".to_string(),
					inputs: vec![NodeInput::node(0, 0)],
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::memo::LetNode<_>")),
					..Default::default()
				},
				DocumentNode {
					name: "RefNode".to_string(),
					inputs: vec![NodeInput::ShortCircut(concrete!(())), NodeInput::lambda(1, 0)],
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::memo::RefNode<_, _>")),
					..Default::default()
				},
			]
			.into_iter()
			.enumerate()
			.map(|(id, node)| (id as NodeId, node))
			.collect(),

			..Default::default()
		}),
		inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
		..Default::default()
	}
}
fn end_scope() -> DocumentNode {
	DocumentNode {
		name: "End Scope".to_string(),
		implementation: DocumentNodeImplementation::proto("graphene_core::memo::EndLetNode<_>"),
		inputs: vec![NodeInput::value(TaggedValue::None, true), NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true)],
		..Default::default()
	}
}
