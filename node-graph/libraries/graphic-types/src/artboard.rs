use crate::graphic::Graphic;
use core_types::blending::AlphaBlending;
use core_types::table::{Table, TableRow};
use core_types::uuid::NodeId;
use core_types::{ATTR_BACKGROUND, ATTR_CLIP, ATTR_DIMENSIONS, ATTR_LOCATION, Color};
use dyn_any::DynAny;
use glam::{DAffine2, IVec2};

// An artboard table is `Table<Table<Graphic>>`: each row's element is the artboard's content
// (a `Table<Graphic>`), with the artboard's metadata stored alongside on the row as attributes
// (see `ATTR_LOCATION`, `ATTR_DIMENSIONS`, `ATTR_BACKGROUND`, `ATTR_CLIP`).
//
// The artboard's user-visible name is the parent layer's display name (resolved live from the
// network interface via the row's `ATTR_EDITOR_LAYER_PATH` attribute) — not stored here, so it
// can never go stale.
//
// These metadata attributes are populated at runtime by the `Artboard` proto node from its
// inputs and therefore aren't persisted in document files; the proto node's input values are
// what get serialized.

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_artboard<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Table<Graphic>>, D::Error> {
	use serde::Deserialize;

	/// Pre-migration shape of the artboard's stored data: the struct that used to live as the element
	/// of `Table<Artboard>`. Kept as a private type so we can deserialize legacy documents into the new
	/// `Table<Table<Graphic>>` (element = `content`, other fields → row attributes).
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
		alpha_blending: Vec<AlphaBlending>,
	}

	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(untagged))]
	enum ArtboardFormat {
		ArtboardGroup(LegacyArtboardGroup),
		OldArtboardTable(OldTable<LegacyArtboard>),
		LegacyArtboardTable(Table<LegacyArtboard>),
		// Note: this variant must come last so older formats above are tried first; an empty
		// `Table<Table<Graphic>>` would otherwise match (since `Table<T>` has the same shell across `T`).
		ArtboardTable(Table<Table<Graphic>>),
	}

	fn legacy_to_row(legacy: LegacyArtboard) -> TableRow<Table<Graphic>> {
		// Legacy `label` field is dropped — the artboard's name now comes from its parent layer's display name.
		TableRow::new_from_element(legacy.content)
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
