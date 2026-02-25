use lzma_rust2::{XzOptions, XzWriter};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::{fs, process};

mod cargo;
mod cef;
mod npm;

use crate::cargo::CargoLicenseSource;
use crate::cef::CefLicenseSource;
use crate::npm::NpmLicenseSource;

pub trait LicenceSource: std::hash::Hash {
	fn licenses(&self) -> Vec<LicenseEntry>;
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
struct RunHashes<'a> {
	output: &'a Vec<u8>,
	cargo: &'a CargoLicenseSource,
	npm: &'a NpmLicenseSource,
	cef: &'a Option<CefLicenseSource>,
}

fn main() {
	let web = std::env::args().any(|arg| arg == "--web");

	let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let workspace_dir = PathBuf::from(env!("CARGO_WORKSPACE_DIR"));
	let output_path = if web {
		workspace_dir.join("frontend/third-party-licenses.txt")
	} else {
		workspace_dir.join("desktop/third-party-licenses.txt.xz")
	};
	let current_hash_path = manifest_dir.join(if web { "web.hash" } else { "desktop.hash" });

	let cargo_source = CargoLicenseSource::new();
	let npm_source = NpmLicenseSource::new(workspace_dir.join("frontend"));
	let cef_source = if web { None } else { Some(CefLicenseSource::new()) };

	let mut run = RunHashes {
		cargo: &cargo_source,
		npm: &npm_source,
		cef: &cef_source,
		output: &fs::read(&output_path).unwrap_or_default(),
	};

	let mut hasher = DefaultHasher::new();
	run.hash(&mut hasher);
	let current_hash = hasher.finish().to_string();

	if current_hash == fs::read_to_string(&current_hash_path).unwrap_or_default() {
		eprintln!("No changes in licenses detected, skipping generation.");
		return;
	}
	eprintln!("Changes in licenses detected, generating new license file.");

	let mut sources = vec![cargo_source.licenses(), npm_source.licenses()];
	if let Some(cef_source) = cef_source.as_ref() {
		sources.push(cef_source.licenses());
	}
	let credits = merge_filter_dedup_and_sort(sources);

	let formatted = format_credits(&credits);

	let output = if web {
		if let Some(parent) = output_path.parent() {
			fs::create_dir_all(parent).unwrap_or_else(|e| {
				eprintln!("Failed to create directory {}: {e}", parent.display());
				std::process::exit(1);
			});
		}
		fs::write(&output_path, &formatted).unwrap_or_else(|e| {
			eprintln!("Failed to write {}: {e}", &output_path.display());
			std::process::exit(1);
		});
		formatted.as_bytes().to_vec()
	} else {
		let compressed = compress(&formatted);
		fs::write(&output_path, &compressed).unwrap_or_else(|e| {
			eprintln!("Failed to write {}: {e}", &output_path.display());
			std::process::exit(1);
		});
		compressed
	};
	run.output = &output;

	let hash = {
		let mut hasher = DefaultHasher::new();
		run.hash(&mut hasher);
		hasher.finish().to_string()
	};

	fs::write(&current_hash_path, hash).unwrap_or_else(|e| {
		eprintln!("Failed to write hash file {}: {e}", current_hash_path.display());
		process::exit(1);
	});
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

fn compress(content: &str) -> Vec<u8> {
	let mut buf = Vec::new();
	let mut writer = XzWriter::new(&mut buf, XzOptions::default()).unwrap_or_else(|e| {
		eprintln!("Failed to create XZ writer: {e}");
		std::process::exit(1);
	});
	writer.write_all(content.as_bytes()).unwrap_or_else(|e| {
		eprintln!("Failed to write compressed credits: {e}");
		std::process::exit(1);
	});
	writer.finish().unwrap_or_else(|e| {
		eprintln!("Failed to finish XZ compression: {e}");
		std::process::exit(1);
	});
	buf
}
