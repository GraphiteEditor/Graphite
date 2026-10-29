//! `cargo run migration`: freezes the stored document types (`graph-storage`'s `schema` module) into `migration-formats`
//! as one format version with the step from the previous one, checks that the live types still match the latest frozen
//! version, and scaffolds node migrations into `migration-nodes`.
//!
//! A new version declares again only the types that changed, plus every type reaching one, and re-exports the rest from
//! the previous version. So format steps convert only what changed, and old versions never depend on the live crate.

mod nodes;
mod plan;
mod scaffold;
mod step;
mod types;

use plan::{Plan, Snapshots};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use step::Step;
use types::Types;

const SCHEMA: &str = "document/graph-storage/src/schema";
const SNAPSHOTS: &str = "document/migrations/formats/src";
const MANIFEST: &str = "document/format/src/manifest.rs";
const SUPPORTED_VERSION_DECLARATION: &str = "pub const SUPPORTED_FORMAT_VERSION: u32 = ";

fn usage() {
	println!();
	println!("USAGE:");
	println!("  cargo run migration <command>");
	println!();
	println!("<command>:");
	println!("  check       Fail if the stored types changed since the latest frozen format version");
	println!("  snapshot    Freeze the stored types as the next format version, with the step from the previous one");
	println!("  new <identifier>");
	println!("              Scaffold the migration of a node from the version before its current `version(N)`");
	println!("  new <old identifier>@<version> <identifier>");
	println!("              Scaffold the migration replacing a removed node by another");
	println!("  rename <old identifier>@<version> <identifier>");
	println!("              Register that a node only moved to a new identifier");
	println!();
	println!("A node is named by its full identifier, or by a function name or identifier suffix only one node has.");
	println!();
}

fn main() -> ExitCode {
	let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
	let workspace = workspace.canonicalize().unwrap_or(workspace);

	let arguments: Vec<String> = std::env::args().skip(1).collect();
	let result = match arguments.first().map(String::as_str) {
		Some("check") => check(&workspace),
		Some("snapshot") => snapshot(&workspace),
		Some(command @ ("new" | "rename")) => scaffold::scaffold(&workspace, command, &arguments[1..]),
		Some("help" | "--help") => {
			usage();
			return ExitCode::SUCCESS;
		}
		_ => {
			usage();
			return ExitCode::FAILURE;
		}
	};

	match result {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("Error: {error}");
			ExitCode::FAILURE
		}
	}
}

fn check(workspace: &Path) -> Result<(), String> {
	let plan = Plan::new(&Types::read_schema(&workspace.join(SCHEMA))?, &Snapshots::read(&workspace.join(SNAPSHOTS))?);
	let Some(latest) = plan.previous else {
		return Err("no frozen format version exists yet, run `cargo run migration snapshot`".to_string());
	};

	if !plan.is_unchanged() {
		return Err(format!(
			"the stored types changed since format version {latest}:\n{}\nRun `cargo run migration snapshot` to freeze them as version {} with a format step from {latest}",
			plan.describe(),
			plan.version
		));
	}

	for version in 2..=latest {
		let step = workspace.join(SNAPSHOTS).join(step_path(version));
		if !step.exists() {
			return Err(format!("format version {version} has no step from {} at {}", version - 1, step.display()));
		}
	}

	let supported = supported_version(workspace)?;
	if supported != latest {
		return Err(format!("`SUPPORTED_FORMAT_VERSION` is {supported}, but the latest frozen format version is {latest}"));
	}

	println!("The stored types match format version {latest}");
	Ok(())
}

fn snapshot(workspace: &Path) -> Result<(), String> {
	let live = Types::read_schema(&workspace.join(SCHEMA))?;
	let snapshots_directory = workspace.join(SNAPSHOTS);
	let snapshots = Snapshots::read(&snapshots_directory)?;
	let plan = Plan::new(&live, &snapshots);

	if let Some(latest) = plan.previous
		&& plan.is_unchanged()
	{
		println!("The stored types match format version {latest}, nothing to freeze");
		return Ok(());
	}

	let file = snapshots_directory.join(format!("v{}.rs", plan.version));
	std::fs::write(&file, plan.render(&live)).map_err(|error| format!("{}: {error}", file.display()))?;
	format(workspace, &file)?;
	register_version(&snapshots_directory.join("lib.rs"), plan.version)?;

	println!("Froze format version {} into {}", plan.version, file.display());

	if let Some(previous) = plan.previous {
		let step = Step::new(&plan, previous, &live, &snapshots);
		let step_file = snapshots_directory.join(step_path(plan.version));
		std::fs::create_dir_all(snapshots_directory.join("steps")).map_err(|error| format!("creating the steps module: {error}"))?;
		std::fs::write(&step_file, step.render()).map_err(|error| format!("{}: {error}", step_file.display()))?;
		format(workspace, &step_file)?;
		register_step(&snapshots_directory.join("steps/mod.rs"), plan.version)?;
		set_supported_version(workspace, plan.version)?;

		println!("{}", plan.describe());
		println!(
			"Wrote the format step from {previous} into {}, and set `SUPPORTED_FORMAT_VERSION` to {}",
			step_file.display(),
			plan.version
		);
		println!("Finish the conversions of the changed types where the step's `compile_error!`s point");
		for name in step.unconverted() {
			println!("Warning: `{name}` changed, but format steps don't convert where it's stored");
		}
	}
	Ok(())
}

