use crate::messages::portfolio::fonts::FALLBACK_FONT_RESOURCE;
use graphene_std::text::{TextAlign, TextContext, TypesettingConfig};

pub fn text_width(text: &str, font_size: f64) -> f64 {
	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio: 1.2,
		letter_spacing: 0.,
		letter_tilt: 0.,
		max_width: None,
		max_height: None,
		align: TextAlign::AlignLeft,
		underline: false,
		overline: false,
		strikethrough: false,
	};

	TextContext::with_thread_local(|text_context| text_context.bounding_box(text, &FALLBACK_FONT_RESOURCE, typesetting, false).x)
}
