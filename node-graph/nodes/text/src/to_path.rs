use super::TypesettingConfig;
use super::text_context::TextContext;
use core_types::blending::BlendMode;
use core_types::list::List;
use core_types::uuid::NodeId;
use core_types::{
	ATTR_BLEND_MODE, ATTR_EDITOR_LAYER_PATH, ATTR_FONT_SIZE, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TEXT_ALIGN, ATTR_TEXT_CHARACTER_SPACING, ATTR_TEXT_FONT, ATTR_TEXT_LINE_HEIGHT,
	ATTR_TEXT_MAX_HEIGHT, ATTR_TEXT_MAX_WIDTH, ATTR_TEXT_TILT, ATTR_TRANSFORM,
};
use glam::{DAffine2, DVec2};
use graphene_resource::Resource;
use vector_types::Vector;

pub fn to_path(text: &str, font: &Resource, typesetting: TypesettingConfig, per_glyph_items: bool) -> List<Vector> {
	TextContext::with_thread_local(|ctx| ctx.to_path(text, font, typesetting, per_glyph_items))
}

pub fn bounding_box(text: &str, font: &Resource, typesetting: TypesettingConfig, for_clipping_test: bool) -> DVec2 {
	TextContext::with_thread_local(|ctx| ctx.bounding_box(text, font, typesetting, for_clipping_test))
}

pub fn lines_clipping(text: &str, font: &Resource, typesetting: TypesettingConfig) -> bool {
	TextContext::with_thread_local(|ctx| ctx.lines_clipping(text, font, typesetting))
}

/// Shapes each string item of a styled `List<String>` into vector geometry, reading its font and typesetting
/// from the item's attributes (as set by the 'Text Layer' node) and re-applying its transform and blending
/// attributes onto the produced paths. With `separate_glyphs`, each glyph becomes its own item.
pub fn shape_text_list(strings: &List<String>, separate_glyphs: bool) -> List<Vector> {
	let mut result = List::new();

	for index in 0..strings.len() {
		let Some(text) = strings.element(index) else { continue };
		if text.is_empty() {
			continue;
		}

		// Use fallback font when none is explicitly attached.
		let font: Resource = {
			let f: Resource = strings.attribute_cloned_or_default(ATTR_TEXT_FONT, index);
			if f.is_empty() { super::FALLBACK_FONT_RESOURCE.clone() } else { f }
		};

		let defaults = TypesettingConfig::default();
		let typesetting = TypesettingConfig {
			font_size: strings.attribute_cloned_or(ATTR_FONT_SIZE, index, defaults.font_size),
			line_height_ratio: strings.attribute_cloned_or(ATTR_TEXT_LINE_HEIGHT, index, defaults.line_height_ratio),
			character_spacing: strings.attribute_cloned_or(ATTR_TEXT_CHARACTER_SPACING, index, defaults.character_spacing),
			max_width: strings.attribute_cloned_or::<Option<f64>>(ATTR_TEXT_MAX_WIDTH, index, defaults.max_width),
			max_height: strings.attribute_cloned_or::<Option<f64>>(ATTR_TEXT_MAX_HEIGHT, index, defaults.max_height),
			tilt: strings.attribute_cloned_or(ATTR_TEXT_TILT, index, defaults.tilt),
			align: strings.attribute_cloned_or(ATTR_TEXT_ALIGN, index, defaults.align),
		};

		let vectors = to_path(text, &font, typesetting, separate_glyphs);
		let transform = strings.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, index);
		let layer_path = strings.attribute_cloned_or_default::<List<NodeId>>(ATTR_EDITOR_LAYER_PATH, index);
		let blend_mode = strings.attribute::<BlendMode>(ATTR_BLEND_MODE, index).copied();
		let opacity = strings.attribute::<f64>(ATTR_OPACITY, index).copied();
		let opacity_fill = strings.attribute::<f64>(ATTR_OPACITY_FILL, index).copied();

		for mut item in vectors.into_iter() {
			if transform != DAffine2::IDENTITY {
				let local = item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
				item.set_attribute(ATTR_TRANSFORM, transform * local);
			}
			if !layer_path.is_empty() {
				item.set_attribute(ATTR_EDITOR_LAYER_PATH, layer_path.clone());
			}
			if let Some(blend_mode) = blend_mode {
				item.set_attribute(ATTR_BLEND_MODE, blend_mode);
			}
			if let Some(opacity) = opacity {
				item.set_attribute(ATTR_OPACITY, opacity);
			}
			if let Some(opacity_fill) = opacity_fill {
				item.set_attribute(ATTR_OPACITY_FILL, opacity_fill);
			}
			result.push(item);
		}
	}

	result
}
