use crate::graphic::Graphic;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::list::List;
use core_types::render_complexity::RenderComplexity;
use dyn_any::DynAny;
use glam::DAffine2;

/// Nominal wrapper around `List<Graphic>` representing a single artboard's content.
///
/// Per-artboard metadata (location, dimensions, background, clip) lives as attributes on the
/// enclosing `List<Artboard>`, not as fields here. This keeps `Artboard` a pure type-system boundary
/// that prevents arbitrary `List<List<...<Graphic>>>` nesting.
#[derive(Clone, Debug, Default, CacheHash, PartialEq, DynAny)]
pub struct Artboard(List<Graphic>);

impl Artboard {
	pub fn new(content: List<Graphic>) -> Self {
		Self(content)
	}

	pub fn as_graphic_list(&self) -> &List<Graphic> {
		&self.0
	}

	pub fn as_graphic_list_mut(&mut self) -> &mut List<Graphic> {
		&mut self.0
	}

	pub fn into_graphic_list(self) -> List<Graphic> {
		self.0
	}
}

impl From<List<Graphic>> for Artboard {
	fn from(content: List<Graphic>) -> Self {
		Self(content)
	}
}

impl From<Artboard> for List<Graphic> {
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
