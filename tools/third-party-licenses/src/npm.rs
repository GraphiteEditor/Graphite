use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::{Error, LicenceSource, LicenseEntry, Package};

pub struct NpmLicenseSource {
	dir: PathBuf,
}
impl NpmLicenseSource {
	pub fn new(dir: PathBuf) -> Self {
		Self { dir }
	}
}

impl LicenceSource for NpmLicenseSource {
	fn licenses(&self) -> Result<Vec<LicenseEntry>, Error> {
		Ok(parse(run(&self.dir)?))
	}
}

impl std::hash::Hash for NpmLicenseSource {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		let lock_path = self.dir.join("package-lock.json");
		fs::read_to_string(lock_path).unwrap().hash(state)
	}
}

type Output = HashMap<String, NpmEntry>;

#[derive(serde::Deserialize)]
struct NpmEntry {
	licenses: Option<String>,
	repository: Option<String>,
	#[serde(rename = "licenseFile")]
	license_file: Option<String>,
	publisher: Option<String>,
	email: Option<String>,
}

fn parse(parsed: Output) -> Vec<LicenseEntry> {
	parsed
		.iter()
		.map(|(name, entry)| {
			let publisher_info = entry.publisher.as_ref().map(|p| {
				let email_part = entry.email.as_ref().map(|e| format!(" <{}>", e)).unwrap_or_default();
				format!("{}{}", p, email_part)
			});

			let pkg = Package {
				name: name.to_string(),
				url: entry.repository.clone(),
				authors: publisher_info.into_iter().collect(),
			};

			let license_text = entry.license_file.as_ref().and_then(|p| fs::read_to_string(p).ok()).map(|s| s.to_string()).unwrap_or_default();

			LicenseEntry {
				name: entry.licenses.clone(),
				text: license_text,
				packages: vec![pkg],
			}
		})
		.collect()
}

fn run(dir: &std::path::Path) -> Result<Output, Error> {
	#[cfg(not(target_os = "windows"))]
	let mut cmd = Command::new("npx");
	#[cfg(target_os = "windows")]
	let mut cmd = Command::new("npx.cmd");
	cmd.args(["license-checker-rseidelsohn", "--production", "--json"]);
	cmd.current_dir(dir);

	let output = cmd.output().map_err(|e| Error::Io(e, "Failed to run npx license-checker-rseidelsohn".into()))?;

	if !output.status.success() {
		return Err(Error::Command(format!("npx license-checker-rseidelsohn failed:\n{}", String::from_utf8_lossy(&output.stderr))));
	}

	let json_str = String::from_utf8(output.stdout).map_err(|e| Error::Utf8(e, "Invalid UTF-8 from license-checker".into()))?;

	serde_json::from_str(&json_str).map_err(|e| Error::Json(e, "Failed to parse license-checker JSON".into()))
}
