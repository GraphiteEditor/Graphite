use graph_craft::document::*;
use graph_craft::graphene_compiler::{Compiler, Executor};
use graph_craft::util::load_network;
use graph_craft::wasm_application_io::EditorPreferences;
use graphene_core::application_io::{ApplicationIo, NodeGraphUpdateSender};
use graphene_core::text::FontCache;
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::DynamicExecutor;

use clap::{arg, command, value_parser, Command};
use fern::colors::{Color, ColoredLevelConfig};
use futures::executor::block_on;
use interpreted_executor::util::wrap_network_in_scope;
use std::{error::Error, path::PathBuf, sync::Arc};

struct UpdateLogger {}

impl NodeGraphUpdateSender for UpdateLogger {
	fn send(&self, message: graphene_core::application_io::NodeGraphUpdateMessage) {
		println!("{message:?}");
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	init_logging();

	let matches = command!()
		.arg(arg!(<document> "Path to the Graphite document").value_parser(value_parser!(PathBuf)))
		.arg(arg!([image] "Image to read").value_parser(value_parser!(PathBuf)))
		.subcommand(Command::new("compile").about("Compile to proto graph and print"))
		.subcommand(Command::new("run").about("Compile and run document displaying output"))
		.subcommand_required(true)
		.get_matches();

	let document_path = matches.get_one::<PathBuf>("document").expect("No document path provided");

	let image_path = matches.get_one::<PathBuf>("image");

	let document_string = std::fs::read_to_string(&document_path).expect("Failed to read document");

	println!("creating gpu context",);
	let mut application_io = block_on(WasmApplicationIo::new());
	if let Some(image_path) = image_path {
		application_io.resources.insert("null".to_string(), Arc::from(std::fs::read(image_path).expect("Failed to read image")));
	}

	let subcommand = matches.subcommand().unwrap().0;
	let run = subcommand == "run";

	if run {
		let device = application_io.gpu_executor().unwrap().context.device.clone();
		std::thread::spawn(move || loop {
			std::thread::sleep(std::time::Duration::from_nanos(10));
			device.poll(wgpu::Maintain::Poll);
		});
	}

	let editor_api = Arc::new(WasmEditorApi {
		font_cache: FontCache::default(),
		application_io: Some(application_io.into()),
		node_graph_message_sender: Box::new(UpdateLogger {}),
		editor_preferences: Box::new(EditorPreferences::default()),
	});

	let executor = create_executor(document_string, editor_api, subcommand == "compile")?;
	let render_config = graphene_core::application_io::RenderConfig::default();

	if run {
		loop {
			let _result = (&executor).execute(render_config).await?;
			std::thread::sleep(std::time::Duration::from_millis(16));
		}
	}

	Ok(())
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
// TODO: remove this and share migrations between the editor and the CLI.
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

fn create_executor(document_string: String, editor_api: Arc<WasmEditorApi>, print_proto: bool) -> Result<DynamicExecutor, Box<dyn Error>> {
	let mut network = load_network(&document_string);
	fix_nodes(&mut network);

	let wrapped_network = wrap_network_in_scope(network.clone(), editor_api);
	let compiler = Compiler {};
	let protograph = compiler.compile_single(wrapped_network)?;
	if print_proto {
		println!("{}", protograph);
	}
	let executor = block_on(DynamicExecutor::new(protograph)).unwrap();
	Ok(executor)
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
