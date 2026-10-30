mod compare;
mod corpus;
mod render;
mod upgrade;

use clap::{Parser, Subcommand};
use compare::Comparison;
use corpus::{Corpus, Entry};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const CORPUS_VARIABLE: &str = "GRAPHITE_REGRESSION_CORPUS";

#[derive(Parser)]
#[clap(name = "regression", about = "Upgrade Graphite documents and check renders of the regression corpus against its references")]
struct App {
	/// The corpus directory, by default from the GRAPHITE_REGRESSION_CORPUS environment variable
	#[clap(long, global = true)]
	corpus: Option<PathBuf>,

	#[clap(subcommand)]
	command: Command,
}

#[derive(Subcommand)]
enum Command {
	/// Rewrite .gdd files through the migrations of this build, as state-only documents
	Upgrade {
		/// Documents, or directories whose .gdd files to upgrade
		#[clap(default_value = "demo-artwork")]
		paths: Vec<PathBuf>,
	},
	/// Copy a document into the corpus and render its references
	Add {
		document: PathBuf,
		/// Name in the corpus, by default the file name
		#[clap(long)]
		name: Option<String>,
		/// Animation times in seconds to render, each to its own reference
		#[clap(long, value_delimiter = ',', default_value = "0")]
		times: Vec<f64>,
		#[clap(long, default_value = "1")]
		scale: f64,
		/// Output size in pixels of a region at the document origin, as WIDTHxHEIGHT, instead of the artwork's bounds
		#[clap(long, value_parser = parse_size)]
		size: Option<(u32, u32)>,
	},
	/// Render the references again, after an intended change in how documents look
	Bless {
		/// Documents to render, by default all of them
		names: Vec<String>,
	},
	/// Render the corpus and compare each image to its reference
	Check {
		/// Documents to check, by default all of them
		names: Vec<String>,
		/// Where to write the renders and diffs of mismatches
		#[clap(long, default_value = "target/regression")]
		output: PathBuf,
	},
}

#[tokio::main]
async fn main() -> ExitCode {
	let app = App::parse();
	init_logging();

	match run(app).await {
		Ok(true) => ExitCode::SUCCESS,
		Ok(false) => ExitCode::FAILURE,
		Err(error) => {
			eprintln!("Error: {error}");
			ExitCode::FAILURE
		}
	}
}

/// Whether every document passed.
async fn run(app: App) -> Result<bool, Box<dyn Error>> {
	let corpus_root = || {
		app.corpus
			.clone()
			.or_else(|| std::env::var_os(CORPUS_VARIABLE).map(PathBuf::from))
			.ok_or(format!("pass --corpus or set {CORPUS_VARIABLE}"))
	};

	match app.command {
		Command::Upgrade { paths } => {
			for path in upgrade::expand(&paths)? {
				let changed = upgrade::upgrade(&path).await.map_err(|error| format!("{}: {error}", path.display()))?;
				println!("{} {}", if changed { "Upgraded" } else { "Up to date" }, path.display());
			}
			Ok(true)
		}
		Command::Add { document, name, times, scale, size } => {
			let mut corpus = Corpus::open(&corpus_root()?)?;
			let name = match name {
				Some(name) => name,
				None => document.file_stem().and_then(|stem| stem.to_str()).ok_or("the document has no file name")?.to_string(),
			};

			let destination = corpus.document(&name);
			create_parent(&destination)?;
			std::fs::copy(&document, &destination)?;

			let entry = Entry::new(name, times, scale, size);
			bless(&corpus, &entry).await?;
			corpus.insert(entry);
			corpus.save()?;
			Ok(true)
		}
		Command::Bless { names } => {
			let corpus = Corpus::open(&corpus_root()?)?;
			for entry in corpus.select(&names)? {
				bless(&corpus, &entry).await?;
			}
			Ok(true)
		}
		Command::Check { names, output } => {
			let corpus = Corpus::open(&corpus_root()?)?;
			let mut passed = true;
			for entry in corpus.select(&names)? {
				passed &= check(&corpus, &entry, &output).await;
			}
			Ok(passed)
		}
	}
}

async fn bless(corpus: &Corpus, entry: &Entry) -> Result<(), Box<dyn Error>> {
	let images = render::render(&corpus.document(&entry.name), entry).await?;
	for (time, image) in entry.times.iter().zip(images) {
		let reference = corpus.reference(&entry.name, *time);
		create_parent(&reference)?;
		image.save(&reference)?;
		println!("Rendered {}", reference.display());
	}
	Ok(())
}

/// Whether every time of `entry` matches its reference. Mismatches leave their render and diff in `output`.
async fn check(corpus: &Corpus, entry: &Entry, output: &Path) -> bool {
	let images = match render::render(&corpus.document(&entry.name), entry).await {
		Ok(images) => images,
		Err(error) => {
			println!("FAIL {}: rendering failed: {error}", entry.name);
			return false;
		}
	};

	let mut passed = true;
	for (time, actual) in entry.times.iter().zip(images) {
		let label = format!("{} at {time:.3}s", entry.name);
		let reference = match image::open(corpus.reference(&entry.name, *time)) {
			Ok(reference) => reference.into_rgba8(),
			Err(error) => {
				println!("FAIL {label}: no reference ({error}). Run `bless {}`", entry.name);
				passed = false;
				continue;
			}
		};

		let (reason, diff) = match compare::compare(&reference, &actual, entry.tolerance) {
			Comparison::Matches => {
				println!("ok   {label}");
				continue;
			}
			Comparison::SizeChanged { reference, actual } => (format!("the size changed from {reference:?} to {actual:?}"), None),
			Comparison::Differs { differing, total, diff } => (format!("{differing} of {total} pixels differ"), Some(diff)),
		};
		passed = false;

		let written = save_mismatch(output, &entry.name, *time, &actual, diff.as_ref());
		let location = written.map_or_else(|error| format!("writing the mismatch failed: {error}"), |directory| format!("see {}", directory.display()));
		println!("FAIL {label}: {reason}, {location}");
	}
	passed
}

fn save_mismatch(output: &Path, name: &str, time: f64, actual: &image::RgbaImage, diff: Option<&image::RgbaImage>) -> Result<PathBuf, Box<dyn Error>> {
	let directory = output.join(name);
	std::fs::create_dir_all(&directory)?;
	actual.save(directory.join(format!("{time:.3}s.actual.png")))?;
	if let Some(diff) = diff {
		diff.save(directory.join(format!("{time:.3}s.diff.png")))?;
	}
	Ok(directory)
}

fn parse_size(size: &str) -> Result<(u32, u32), String> {
	let (width, height) = size.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
	let dimension = |text: &str| text.parse::<u32>().map_err(|error| format!("{text}: {error}"));
	Ok((dimension(width)?, dimension(height)?))
}

fn create_parent(path: &Path) -> Result<(), Box<dyn Error>> {
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent)?;
	}
	Ok(())
}

/// Migration progress and errors, without the GPU stack's chatter.
fn init_logging() {
	let result = fern::Dispatch::new()
		.level(log::LevelFilter::Off)
		.level_for("document_format", log::LevelFilter::Info)
		.level_for("graphene_cli", log::LevelFilter::Warn)
		.format(|out, message, record| out.finish(format_args!("[{}] {message}", record.level())))
		.chain(std::io::stderr())
		.apply();
	if let Err(error) = result {
		eprintln!("Setting up logging failed: {error}");
	}
}
