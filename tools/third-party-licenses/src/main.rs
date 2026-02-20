//! Gathers license/credit information from all three dependency sources:
//!
//! 1. **Cargo (Rust)** — via `cargo about generate --format json --frozen`
//! 2. **CEF/Chromium** — by parsing a `credits.html` file
//! 3. **npm** — via `npx license-checker-rseidelsohn --json`
//!
//! The results are merged, deduplicated by license text, and printed as
//! JSON or human-readable text.

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs, process};

#[derive(Serialize)]
struct LicenseEntry {
	name: Option<String>,
	text: String,
	packages: Vec<Package>,
}

#[derive(Serialize)]
struct Package {
	name: String,
	authors: Vec<String>,
	url: Option<String>,
}

fn main() {
	let args: Vec<String> = env::args().collect();
	let text_mode = args.iter().any(|a| a == "--text");

	let mut positional = args.iter().skip(1).filter(|a| !a.starts_with("--"));
	let cef_path = positional.next().map(PathBuf::from);
	let npm_dir = positional.next().map(PathBuf::from);

	let cef_path = match cef_path {
		Some(p) => p,
		None => {
			eprintln!("Usage: third-party-licenses [--text] <path-to-credits.html> [npm-dir]");
			process::exit(1);
		}
	};

	eprintln!("Parsing credits from:");
	eprintln!("  cargo about");
	let cargo_credits = parse_cargo_about_credits();

	eprintln!("  cef");
	let html = fs::read_to_string(&cef_path).unwrap_or_else(|e| {
		eprintln!("Failed to read {}: {e}", cef_path.display());
		process::exit(1);
	});
	let cef_credits = parse_cef_credits(&html);

	eprintln!("  npm");
	let npm_credits = parse_npm_credits(npm_dir.as_deref());

	eprintln!("Merging and deduplicating credits...");
	let credits = merge_dedup_and_sort(vec![cargo_credits, cef_credits, npm_credits]);

	eprintln!("Outputting credits");
	if text_mode {
		print!("{}", format_credits_as_text(&credits));
	} else {
		let json = serde_json::to_string_pretty(&credits).expect("Failed to serialize credits");
		println!("{json}");
	}
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

fn parse_cargo_about_credits() -> Vec<LicenseEntry> {
	let output = Command::new("cargo").args(["about", "generate", "--format", "json", "--frozen"]).output().unwrap_or_else(|e| {
		eprintln!("Failed to run cargo about generate: {e}");
		process::exit(1);
	});

	if !output.status.success() {
		eprintln!("cargo about generate failed:\n{}", String::from_utf8_lossy(&output.stderr));
		process::exit(1);
	}

	let json_str = String::from_utf8(output.stdout).expect("Invalid UTF-8 from cargo about");

	let parsed: Value = serde_json::from_str(&json_str).unwrap_or_else(|e| {
		eprintln!("Failed to parse cargo about JSON: {e}");
		process::exit(1);
	});

	let licenses_array = parsed["licenses"].as_array().unwrap_or_else(|| {
		eprintln!("Expected 'licenses' array in cargo about JSON output");
		process::exit(1);
	});

	licenses_array
		.iter()
		.map(|license| {
			let license_name = license["name"].as_str().map(String::from);
			let license_text = license["text"].as_str().unwrap_or_default().trim().to_string();

			let used_by = license["used_by"].as_array().cloned().unwrap_or_default();

			let packages: Vec<Package> = used_by
				.iter()
				.map(|entry| {
					let crate_obj = &entry["crate"];
					let name = crate_obj["name"].as_str().unwrap_or_default();
					let version = crate_obj["version"].as_str().unwrap_or_default();
					let display_name = if version.is_empty() { name.to_string() } else { format!("{name}@{version}") };

					let authors = crate_obj["authors"]
						.as_array()
						.map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
						.unwrap_or_default();

					let repository = crate_obj["repository"].as_str().and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });

					Package {
						name: display_name,
						authors,
						url: repository,
					}
				})
				.collect();

			LicenseEntry {
				name: license_name,
				text: license_text,
				packages,
			}
		})
		.collect()
}

