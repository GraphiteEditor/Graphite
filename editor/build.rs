use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

const GRAPHITE_RELEASE_SERIES: &str = "Alpha Milestone 1";

fn set_version_from_git() -> Result<(), Box<dyn Error>> {
	// Execute a Git command for its stdout. Early exit if it fails for any of the possible reasons.
	let try_git_command = |args: &[&str]| -> Option<String> {
		let git_output = Command::new("git").args(args).output().ok()?;
		let maybe_empty = String::from_utf8(git_output.stdout).ok()?;
		let command_result = (!maybe_empty.is_empty()).then_some(maybe_empty)?;
		Some(command_result)
	};
	// Execute a Git command for its output. Return "unknown" if it fails for any of the possible reasons.
	let git_command = |args| -> String { try_git_command(args).unwrap_or_else(|| String::from("unknown")) };

	// Rather than printing to any terminal, these commands set environment variables in the Cargo toolchain.
	// They are accessed with the `env!("...")` macro in the codebase.
	println!("cargo:rustc-env=GRAPHITE_GIT_COMMIT_DATE={}", git_command(&["log", "-1", "--format=%cd"]));
	println!("cargo:rustc-env=GRAPHITE_GIT_COMMIT_HASH={}", git_command(&["rev-parse", "HEAD"]));
	println!("cargo:rustc-env=GRAPHITE_GIT_COMMIT_BRANCH={}", git_command(&["rev-parse", "--abbrev-ref", "HEAD"]));
	println!("cargo:rustc-env=GRAPHITE_RELEASE_SERIES={}", GRAPHITE_RELEASE_SERIES);
	Ok(())
}

/// generate types.d.ts
#[allow(unused)]
fn generate_typescript() -> Result<(), Box<dyn Error>> {
	let cwd = Path::new(std::env!("CARGO_MANIFEST_DIR"));
	let f = File::open(cwd.join("types.d.ts"))?;
	let mut command_build = Command::new("cargo");
 	let command_build = command_build
		.current_dir(cwd)
		.args(["run", "--example", "trace_types"])
		.stdout(Stdio::from(f));
	let mut child = command_build.spawn()?;
	assert_eq!(child.wait()?.code(), Some(0));
	Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
	// disabled due to too slow
	// generate_typescript()?;
	set_version_from_git()?;
	Ok(())
}
