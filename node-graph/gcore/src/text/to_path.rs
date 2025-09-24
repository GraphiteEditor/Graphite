use super::text_context::TextContext;
use super::{Font, FontCache, TypesettingConfig};
use crate::table::Table;
use crate::vector::Vector;
use glam::DVec2;
use parley::fontique::Blob;
use std::sync::Arc;

pub fn to_path(text: &str, font: &Font, font_cache: &FontCache, typesetting: TypesettingConfig, per_glyph_instances: bool) -> Table<Vector> {
	TextContext::with_thread_local(|ctx| ctx.to_path(text, font, font_cache, typesetting, per_glyph_instances))
}

pub fn bounding_box(text: &str, font: &Font, font_cache: &FontCache, typesetting: TypesettingConfig, for_clipping_test: bool) -> DVec2 {
	TextContext::with_thread_local(|ctx| ctx.bounding_box(text, font, font_cache, typesetting, for_clipping_test))
}

pub fn load_font(data: &[u8]) -> Blob<u8> {
	Blob::new(Arc::new(data.to_vec()))
}

pub fn lines_clipping(text: &str, font: &Font, font_cache: &FontCache, typesetting: TypesettingConfig) -> bool {
	TextContext::with_thread_local(|ctx| ctx.lines_clipping(text, font, font_cache, typesetting))
}
