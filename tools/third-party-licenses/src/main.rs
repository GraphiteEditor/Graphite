use std::collections::HashMap;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::process::ExitCode;

mod cargo;
#[cfg(feature = "desktop")]
mod cef;
mod npm;

use crate::cargo::CargoLicenseSource;
#[cfg(feature = "desktop")]
use crate::cef::CefLicenseSource;
use crate::npm::NpmLicenseSource;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("{1}: {0}")]
	Io(#[source] std::io::Error, String),

	#[error("{1}: {0}")]
	Json(#[source] serde_json::Error, String),

	#[error("{1}: {0}")]
	Utf8(#[source] std::string::FromUtf8Error, String),

	#[error("{0}")]
	Command(String),

	#[cfg(feature = "desktop")]
	#[error("Could not find CREDITS.html or CREDITS.html.xz in {0}")]
	CefCreditsNotFound(PathBuf),
}

pub trait LicenceSource: std::hash::Hash {
	fn licenses(&self) -> Result<Vec<LicenseEntry>, Error>;
}

pub struct LicenseEntry {
	name: Option<String>,
	text: String,
	packages: Vec<Package>,
}

pub struct Package {
	name: String,
	authors: Vec<String>,
	url: Option<String>,
}

#[derive(Hash)]
struct Run<'a> {
	output: &'a Vec<u8>,
	cargo: &'a CargoLicenseSource,
	npm: &'a NpmLicenseSource,
	#[cfg(feature = "desktop")]
	cef: &'a CefLicenseSource,
}

fn main() -> ExitCode {
	if let Err(e) = run() {
		eprintln!("Error: {e}");
		return ExitCode::FAILURE;
	}
	ExitCode::SUCCESS
}

fn run() -> Result<(), Error> {
	let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let workspace_dir = PathBuf::from(env!("CARGO_WORKSPACE_DIR"));

	#[cfg(feature = "desktop")]
	let output_path = workspace_dir.join("desktop/third-party-licenses.txt.xz");
	#[cfg(not(feature = "desktop"))]
	let output_path = workspace_dir.join("frontend/third-party-licenses.txt");

	#[cfg(feature = "desktop")]
	let current_hash_path = manifest_dir.join("desktop.hash");
	#[cfg(not(feature = "desktop"))]
	let current_hash_path = manifest_dir.join("web.hash");

	let cargo_source = CargoLicenseSource::new();
	let npm_source = NpmLicenseSource::new(workspace_dir.join("frontend"));
	#[cfg(feature = "desktop")]
	let cef_source = CefLicenseSource::new();

	let mut run = Run {
		cargo: &cargo_source,
		npm: &npm_source,
		#[cfg(feature = "desktop")]
		cef: &cef_source,
		output: &fs::read(&output_path).unwrap_or_default(),
	};

	let mut hasher = DefaultHasher::new();
	run.hash(&mut hasher);
	let current_hash = format!("{:016x}", hasher.finish());

	if current_hash == fs::read_to_string(&current_hash_path).unwrap_or_default() {
		eprintln!("No changes in licenses detected, skipping generation.");
		return Ok(());
	}
	eprintln!("Changes in licenses detected, generating new license file.");

	let licenses = merge_filter_dedup_and_sort(vec![
		cargo_source.licenses()?,
		npm_source.licenses()?,
		#[cfg(feature = "desktop")]
		cef_source.licenses()?,
	]);
	let formatted = format_credits(&licenses);

	#[cfg(feature = "desktop")]
	let output = compress(&formatted)?;
	#[cfg(not(feature = "desktop"))]
	let output = formatted.as_bytes().to_vec();
	if let Some(parent) = output_path.parent() {
		fs::create_dir_all(parent).map_err(|e| Error::Io(e, format!("Failed to create directory {}", parent.display())))?;
	}
	fs::write(&output_path, &output).map_err(|e| Error::Io(e, format!("Failed to write {}", output_path.display())))?;
	run.output = &output;

	let hash = {
		let mut hasher = DefaultHasher::new();
		run.hash(&mut hasher);
		format!("{:016x}", hasher.finish())
	};

	fs::write(&current_hash_path, hash).map_err(|e| Error::Io(e, format!("Failed to write hash file {}", current_hash_path.display())))?;

	Ok(())
}

fn format_credits(licenses: &Vec<LicenseEntry>) -> String {
	let mut out = String::new();

	out.push_str("▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐\n");
	out.push_str("▐▐                                                   ▐▐\n");
	out.push_str("▐▐   GRAPHITE THIRD-PARTY SOFTWARE LICENSE NOTICES   ▐▐\n");
	out.push_str("▐▐                                                   ▐▐\n");
	out.push_str("▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐▐\n");

	for license in licenses {
		let package_lines: Vec<String> = license
			.packages
			.iter()
			.map(|pkg| match &pkg {
				Package { name, authors, url: Some(url) } if !authors.is_empty() => format!("{} - [{}] - {}", name, authors.join(", "), url),
				Package { name, authors: _, url: Some(url) } => format!("{} - {}", name, url),
				Package { name, authors, url: None } if !authors.is_empty() => format!("{} - [{}]", name, authors.join(", ")),
				_ => pkg.name.clone(),
			})
			.collect();

		let multi = package_lines.len() > 1;

		let header = format!(
			"The package{} listed here {} licensed under the terms of the {} printed beneath",
			if multi { "s" } else { "" },
			if multi { "are" } else { "is" },
			if let Some(license) = license.name.as_ref() { license.to_string() } else { "license".to_string() }
		);

		let max_len = std::iter::once(header.len()).chain(package_lines.iter().map(|l| l.chars().count())).max().unwrap_or(0);

		let padded_packages: Vec<String> = package_lines
			.iter()
			.map(|line| {
				let pad = max_len - line.chars().count();
				format!("│ {}{} │", line, " ".repeat(pad))
			})
			.collect();

		out.push_str(&format!("\n {}\n", "_".repeat(max_len + 2)));
		out.push_str(&format!("│ {} │\n", " ".repeat(max_len)));
		out.push_str(&format!("│ {}{} │\n", header, " ".repeat(max_len - header.len())));
		out.push_str(&format!("│{}│\n", "_".repeat(max_len + 2)));
		out.push_str(&padded_packages.join("\n"));
		out.push('\n');
		out.push_str(&format!(" {}", "\u{203e}".repeat(max_len + 2)));
		for line in license.text.lines() {
			if line.is_empty() {
				out.push('\n');
				continue;
			}
			out.push('\n');
			out.push_str("    ");
			out.push_str(line);
		}
		out.truncate(out.trim_end().len());
		out.push('\n');
	}

	out
}

fn merge_filter_dedup_and_sort(sources: Vec<Vec<LicenseEntry>>) -> Vec<LicenseEntry> {
	let mut all = Vec::new();
	for source in sources {
		all.extend(source);
	}
	filter(&mut all);
	let mut all = dedup_by_licence_text(all);
	all.sort_by(|a, b| b.packages.len().cmp(&a.packages.len()).then(a.text.len().cmp(&b.text.len())));
	all
}

fn filter(licenses: &mut Vec<LicenseEntry>) {
	licenses.iter_mut().for_each(|l| {
		l.packages.retain(|p| !(p.authors.len() == 1 && p.authors[0].contains("contact@graphite.art")));
	});
	licenses.retain(|l| !l.packages.is_empty());
}

fn dedup_by_licence_text(vec: Vec<LicenseEntry>) -> Vec<LicenseEntry> {
	let mut map: HashMap<String, LicenseEntry> = HashMap::new();

	for entry in vec {
		match map.entry(entry.text.clone()) {
			std::collections::hash_map::Entry::Occupied(mut e) => {
				e.get_mut().packages.extend(entry.packages);
			}
			std::collections::hash_map::Entry::Vacant(e) => {
				e.insert(entry);
			}
		}
	}

	map.into_values().collect()
}

#[cfg(feature = "desktop")]
fn compress(content: &str) -> Result<Vec<u8>, Error> {
	use std::io::Write;
	let mut buf = Vec::new();
	let mut writer = lzma_rust2::XzWriter::new(&mut buf, lzma_rust2::XzOptions::default()).map_err(|e| Error::Io(e, "Failed to create XZ writer".into()))?;
	writer.write_all(content.as_bytes()).map_err(|e| Error::Io(e, "Failed to write compressed credits".into()))?;
	writer.finish().map_err(|e| Error::Io(e, "Failed to finish XZ compression".into()))?;
	Ok(buf)
}
