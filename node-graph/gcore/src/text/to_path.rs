use super::text_context::TextContext;
use super::{Font, FontCache, TypesettingConfig};
use crate::table::Table;
use crate::text::Typography;
use crate::vector::Vector;
use glam::DVec2;
use graphene_core_shaders::color::Color;
use parley::fontique::Blob;
use std::sync::Arc;

pub fn to_typography(text: &str, font: &Font, font_cache: &FontCache, typesetting: TypesettingConfig) -> Option<Typography> {
	TextContext::with_thread_local(|ctx| {
		let layout = ctx.layout_text(text, font, font_cache, typesetting)?;
		let (family_name, _) = ctx.get_font_info(font, font_cache)?;
		Some(Typography {
			layout,
			family_name,
			color: Color::BLACK,
			stroke: None,
		})
	})
}

pub fn to_path(text: &str, font: &Font, font_cache: &FontCache, typesetting: TypesettingConfig, per_glyph_instances: bool) -> Table<Vector> {
	TextContext::with_thread_local(|ctx| ctx.text_to_path(text, font, font_cache, typesetting, per_glyph_instances))
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
