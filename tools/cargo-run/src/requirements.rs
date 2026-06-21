use semver::{Version, VersionReq};
use std::io::IsTerminal;

use crate::cmd::prelude::*;
use crate::*;

mod wasm_opt;

#[derive(Default, Clone)]
pub struct Requirement {
	command: &'static str,
	args: &'static [&'static str],
	name: &'static str,
	check: Check,
	version: Option<&'static str>,
	install: InstallAction,
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
			command: "rustc",
			args: &["--print", "target-libdir", "--target", "wasm32-unknown-unknown"],
			check: Check::Matches(&|out| std::path::Path::new(out.trim()).is_dir()),
			name: "Rust Wasm Toolchain",
			install: "rustup target add wasm32-unknown-unknown".into(),
			skip: Some(&|task| matches!(task.target, Target::Cli)),
			..Default::default()
		},
		Requirement {
			command: "cargo-about",
			args: &["--version"],
			name: "Cargo About",
			install: "cargo install cargo-about".into(),
			skip: Some(&|task| matches!(task.target, Target::Cli)),
			..Default::default()
		},
		Requirement {
			command: "wasm-opt",
			args: &["--version"],
			name: "Wasm Opt",
			version: Some(">=130"),
			install: wasm_opt::install_action(),
			skip: Some(&|task| {
				matches!(task.target, Target::Cli)
					|| match task.profile {
						Profile::Debug => true,
						Profile::Release => false,
						Profile::Default => matches!(task.action, Action::Run),
					}
			}),
			..Default::default()
		},
		Requirement {
			command: "wasm-bindgen",
			args: &["--version"],
			name: "Wasm Bindgen",
			version: Some("=0.2.121"),
			install: "cargo install -f wasm-bindgen-cli@0.2.121".into(),
			skip: Some(&|task| matches!(task.target, Target::Cli)),
			..Default::default()
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
		match cmd(dep.command, dep.args.iter().copied()).output_unchecked() {
			Ok(output) if output.status.success() => {
				let stdout = String::from_utf8_lossy(&output.stdout);
				match dep.check {
					Check::PrintsVersion => {
						let line = stdout.lines().next().unwrap_or_default().trim();
						match dep.version {
							None => eprintln!(" ✓ {} ({line})", dep.name),
							Some(req_str) => {
								let req = VersionReq::parse(req_str).expect("invalid semver requirement");
								match extract_version(line) {
									Some(version) if req.matches(&version) => eprintln!(" ✓ {} ({version})", dep.name),
									Some(version) => {
										eprintln!(" ✗ {} (found {version}, requires {req_str})", dep.name);
										if dep.install.is_some() {
											installable.push(dep);
										} else {
											failures.push(format!("{}: version mismatch (found {version}, requires {req_str})", dep.name));
										}
									}
									None => {
										eprintln!(" ✗ {} (could not parse version from '{line}')", dep.name);
										failures.push(format!("{}: could not parse version from '{line}'", dep.name));
									}
								}
							}
						}
					}
					Check::Matches(check) => {
						if !check(stdout.to_string()) {
							eprintln!(" ✗ {} - check failed", dep.name);
							if dep.install.is_some() {
								installable.push(dep);
							}
						} else {
							eprintln!(" ✓ {}", dep.name);
						}
					}
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
		eprintln!("  - {}", dep.name);
	}
	for msg in &failures {
		eprintln!("  - {msg}");
	}

	if !failures.is_empty() {
		eprintln!();
		eprintln!("See: https://graphite.art/volunteer/guide/project-setup/");
	}

	let is_interactive = std::io::stdout().is_terminal() && std::io::stderr().is_terminal() && std::io::stdin().is_terminal();

	// Don't prompt for automatic installation if we're not interactive session
	if !is_interactive {
		return Ok(());
	}

	if installable.is_empty() && failures.is_empty() {
		return Ok(());
	}

	if !installable.is_empty() {
		eprintln!();
		eprintln!("The following can be resolved automatically:");
		for dep in &installable {
			match &dep.install {
				InstallAction::Command(cmd) => eprintln!("  - {}: {}", dep.name, cmd),
				InstallAction::Expression { description, .. } => eprintln!("  - {description}"),
				InstallAction::None => unreachable!(),
			}
		}
		eprintln!();
		if installable.len() == 1 {
			eprint!("Install it now? [y/N] ");
		} else {
			eprint!("Install them now? [y/N] ");
		}

		let mut input = String::new();
		std::io::stdin().read_line(&mut input).map_err(|e| Error::Io(e, "Failed to read from stdin".into()))?;
		let input = input.trim();

		if input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes") {
			for dep in installable.into_iter() {
				eprintln!("Installing {}...", dep.name);
				match &dep.install {
					InstallAction::Command(install_cmd) => {
						let parts: Vec<&str> = install_cmd.split_whitespace().collect();
						let expr = cmd(parts[0], parts[1..].iter().copied()).unchecked();
						match Expression::run(&expr) {
							Ok(output) if !output.status.success() => {
								let stderr = String::from_utf8_lossy(&output.stderr);
								failures.push(format!("{} installation command failed: {}", dep.name, stderr.trim()));
							}
							Err(e) => return Err(Error::Command(e)),
							_ => {}
						}
					}
					InstallAction::Expression { expression, .. } => {
						if let Err(e) = expression.clone().run() {
							failures.push(format!("{}: failed to install ({e})", dep.name));
							eprintln!("{e}");
							eprintln!("Failed to install {}", dep.name);
						}
					}
					InstallAction::None => unreachable!(),
				}
			}
		}
	}

	if !failures.is_empty() {
		eprintln!();
		eprintln!("The following requirements must be resolved manually:");
		for msg in &failures {
			eprintln!("  - {msg}");
		}
	}

	if (!failures.is_empty()) && is_interactive {
		eprintln!();
		eprintln!("Continue without resolving these requirements? [y/N]");

		let mut input = String::new();
		std::io::stdin().read_line(&mut input).map_err(|e| Error::Io(e, "Failed to read from stdin".into()))?;
		let input = input.trim();

		if !input.eq_ignore_ascii_case("y") && !input.eq_ignore_ascii_case("yes") {
			return Err(Error::RequirementsNotMet);
		}
	}
	Ok(())
}

fn extract_version(line: &str) -> Option<Version> {
	line.split_whitespace().find_map(|token| {
		let token = token.trim_start_matches('v').trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
		if token.is_empty() {
			return None;
		}
		if let Ok(version) = Version::parse(token) {
			return Some(version);
		}
		let (core, suffix) = match token.find(['-', '+']) {
			Some(i) => token.split_at(i),
			None => (token, ""),
		};
		let parts: Vec<&str> = core.split('.').collect();
		if parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
			return None;
		}
		let major = parts[0];
		let minor = parts.get(1).copied().unwrap_or("0");
		let patch = parts.get(2).copied().unwrap_or("0");
		Version::parse(&format!("{major}.{minor}.{patch}{suffix}")).ok()
	})
}

#[derive(Clone, Default)]
enum Check {
	#[default]
	PrintsVersion,
	Matches(&'static dyn Fn(String) -> bool),
}

#[derive(Clone, Default)]
enum InstallAction {
	#[default]
	None,
	Command(&'static str),
	Expression {
		description: String,
		expression: Expression,
	},
}

impl InstallAction {
	fn is_some(&self) -> bool {
		!matches!(self, InstallAction::None)
	}
}

impl From<&'static str> for InstallAction {
	fn from(value: &'static str) -> Self {
		InstallAction::Command(value)
	}
}
