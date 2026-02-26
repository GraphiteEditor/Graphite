use std::process::Command;

struct Dependency {
	command: &'static str,
	args: &'static [&'static str],
	name: &'static str,
	expected_version: Option<&'static str>,
}

const DEPENDENCIES: &[Dependency] = &[
	Dependency {
		command: "rustc",
		args: &["--version"],
		name: "Rust",
		expected_version: None,
	},
	Dependency {
		command: "cargo-about",
		args: &["--version"],
		name: "cargo-about",
		expected_version: None,
	},
	Dependency {
		command: "cargo-watch",
		args: &["--version"],
		name: "cargo-watch",
		expected_version: None,
	},
	Dependency {
		command: "wasm-bindgen",
		args: &["--version"],
		name: "wasm-bindgen-cli",
		expected_version: Some("0.2.100"),
	},
	Dependency {
		command: "wasm-pack",
		args: &["--version"],
		name: "wasm-pack",
		expected_version: None,
	},
	Dependency {
		command: "node",
		args: &["--version"],
		name: "Node.js",
		expected_version: None,
	},
];

pub fn check_dependencies() {
	let mut failures = Vec::new();

	for dep in DEPENDENCIES {
		match Command::new(dep.command).args(dep.args).output() {
			Ok(output) if output.status.success() => {
				let version_output = String::from_utf8_lossy(&output.stdout);
				let version = version_output.trim();

				if let Some(expected) = dep.expected_version {
					if version.contains(expected) {
						eprintln!("  ✓ {} ({})", dep.name, version);
					} else {
						eprintln!("  ✗ {} (found {}, expected {})", dep.name, version, expected);
						failures.push(format!("{}: version mismatch (found {}, expected {})", dep.name, version, expected));
					}
				} else {
					eprintln!("  ✓ {} ({})", dep.name, version);
				}
			}
			Ok(output) => {
				let stderr = String::from_utf8_lossy(&output.stderr);
				eprintln!("  ✗ {} — command failed: {}", dep.name, stderr.trim());
				failures.push(format!("{}: not installed or not working", dep.name));
			}
			Err(_) => {
				eprintln!("  ✗ {} — not found", dep.name);
				failures.push(format!("{}: not found in PATH", dep.name));
			}
		}
	}

	eprintln!();

	if failures.is_empty() {
		eprintln!("All dependencies are installed.");
	} else {
		eprintln!("{} missing or misconfigured dependenc{}:", failures.len(), if failures.len() == 1 { "y" } else { "ies" });
		for failure in &failures {
			eprintln!("  - {failure}");
		}
		eprintln!();
		eprintln!("See: https://graphite.rs/volunteer/guide/project-setup/");
	}
}
