mod live;
mod repl;
mod watch;

use clap::{Args, Parser, Subcommand};
use document_live::SessionToken;
use fern::colors::{Color, ColoredLevelConfig};
use graphene_cli::{compile_document, dump, export, open_document, open_gdd};
use std::error::Error;
use std::path::PathBuf;

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
	#[clap(group(clap::ArgGroup::new("filter").args(["upstream", "downstream"]).multiple(true)))]
	Compile {
		/// Print the proto network in this format
		#[clap(long, short = 'f', value_enum)]
		format: Option<dump::Format>,

		/// Only print the nodes matching this query and their inputs: a node ID prefix, or part of an identifier or document location
		#[clap(long, requires = "format")]
		upstream: Option<String>,

		/// Only print the nodes matching this query and the nodes using them
		#[clap(long, requires = "format")]
		downstream: Option<String>,

		/// Follow at most this many document nodes from the matching nodes, passing the nodes inserted between them for free
		#[clap(long, requires = "filter")]
		depth: Option<usize>,

		/// Path to the .graphite document
		document: PathBuf,
	},
	/// Export a .graphite document to a file (SVG, PNG, JPG, WebP, TIFF, BMP, TGA, ICO, or GIF).
	Export {
		/// Path to the .graphite document
		document: PathBuf,

		/// Output file path (extension determines format: .svg, .png, .jpg, .webp, .tiff, .bmp, .tga, .ico, .gif)
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

		/// Keep transparency in PNG, WebP, TIFF, BMP, TGA, and ICO exports instead of compositing onto white
		#[clap(long)]
		transparent: bool,

		/// Frames per second for GIF animation or an image sequence (default: 30)
		#[clap(long, default_value = "30")]
		fps: f64,

		/// Total number of frames for GIF animation or an image sequence (an export of more than one frame to a format besides GIF writes numbered files)
		#[clap(long)]
		frames: Option<u32>,

		/// Animation duration in seconds for GIF animation or an image sequence (takes precedence over --frames)
		#[clap(long)]
		duration: Option<f64>,
	},
	ListNodeIdentifiers,

	/// Extract embedded legacy .graphite file from the new .gdd file
	ExtractLegacyDoc {
		document: PathBuf,
	},
	/// Host a collaborative session on a .gdd document, or an empty one, driven from stdin.
	Host {
		document: Option<PathBuf>,
		#[clap(long, default_value = live::DEFAULT_SIGNALING_SERVER)]
		signaling: String,
	},
	/// Join a collaborative session and drive it from stdin.
	Join {
		token: SessionToken,
		#[clap(long, default_value = live::DEFAULT_SIGNALING_SERVER)]
		signaling: String,
	},
	/// Join a collaborative session and show the document in a window.
	Watch {
		token: SessionToken,
		#[clap(long, default_value = live::DEFAULT_SIGNALING_SERVER)]
		signaling: String,
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

	init_logging(app.global_opts.verbose);

	let document_path = match &app.command {
		Command::Host { document, signaling } => return live::host(signaling, document.as_deref()).await,
		Command::Join { token, signaling } => return live::join(signaling, *token).await,
		Command::Watch { token, signaling } => return live::watch(signaling, *token).await,
		Command::Compile { document, .. } | Command::Export { document, .. } | Command::ExtractLegacyDoc { document } => document,
		Command::ListNodeIdentifiers => {
			let mut nodes: Vec<_> = graphene_std::registry::NODE_METADATA.lock().unwrap().keys().cloned().collect();
			nodes.sort_by_key(|x| x.as_str().to_string());
			for id in nodes {
				println!("{}", id.as_str());
			}
			return Ok(());
		}
	};

	// Before the document converts to a network, so a registry that can't convert doesn't block recovering the legacy file
	if let Command::ExtractLegacyDoc { document: path } = &app.command {
		if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("gdd")) {
			return Err("ExtractLegacyDoc requires a .gdd document".into());
		}
		let archive = std::fs::read(path).map_err(|error| format!("Failed to read document {}: {error}", path.display()))?;
		let gdd = open_gdd(&archive).await?;
		let Some(legacy_doc) = gdd.read_legacy_document().await else {
			return Err("gdd file did not contain a legacy .graphite document".into());
		};
		let new_path = path.with_extension("graphite");
		std::fs::write(&new_path, legacy_doc).map_err(|error| format!("Failed to write .graphite file: {error}"))?;
		eprintln!("Saved file to {}", new_path.to_string_lossy());
		return Ok(());
	}

	let print = matches!(app.command, Command::Compile { format: Some(_), .. });
	let mut document = open_document(document_path, print).await?;

	let dump_document = document.names.take().map(|names| dump::document(&document.network, names));
	let compiled = compile_document(document).await?;

	match app.command {
		Command::Compile {
			format, upstream, downstream, depth, ..
		} => {
			if let Some((format, dump_document)) = format.zip(dump_document) {
				dump::print(&compiled.proto_network, &dump_document, format, upstream.as_deref(), downstream.as_deref(), depth)?;
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
			let file_type = export::detect_file_type(&output)?;
			if fps <= 0. {
				return Err("Fps number must be positive".into());
			}

			let renderer = compiled.into_renderer()?;
			let (executor, wgpu_executor, runtime, spawner) = (&renderer.executor, renderer.wgpu_executor(), &renderer.runtime, renderer.spawner.as_ref());

			// A multi-frame export to a format besides GIF becomes a numbered image sequence
			let settings = export::ExportSettings {
				file_type,
				scale,
				size: (width, height),
				transparent,
				framing: None,
			};
			let settings = renderer.frame(settings).await?;
			let animation = export::AnimationParams::new(fps, frames, duration);
			if file_type == export::FileType::Gif {
				export::export_gif(executor, wgpu_executor, output, settings, animation, runtime, spawner).await?;
			} else if animation.frames > 1 {
				if file_type == export::FileType::Ico {
					return Err("ICO can't be exported as an image sequence".into());
				}
				export::export_image_sequence(executor, wgpu_executor, output, settings, animation, runtime, spawner).await?;
			} else {
				export::export_document(executor, wgpu_executor, output, settings, runtime, spawner).await?;
			}
		}
		Command::ListNodeIdentifiers | Command::ExtractLegacyDoc { .. } | Command::Host { .. } | Command::Join { .. } | Command::Watch { .. } => unreachable!("Handled before compiling"),
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
		.chain(std::io::stderr())
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
