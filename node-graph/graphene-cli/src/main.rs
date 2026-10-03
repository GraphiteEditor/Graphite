mod export;

use clap::{Args, Parser, Subcommand};
use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use fern::colors::{Color, ColoredLevelConfig};
use futures::executor::block_on;
use graph_craft::application_io::EditorPreferences;
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::*;
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::ProtoNetwork;
use graph_craft::util::load_network;
use graphene_std::application_io::{ApplicationIo, NodeGraphUpdateMessage, NodeGraphUpdateSender};
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
	/// Export a .graphite document to a file (SVG, PNG, JPG, or GIF).
	Export {
		/// Path to the .graphite document
		document: PathBuf,

		/// Output file path (extension determines format: .svg, .png, .jpg, .gif)
		#[clap(long, short = 'o')]
		output: PathBuf,

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

		/// Frames per second for GIF animation (default: 30)
		#[clap(long, default_value = "30")]
		fps: f64,

		/// Total number of frames for GIF animation
		#[clap(long)]
		frames: Option<u32>,

		/// Animation duration in seconds for GIF (takes precedence over --frames)
		#[clap(long)]
		duration: Option<f64>,
	},
	ListNodeIdentifiers,

	/// Extract embedded legacy .graphite file from the new .gdd file
	ExtractLegacyDoc {
		document: PathBuf,
	},
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
		Command::ExtractLegacyDoc { ref document } => document,
		Command::ListNodeIdentifiers => {
			let mut nodes: Vec<_> = graphene_std::registry::NODE_METADATA.lock().unwrap().keys().cloned().collect();
			nodes.sort_by_key(|x| x.as_str().to_string());
			for id in nodes {
				println!("{}", id.as_str());
			}
			return Ok(());
		}
	};

	// Load the document by extension: `.gdd` opens the new archive format, anything else is treated as a
	// legacy `.graphite` document. The legacy path has no `Gdd`, so resources fall back to the default registry.
	let is_gdd = document_path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("gdd"));

	let gdd = if is_gdd {
		let archive = std::fs::read(document_path).map_err(|error| format!("Failed to read document {}: {error}", document_path.display()))?;
		let container = AnyContainer::Memory(MemoryBackend::new());
		let gdd = document_format::Gdd::open_from_archive(archive.as_ref(), container, GddV1Layout)
			.await
			.map_err(|error| format!("Failed to open document: {error}"))?;
		Some(gdd)
	} else {
		None
	};

	if let Command::ExtractLegacyDoc { ref document } = app.command {
		let Some(gdd) = &gdd else { return Err("ExtractLegacyDoc requires a .gdd document".into()) };
		let Some(legacy_doc) = gdd.read_legacy_document().await else {
			return Err("gdd file did not contain a legacy .graphite document".into());
		};
		let mut new_path = document.clone();
		new_path.set_extension("graphite");
		std::fs::write(&new_path, legacy_doc).map_err(|error| format!("Failed to write .graphite file: {error}"))?;
		eprintln!("Saved file to {}", new_path.to_string_lossy());
		return Ok(());
	}

	// Build the runtime network: from the `.gdd` registry, or by loading a legacy `.graphite` document.
	let node_network = match &gdd {
		Some(gdd) => {
			let declarations = gdd.declarations(gdd).await;
			let (node_network, _metadata) = gdd.registry().to_runtime_with_metadata(&declarations)?;
			node_network
		}
		None => {
			let document_string = std::fs::read_to_string(document_path).map_err(|error| format!("Failed to read document {}: {error}", document_path.display()))?;
			load_network(&document_string)
		}
	};

	log::info!("Creating GPU context");
	let mut application_io = PlatformApplicationIo::new().await;
	if let Some(gdd) = &gdd {
		application_io.inject_resource_proxy(Box::new(gdd.resource_proxy()));
	}

	// Convert application_io to Arc first
	let application_io_arc = Arc::new(application_io);

	// Clone the application_io Arc before borrowing to extract executor
	let application_io_for_api = application_io_arc.clone();

	// Get reference to wgpu executor and clone device handle
	let wgpu_executor_ref = application_io_arc.gpu_executor().unwrap();
	let device = wgpu_executor_ref.context().device.clone();

	let preferences = EditorPreferences {
		max_render_region_size: EditorPreferences::default().max_render_region_size,
	};
	let editor_api = Arc::new(PlatformEditorApi {
		application_io: Some(application_io_for_api),
		node_graph_message_sender: Box::new(UpdateLogger {}),
		editor_preferences: Box::new(preferences),
	});
	let proto_graph = compile_graph(node_network, editor_api, gdd.as_ref())?;

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
			fps,
			frames,
			duration,
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

			if fps <= 0. {
				return Err("Fps number must be positive".into());
			}

			// Perform export based on file type
			if file_type == export::FileType::Gif {
				let animation = export::AnimationParams::new(fps, frames, duration);
				export::export_gif(&executor, wgpu_executor_ref, output, scale, (width, height), animation).await?;
			} else {
				export::export_document(&executor, wgpu_executor_ref, output, file_type, scale, (width, height), transparent).await?;
			}
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

fn compile_graph(network: NodeNetwork, editor_api: Arc<PlatformEditorApi>, gdd: Option<&GddV1>) -> Result<ProtoNetwork, Box<dyn Error>> {
	let preprocessor = preprocessor::Preprocessor::new();

	let mut network = wrap_network_in_scope(network, editor_api);

	// A `.gdd` resolves resource hashes from its registry; a legacy `.graphite` has no resource store, so it
	// preprocesses against an empty registry (matching the pre-`.gdd` CLI behavior).
	match gdd {
		Some(gdd) => preprocessor
			.preprocess(&mut network, &|resource_id| gdd.registry().resources.get(&resource_id).and_then(|r| r.hash))
			.expect("Failed to expand network"),
		None => { preprocessor.preprocess(&mut network, &|_| None) }.expect("Failed to expand network"),
	}

	let compiler = Compiler {};
	compiler.compile_single(network).map_err(|x| x.into())
}

fn create_executor(proto_network: ProtoNetwork) -> Result<DynamicExecutor, Box<dyn Error>> {
	let executor = block_on(DynamicExecutor::new(proto_network)).map_err(|errors| errors.iter().map(|e| format!("{e:?}")).reduce(|acc, e| format!("{acc}\n{e}")).unwrap_or_default())?;
	Ok(executor)
}
