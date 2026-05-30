use core_types::graphene_hash::CacheHash;
use dyn_any::DynAny;

/// A font type (storing font family and font style)
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Eq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Font {
	#[cfg_attr(feature = "serde", serde(rename = "fontFamily"))]
	pub font_family: String,
	#[cfg_attr(feature = "serde", serde(rename = "fontStyle", deserialize_with = "migrate_font_style"))]
	pub font_style: String,
}

impl std::hash::Hash for Font {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.font_family.hash(state);
		self.font_style.hash(state);
	}
}

impl CacheHash for Font {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.font_family.cache_hash(state);
		self.font_style.cache_hash(state);
	}
}

impl PartialEq for Font {
	fn eq(&self, other: &Self) -> bool {
		self.font_family == other.font_family && self.font_style == other.font_style
	}
}

impl Font {
	pub fn new(font_family: String, font_style: String) -> Self {
		Self { font_family, font_style }
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

// TODO: Eventually remove this migration document upgrade code
fn migrate_font_style<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
	use serde::Deserialize;
	String::deserialize(deserializer).map(|name| if name == "Normal (400)" { "Regular (400)".to_string() } else { name })
}
