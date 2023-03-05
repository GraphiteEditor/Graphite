use super::style::ViewMode;
use super::text_layer::FontCache;

use glam::DVec2;

/// Contains metadata for rendering the document as an svg
#[derive(Debug, Clone, Copy)]
pub struct RenderData<'a> {
	pub font_cache: &'a FontCache,
	pub view_mode: ViewMode,
	pub culling_bounds: Option<[DVec2; 2]>,
}

impl<'a> RenderData<'a> {
	pub fn new(font_cache: &'a FontCache, view_mode: ViewMode, culling_bounds: Option<[DVec2; 2]>) -> Self {
		Self {
			font_cache,
			view_mode,
			culling_bounds,
		}
	}
}
