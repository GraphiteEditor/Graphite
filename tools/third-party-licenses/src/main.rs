use std::collections::BTreeMap;
use std::path::PathBuf;
use std::{env, process};

mod cargo;
mod cef;
mod npm;

use crate::cargo::CargoAboutLicenseSource;
use crate::cef::CefLicenseSource;
use crate::npm::NpmLicenseSource;

pub trait LicenceSource {
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

fn main() {
	let npm_dir = PathBuf::from(env!("CARGO_WORKSPACE_DIR")).join("frontend");

	let cef_path = match env::args().nth(1).map(PathBuf::from) {
		Some(p) => p,
		None => {
			eprintln!("Usage: cargo run -p third-party-licenses -- <path-to-credits.html>");
			process::exit(1);
		}
	};

	let cargo_source = CargoAboutLicenseSource::new();
	let cef_source = CefLicenseSource::new(cef_path);
	let npm_source = NpmLicenseSource::new(npm_dir);

	let credits = merge_dedup_and_sort(vec![cargo_source.licenses(), cef_source.licenses(), npm_source.licenses()]);

	print!("{}", format_credits(&credits));
}

fn dedup_by_licence_text(vec: Vec<LicenseEntry>) -> Vec<LicenseEntry> {
	let mut map: BTreeMap<String, LicenseEntry> = BTreeMap::new();

	for entry in vec {
		match map.entry(entry.text.clone()) {
			std::collections::btree_map::Entry::Occupied(mut e) => {
				e.get_mut().packages.extend(entry.packages);
			}
			std::collections::btree_map::Entry::Vacant(e) => {
				e.insert(entry);
			}
		}
	}

	map.into_values().collect()
}

fn merge_dedup_and_sort(sources: Vec<Vec<LicenseEntry>>) -> Vec<LicenseEntry> {
	let mut all = Vec::new();
	for source in sources {
		all.extend(source);
	}
	let mut all = dedup_by_licence_text(all);
	all.sort_by_cached_key(|e| e.packages.len());
	all.reverse();
	all
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
				Package { name, authors, url: Some(url) } if !authors.is_empty() => format!("{} ({}) - {}", name, authors.join(", "), url),
				Package { name, authors: _, url: Some(url) } => format!("{} - {}", name, url),
				Package { name, authors, url: None } if !authors.is_empty() => format!("{} ({})", name, authors.join(", ")),
				_ => pkg.name.clone(),
			})
			.collect();

		let multi = package_lines.len() > 1;

		let header = format!(
			"The package{} listed here {} licensed under the terms of the {} printed beneath",
			if multi { "s" } else { "" },
			if multi { "are" } else { "is" },
			if let Some(license) = license.name.as_ref() {
				format!("\"{}\" license", license)
			} else {
				"license".to_string()
			}
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
		out.push_str(&format!(" {}\n", "\u{203e}".repeat(max_len + 2)));
		for line in license.text.lines() {
			if line.is_empty() {
				out.push('\n');
				continue;
			}
			out.push('\n');
			out.push_str("    ");
			out.push_str(line);
		}
	}

	out.push('\n');
	out
}
