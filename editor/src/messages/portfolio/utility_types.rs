use graphene_std::text::{Font, FontCache};

#[derive(Debug, Default)]
pub struct PersistentData {
	pub font_cache: FontCache,
	pub font_catalog: FontCatalog,
	pub use_vello: bool,
}

// TODO: Should this be a BTreeMap instead?
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalog(pub Vec<FontCatalogFamily>);

impl FontCatalog {
	pub fn find_font_style_in_catalog(&self, font: &Font) -> Option<FontCatalogStyle> {
		let family = self.0.iter().find(|family| family.name == font.font_family);

		let found_style = family.map(|family| {
			let FontCatalogStyle { weight, italic, .. } = FontCatalogStyle::from_named_style(&font.font_style, "");
			family.closest_style(weight, italic).clone()
		});

		if found_style.is_none() {
			log::warn!("Font not found in catalog: {:?}", font);
		}

		found_style
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalogFamily {
	/// The font family name.
	pub name: String,
	/// The font styles (variants) available for the font family.
	pub styles: Vec<FontCatalogStyle>,
}

impl FontCatalogFamily {
	/// Finds the closest style to the given weight and italic setting.
	/// Aims to find the nearest weight while maintaining the italic setting if possible, but italic may change if no other option is available.
	pub fn closest_style(&self, weight: u32, italic: bool) -> &FontCatalogStyle {
		self.styles
			.iter()
			.map(|style| ((style.weight as i32 - weight as i32).unsigned_abs() + 10000 * (style.italic != italic) as u32, style))
			.min_by_key(|(distance, _)| *distance)
			.map(|(_, style)| style)
			.unwrap_or(&self.styles[0])
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalogStyle {
	pub weight: u32,
	pub italic: bool,
	pub url: String,
}

impl FontCatalogStyle {
	pub fn to_named_style(&self) -> String {
		let weight = self.weight;
		let italic = self.italic;

		let named_weight = Font::named_weight(weight);
		let maybe_italic = if italic { " Italic" } else { "" };

		format!("{named_weight}{maybe_italic} ({weight})")
	}

	pub fn from_named_style(named_style: &str, url: impl Into<String>) -> FontCatalogStyle {
		let weight = named_style.split_terminator(['(', ')']).next_back().and_then(|x| x.parse::<u32>().ok()).unwrap_or(400);
		let italic = named_style.contains("Italic (");
		FontCatalogStyle { weight, italic, url: url.into() }
	}

	/// Get the URL for the stylesheet for loading a font preview for this style of the given family name, subsetted to only the letters in the family name.
	pub fn preview_url(&self, family: impl Into<String>) -> String {
		let name = family.into().replace(' ', "+");
		let italic = if self.italic { "ital," } else { "" };
		let weight = self.weight;
		format!("https://fonts.googleapis.com/css2?display=swap&family={name}:{italic}wght@{weight}&text={name}")
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum Platform {
	#[default]
	Unknown,
	Windows,
	Mac,
	Linux,
}

impl Platform {
	pub fn as_keyboard_platform_layout(&self) -> KeyboardPlatformLayout {
		match self {
			Platform::Mac => KeyboardPlatformLayout::Mac,
			Platform::Windows | Platform::Linux => KeyboardPlatformLayout::Standard,
			Platform::Unknown => {
				warn!("The platform has not been set, remember to send `GlobalsMessage::SetPlatform` during editor initialization.");
				KeyboardPlatformLayout::Standard
			}
		}
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum KeyboardPlatformLayout {
	/// Standard keyboard mapping used by Windows and Linux
	#[default]
	Standard,
	/// Keyboard mapping used by Macs where Command is sometimes used in favor of Control
	Mac,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum PanelType {
	#[default]
	Document,
	Welcome,
	Layers,
	Properties,
	DataPanel,
}

impl From<String> for PanelType {
	fn from(value: String) -> Self {
		match value.as_str() {
			"Document" => PanelType::Document,
			"Welcome" => PanelType::Welcome,
			"Layers" => PanelType::Layers,
			"Properties" => PanelType::Properties,
			"Data" => PanelType::DataPanel,
			_ => panic!("Unknown panel type: {value}"),
		}
	}
}
