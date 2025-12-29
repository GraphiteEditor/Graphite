use dyn_any::DynAny;
use parley::fontique::Blob;
use std::collections::HashMap;
use std::sync::Arc;

// Import specta so derive macros can find it
use core_types::specta;

/// A font type (storing font family and font style and an optional preview URL)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, DynAny, core_types::specta::Type)]
pub struct Font {
	#[serde(rename = "fontFamily")]
	pub font_family: String,
	#[serde(rename = "fontStyle", deserialize_with = "migrate_font_style")]
	pub font_style: String,
	#[serde(skip)]
	pub font_style_to_restore: Option<String>,
}

impl std::hash::Hash for Font {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.font_family.hash(state);
		self.font_style.hash(state);
		// Don't consider `font_style_to_restore` in the HashMaps
	}
}

impl PartialEq for Font {
	fn eq(&self, other: &Self) -> bool {
		// Don't consider `font_style_to_restore` in the HashMaps
		self.font_family == other.font_family && self.font_style == other.font_style
	}
}

impl Font {
	pub fn new(font_family: String, font_style: String) -> Self {
		Self {
			font_family,
			font_style,
			font_style_to_restore: None,
		}
	}

	pub fn named_weight(weight: u32) -> &'static str {
		// From https://developer.mozilla.org/en-US/docs/Web/CSS/font-weight#common_weight_name_mapping
		match weight {
			100 => "Thin",
			200 => "Extra Light",
			300 => "Light",
			400 => "Regular",
			500 => "Medium",
			600 => "Semi Bold",
			700 => "Bold",
			800 => "Extra Bold",
			900 => "Black",
			950 => "Extra Black",
			_ => "Regular",
		}
	}
}
impl Default for Font {
	fn default() -> Self {
		Self::new(core_types::consts::DEFAULT_FONT_FAMILY.into(), core_types::consts::DEFAULT_FONT_STYLE.into())
	}
}

/// A cache of all loaded font data and preview urls along with the default font (send from `init_app` in `editor_api.rs`)
#[derive(Clone, serde::Serialize, serde::Deserialize, Default, DynAny)]
pub struct FontCache {
	/// Actual font file data used for rendering a font
	font_file_data: HashMap<Font, Vec<u8>>,
}

impl std::fmt::Debug for FontCache {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FontCache").field("font_file_data", &self.font_file_data.keys().collect::<Vec<_>>()).finish()
	}
}

impl std::hash::Hash for FontCache {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.font_file_data.len().hash(state);
		self.font_file_data.keys().for_each(|font| font.hash(state));
	}
}

impl PartialEq for FontCache {
	fn eq(&self, other: &Self) -> bool {
		if self.font_file_data.len() != other.font_file_data.len() {
			return false;
		}
		self.font_file_data.keys().all(|font| other.font_file_data.contains_key(font))
	}
}

impl FontCache {
	/// Returns the font family name if the font is cached, otherwise returns the fallback font family name if that is cached
	pub fn resolve_font<'a>(&'a self, font: &'a Font) -> Option<&'a Font> {
		if self.font_file_data.contains_key(font) {
			Some(font)
		} else {
			self.font_file_data
				.keys()
				.find(|font| font.font_family == core_types::consts::DEFAULT_FONT_FAMILY && font.font_style == core_types::consts::DEFAULT_FONT_STYLE)
		}
	}

	/// Try to get the bytes for a font
	pub fn get<'a>(&'a self, font: &'a Font) -> Option<(&'a Vec<u8>, &'a Font)> {
		self.resolve_font(font).and_then(|font| self.font_file_data.get(font).map(|data| (data, font)))
	}

	/// Get font data as a Blob for use with parley/skrifa
	pub fn get_blob<'a>(&'a self, font: &'a Font) -> Option<(Blob<u8>, &'a Font)> {
		self.get(font).map(|(data, font)| (Blob::new(Arc::new(data.clone())), font))
	}

	/// Check if the font is already loaded
	pub fn loaded_font(&self, font: &Font) -> bool {
		self.font_file_data.contains_key(font)
	}

	/// Insert a new font into the cache
	pub fn insert(&mut self, font: Font, preview_url: String, data: Vec<u8>) {
		self.font_file_data.insert(font.clone(), data);
		self.preview_urls.insert(font, preview_url);
	}

	/// Gets the preview URL for showing in text field when live editing
	pub fn get_preview_url(&self, font: &Font) -> Option<&String> {
		self.preview_urls.get(font)
	}
}

impl std::hash::Hash for FontCache {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.preview_urls.len().hash(state);
		self.preview_urls.iter().for_each(|(font, url)| {
			font.hash(state);
			url.hash(state)
		});
		self.font_file_data.len().hash(state);
		self.font_file_data.keys().for_each(|font| font.hash(state));
	pub fn insert(&mut self, font: Font, data: Vec<u8>) {
		self.font_file_data.insert(font.clone(), data);
	}
}

// TODO: Eventually remove this migration document upgrade code
fn migrate_font_style<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
	use serde::Deserialize;
	String::deserialize(deserializer).map(|name| if name == "Normal (400)" { "Regular (400)".to_string() } else { name })
}
