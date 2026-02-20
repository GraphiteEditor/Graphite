use std::{fs, path::PathBuf, process};

use scraper::{Html, Selector};

use crate::{LicenceSource, LicenseEntry, Package};

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
