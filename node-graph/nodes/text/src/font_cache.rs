use dyn_any::DynAny;
use parley::fontique::Blob;
use std::collections::HashMap;
use std::sync::Arc;
use serde::Deserialize;

/// A font type (storing font family and font style and an optional preview URL)
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, DynAny)]
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
	}
}

impl PartialEq for Font {
	fn eq(&self, other: &Self) -> bool {
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
}

impl Default for Font {
	fn default() -> Self {
		Self {
			font_family: core_types::consts::DEFAULT_FONT_FAMILY.into(),
			font_style: core_types::consts::DEFAULT_FONT_STYLE.into(),
			font_style_to_restore: None,
		}
	}
}

/// A cache of fonts
#[derive(Debug, Default, Clone, DynAny)]
pub struct FontCache {
	/// Mapping of font family name to font style name to font data
	pub font_file_data: HashMap<Font, Arc<Vec<u8>>>,
}

impl FontCache {
	/// Get the font data for a font
	pub fn get_data(&self, font: &Font) -> Option<Arc<Vec<u8>>> {
		self.font_file_data.get(font).cloned()
	}

	/// Insert font data for a font
	pub fn insert(&mut self, font: Font, data: Arc<Vec<u8>>) {
		self.font_file_data.insert(font, data);
	}

	/// Check if the font data for a font is cached
	pub fn has(&self, font: &Font) -> bool {
		self.font_file_data.contains_key(font)
	}

	/// Get the number of fonts in the cache
	pub fn len(&self) -> usize {
		self.font_file_data.len()
	}

	/// Check if the cache is empty
	pub fn is_empty(&self) -> bool {
		self.font_file_data.is_empty()
	}

	/// Get an iterator over the fonts in the cache
	pub fn fonts(&self) -> impl Iterator<Item = &Font> {
		self.font_file_data.keys()
	}

	/// Returns the font family name if the font is cached, otherwise returns the fallback font family name if that is cached
	pub fn resolve_font<'a>(&'a self, font: &'a Font) -> Option<&'a Font> {
		if self.font_file_data.contains_key(font) {
			Some(font)
		} else {
			let fallback = self.font_file_data
				.keys()
				.find(|font| font.font_family == core_types::consts::DEFAULT_FONT_FAMILY && font.font_style == core_types::consts::DEFAULT_FONT_STYLE)
				.or_else(|| self.font_file_data.keys().next());			
			fallback
		}
	}


	/// Try to get the bytes for a font
	pub fn get<'a>(&'a self, font: &'a Font) -> Option<(&'a Vec<u8>, &'a Font)> {
		let resolved = self.resolve_font(font)?;
		self.font_file_data.get(resolved).map(|data| (data.as_ref(), resolved))
	}

	pub fn get_blob<'a>(&'a self, font: &'a Font) -> Option<(Blob<u8>, &'a Font)> {
		let resolved = self.resolve_font(font)?;
		self.font_file_data.get(resolved).map(|data| (Blob::new(data.clone()), resolved))
	}
}

// TODO: Eventually remove this migration document upgrade code
fn migrate_font_style<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
	String::deserialize(deserializer).map(|name| if name == "Normal (400)" { "Regular (400)".to_string() } else { name })
}
