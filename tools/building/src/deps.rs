use std::io::IsTerminal;
use std::process::Command;

struct Dependency {
	command: &'static str,
	args: &'static [&'static str],
	name: &'static str,
	version: Option<&'static str>,
	install: Option<&'static str>,
}

const DEPENDENCIES: &[Dependency] = &[
	Dependency {
		command: "rustc",
		args: &["--version"],
		name: "Rust",
		version: None,
		install: None,
	},
	Dependency {
		command: "cargo-about",
		args: &["--version"],
		name: "cargo-about",
		version: None,
		install: Some("cargo install cargo-about"),
	},
	Dependency {
		command: "cargo-watch",
		args: &["--version"],
		name: "cargo-watch",
		version: None,
		install: Some("cargo install cargo-watch"),
	},
	Dependency {
		command: "wasm-bindgen",
		args: &["--version"],
		name: "wasm-bindgen-cli",
		version: Some("0.2.100"),
		install: Some("cargo install -f wasm-bindgen-cli@0.2.100"),
	},
	Dependency {
		command: "wasm-pack",
		args: &["--version"],
		name: "wasm-pack",
		version: None,
		install: Some("cargo install wasm-pack"),
	},
	Dependency {
		command: "node",
		args: &["--version"],
		name: "Node.js",
		version: None,
		install: None,
	},
];

const DESKTOP_DEPENDENCIES: &[Dependency] = &[
	Dependency {
		command: "cmake",
		args: &["--version"],
		name: "CMake",
		version: None,
		install: None,
	},
	#[cfg(target_os = "windows")]
	Dependency {
		command: "ninja",
		args: &["--version"],
		name: "Ninja",
		version: None,
		install: None,
	},
];

pub fn check(desktop: bool) {
	eprintln!();
	eprintln!("Checking dependencies:");

	let mut installable: Vec<&Dependency> = Vec::new();
	let mut failures: Vec<String> = Vec::new();

	let deps: Box<dyn Iterator<Item = &Dependency>> = if desktop {
		Box::new(DEPENDENCIES.iter().chain(DESKTOP_DEPENDENCIES.iter()))
	} else {
		Box::new(DEPENDENCIES.iter())
	};

	for dep in deps {
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
		return;
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

	if !installable.is_empty() && std::io::stdout().is_terminal() {
		eprintln!();
		eprintln!("The following can be installed automatically:");
		for dep in &installable {
			eprintln!("  {}", dep.install.unwrap());
		}
		eprintln!();
		eprint!("Install them now? [Y/n] ");

		let mut input = String::new();
		std::io::stdin().read_line(&mut input).unwrap();
		let input = input.trim();

		if input.is_empty() || input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes") {
			for dep in &installable {
				let parts: Vec<&str> = dep.install.unwrap().split_whitespace().collect();
				eprintln!("Running: {}...", dep.install.unwrap());
				let status = Command::new(parts[0])
					.args(&parts[1..])
					.status()
					.unwrap_or_else(|e| panic!("Failed to run '{}': {e}", dep.install.unwrap()));
				if !status.success() {
					eprintln!("Failed to install {}", dep.name);
				}
			}
		}
	}
}
