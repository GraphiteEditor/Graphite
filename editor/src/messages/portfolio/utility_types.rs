use graphene_std::Color;
use graphene_std::raster::Image;
use graphene_std::text::{Font, FontCache};

#[derive(Debug, Default)]
pub struct PersistentData {
	pub font_cache: FontCache,
	pub font_catalog: FontCatalog,
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

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum PanelType {
	Welcome,
	Document,
	Layers,
	Properties,
	Data,
}

impl PanelType {
	/// Returns the default panel area for this panel type. Only meaningful for dockable panels (not Document or Welcome).
	pub fn default_area(self) -> PanelAreaId {
		match self {
			PanelType::Properties => PanelAreaId::Properties,
			PanelType::Layers => PanelAreaId::Layers,
			PanelType::Data => PanelAreaId::Data,
			PanelType::Document | PanelType::Welcome => panic!("PanelType::{self:?} has no default area (not a dockable panel)"),
		}
	}
}

impl From<String> for PanelType {
	fn from(value: String) -> Self {
		match value.as_str() {
			"Welcome" => PanelType::Welcome,
			"Document" => PanelType::Document,
			"Layers" => PanelType::Layers,
			"Properties" => PanelType::Properties,
			"Data" => PanelType::Data,
			_ => panic!("Unknown panel type: {value}"),
		}
	}
}

/// Identifies a panel area in the workspace that can hold tabbed panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PanelAreaId {
	Properties,
	Layers,
	Data,
}

impl From<String> for PanelAreaId {
	fn from(value: String) -> Self {
		match value.as_str() {
			"Properties" => PanelAreaId::Properties,
			"Layers" => PanelAreaId::Layers,
			"Data" => PanelAreaId::Data,
			_ => panic!("Unknown panel area: {value}"),
		}
	}
}

/// State of a single panel area in the workspace.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PanelAreaState {
	pub tabs: Vec<PanelType>,
	#[serde(rename = "activeTabIndex")]
	pub active_tab_index: usize,
}

impl PanelAreaState {
	pub fn active_panel_type(&self) -> Option<PanelType> {
		self.tabs.get(self.active_tab_index).copied()
	}

	pub fn contains(&self, panel_type: PanelType) -> bool {
		self.tabs.contains(&panel_type)
	}

	pub fn is_visible(&self, panel_type: PanelType) -> bool {
		self.active_panel_type() == Some(panel_type)
	}
}

/// The complete workspace panel layout describing which dockable panels are in which areas.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspacePanelLayout {
	#[serde(rename = "propertiesArea")]
	pub properties_area: PanelAreaState,
	#[serde(rename = "layersArea")]
	pub layers_area: PanelAreaState,
	#[serde(rename = "dataArea")]
	pub data_area: PanelAreaState,
}

impl Default for WorkspacePanelLayout {
	fn default() -> Self {
		Self {
			properties_area: PanelAreaState {
				tabs: vec![PanelType::Properties],
				active_tab_index: 0,
			},
			layers_area: PanelAreaState {
				tabs: vec![PanelType::Layers],
				active_tab_index: 0,
			},
			data_area: PanelAreaState { tabs: vec![], active_tab_index: 0 },
		}
	}
}

impl WorkspacePanelLayout {
	pub fn panel_area(&self, panel_area_id: PanelAreaId) -> &PanelAreaState {
		match panel_area_id {
			PanelAreaId::Properties => &self.properties_area,
			PanelAreaId::Layers => &self.layers_area,
			PanelAreaId::Data => &self.data_area,
		}
	}

	pub fn panel_area_mut(&mut self, panel_area_id: PanelAreaId) -> &mut PanelAreaState {
		match panel_area_id {
			PanelAreaId::Properties => &mut self.properties_area,
			PanelAreaId::Layers => &mut self.layers_area,
			PanelAreaId::Data => &mut self.data_area,
		}
	}

	/// Find which panel area contains a given panel type.
	pub fn find_panel(&self, panel_type: PanelType) -> Option<PanelAreaId> {
		[PanelAreaId::Properties, PanelAreaId::Layers, PanelAreaId::Data]
			.into_iter()
			.find(|&area_id| self.panel_area(area_id).contains(panel_type))
	}

	/// Check if a panel type is the active (visible) tab in any panel area.
	pub fn is_panel_visible(&self, panel_type: PanelType) -> bool {
		for area_id in [PanelAreaId::Properties, PanelAreaId::Layers, PanelAreaId::Data] {
			if self.panel_area(area_id).is_visible(panel_type) {
				return true;
			}
		}
		false
	}

	/// Check if a panel type is present (as any tab) in any panel area, whether or not it's the active tab.
	pub fn is_panel_present(&self, panel_type: PanelType) -> bool {
		self.find_panel(panel_type).is_some()
	}
}

pub enum FileContent {
	/// A Graphite document.
	Document(String),
	/// A bitmap image.
	Image(Image<Color>),
	/// An SVG file string.
	Svg(String),
	/// Any other unsupported/unrecognized file type.
	Unsupported,
}
