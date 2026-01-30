mod export;

use clap::{Args, Parser, Subcommand};
use fern::colors::{Color, ColoredLevelConfig};
use futures::executor::block_on;
use graph_craft::document::*;
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::ProtoNetwork;
use graph_craft::util::load_network;
use graph_craft::wasm_application_io::EditorPreferences;
use graphene_std::application_io::{ApplicationIo, NodeGraphUpdateMessage, NodeGraphUpdateSender};
use graphene_std::text::FontCache;
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

struct UpdateLogger {}

impl NodeGraphUpdateSender for UpdateLogger {
	fn send(&self, message: NodeGraphUpdateMessage) {
		println!("{message:?}");
	}
}

#[derive(Debug, Parser)]
#[clap(name = "graphene-cli", version)]
pub struct App {
	#[clap(flatten)]
	global_opts: GlobalOpts,

	#[clap(subcommand)]
	command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
	/// Help message for compile.
	Compile {
		/// Print proto network
		#[clap(long, short = 'p')]
		print_proto: bool,

		/// Path to the .graphite document
		document: PathBuf,
	},
	/// Export a .graphite document to a file (SVG, PNG, or JPG).
	Export {
		/// Path to the .graphite document
		document: PathBuf,

		/// Output file path (extension determines format: .svg, .png, .jpg)
		#[clap(long, short = 'o')]
		output: PathBuf,

		/// Optional input image resource
		#[clap(long)]
		image: Option<PathBuf>,

		/// Scale factor for export (default: 1.0)
		#[clap(long, default_value = "1.0")]
		scale: f64,

		/// Output width in pixels
		#[clap(long)]
		width: Option<u32>,

		/// Output height in pixels
		#[clap(long)]
		height: Option<u32>,

		/// Transparent background for PNG exports
		#[clap(long)]
		transparent: bool,
	},
	ListNodeIdentifiers,
}

#[derive(Debug, Args)]
struct GlobalOpts {
	/// Verbosity level (can be specified multiple times)
	#[clap(long, short, global = true, action = clap::ArgAction::Count)]
	verbose: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let app = App::parse();

	let log_level = app.global_opts.verbose;

	init_logging(log_level);

	let document_path = match app.command {
		Command::Compile { ref document, .. } => document,
		Command::Export { ref document, .. } => document,
		Command::ListNodeIdentifiers => {
			let mut nodes: Vec<_> = graphene_std::registry::NODE_METADATA.lock().unwrap().keys().cloned().collect();
			nodes.sort_by_key(|x| x.as_str().to_string());
			for id in nodes {
				println!("{}", id.as_str());
			}
			return Ok(());
		}
	};

	let document_string = std::fs::read_to_string(document_path).expect("Failed to read document");

	log::info!("Creating GPU context");
	let mut application_io = block_on(WasmApplicationIo::new_offscreen());

	if let Command::Export { image: Some(ref image_path), .. } = app.command {
		application_io.resources.insert("null".to_string(), Arc::from(std::fs::read(image_path).expect("Failed to read image")));
	}

	// Convert application_io to Arc first
	let application_io_arc = Arc::new(application_io);

	// Clone the application_io Arc before borrowing to extract executor
	let application_io_for_api = application_io_arc.clone();

	// Get reference to wgpu executor and clone device handle
	let wgpu_executor_ref = application_io_arc.gpu_executor().unwrap();
	let device = wgpu_executor_ref.context.device.clone();

	let preferences = EditorPreferences { use_vello: true };
	let editor_api = Arc::new(WasmEditorApi {
		font_cache: FontCache::default(),
		application_io: Some(application_io_for_api),
		node_graph_message_sender: Box::new(UpdateLogger {}),
		editor_preferences: Box::new(preferences),
	});

	let proto_graph = compile_graph(document_string, editor_api)?;

	match app.command {
		Command::Compile { print_proto, .. } => {
			if print_proto {
				println!("{proto_graph}");
			}
		}
		Command::Export {
			output,
			scale,
			width,
			height,
			transparent,
			..
		} => {
			// Spawn thread to poll GPU device
			std::thread::spawn(move || {
				loop {
					std::thread::sleep(std::time::Duration::from_nanos(10));
					device.poll(wgpu::PollType::Poll).unwrap();
				}
			});

			// Detect output file type
			let file_type = export::detect_file_type(&output)?;

			// Create executor
			let executor = create_executor(proto_graph)?;

			// Perform export
			export::export_document(&executor, wgpu_executor_ref, output, file_type, scale, (width, height), transparent).await?;
		}
		_ => unreachable!("All other commands should be handled before this match statement is run"),
	}

	Ok(())
}

fn init_logging(log_level: u8) {
	let default_level = match log_level {
		0 => log::LevelFilter::Error,
		1 => log::LevelFilter::Info,
		2 => log::LevelFilter::Debug,
		_ => log::LevelFilter::Trace,
	};
	let colors = ColoredLevelConfig::new().debug(Color::Magenta).info(Color::Green).error(Color::Red);
	fern::Dispatch::new()
		.chain(std::io::stdout())
		.level_for("wgpu", log::LevelFilter::Error)
		.level_for("naga", log::LevelFilter::Error)
		.level_for("wgpu_hal", log::LevelFilter::Error)
		.level_for("wgpu_core", log::LevelFilter::Error)
		.level(default_level)
		.format(move |out, message, record| {
			out.finish(format_args!(
				"[{}]{}{} {}",
				// This will color the log level only, not the whole line. Just a touch.
				colors.color(record.level()),
				chrono::Utc::now().format("[%Y-%m-%d %H:%M:%S]"),
				record.module_path().unwrap_or(""),
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
				if (proto_node_identifier.as_str().starts_with("graphene_core::ConstructLayerNode") || proto_node_identifier.as_str().starts_with("graphene_core::AddArtboardNode"))
					&& node.inputs.len() < 3 =>
			{
				node.inputs.push(NodeInput::Reflection(DocumentNodeMetadata::DocumentNodePath));
			}
			_ => {}
		}
	}
}
fn compile_graph(document_string: String, editor_api: Arc<WasmEditorApi>) -> Result<ProtoNetwork, Box<dyn Error>> {
	let mut network = load_network(&document_string);
	fix_nodes(&mut network);

	let substitutions = preprocessor::generate_node_substitutions();
	preprocessor::expand_network(&mut network, &substitutions);

	let wrapped_network = wrap_network_in_scope(network.clone(), editor_api);

	let compiler = Compiler {};
	compiler.compile_single(wrapped_network).map_err(|x| x.into())
}

fn create_executor(proto_network: ProtoNetwork) -> Result<DynamicExecutor, Box<dyn Error>> {
	let executor = block_on(DynamicExecutor::new(proto_network)).map_err(|errors| errors.iter().map(|e| format!("{e:?}")).reduce(|acc, e| format!("{acc}\n{e}")).unwrap_or_default())?;
	Ok(executor)
}