fn parse_cef_credits(html: &str) -> Vec<LicenseEntry> {
	let document = Html::parse_document(html);

	let product_sel = Selector::parse("div.product").unwrap();
	let title_sel = Selector::parse("span.title").unwrap();
	let homepage_sel = Selector::parse("span.homepage a").unwrap();
	let license_sel = Selector::parse("div.license pre").unwrap();

	document
		.select(&product_sel)
		.filter_map(|product| {
			let name: String = product.select(&title_sel).next().map(|el| el.text().collect()).unwrap_or_default();

			if name.is_empty() {
				return None;
			}

			let homepage = product.select(&homepage_sel).next().and_then(|el| el.value().attr("href").map(String::from));

			let license_text: String = product.select(&license_sel).next().map(|el| el.text().collect::<String>()).unwrap_or_default().trim().to_string();

			let pkg = Package {
				name,
				url: homepage,
				authors: Vec::new(),
			};

			Some(LicenseEntry {
				name: None,
				text: license_text,
				packages: vec![pkg],
			})
		})
		.collect()
}

#[derive(Deserialize)]
struct NpmEntry {
	licenses: Option<String>,
	repository: Option<String>,
	#[serde(rename = "licenseFile")]
	license_file: Option<String>,
	publisher: Option<String>,
}

fn parse_npm_credits(dir: Option<&std::path::Path>) -> Vec<LicenseEntry> {
	let mut cmd = Command::new("npx");
	cmd.args(["license-checker-rseidelsohn", "--json"]);
	if let Some(dir) = dir {
		cmd.current_dir(dir);
	}

	let output = cmd.output().unwrap_or_else(|e| {
		eprintln!("Failed to run npx license-checker-rseidelsohn: {e}");
		process::exit(1);
	});

	if !output.status.success() {
		eprintln!("npx license-checker-rseidelsohn failed:\n{}", String::from_utf8_lossy(&output.stderr));
		process::exit(1);
	}

	let json_str = String::from_utf8(output.stdout).expect("Invalid UTF-8 from license-checker");
	let entries: BTreeMap<String, NpmEntry> = serde_json::from_str(&json_str).unwrap_or_else(|e| {
		eprintln!("Failed to parse license-checker JSON: {e}");
		process::exit(1);
	});

	entries
		.iter()
		.map(|(name, entry)| {
			let license_text = entry
				.license_file
				.as_ref()
				.and_then(|p| fs::read_to_string(p).ok())
				.map(|s| s.trim().to_string())
				.unwrap_or_else(|| entry.licenses.clone().unwrap_or_default());

			let pkg = Package {
				name: name.to_string(),
				url: entry.repository.clone(),
				authors: entry.publisher.as_ref().map(|p| vec![p.clone()]).unwrap_or_default(),
			};

			LicenseEntry {
				name: None,
				text: license_text,
				packages: vec![pkg],
			}
		})
		.collect()
}

fn format_credits_as_text(licenses: &Vec<LicenseEntry>) -> String {
	let mut out = String::new();

	for license in licenses {
		let package_lines: Vec<String> = license
			.packages
			.iter()
			.map(|pkg| match &pkg.url {
				Some(url) if !url.is_empty() => format!("{} - {}", pkg.name, url),
				_ => pkg.name.clone(),
			})
			.collect();

		let multi = package_lines.len() > 1;

		let header = format!(
			"The package{} listed here {} licensed under the terms of the license printed beneath",
			if multi { "s" } else { "" },
			if multi { "are" } else { "is" },
		);

		let max_len = std::iter::once(header.len()).chain(package_lines.iter().map(|l| l.chars().count())).max().unwrap_or(0);

		let padded_packages: Vec<String> = package_lines
			.iter()
			.map(|line| {
				let pad = max_len - line.chars().count();
				format!("│ {}{} │", line, " ".repeat(pad))
			})
			.collect();

		out.push('\n');
		out.push_str(&format!(" {}\n", "_".repeat(max_len + 2)));
		out.push_str(&format!("│ {} │\n", " ".repeat(max_len)));
		out.push_str(&format!("│ {}{} │\n", header, " ".repeat(max_len - header.len())));
		out.push_str(&format!("│{}│\n", "_".repeat(max_len + 2)));
		out.push_str(&padded_packages.join("\n"));
		out.push('\n');
		out.push_str(&format!(" {}\n", "\u{203e}".repeat(max_len + 2)));
		for line in license.text.lines() {
			out.push_str("    ");
			out.push_str(line);
			out.push('\n');
		}
	}

	out.push('\n');
	out
}
