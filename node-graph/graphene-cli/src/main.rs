use fern::colors::{Color, ColoredLevelConfig};
use std::{error::Error, sync::Arc};

use document_legacy::{document::Document, layers::layer_info::LegacyLayerType};
use futures::executor::block_on;
use graph_craft::{
	concrete,
	document::*,
	graphene_compiler::{Compiler, Executor},
	imaginate_input::ImaginatePreferences,
	ProtoNodeIdentifier,
};
use graphene_core::{
	application_io::{ApplicationIo, NodeGraphUpdateSender},
	text::FontCache,
};
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::DynamicExecutor;

struct UpdateLogger {}

impl NodeGraphUpdateSender for UpdateLogger {
	fn send(&self, message: graphene_core::application_io::NodeGraphUpdateMessage) {
		println!("{message:?}");
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	init_logging();

	let document_path = std::env::args().nth(1).expect("No document path provided");

	let image_path = std::env::args().nth(2);

	let document_string = std::fs::read_to_string(&document_path).expect("Failed to read document");

	let executor = create_executor(document_string)?;
	println!("creating gpu context",);
	let mut application_io = block_on(WasmApplicationIo::new());
	if let Some(image_path) = image_path {
		application_io.resources.insert("null".to_string(), Arc::from(std::fs::read(image_path).expect("Failed to read image")));
	}

	let device = application_io.gpu_executor().unwrap().context.device.clone();
	std::thread::spawn(move || loop {
		std::thread::sleep(std::time::Duration::from_nanos(10));
		device.poll(wgpu::Maintain::Poll);
	});

	let editor_api = WasmEditorApi {
		image_frame: None,
		font_cache: &FontCache::default(),
		application_io: &application_io,
		node_graph_message_sender: &UpdateLogger {},
		imaginate_preferences: &ImaginatePreferences::default(),
		render_config: graphene_core::application_io::RenderConfig::default(),
	};

	loop {
		//println!("executing");
		let _result = (&executor).execute(editor_api.clone()).await?;
		//println!("result: {result:?}");
		std::thread::sleep(std::time::Duration::from_millis(16));
	}
}

fn init_logging() {
	let colors = ColoredLevelConfig::new().debug(Color::Magenta).info(Color::Green).error(Color::Red);
	fern::Dispatch::new()
		.chain(std::io::stdout())
		.level_for("iced", log::LevelFilter::Trace)
		.level_for("wgpu", log::LevelFilter::Debug)
		.level(log::LevelFilter::Trace)
		.format(move |out, message, record| {
			out.finish(format_args!(
				"[{}]{} {}",
				// This will color the log level only, not the whole line. Just a touch.
				colors.color(record.level()),
				chrono::Utc::now().format("[%Y-%m-%d %H:%M:%S]"),
				message
			))
		})
		.apply()
		.unwrap();
}

fn create_executor(document_string: String) -> Result<DynamicExecutor, Box<dyn Error>> {
	let document: serde_json::Value = serde_json::from_str(&document_string).expect("Failed to parse document");
	let document = serde_json::from_value::<Document>(document["document_legacy"].clone()).expect("Failed to parse document");
	let Some(LegacyLayerType::Layer(ref node_graph)) = document.root.iter().find(|layer| matches!(layer.data, LegacyLayerType::Layer(_))).map(|x| &x.data) else {
		panic!("Failed to extract node graph from document")
	};
	let network = &node_graph.network;
	let wrapped_network = wrap_network_in_scope(network.clone());
	let compiler = Compiler {};
	let protograph = compiler.compile_single(wrapped_network)?;
	let executor = block_on(DynamicExecutor::new(protograph))?;
	Ok(executor)
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
	let nodes = vec![
		begin_scope(),
		inner_network,
		DocumentNode {
			name: "End Scope".to_string(),
			implementation: DocumentNodeImplementation::proto("graphene_core::memo::EndLetNode<_, _>"),
			inputs: vec![NodeInput::node(0, 0), NodeInput::node(1, 0)],
			..Default::default()
		},
	];
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
					manual_composition: Some(concrete!(WasmEditorApi)),
					implementation: DocumentNodeImplementation::Unresolved(ProtoNodeIdentifier::new("graphene_core::ops::SomeNode")),
					..Default::default()
				},
				DocumentNode {
					name: "LetNode".to_string(),
					inputs: vec![NodeInput::node(0, 0)],
					implementation: DocumentNodeImplementation::Unresolved(ProtoNodeIdentifier::new("graphene_core::memo::LetNode<_>")),
					..Default::default()
				},
				DocumentNode {
					name: "RefNode".to_string(),
					manual_composition: Some(concrete!(WasmEditorApi)),
					inputs: vec![NodeInput::lambda(1, 0)],
					implementation: DocumentNodeImplementation::Unresolved(ProtoNodeIdentifier::new("graphene_core::memo::RefNode<_, _>")),
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

#[cfg(test)]
mod test {
	use super::*;

	#[tokio::test]
	#[cfg_attr(not(feature = "wayland"), ignore)]
	async fn grays_scale() {
		let document_string = include_str!("../test_files/gray.graphite");
		let executor = create_executor(document_string.to_string()).unwrap();
		let editor_api = WasmEditorApi {
			image_frame: None,
			font_cache: &FontCache::default(),
			application_io: &block_on(WasmApplicationIo::new()),
			node_graph_message_sender: &UpdateLogger {},
			imaginate_preferences: &ImaginatePreferences::default(),
			render_config: graphene_core::application_io::RenderConfig::default(),
		};
		let result = (&executor).execute(editor_api.clone()).await.unwrap();
		println!("result: {result:?}");
	}

	#[tokio::test]
	#[cfg_attr(not(feature = "wayland"), ignore)]
	async fn hue() {
		let document_string = include_str!("../test_files/hue.graphite");
		let executor = create_executor(document_string.to_string()).unwrap();
		let editor_api = WasmEditorApi {
			image_frame: None,
			font_cache: &FontCache::default(),
			application_io: &block_on(WasmApplicationIo::new()),
			node_graph_message_sender: &UpdateLogger {},
			imaginate_preferences: &ImaginatePreferences::default(),
			render_config: graphene_core::application_io::RenderConfig::default(),
		};
		let result = (&executor).execute(editor_api.clone()).await.unwrap();
		println!("result: {result:?}");
	}
}
