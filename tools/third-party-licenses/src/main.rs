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
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs, process};

use crate::cargo_about::CargoAboutLicenseSource;
use crate::cef::CefLicenseSource;
use crate::npm::NpmLicenseSource;

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

trait LicenceSource {
	fn licenses(&self) -> Vec<LicenseEntry>;
}

fn main() {
	let args: Vec<String> = env::args().collect();

	let mut positional = args.iter().skip(1).filter(|a| !a.starts_with("--"));
	let cef_path = positional.next().map(PathBuf::from);
	let npm_dir = positional.next().map(PathBuf::from);

	let cef_path = match cef_path {
		Some(p) => p,
		None => {
			eprintln!("Usage: cargo run -p third-party-licenses -- <path-to-credits.html> <npm-dir>");
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

mod cargo_about {
	use super::*;

	pub struct CargoAboutLicenseSource {}

	impl CargoAboutLicenseSource {
		pub fn new() -> Self {
			Self {}
		}
	}

	impl LicenceSource for CargoAboutLicenseSource {
		fn licenses(&self) -> Vec<LicenseEntry> {
			parse(run())
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
		let output = Command::new("cargo").args(["about", "generate", "--format", "json", "--frozen"]).output().unwrap_or_else(|e| {
			eprintln!("Failed to run cargo about generate: {e}");
			process::exit(1);
		});

		if !output.status.success() {
			eprintln!("cargo about generate failed:\n{}", String::from_utf8_lossy(&output.stderr));
			process::exit(1);
		}

		serde_json::from_str(&String::from_utf8(output.stdout).expect("cargo about generate should return valid UTF-8")).unwrap_or_else(|e| {
			eprintln!("Failed to parse cargo about generate JSON: {e}");
			process::exit(1);
		})
	}
}

mod cef {
	use super::*;

	pub struct CefLicenseSource {
		cef_credits_html: PathBuf,
	}

	impl CefLicenseSource {
		pub fn new(cef_credits_html: PathBuf) -> Self {
			Self { cef_credits_html }
		}
	}

	impl LicenceSource for CefLicenseSource {
		fn licenses(&self) -> Vec<LicenseEntry> {
			let html = fs::read_to_string(&self.cef_credits_html).unwrap_or_else(|e| {
				eprintln!("Failed to read CEF CREDITS.html {}: {e}", self.cef_credits_html.display());
				process::exit(1);
			});
			parse(&html)
		}
	}

	fn parse(html: &str) -> Vec<LicenseEntry> {
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
}

mod npm {
	use super::*;

	pub struct NpmLicenseSource {
		dir: Option<PathBuf>,
	}
	impl NpmLicenseSource {
		pub fn new(dir: Option<PathBuf>) -> Self {
			Self { dir }
		}
	}

	impl LicenceSource for NpmLicenseSource {
		fn licenses(&self) -> Vec<LicenseEntry> {
			let json_str = run(self.dir.as_deref());
			parse(&json_str)
		}
	}

	#[derive(Deserialize)]
	struct NpmEntry {
		licenses: Option<String>,
		repository: Option<String>,
		#[serde(rename = "licenseFile")]
		license_file: Option<String>,
		publisher: Option<String>,
	}

	pub fn run(dir: Option<&std::path::Path>) -> String {
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
