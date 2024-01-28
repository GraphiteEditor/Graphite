use dyn_any::{DynAny, StaticType};

use alloc::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;

/// A font type (storing font family and font style and an optional preview URL)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, DynAny, specta::Type)]
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
#[derive(Debug, Default, Clone)]
pub struct FontCache {
	/// Actual font file data used for rendering a font
	pub font_attrs: HashMap<Font, cosmic_text::AttrsOwned>,
	/// Web font preview URLs used for showing fonts when live editing
	preview_urls: HashMap<Font, String>,
	/// The default font (used as a fallback)
	pub default_font: Option<Font>,
	system: Option<Arc<Mutex<cosmic_text::FontSystem>>>,
}
impl FontCache {
	#[must_use]
	fn font_system() -> Arc<Mutex<cosmic_text::FontSystem>> {
		// TODO: better locale
		Arc::new(Mutex::new(cosmic_text::FontSystem::new_with_locale_and_db("en".to_string(), cosmic_text::fontdb::Database::new())))
	}

	/// Check if the font is already loaded
	#[must_use]
	pub fn loaded_font(&self, font: &Font) -> bool {
		self.font_attrs.contains_key(font)
	}

	/// Insert a new font into the cache
	pub fn insert(&mut self, font: Font, perview_url: String, data: Vec<u8>, is_default: bool) {
		if is_default {
			self.default_font = Some(font.clone());
		}
		let mut font_system = self.system.get_or_insert_with(Self::font_system).lock().expect("acquire font system");
		let data = Arc::new(data);
		let db = font_system.db_mut();
		let id = db.load_font_source(cosmic_text::fontdb::Source::Binary(data.clone()))[0];
		if let Some(face) = db.face(id) {
			info!("Face {face:#?}");
			let attrs = cosmic_text::AttrsOwned::new(
				cosmic_text::Attrs::new()
					.family(cosmic_text::Family::Name(&face.families[0].0))
					.stretch(face.stretch)
					.style(face.style)
					.weight(face.weight),
			);
			self.font_attrs.insert(font.clone(), attrs);
		}

		self.preview_urls.insert(font, perview_url);
	}

	/// Checks if the font cache has a default font
	#[must_use]
	pub fn has_default(&self) -> bool {
		self.default_font.is_some()
	}

	/// Gets the preview URL for showing in text field when live editing
	#[must_use]
	pub fn get_preview_url(&self, font: &Font) -> Option<&String> {
		self.preview_urls.get(font)
	}

	#[must_use]
	pub fn get_system(&self) -> Option<std::sync::MutexGuard<cosmic_text::FontSystem>> {
		self.system.as_ref().map(|system| system.lock().expect("acquire font system"))
	}
}

impl core::hash::Hash for FontCache {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.preview_urls.len().hash(state);
		self.preview_urls.iter().for_each(|(font, url)| {
			font.hash(state);
			url.hash(state)
		});
		self.font_attrs.len().hash(state);
		self.font_attrs.keys().for_each(|font| font.hash(state));
	}
}

impl core::cmp::PartialEq for FontCache {
	fn eq(&self, other: &Self) -> bool {
		self.font_attrs == other.font_attrs
	}
}
