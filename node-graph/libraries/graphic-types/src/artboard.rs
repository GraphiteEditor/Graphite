use crate::graphic::Graphic;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::render_complexity::RenderComplexity;
use core_types::table::Table;
use dyn_any::DynAny;
use glam::DAffine2;

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
