use std::io::IsTerminal;
use std::process::Command;

use crate::*;

/// The Binaryen release version that [`install_binaryen`] downloads.
/// NOTICE: keep in sync with the `BINARYEN_VERSION` pinned across the CI workflows.
const BINARYEN_VERSION: &str = "129";
const WASM_OPT_INSTALL: &str = "automatically download Binaryen (wasm-opt) from its official GitHub releases";

#[derive(Default, Clone)]
struct Requirement {
	command: &'static str,
	args: &'static [&'static str],
	name: &'static str,
	/// An exact version which must appear in the version output, for tools pinned to one specific version.
	version: Option<&'static str>,
	/// The command to install the tool, or with `install_action` present, a description of what it will do.
	install: Option<&'static str>,
	/// An installation procedure to run instead of executing `install` as a command.
	install_action: Option<&'static dyn Fn() -> Result<(), Error>>,
	skip: Option<&'static dyn Fn(&Task) -> bool>,
}

fn requirements(task: &Task) -> Vec<Requirement> {
	[
		Requirement {
			command: "rustc",
			args: &["--version"],
			name: "Rust",
			..Default::default()
		},
		Requirement {
			command: "cargo-about",
			args: &["--version"],
			name: "Cargo About",
			install: Some("cargo install cargo-about"),
			skip: Some(&|task| matches!(task.target, Target::Cli)),
			..Default::default()
		},
		Requirement {
			command: "cargo-watch",
			args: &["--version"],
			name: "Cargo Watch",
			install: Some("cargo install cargo-watch"),
			skip: Some(&|task| {
				!matches!(
					task,
					Task {
						target: Target::Web,
						action: Action::Run,
						..
					}
				)
			}),
			..Default::default()
		},
		Requirement {
			command: "wasm-bindgen",
			args: &["--version"],
			name: "Wasm Bindgen",
			version: Some("0.2.121"),
			install: Some("cargo install -f wasm-bindgen-cli@0.2.121"),
			skip: Some(&|task| matches!(task.target, Target::Cli)),
			..Default::default()
		},
		Requirement {
			command: "wasm-opt",
			args: &["--version"],
			name: "Wasm Opt",
			version: Some(BINARYEN_VERSION),
			install: Some(WASM_OPT_INSTALL),
			install_action: Some(&install_binaryen),
			// Only release builds are optimized with wasm-opt
			skip: Some(&|task| {
				matches!(task.target, Target::Cli)
					|| match task.profile {
						Profile::Debug => true,
						Profile::Release => false,
						Profile::Default => matches!(task.action, Action::Run),
					}
			}),
		},
		Requirement {
			command: "node",
			args: &["--version"],
			name: "Node.js",
			skip: Some(&|task| matches!(task.target, Target::Cli)),
			..Default::default()
		},
		Requirement {
			command: "cmake",
			args: &["--version"],
			name: "CMake",
			skip: Some(&|task| !matches!(task.target, Target::Desktop) || cfg!(target_os = "linux")),
			..Default::default()
		},
		Requirement {
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
	eprintln!("Checking Requirements:");

	let mut installable: Vec<Requirement> = Vec::new();
	let mut failures: Vec<String> = Vec::new();

	for dep in requirements(task) {
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
	eprintln!("{total} requirement{} not met:", if total > 1 { "s" } else { "" });
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
	if installable.len() == 1 {
		eprint!("Install it now? [Y/n] ");
	} else {
		eprint!("Install them now? [Y/n] ");
	}

	let mut input = String::new();
	std::io::stdin().read_line(&mut input).map_err(|e| Error::Io(e, "Failed to read from stdin".into()))?;
	let input = input.trim();

	if input.is_empty() || input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes") {
		for dep in &installable {
			eprintln!("Running: {}...", dep.install.unwrap());

			if let Some(action) = dep.install_action {
				if let Err(e) = action() {
					eprintln!("{e}");
					eprintln!("Failed to install {}", dep.name);
				}
				continue;
			}

			let parts: Vec<&str> = dep.install.unwrap().split_whitespace().collect();
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

/// Downloads the pinned Binaryen release into the workspace's target directory and puts its tools on this process's PATH.
/// Windows, Mac, and Linux all ship with `curl` and `tar`, so no package manager is needed.
fn install_binaryen() -> Result<(), Error> {
	let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
		("windows", "x86_64") => "x86_64-windows",
		("macos", "aarch64") => "arm64-macos",
		("macos", "x86_64") => "x86_64-macos",
		("linux", "x86_64") => "x86_64-linux",
		("linux", "aarch64") => "aarch64-linux",
		(os, arch) => {
			let error = std::io::Error::other(format!("no official Binaryen release exists for {os} on {arch}"));
			return Err(Error::Io(error, "Failed to download Binaryen".into()));
		}
	};

	let target_dir = wasm::target_dir();
	std::fs::create_dir_all(&target_dir).map_err(|e| Error::Io(e, format!("Failed to create directory '{}'", target_dir.display())))?;

	let url = format!("https://github.com/WebAssembly/binaryen/releases/download/version_{BINARYEN_VERSION}/binaryen-version_{BINARYEN_VERSION}-{platform}.tar.gz");
	let tarball = target_dir.join("binaryen.tar.gz");

	let mut download = Command::new("curl");
	download.args(["-sSfL", &url, "-o"]).arg(&tarball);
	wasm::run_command(download)?;

	let mut extract = Command::new("tar");
	extract.arg("-xzf").arg(&tarball).arg("-C").arg(&target_dir);
	wasm::run_command(extract)?;

	let _ = std::fs::remove_file(&tarball);

	use_managed_binaryen();
	Ok(())
}

/// Prepends the managed Binaryen installation (if present) to this process's PATH, which child processes inherit.
/// Prepending lets the pinned version win over any other installed wasm-opt.
pub fn use_managed_binaryen() {
	let bin_dir = wasm::target_dir().join(format!("binaryen-version_{BINARYEN_VERSION}")).join("bin");
	if !bin_dir.is_dir() {
		return;
	}

	let mut paths = vec![bin_dir];
	if let Some(path) = std::env::var_os("PATH") {
		paths.extend(std::env::split_paths(&path));
	}
	if let Ok(joined) = std::env::join_paths(paths) {
		// Safety: this runs before any other threads are spawned
		unsafe { std::env::set_var("PATH", joined) };
	}
}
