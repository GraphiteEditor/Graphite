use dyn_any::{DynAny, StaticType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A font type (storing font family and font style and an optional preview URL)
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, DynAny, specta::Type)]
pub struct Font {
	#[serde(rename = "fontFamily")]
	pub font_family: String,
	#[serde(rename = "fontStyle")]
	pub font_style: String,
}
impl Font {
	pub fn new(font_family: String, font_style: String) -> Self {
		Self { font_family, font_style }
	}
}

/// A cache of all loaded font data and preview urls along with the default font (send from `init_app` in `editor_api.rs`)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FontCache {
	/// Actual font file data used for rendering a font with ttf_parser and rustybuzz
	font_file_data: HashMap<Font, Vec<u8>>,
	/// Web font preview URLs used for showing fonts when live editing
	preview_urls: HashMap<Font, String>,
	/// The default font (used as a fallback)
	default_font: Option<Font>,
}
impl FontCache {
	/// Returns the font family name if the font is cached, otherwise returns the default font family name if that is cached
	pub fn resolve_font<'a>(&'a self, font: &'a Font) -> Option<&'a Font> {
		if self.loaded_font(font) {
			Some(font)
		} else {
			self.default_font.as_ref().filter(|font| self.loaded_font(font))
		}
	}

	/// Try to get the bytes for a font
	pub fn get<'a>(&'a self, font: &Font) -> Option<&'a Vec<u8>> {
		self.resolve_font(font).and_then(|font| self.font_file_data.get(font))
	}

	/// Check if the font is already loaded
	pub fn loaded_font(&self, font: &Font) -> bool {
		self.font_file_data.contains_key(font)
	}

	/// Insert a new font into the cache
	pub fn insert(&mut self, font: Font, perview_url: String, data: Vec<u8>, is_default: bool) {
		if is_default {
			self.default_font = Some(font.clone());
		}
		self.font_file_data.insert(font.clone(), data);
		self.preview_urls.insert(font, perview_url);
	}

	/// Checks if the font cache has a default font
	pub fn has_default(&self) -> bool {
		self.default_font.is_some()
	}

	/// Gets the preview URL for showing in text field when live editing
	pub fn get_preview_url(&self, font: &Font) -> Option<&String> {
		self.preview_urls.get(font)
	}
}

impl core::hash::Hash for FontCache {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.preview_urls.len().hash(state);
		self.preview_urls.iter().for_each(|(font, url)| {
			font.hash(state);
			url.hash(state)
		});
		self.font_file_data.len().hash(state);
		self.font_file_data.keys().for_each(|font| font.hash(state));
	}
}
