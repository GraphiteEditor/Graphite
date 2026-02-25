use crate::{LicenceSource, LicenseEntry, Package};
use serde::Deserialize;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::{self, Command};

pub struct CargoLicenseSource {}

impl CargoLicenseSource {
	pub fn new() -> Self {
		Self {}
	}
}

impl LicenceSource for CargoLicenseSource {
	fn licenses(&self) -> Vec<LicenseEntry> {
		parse(run())
	}
}

impl Hash for CargoLicenseSource {
	fn hash<H: Hasher>(&self, state: &mut H) {
		let lock_path = PathBuf::from(env!("CARGO_WORKSPACE_DIR")).join("Cargo.lock");
		fs::read_to_string(lock_path).unwrap().hash(state)
	}
}

#[derive(Deserialize)]
struct Output {
	licenses: Vec<License>,
}

#[derive(Deserialize)]
struct License {
	name: Option<String>,
	text: Option<String>,
	used_by: Vec<UsedBy>,
}

#[derive(Deserialize)]
struct UsedBy {
	#[serde(rename = "crate")]
	crate_info: Crate,
}

#[derive(Deserialize)]
struct Crate {
	name: Option<String>,
	version: Option<String>,
	authors: Option<Vec<String>>,
	repository: Option<String>,
}

fn parse(parsed: Output) -> Vec<LicenseEntry> {
	parsed
		.licenses
		.into_iter()
		.map(|license| {
			let packages = license
				.used_by
				.into_iter()
				.map(|used| {
					let name = used.crate_info.name.as_deref().unwrap_or_default();
					let version = used.crate_info.version.as_deref().unwrap_or_default();
					let display_name = if version.is_empty() { name.to_string() } else { format!("{name}@{version}") };

					let repository = used.crate_info.repository.filter(|s| !s.is_empty());

					Package {
						name: display_name,
						authors: used.crate_info.authors.unwrap_or_default(),
						url: repository,
					}
				})
				.collect();

			LicenseEntry {
				name: license.name,
				text: license.text.as_deref().unwrap_or_default().to_string(),
				packages,
			}
		})
		.collect()
}

fn run() -> Output {
	let output = Command::new("cargo")
		.args(["about", "generate", "--format", "json", "--frozen"])
		.current_dir(env!("CARGO_WORKSPACE_DIR"))
		.output()
		.unwrap_or_else(|e| {
			eprintln!("Failed to run cargo about generate: {e}");
			process::exit(1)
		});

	if !output.status.success() {
		eprintln!("cargo about generate failed:\n{}", String::from_utf8_lossy(&output.stderr));
		process::exit(1)
	}

	serde_json::from_str(&String::from_utf8(output.stdout).expect("cargo about generate should return valid UTF-8")).unwrap_or_else(|e| {
		eprintln!("Failed to parse cargo about generate JSON: {e}");
		process::exit(1)
	})
}
