use crate::graphic::Graphic;
use core_types::blending::BlendMode;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::render_complexity::RenderComplexity;
use core_types::table::{Table, TableRow};
use core_types::uuid::NodeId;
use core_types::{ATTR_BACKGROUND, ATTR_CLIP, ATTR_DIMENSIONS, ATTR_LOCATION, Color};
use dyn_any::DynAny;
use glam::{DAffine2, IVec2};

/// Nominal wrapper around `Table<Graphic>` representing a single artboard's content.
///
/// Per-artboard metadata (location, dimensions, background, clip) lives as row attributes on the
/// enclosing `Table<Artboard>`, not as fields here. This keeps `Artboard` a pure type-system boundary
/// that prevents arbitrary `Table<Table<...<Graphic>>>` nesting.
#[derive(Clone, Debug, Default, CacheHash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Artboard(Table<Graphic>);

impl Artboard {
	pub fn new(content: Table<Graphic>) -> Self {
		Self(content)
	}

	pub fn as_graphic_table(&self) -> &Table<Graphic> {
		&self.0
	}

	pub fn as_graphic_table_mut(&mut self) -> &mut Table<Graphic> {
		&mut self.0
	}

	pub fn into_graphic_table(self) -> Table<Graphic> {
		self.0
	}
}

impl From<Table<Graphic>> for Artboard {
	fn from(content: Table<Graphic>) -> Self {
		Self(content)
	}
}

impl From<Artboard> for Table<Graphic> {
	fn from(artboard: Artboard) -> Self {
		artboard.0
	}
}

impl BoundingBox for Artboard {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		self.0.bounding_box(transform, include_stroke)
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		self.0.thumbnail_bounding_box(transform, include_stroke)
	}
}

impl RenderComplexity for Artboard {
	fn render_complexity(&self) -> usize {
		self.0.render_complexity()
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_artboard<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Artboard>, D::Error> {
	use serde::Deserialize;

	/// Mirrors the removed `AlphaBlending` struct for legacy document deserialization.
	#[derive(Clone, Debug, Default)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(default))]
	pub struct LegacyAlphaBlending {
		pub blend_mode: BlendMode,
		pub opacity: f32,
		pub fill: f32,
		pub clip: bool,
	}

	/// Legacy artboard struct shape, kept for deserializing old documents into `Table<Artboard>`.
	#[derive(Clone, Debug, DynAny)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct LegacyArtboard {
		pub content: Table<Graphic>,
		pub label: String,
		pub location: IVec2,
		pub dimensions: IVec2,
		pub background: Color,
		pub clip: bool,
	}

	#[derive(Clone, Default, Debug, DynAny)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct LegacyArtboardGroup {
		pub artboards: Vec<(LegacyArtboard, Option<NodeId>)>,
	}

	#[derive(Clone, Debug)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct OldTable<T> {
		#[cfg_attr(feature = "serde", serde(alias = "instances", alias = "instance"))]
		element: Vec<T>,
		transform: Vec<DAffine2>,
		alpha_blending: Vec<LegacyAlphaBlending>,
	}

	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(untagged))]
	enum ArtboardFormat {
		ArtboardGroup(LegacyArtboardGroup),
		OldArtboardTable(OldTable<LegacyArtboard>),
		LegacyArtboardTable(Table<LegacyArtboard>),
		// NOTE: Must come last so older tagged formats above are tried first.
		// Also covers the intermediate `Table<Table<Graphic>>` shape since `Artboard` deserializes transparently.
		ArtboardTable(Table<Artboard>),
	}

	fn legacy_to_row(legacy: LegacyArtboard) -> TableRow<Artboard> {
		// Legacy `label` field is dropped (the artboard's name comes from its parent layer's display name)
		TableRow::new_from_element(Artboard::new(legacy.content))
			.with_attribute(ATTR_LOCATION, legacy.location.as_dvec2())
			.with_attribute(ATTR_DIMENSIONS, legacy.dimensions.as_dvec2())
			.with_attribute(ATTR_BACKGROUND, legacy.background)
			.with_attribute(ATTR_CLIP, legacy.clip)
	}

	Ok(match ArtboardFormat::deserialize(deserializer)? {
		ArtboardFormat::ArtboardGroup(group) => group.artboards.into_iter().map(|(artboard, _)| legacy_to_row(artboard)).collect(),
		ArtboardFormat::OldArtboardTable(old_table) => old_table.element.into_iter().map(legacy_to_row).collect(),
		ArtboardFormat::LegacyArtboardTable(legacy_table) => legacy_table.into_iter().map(|row| legacy_to_row(row.into_element())).collect(),
		ArtboardFormat::ArtboardTable(artboard_table) => artboard_table,
	})
}
