//! Freezes the stored document types (`graph-storage`'s `schema` module) into `migration-formats` as one format version,
//! and checks that the live types still match the latest frozen version.
//!
//! A new version declares again only the types that changed, plus every type reaching one, and re-exports the rest from
//! the previous version. So format steps convert only what changed, and old versions never depend on the live crate.

mod plan;
mod types;

use plan::{Plan, Snapshots};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
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
	println!("  snapshot    Freeze the stored types as the next format version");
	println!();
}

fn main() -> ExitCode {
	let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
	let workspace = workspace.canonicalize().unwrap_or(workspace);

	let result = match std::env::args().nth(1).as_deref() {
		Some("check") => check(&workspace),
		Some("snapshot") => snapshot(&workspace),
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
			"the stored types changed since format version {latest}:\n{}\nRun `cargo run migration snapshot` to freeze them as version {}, then bump `SUPPORTED_FORMAT_VERSION` and add a format step from {latest}",
			plan.describe(),
			plan.version
		));
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
	let plan = Plan::new(&live, &Snapshots::read(&snapshots_directory)?);

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
	if plan.previous.is_some() {
		println!("{}", plan.describe());
		println!("Next, set `SUPPORTED_FORMAT_VERSION` to {} and add a format step from {}", plan.version, plan.version - 1);
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

fn supported_version(workspace: &Path) -> Result<u32, String> {
	let manifest = workspace.join(MANIFEST);
	let source = std::fs::read_to_string(&manifest).map_err(|error| format!("{}: {error}", manifest.display()))?;

	source
		.lines()
		.find_map(|line| line.strip_prefix(SUPPORTED_VERSION_DECLARATION))
		.and_then(|rest| rest.trim_end_matches(';').parse().ok())
		.ok_or_else(|| format!("{}: no `{SUPPORTED_VERSION_DECLARATION}<number>;`", manifest.display()))
}
