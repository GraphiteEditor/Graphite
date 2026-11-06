use super::TypesettingConfig;
use super::text_context::{TextContext, TextToVectorError};
use core_types::blending::BlendMode;
use core_types::list::{Item, List, NodeIdPath};
use core_types::{
	ATTR_BLEND_MODE, ATTR_EDITOR_LAYER_PATH, ATTR_FONT, ATTR_FONT_SIZE, ATTR_LETTER_SPACING, ATTR_LETTER_TILT, ATTR_LINE_HEIGHT, ATTR_MAX_HEIGHT, ATTR_MAX_WIDTH, ATTR_OPACITY, ATTR_OPACITY_FILL,
	ATTR_TEXT_ALIGN, ATTR_TRANSFORM,
};
use glam::{DAffine2, DVec2};
use graphene_resource::Resource;
use vector_types::Vector;

pub fn to_path(text: &str, font: &Resource, typesetting: TypesettingConfig, per_glyph_items: bool) -> Result<List<Vector>, TextToVectorError> {
	TextContext::with_thread_local(|ctx| ctx.to_path(text, font, typesetting, per_glyph_items))
}

pub fn bounding_box(text: &str, font: &Resource, typesetting: TypesettingConfig, for_clipping_test: bool) -> DVec2 {
	TextContext::with_thread_local(|ctx| ctx.bounding_box(text, font, typesetting, for_clipping_test))
}

pub fn lines_clipping(text: &str, font: &Resource, typesetting: TypesettingConfig) -> bool {
	TextContext::with_thread_local(|ctx| ctx.lines_clipping(text, font, typesetting))
}

/// The item's typography attribute, `None` when it is absent, or an error when it holds a type that text doesn't read.
fn typography_attribute<T: Clone + 'static>(item: &Item<String>, key: &'static str) -> Result<Option<T>, TextToVectorError> {
	let Some(value) = item.attributes().get_any(key) else { return Ok(None) };
	value.downcast_ref::<T>().cloned().map(Some).ok_or(TextToVectorError::MistypedAttribute(key))
}

/// Shapes a single styled string item into vector geometry, reading its font and typesetting from the item's
/// attributes (as set by the 'Text' node) and re-applying its transform and blending attributes onto the produced
/// paths. With `separate_glyphs`, each glyph becomes its own item; otherwise a single compound path is produced.
pub fn shape_text_item(item: &Item<String>, separate_glyphs: bool) -> Result<List<Vector>, TextToVectorError> {
	let text = item.element();
	if text.is_empty() {
		return Ok(List::new());
	}

	// Use fallback font when none is explicitly attached.
	let font = typography_attribute::<Resource>(item, ATTR_FONT)?.filter(|font| !font.is_empty());
	let font = font.unwrap_or_else(|| super::FALLBACK_FONT_RESOURCE.clone());

	let defaults = TypesettingConfig::default();
	let typesetting = TypesettingConfig {
		font_size: typography_attribute(item, ATTR_FONT_SIZE)?.unwrap_or(defaults.font_size),
		line_height_ratio: typography_attribute(item, ATTR_LINE_HEIGHT)?.unwrap_or(defaults.line_height_ratio),
		letter_spacing: typography_attribute(item, ATTR_LETTER_SPACING)?.unwrap_or(defaults.letter_spacing),
		letter_tilt: typography_attribute(item, ATTR_LETTER_TILT)?.unwrap_or(defaults.letter_tilt),
		max_width: typography_attribute::<f64>(item, ATTR_MAX_WIDTH)?.filter(|width| *width > 0.),
		max_height: typography_attribute::<f64>(item, ATTR_MAX_HEIGHT)?.filter(|height| *height > 0.),
		align: typography_attribute(item, ATTR_TEXT_ALIGN)?.unwrap_or(defaults.align),
	};

	let vectors = to_path(text, &font, typesetting, separate_glyphs)?;
	let transform = item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
	let layer_path = item.attribute::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH).cloned();
	let blend_mode = item.attribute::<BlendMode>(ATTR_BLEND_MODE).copied();
	let opacity = item.attribute::<f64>(ATTR_OPACITY).copied();
	let opacity_fill = item.attribute::<f64>(ATTR_OPACITY_FILL).copied();

	let mut result = List::new();
	for mut produced in vectors.into_iter() {
		if transform != DAffine2::IDENTITY {
			let local = produced.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
			produced.set_attribute(ATTR_TRANSFORM, transform * local);
		}
		if let Some(layer_path) = &layer_path {
			produced.set_attribute(ATTR_EDITOR_LAYER_PATH, layer_path.clone());
		}
		if let Some(blend_mode) = blend_mode {
			produced.set_attribute(ATTR_BLEND_MODE, blend_mode);
		}
		if let Some(opacity) = opacity {
			produced.set_attribute(ATTR_OPACITY, opacity);
		}
		if let Some(opacity_fill) = opacity_fill {
			produced.set_attribute(ATTR_OPACITY_FILL, opacity_fill);
		}
		result.push(produced);
	}

	Ok(result)
}

/// Shapes each string item of a styled `List<String>` into vector geometry, flattening the per-item results.
pub fn shape_text_list(strings: &List<String>, separate_glyphs: bool) -> Result<List<Vector>, TextToVectorError> {
	let mut result = List::new();

	for index in 0..strings.len() {
		let Some(item) = strings.clone_item(index) else { continue };
		for produced in shape_text_item(&item, separate_glyphs)?.into_iter() {
			result.push(produced);
		}
	}

	Ok(result)
}