fn format(workspace: &Path, file: &PathBuf) -> Result<(), String> {
	let status = Command::new("rustfmt")
		.arg("--edition=2024")
		.arg("--config-path")
		.arg(workspace.join("rustfmt.toml"))
		.arg(file)
		.status()
		.map_err(|error| format!("running rustfmt: {error}"))?;

	if !status.success() {
		return Err(format!("rustfmt failed on {}", file.display()));
	}
	Ok(())
}

/// Declares `v<version>` in the snapshots crate and points `latest` at it.
fn register_version(library: &Path, version: u32) -> Result<(), String> {
	let source = std::fs::read_to_string(library).map_err(|error| format!("{}: {error}", library.display()))?;
	let module = format!("pub mod v{version};");
	let latest = format!("pub use v{version} as latest;");

	let mut lines: Vec<String> = source
		.lines()
		.filter(|line| !(line.starts_with("pub use v") && line.ends_with(" as latest;")))
		.map(str::to_string)
		.collect();
	if !lines.contains(&module) {
		let after_modules = lines.iter().rposition(|line| line.starts_with("pub mod v")).map_or(lines.len(), |index| index + 1);
		lines.insert(after_modules, module);
	}
	let after_modules = lines.iter().rposition(|line| line.starts_with("pub mod v")).map_or(lines.len(), |index| index + 1);
	lines.insert(after_modules, latest);

	std::fs::write(library, lines.join("\n") + "\n").map_err(|error| format!("{}: {error}", library.display()))
}

/// The step into `version`, relative to the snapshots crate's source.
fn step_path(version: u32) -> PathBuf {
	PathBuf::from(format!("steps/v{}_to_v{version}.rs", version - 1))
}

/// Declares the step into `version` in the steps module, creating it and declaring it in the crate on the first step.
fn register_step(steps: &Path, version: u32) -> Result<(), String> {
	let module = format!("mod v{}_to_v{version};", version - 1);
	let source = std::fs::read_to_string(steps).unwrap_or_else(|_| "//! One format step per version, each registering itself.\n\n".to_string());
	if !source.lines().any(|line| line == module) {
		std::fs::write(steps, format!("{source}{module}\n")).map_err(|error| format!("{}: {error}", steps.display()))?;
	}

	let Some(directory) = steps.parent().and_then(Path::parent) else { return Ok(()) };
	let library = directory.join("lib.rs");
	let library_source = std::fs::read_to_string(&library).map_err(|error| format!("{}: {error}", library.display()))?;
	if !library_source.lines().any(|line| line == "mod steps;") {
		std::fs::write(&library, format!("{library_source}\nmod steps;\n")).map_err(|error| format!("{}: {error}", library.display()))?;
	}
	Ok(())
}

fn set_supported_version(workspace: &Path, version: u32) -> Result<(), String> {
	let manifest = workspace.join(MANIFEST);
	let source = std::fs::read_to_string(&manifest).map_err(|error| format!("{}: {error}", manifest.display()))?;
	let updated: Vec<String> = source
		.lines()
		.map(|line| match line.strip_prefix(SUPPORTED_VERSION_DECLARATION) {
			Some(_) => format!("{SUPPORTED_VERSION_DECLARATION}{version};"),
			None => line.to_string(),
		})
		.collect();

	std::fs::write(&manifest, updated.join("\n") + "\n").map_err(|error| format!("{}: {error}", manifest.display()))
}

fn supported_version(workspace: &Path) -> Result<u32, String> {
	let manifest = workspace.join(MANIFEST);
	let source = std::fs::read_to_string(&manifest).map_err(|error| format!("{}: {error}", manifest.display()))?;

	source
		.lines()
		.find_map(|line| line.strip_prefix(SUPPORTED_VERSION_DECLARATION))
		.and_then(|rest| rest.trim_end_matches(';').parse().ok())
		.ok_or_else(|| format!("{}: no `{SUPPORTED_VERSION_DECLARATION}<number>;`", manifest.display()))
}
