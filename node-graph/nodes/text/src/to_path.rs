use super::TypesettingConfig;
use super::text_context::TextContext;
use core_types::attr;
use core_types::list::{Item, List};
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

/// Shapes a single styled string item into vector geometry, reading its font and typesetting from the item's
/// attributes (as set by the 'Text' node) and re-applying its transform and blending attributes onto the produced
/// paths. With `separate_glyphs`, each glyph becomes its own item; otherwise a single compound path is produced.
pub fn shape_text_item(item: &Item<String>, separate_glyphs: bool) -> List<Vector> {
	let text = item.element();
	if text.is_empty() {
		return List::new();
	}

	// Use fallback font when none is explicitly attached.
	let font: Resource = {
		let font: Resource = item.attr_cloned_or_default::<crate::attr::Font>();
		if font.is_empty() { super::FALLBACK_FONT_RESOURCE.clone() } else { font }
	};

	let defaults = TypesettingConfig::default();
	let typesetting = TypesettingConfig {
		font_size: item.attr_cloned_or::<attr::FontSize>(defaults.font_size),
		line_height_ratio: item.attr_cloned_or::<attr::LineHeight>(defaults.line_height_ratio),
		letter_spacing: item.attr_cloned_or::<attr::LetterSpacing>(defaults.letter_spacing),
		letter_tilt: item.attr_cloned_or::<attr::LetterTilt>(defaults.letter_tilt),
		max_width: item.attr_cloned_or::<attr::MaxWidth>(defaults.max_width),
		max_height: item.attr_cloned_or::<attr::MaxHeight>(defaults.max_height),
		align: item.attr_cloned_or::<crate::attr::TextAlign>(defaults.align),
	};

	let vectors = to_path(text, &font, typesetting, separate_glyphs);
	let transform = item.attr_cloned_or_default::<attr::Transform>();
	let layer_path = item.attr::<attr::editor::LayerPath>().cloned();
	let blend_mode = item.attr::<attr::BlendMode>().copied();
	let opacity = item.attr::<attr::Opacity>().copied();
	let opacity_fill = item.attr::<attr::OpacityFill>().copied();

	let mut result = List::new();
	for mut produced in vectors.into_iter() {
		if transform != DAffine2::IDENTITY {
			let local = produced.attr_cloned_or_default::<attr::Transform>();
			produced.set_attr::<attr::Transform>(transform * local);
		}
		if let Some(layer_path) = &layer_path {
			produced.set_attr::<attr::editor::LayerPath>(layer_path.clone());
		}
		if let Some(blend_mode) = blend_mode {
			produced.set_attr::<attr::BlendMode>(blend_mode);
		}
		if let Some(opacity) = opacity {
			produced.set_attr::<attr::Opacity>(opacity);
		}
		if let Some(opacity_fill) = opacity_fill {
			produced.set_attr::<attr::OpacityFill>(opacity_fill);
		}
		result.push(produced);
	}

	result
}

/// Shapes each string item of a styled `List<String>` into vector geometry, flattening the per-item results.
pub fn shape_text_list(strings: &List<String>, separate_glyphs: bool) -> List<Vector> {
	let mut result = List::new();

	for index in 0..strings.len() {
		let Some(item) = strings.clone_item(index) else { continue };
		for produced in shape_text_item(&item, separate_glyphs).into_iter() {
			result.push(produced);
		}
	}

	result
}
