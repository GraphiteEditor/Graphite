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

	let document = graph_craft::util::load_from_name(document_path.as_str());
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

// Migrations are done in the editor which is unfortunately not available here.
// TODO: remove this and share migrations between the edtior and the CLI.
fn fix_nodes(network: &mut NodeNetwork) {
	for node in network.nodes.values_mut() {
		match &mut node.implementation {
			// Recursively fix
			DocumentNodeImplementation::Network(network) => fix_nodes(network),
			// This replicates the migration from the editor linked:
			// https://github.com/GraphiteEditor/Graphite/blob/d68f91ccca69e90e6d2df78d544d36cd1aaf348e/editor/src/messages/portfolio/portfolio_message_handler.rs#L535
			// Since the CLI doesn't have the document node definitions, a less robust method of just patching the inputs is used.
			DocumentNodeImplementation::ProtoNode(proto_node_identifier)
				if (proto_node_identifier.name.starts_with("graphene_core::ConstructLayerNode") || proto_node_identifier.name.starts_with("graphene_core::AddArtboardNode"))
					&& node.inputs.len() < 3 =>
			{
				node.inputs.push(NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::DocumentNodePath));
			}
			_ => {}
		}
	}
}

fn create_executor(document_string: String, editor_api: Arc<WasmEditorApi>) -> Result<DynamicExecutor, Box<dyn Error>> {
	let document: serde_json::Value = serde_json::from_str(&document_string).expect("Failed to parse document");
	let mut network = serde_json::from_value::<NodeNetwork>(document["network_interface"]["network"].clone()).expect("Failed to parse document");
	fix_nodes(&mut network);

	let wrapped_network = wrap_network_in_scope(network.clone(), editor_api);
	let compiler = Compiler {};
	let protograph = compiler.compile_single(wrapped_network)?;
	let executor = block_on(DynamicExecutor::new(protograph)).unwrap();
	Ok(executor)
}

// TODO: this is copy pasta from the editor (and does get out of sync)
pub fn wrap_network_in_scope(mut network: NodeNetwork, editor_api: Arc<WasmEditorApi>) -> NodeNetwork {
	network.generate_node_paths(&[]);

	let inner_network = DocumentNode {
		implementation: DocumentNodeImplementation::Network(network),
		inputs: vec![],
		..Default::default()
	};

	// TODO: Replace with "Output" definition?
	// let render_node = resolve_document_node_type("Output")
	// 	.expect("Output node type not found")
	// 	.node_template_input_override(vec![Some(NodeInput::node(NodeId(1), 0)), Some(NodeInput::node(NodeId(0), 1))])
	// 	.document_node;

	let render_node = graph_craft::document::DocumentNode {
		inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(2), 0)],
		implementation: graph_craft::document::DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(2), 0)],
			nodes: [
				DocumentNode {
					inputs: vec![NodeInput::scope("editor-api")],
					manual_composition: Some(concrete!(())),
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateGpuSurfaceNode")),
					skip_deduplication: true,
					..Default::default()
				},
				DocumentNode {
					manual_composition: Some(concrete!(())),
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
					..Default::default()
				},
				// TODO: Add conversion step
				DocumentNode {
					manual_composition: Some(concrete!(graphene_std::application_io::RenderConfig)),
					inputs: vec![
						NodeInput::scope("editor-api"),
						NodeInput::network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(generic!(T))), 0),
						NodeInput::node(NodeId(1), 0),
					],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RenderNode")),
					..Default::default()
				},
			]
			.into_iter()
			.enumerate()
			.map(|(id, node)| (NodeId(id as u64), node))
			.collect(),
			..Default::default()
		}),
		..Default::default()
	};

	// wrap the inner network in a scope
	let nodes = vec![
		inner_network,
		render_node,
		DocumentNode {
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![NodeInput::value(TaggedValue::EditorApi(editor_api), false)],
			..Default::default()
		},
	];

	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: [("editor-api".to_string(), (NodeId(2), concrete!(&WasmEditorApi)))].into_iter().collect(),
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
