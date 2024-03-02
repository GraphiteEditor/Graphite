use std::process::Command;

const GRAPHITE_RELEASE_SERIES: &str = "Alpha 3";

fn main() {
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
	let branch = std::env::var("GITHUB_HEAD_REF").unwrap_or_default();
	let branch = if branch.is_empty() { git_command(&["name-rev", "--name-only", "HEAD"]) } else { branch };
	println!("cargo:rustc-env=GRAPHITE_GIT_COMMIT_BRANCH={branch}");
	println!("cargo:rustc-env=GRAPHITE_RELEASE_SERIES={GRAPHITE_RELEASE_SERIES}");
}
