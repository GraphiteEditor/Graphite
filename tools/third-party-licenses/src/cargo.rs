use crate::{Error, LicenceSource, LicenseEntry, Package};
use serde::Deserialize;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;

pub struct CargoLicenseSource {}

impl CargoLicenseSource {
	pub fn new() -> Self {
		Self {}
	}
}

impl LicenceSource for CargoLicenseSource {
	fn licenses(&self) -> Result<Vec<LicenseEntry>, Error> {
		Ok(parse(run()?))
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

fn run() -> Result<Output, Error> {
	// Try normal stdout capture first
	let output = Command::new("cargo")
		.args(["about", "generate", "--format", "json", "--frozen"])
		.current_dir(env!("CARGO_WORKSPACE_DIR"))
		.output()
		.map_err(|e| Error::Io(e, "Failed to run cargo about generate".into()))?;

	// On Windows, if cargo-about fails (often due to PowerShell detection in process ancestry),
	// fall back to using a temporary file to work around the issue.
	// TODO: Add an option to cargo-about to disable the PowerShell check (see issue: https://discord.com/channels/731730685944922173/731738914812854303/1479960786871779459)
	#[cfg(target_os = "windows")]
	if !output.status.success() {
		eprintln!("cargo-about failed with stdout capture, retrying with temporary file...");

		let temp_file = PathBuf::from(env!("CARGO_WORKSPACE_DIR")).join(".cargo-about-temp.json");

		let status = Command::new("cargo")
			.args(["about", "generate", "--format", "json", "--frozen", "--output-file"])
			.arg(&temp_file)
			.current_dir(env!("CARGO_WORKSPACE_DIR"))
			.status()
			.map_err(|e| Error::Io(e, "Failed to run cargo about generate with temp file".into()))?;

		if !status.success() {
			return Err(Error::Command(format!(
				"cargo about generate failed:\nOriginal error: {}\nTemp file error: command exited with non-zero status",
				String::from_utf8_lossy(&output.stderr)
			)));
		}

		let json_content = fs::read_to_string(&temp_file).map_err(|e| Error::Io(e, format!("Failed to read cargo about output from {}", temp_file.display())))?;

		// Clean up temp file
		let _ = fs::remove_file(&temp_file);

		return serde_json::from_str(&json_content).map_err(|e| Error::Json(e, "Failed to parse cargo about generate JSON from temp file".into()));
	}

	// Handle other error cases
	if !output.status.success() {
		return Err(Error::Command(format!("cargo about generate failed:\n{}", String::from_utf8_lossy(&output.stderr))));
	}

	let stdout = String::from_utf8(output.stdout).map_err(|e| Error::Utf8(e, "cargo about generate returned invalid UTF-8".into()))?;

	serde_json::from_str(&stdout).map_err(|e| Error::Json(e, "Failed to parse cargo about generate JSON".into()))
}
