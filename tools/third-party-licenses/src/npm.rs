use serde::Deserialize;
use sha256::TrySha256Digest;
use std::{
	collections::BTreeMap,
	fs,
	path::PathBuf,
	process::{self, Command},
};

use crate::{LicenceSource, LicenseEntry, Package};

pub struct NpmLicenseSource {
	dir: PathBuf,
}
impl NpmLicenseSource {
	pub fn new(dir: PathBuf) -> Self {
		Self { dir }
	}
}

impl LicenceSource for NpmLicenseSource {
	fn licenses(&self) -> Vec<LicenseEntry> {
		parse(&run(&self.dir))
	}
	fn hash(&self) -> String {
		let lock_path = self.dir.join("package-lock.json");
		lock_path.digest().unwrap_or_else(|e| {
			eprintln!("Failed to hash npm package-lock.json: {e}");
			process::exit(1);
		})
	}
}

#[derive(Deserialize)]
struct NpmEntry {
	licenses: Option<String>,
	repository: Option<String>,
	#[serde(rename = "licenseFile")]
	license_file: Option<String>,
	publisher: Option<String>,
	email: Option<String>,
}

pub fn run(dir: &std::path::Path) -> String {
	#[cfg(not(target_os = "windows"))]
	let mut cmd = Command::new("npx");
	#[cfg(target_os = "windows")]
	let mut cmd = Command::new("npx.cmd");
	cmd.args(["license-checker-rseidelsohn", "--production", "--json"]);
	cmd.current_dir(dir);

	let output = cmd.output().unwrap_or_else(|e| {
		eprintln!("Failed to run npx license-checker-rseidelsohn: {e}");
		process::exit(1);
	});

	if !output.status.success() {
		eprintln!("npx license-checker-rseidelsohn failed:\n{}", String::from_utf8_lossy(&output.stderr));
		process::exit(1);
	}

	String::from_utf8(output.stdout).expect("Invalid UTF-8 from license-checker")
}

pub fn parse(json_str: &str) -> Vec<LicenseEntry> {
	let entries: BTreeMap<String, NpmEntry> = serde_json::from_str(json_str).unwrap_or_else(|e| {
		eprintln!("Failed to parse license-checker JSON: {e}");
		process::exit(1);
	});

	entries
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
