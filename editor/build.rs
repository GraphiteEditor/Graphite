use std::env;
use std::process::Command;

const GRAPHITE_RELEASE_SERIES: &str = "Alpha 4";

fn main() {
	// Instruct Cargo to rerun this build script if any of these environment variables change.
	println!("cargo:rerun-if-env-changed=GRAPHITE_GIT_COMMIT_DATE");
	println!("cargo:rerun-if-env-changed=GRAPHITE_GIT_COMMIT_HASH");
	println!("cargo:rerun-if-env-changed=GRAPHITE_GIT_COMMIT_BRANCH");
	println!("cargo:rerun-if-env-changed=GITHUB_HEAD_REF");

	// Try to get the commit information from the environment (e.g. set by CI), otherwise fall back to Git commands.
	let commit_date = env_or_else("GRAPHITE_GIT_COMMIT_DATE", || git_or_unknown(&["log", "-1", "--format=%cI"]));
	let commit_hash = env_or_else("GRAPHITE_GIT_COMMIT_HASH", || git_or_unknown(&["rev-parse", "HEAD"]));
	let commit_branch = env_or_else("GRAPHITE_GIT_COMMIT_BRANCH", || {
		let gh = env::var("GITHUB_HEAD_REF").unwrap_or_default();
		if !gh.trim().is_empty() {
			gh.trim().to_string()
		} else {
			git_or_unknown(&["rev-parse", "--abbrev-ref", "HEAD"])
		}
	});

	// Instruct Cargo to set environment variables for compile time.
	// They are accessed with the `env!("GRAPHITE_*")` macro in the codebase.
	println!("cargo:rustc-env=GRAPHITE_GIT_COMMIT_DATE={commit_date}");
	println!("cargo:rustc-env=GRAPHITE_GIT_COMMIT_HASH={commit_hash}");
	println!("cargo:rustc-env=GRAPHITE_GIT_COMMIT_BRANCH={commit_branch}");
	println!("cargo:rustc-env=GRAPHITE_RELEASE_SERIES={GRAPHITE_RELEASE_SERIES}");
}

/// Get an environment variable, or if it is not set or empty, use the provided fallback function. Returns a string with trimmed whitespace.
fn env_or_else(key: &str, fallback: impl FnOnce() -> String) -> String {
	match env::var(key) {
		Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
		_ => fallback().trim().to_string(),
	}
}

/// Execute a Git command to obtain its output. Return "unknown" if it fails for any of the possible reasons.
fn git_or_unknown(args: &[&str]) -> String {
	git(args).unwrap_or_else(|| "unknown".to_string())
}

/// Run a git command and capture trimmed stdout.
/// Returns None if git is missing, exits with error, or stdout is empty/non-UTF8.
fn git(args: &[&str]) -> Option<String> {
	let output = Command::new("git").args(args).output().ok()?;
	if !output.status.success() {
		return None;
	}
	let s = String::from_utf8(output.stdout).ok()?;
	let t = s.trim();
	if t.is_empty() { None } else { Some(t.to_string()) }
}
