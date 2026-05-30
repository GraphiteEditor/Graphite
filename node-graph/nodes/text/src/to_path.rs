use super::TypesettingConfig;
use super::text_context::TextContext;
use core_types::list::List;
use glam::DVec2;
use parley::fontique::Blob;
use std::sync::Arc;
use vector_types::Vector;

pub fn to_path(text: &str, font_data: &Blob<u8>, typesetting: TypesettingConfig, per_glyph_items: bool) -> List<Vector> {
	TextContext::with_thread_local(|ctx| ctx.to_path(text, font_data, typesetting, per_glyph_items))
}

pub fn bounding_box(text: &str, font_data: &Blob<u8>, typesetting: TypesettingConfig, for_clipping_test: bool) -> DVec2 {
	TextContext::with_thread_local(|ctx| ctx.bounding_box(text, font_data, typesetting, for_clipping_test))
}

pub fn load_font(data: &[u8]) -> Blob<u8> {
	Blob::new(Arc::new(data.to_vec()))
}

pub fn lines_clipping(text: &str, font_data: &Blob<u8>, typesetting: TypesettingConfig) -> bool {
	TextContext::with_thread_local(|ctx| ctx.lines_clipping(text, font_data, typesetting))
}
