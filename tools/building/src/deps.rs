use std::io::IsTerminal;
use std::process::Command;

use crate::*;

#[derive(Default, Clone)]
struct Dependency {
	command: &'static str,
	args: &'static [&'static str],
	name: &'static str,
	version: Option<&'static str>,
	install: Option<&'static str>,
	skip: Option<&'static dyn Fn(&Task) -> bool>,
}

fn dependencies(task: &Task) -> Vec<Dependency> {
	[
		Dependency {
			command: "rustc",
			args: &["--version"],
			name: "Rust",
			..Default::default()
		},
		Dependency {
			command: "cargo-about",
			args: &["--version"],
			name: "cargo-about",
			install: Some("cargo install cargo-about"),
			..Default::default()
		},
		Dependency {
			command: "cargo-watch",
			args: &["--version"],
			name: "cargo-watch",
			install: Some("cargo install cargo-watch"),
			skip: Some(&|task| {
				!matches!(
					task,
					Task {
						target: Target::Web,
						action: Action::Run,
						profile: _
					}
				)
			}),
			..Default::default()
		},
		Dependency {
			command: "wasm-bindgen",
			args: &["--version"],
			name: "wasm-bindgen-cli",
			version: Some("0.2.100"),
			install: Some("cargo install -f wasm-bindgen-cli@0.2.100"),
			..Default::default()
		},
		Dependency {
			command: "wasm-pack",
			args: &["--version"],
			name: "wasm-pack",
			install: Some("cargo install wasm-pack"),
			..Default::default()
		},
		Dependency {
			command: "node",
			args: &["--version"],
			name: "Node.js",
			..Default::default()
		},
		Dependency {
			command: "cmake",
			args: &["--version"],
			name: "CMake",
			skip: Some(&|task| !matches!(task.target, Target::Desktop) || cfg!(target_os = "linux")),
			..Default::default()
		},
		Dependency {
			command: "ninja",
			args: &["--version"],
			name: "Ninja",
			skip: Some(&|task| !matches!(task.target, Target::Desktop) || cfg!(target_os = "linux")),
			..Default::default()
		},
	]
	.iter()
	.filter(|d| if let Some(skip) = d.skip { !skip(task) } else { true })
	.cloned()
	.collect()
}

pub fn check(task: &Task) -> Result<(), Error> {
	eprintln!();
	eprintln!("Checking dependencies:");

	let mut installable: Vec<Dependency> = Vec::new();
	let mut failures: Vec<String> = Vec::new();

	for dep in dependencies(task) {
		match Command::new(dep.command).args(dep.args).output() {
			Ok(output) if output.status.success() => {
				let version = String::from_utf8_lossy(&output.stdout);
				let version = version.lines().next().unwrap_or_default().trim();

				if let Some(expected) = dep.version {
					if version.contains(expected) {
						eprintln!(" ✓ {} ({})", dep.name, version);
					} else {
						eprintln!(" ✗ {} (found {}, expected {})", dep.name, version, expected);
						if dep.install.is_some() {
							installable.push(dep);
						} else {
							failures.push(format!("{}: version mismatch (found {version}, expected {expected})", dep.name));
						}
					}
				} else {
					eprintln!(" ✓ {} ({})", dep.name, version);
				}
			}
			Ok(output) => {
				let stderr = String::from_utf8_lossy(&output.stderr);
				eprintln!(" ✗ {} - command failed: {}", dep.name, stderr.trim());
				if dep.install.is_some() {
					installable.push(dep);
				} else {
					failures.push(format!("{}: not installed or not working", dep.name));
				}
			}
			Err(_) => {
				eprintln!(" ✗ {} - not found", dep.name);
				if dep.install.is_some() {
					installable.push(dep);
				} else {
					failures.push(format!("{}: not found in PATH", dep.name));
				}
			}
		}
	}

	eprintln!();

	if installable.is_empty() && failures.is_empty() {
		return Ok(());
	}

	let total = installable.len() + failures.len();
	eprintln!("{total} missing or misconfigured dependenc{}:", if total == 1 { "y" } else { "ies" });
	for dep in &installable {
		eprintln!("  - {}: {}", dep.name, dep.install.unwrap());
	}
	for msg in &failures {
		eprintln!("  - {msg}");
	}

	if !failures.is_empty() {
		eprintln!();
		eprintln!("See: https://graphite.art/volunteer/guide/project-setup/");
	}

	// Don't prompt for automatic installation if we're not interactive session
	if !std::io::stdout().is_terminal() || !std::io::stderr().is_terminal() || !std::io::stdin().is_terminal() {
		return Ok(());
	}

	if installable.is_empty() {
		return Ok(());
	}

	eprintln!();
	eprintln!("The following can be installed automatically:");
	for dep in &installable {
		eprintln!("  {}", dep.install.unwrap());
	}
	eprintln!();
	eprint!("Install them now? [Y/n] ");

	let mut input = String::new();
	std::io::stdin().read_line(&mut input).map_err(|e| Error::Io(e, "Failed to read from stdin".into()))?;
	let input = input.trim();

	if input.is_empty() || input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes") {
		for dep in &installable {
			let parts: Vec<&str> = dep.install.unwrap().split_whitespace().collect();
			eprintln!("Running: {}...", dep.install.unwrap());
			let status = Command::new(parts[0])
				.args(&parts[1..])
				.status()
				.map_err(|e| Error::Io(e, format!("Failed to run '{}'", dep.install.unwrap())))?;
			if !status.success() {
				eprintln!("Failed to install {}", dep.name);
			}
		}
	}
	Ok(())
}
