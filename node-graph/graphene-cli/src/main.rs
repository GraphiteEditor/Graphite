use graph_craft::document::value::TaggedValue;
use graph_craft::graphene_compiler::{Compiler, Executor};
use graph_craft::wasm_application_io::EditorPreferences;
use graph_craft::{concrete, ProtoNodeIdentifier};
use graph_craft::{document::*, generic};
use graphene_core::application_io::{ApplicationIo, NodeGraphUpdateSender};
use graphene_core::text::FontCache;
use graphene_std::transform::Footprint;
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::DynamicExecutor;

use fern::colors::{Color, ColoredLevelConfig};
use futures::executor::block_on;
use std::{error::Error, sync::Arc};

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

	let editor_api = Arc::new(WasmEditorApi {
		font_cache: FontCache::default(),
		application_io: Some(application_io.into()),
		node_graph_message_sender: Box::new(UpdateLogger {}),
		editor_preferences: Box::new(EditorPreferences::default()),
	});
	let executor = create_executor(document_string, editor_api)?;
	let render_config = graphene_core::application_io::RenderConfig::default();

	loop {
		let _result = (&executor).execute(render_config).await?;
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

fn create_executor(document_string: String, editor_api: Arc<WasmEditorApi>) -> Result<DynamicExecutor, Box<dyn Error>> {
	let document: serde_json::Value = serde_json::from_str(&document_string).expect("Failed to parse document");
	let network = serde_json::from_value::<NodeNetwork>(document["network"].clone()).expect("Failed to parse document");
	let wrapped_network = wrap_network_in_scope(network.clone(), editor_api);
	let compiler = Compiler {};
	let protograph = compiler.compile_single(wrapped_network)?;
	let executor = block_on(DynamicExecutor::new(protograph)).unwrap();
	Ok(executor)
}

pub fn wrap_network_in_scope(mut network: NodeNetwork, editor_api: Arc<WasmEditorApi>) -> NodeNetwork {
	network.generate_node_paths(&[]);

	let inner_network = DocumentNode {
		name: "Scope".to_string(),
		implementation: DocumentNodeImplementation::Network(network),
		inputs: vec![NodeInput::node(NodeId(0), 1)],
		metadata: DocumentNodeMetadata::position((-10, 0)),
		..Default::default()
	};

	let render_node = graph_craft::document::DocumentNode {
		name: "Output".into(),
		inputs: vec![NodeInput::node(NodeId(1), 0), NodeInput::node(NodeId(0), 1)],
		implementation: graph_craft::document::DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(2), 0)],
			nodes: [
				DocumentNode {
					name: "Create Canvas".to_string(),
					inputs: vec![NodeInput::scope("editor-api")],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
					skip_deduplication: true,
					..Default::default()
				},
				DocumentNode {
					name: "Cache".to_string(),
					manual_composition: Some(concrete!(())),
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
					..Default::default()
				},
				DocumentNode {
					name: "RenderNode".to_string(),
					inputs: vec![
						NodeInput::network(concrete!(WasmEditorApi), 1),
						NodeInput::network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(generic!(T))), 0),
						NodeInput::node(NodeId(1), 0),
					],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RenderNode<_, _, _, _>")),
					..Default::default()
				},
			]
			.into_iter()
			.enumerate()
			.map(|(id, node)| (NodeId(id as u64), node))
			.collect(),
			..Default::default()
		}),
		metadata: DocumentNodeMetadata::position((-3, 0)),
		..Default::default()
	};

	// wrap the inner network in a scope
	let nodes = vec![
		inner_network,
		render_node,
		DocumentNode {
			name: "Editor Api".into(),
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![NodeInput::value(TaggedValue::EditorApi(editor_api), false)],
			..Default::default()
		},
	];

	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(3), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: [("editor-api".to_string(), (NodeId(2), concrete!(&WasmEditorApi)))].into_iter().collect(),
		..Default::default()
	}
}

// #[cfg(test)]
// mod test {
// 	use super::*;

// 	#[tokio::test]
// 	#[cfg_attr(not(feature = "wayland"), ignore)]
// 	async fn grays_scale() {
// 		let document_string = include_str!("../test_files/gray.graphite");
// 		let executor = create_executor(document_string.to_string()).unwrap();
// 		let editor_api = WasmEditorApi {
// 			image_frame: None,
// 			font_cache: &FontCache::default(),
// 			application_io: &block_on(WasmApplicationIo::new()),
// 			node_graph_message_sender: &UpdateLogger {},
// 			editor_preferences: &EditorPreferences::default(),
// 			render_config: graphene_core::application_io::RenderConfig::default(),
// 		};
// 		let result = (&executor).execute(editor_api.clone()).await.unwrap();
// 		println!("result: {result:?}");
// 	}

// 	#[tokio::test]
// 	#[cfg_attr(not(feature = "wayland"), ignore)]
// 	async fn hue() {
// 		let document_string = include_str!("../test_files/hue.graphite");
// 		let executor = create_executor(document_string.to_string()).unwrap();
// 		let editor_api = WasmEditorApi {
// 			image_frame: None,
// 			font_cache: &FontCache::default(),
// 			application_io: &block_on(WasmApplicationIo::new()),
// 			node_graph_message_sender: &UpdateLogger {},
// 			editor_preferences: &EditorPreferences::default(),
// 			render_config: graphene_core::application_io::RenderConfig::default(),
// 		};
// 		let result = (&executor).execute(editor_api.clone()).await.unwrap();
// 		println!("result: {result:?}");
// 	}
// }
