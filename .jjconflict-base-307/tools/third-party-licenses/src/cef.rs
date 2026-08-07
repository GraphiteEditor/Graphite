use lzma_rust2::XzReader;
use scraper::{Html, Selector};
use std::fs;
use std::hash::Hash;
use std::io::Read;
use std::path::PathBuf;

use crate::{Error, LicenceSource, LicenseEntry, Package};

pub struct CefLicenseSource;

impl CefLicenseSource {
	pub fn new() -> Self {
		Self {}
	}
}

impl LicenceSource for CefLicenseSource {
	fn licenses(&self) -> Result<Vec<LicenseEntry>, Error> {
		let html = read()?;
		Ok(parse(&html))
	}
}

impl Hash for CefLicenseSource {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		read().unwrap().hash(state)
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

fn read() -> Result<String, Error> {
	let cef_path = PathBuf::from(env!("CEF_PATH"));
	let cef_credits = std::fs::read_dir(&cef_path)
		.map_err(|e| Error::Io(e, format!("Failed to read CEF_PATH directory {}", cef_path.display())))?
		.filter_map(|entry| entry.ok())
		.find(|entry| {
			let name = entry.file_name();
			name.eq_ignore_ascii_case("credits.html") || name.eq_ignore_ascii_case("credits.html.xz")
		})
		.map(|entry| entry.path())
		.ok_or_else(|| Error::CefCreditsNotFound(cef_path.clone()))?;

	let decompress_xz = cef_credits.extension().map(|ext| ext.eq_ignore_ascii_case("xz")).unwrap_or(false);

	if decompress_xz {
		let file = fs::File::open(&cef_credits).map_err(|e| Error::Io(e, format!("Failed to open CEF credits file {}", cef_credits.display())))?;
		let mut reader = XzReader::new(file, false);
		let mut html = String::new();
		reader
			.read_to_string(&mut html)
			.map_err(|e| Error::Io(e, format!("Failed to decompress CEF credits file {}", cef_credits.display())))?;
		Ok(html)
	} else {
		fs::read_to_string(&cef_credits).map_err(|e| Error::Io(e, format!("Failed to read CEF credits file {}", cef_credits.display())))
	}
}
