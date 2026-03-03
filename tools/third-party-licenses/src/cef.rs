use lzma_rust2::XzReader;
use scraper::{Html, Selector};
use std::hash::Hash;
use std::io::Read;
use std::path::PathBuf;
use std::{fs, process};

use crate::{LicenceSource, LicenseEntry, Package};

pub struct CefLicenseSource;

impl CefLicenseSource {
	pub fn new() -> Self {
		Self {}
	}
}

impl LicenceSource for CefLicenseSource {
	fn licenses(&self) -> Vec<LicenseEntry> {
		let html = read();
		parse(&html)
	}
}

impl Hash for CefLicenseSource {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		read().hash(state)
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

fn read() -> String {
	let cef_path = PathBuf::from(env!("CEF_PATH"));
	let cef_credits = std::fs::read_dir(&cef_path)
		.unwrap_or_else(|e| {
			eprintln!("Failed to read CEF_PATH directory {}: {e}", cef_path.display());
			process::exit(1);
		})
		.filter_map(|entry| entry.ok())
		.find(|entry| {
			let name = entry.file_name();
			name.eq_ignore_ascii_case("credits.html") || name.eq_ignore_ascii_case("credits.html.xz")
		})
		.map(|entry| entry.path())
		.unwrap_or_else(|| {
			eprintln!("Could not find CREDITS.html or CREDITS.html.xz in {}", cef_path.display());
			process::exit(1);
		});

	let decompress_xz = cef_credits.extension().map(|ext| ext.eq_ignore_ascii_case("xz")).unwrap_or(false);

	if decompress_xz {
		let file = fs::File::open(&cef_credits).unwrap_or_else(|e| {
			eprintln!("Failed to open CEF credits file {}: {e}", cef_credits.display());
			process::exit(1);
		});
		let mut reader = XzReader::new(file, false);
		let mut html = String::new();
		reader.read_to_string(&mut html).unwrap_or_else(|e| {
			eprintln!("Failed to decompress CEF credits file {}: {e}", cef_credits.display());
			process::exit(1);
		});
		html
	} else {
		fs::read_to_string(&cef_credits).unwrap_or_else(|e| {
			eprintln!("Failed to read CEF credits file {}: {e}", cef_credits.display());
			process::exit(1);
		})
	}
}
