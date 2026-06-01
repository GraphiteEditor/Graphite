use graphene_std::text::Font;

// TODO: Should this be a BTreeMap instead?
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalog(Vec<FontCatalogFamily>);

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

	pub fn download_url(&self, font: &Font) -> Option<String> {
		let catalog_family = self.0.iter().find(|catalog_family| catalog_family.name == font.font_family)?;
		let FontCatalogStyle { weight, italic, .. } = FontCatalogStyle::from_named_style(&font.font_style, "");
		Some(catalog_family.closest_style(weight, italic).url.clone())
	}

	pub fn iter(&self) -> impl Iterator<Item = &FontCatalogFamily> {
		self.0.iter()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
}

impl From<Vec<FontCatalogFamily>> for FontCatalog {
	fn from(value: Vec<FontCatalogFamily>) -> Self {
		Self(value)
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(from_wasm_abi))]
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

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
