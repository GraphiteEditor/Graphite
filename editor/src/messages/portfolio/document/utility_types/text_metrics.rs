use crate::messages::portfolio::fonts::FALLBACK_FONT_RESOURCE;
use graphene_std::text::{TextAlign, TextContext, TypesettingConfig};
use std::sync::{LazyLock, Mutex};

pub static GLOBAL_TEXT_CONTEXT: LazyLock<Mutex<TextContext>> = LazyLock::new(|| Mutex::new(TextContext::default()));

pub fn text_width(text: &str, font_size: f64) -> f64 {
	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio: 1.2,
		letter_spacing: 0.,
		letter_tilt: 0.,
		max_width: None,
		max_height: None,
		align: TextAlign::AlignLeft,
	};

	let mut text_context = GLOBAL_TEXT_CONTEXT.lock().expect("Failed to lock global text context");
	let bounds = text_context.bounding_box(text, &FALLBACK_FONT_RESOURCE, typesetting, false);
	bounds.x
}
