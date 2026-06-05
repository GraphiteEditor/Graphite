use super::TypesettingConfig;
use core::cell::RefCell;
use core_types::list::List;
use glam::DVec2;
use graphene_resource::{Resource, ResourceHash};
use parley::fontique::{Blob, FamilyId, FontInfo};
use parley::{AlignmentOptions, FontContext, Layout, LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty};
use std::collections::HashMap;
use vector_types::Vector;

use super::path_builder::PathBuilder;

thread_local! {
	static THREAD_TEXT: RefCell<TextContext> = RefCell::new(TextContext::default());
}

/// Unified thread-local text processing context that combines font and layout management
/// for efficient text rendering operations.
#[derive(Default)]
pub struct TextContext {
	font_context: FontContext,
	layout_context: LayoutContext<()>,
	font_info_cache: HashMap<ResourceHash, (FamilyId, FontInfo)>,
}

impl TextContext {
	/// Access the thread-local TextContext instance for text processing operations
	pub fn with_thread_local<F, R>(f: F) -> R
	where
		F: FnOnce(&mut TextContext) -> R,
	{
		THREAD_TEXT.with_borrow_mut(f)
	}

	/// Get or cache font information for the given font resource.
	fn get_font_info(&mut self, font: &Resource) -> Option<(String, FontInfo)> {
		let hash = font.hash();
		if let Some((family_id, font_info)) = self.font_info_cache.get(&hash)
			&& let Some(family_name) = self.font_context.collection.family_name(*family_id)
		{
			return Some((family_name.to_string(), font_info.clone()));
		}

		let families = self.font_context.collection.register_fonts(Blob::new(font.into()), None);

		families.first().and_then(|(family_id, fonts_info)| {
			fonts_info.first().and_then(|font_info| {
				self.font_context.collection.family_name(*family_id).map(|family_name| {
					self.font_info_cache.insert(hash, (*family_id, font_info.clone()));
					(family_name.to_string(), font_info.clone())
				})
			})
		})
	}

	/// Create a text layout from the given font resource and typesetting configuration.
	pub fn layout_text(&mut self, text: &str, font: &Resource, typesetting: TypesettingConfig) -> Option<Layout<()>> {
		let (font_family, font_info) = self.get_font_info(font)?;

		const DISPLAY_SCALE: f32 = 1.;
		let mut builder = self.layout_context.ranged_builder(&mut self.font_context, text, DISPLAY_SCALE, false);

		builder.push_default(StyleProperty::FontSize(typesetting.font_size as f32));
		builder.push_default(StyleProperty::LetterSpacing(typesetting.character_spacing as f32));
		builder.push_default(StyleProperty::FontFamily(parley::FontFamily::Single(parley::FontFamilyName::Named(std::borrow::Cow::Owned(
			font_family,
		)))));
		builder.push_default(StyleProperty::FontWeight(font_info.weight()));
		builder.push_default(StyleProperty::FontStyle(font_info.style()));
		builder.push_default(StyleProperty::FontWidth(font_info.width()));
		builder.push_default(LineHeight::FontSizeRelative(typesetting.line_height_ratio as f32));

		let mut layout: Layout<()> = builder.build(text);

		layout.break_all_lines(typesetting.max_width.map(|mw| mw as f32));
		layout.align(typesetting.align.into(), AlignmentOptions::default());

		Some(layout)
	}

	/// Convert text to vector paths using the specified font and typesetting configuration
	pub fn to_path(&mut self, text: &str, font: &Resource, typesetting: TypesettingConfig, per_glyph_items: bool) -> List<Vector> {
		let Some(layout) = self.layout_text(text, font, typesetting) else {
			return List::new_from_element(Vector::default());
		};

		let text_frame_size = DVec2::new(
			typesetting.max_width.unwrap_or_else(|| layout.full_width() as f64),
			typesetting.max_height.unwrap_or_else(|| layout.height() as f64),
		);

		// First glyph offset (pre-height-filter) so the empty placeholder item in `per_glyph_items`
		// mode keeps the same item 0's transform, preventing `local_transforms` from jumping mid-drag
		let first_glyph_offset = layout
			.lines()
			.flat_map(|line| line.items())
			.find_map(|item| match item {
				PositionedLayoutItem::GlyphRun(run) => run.glyphs().next().map(|glyph| DVec2::new((run.offset() + glyph.x) as f64, (run.baseline() - glyph.y) as f64)),
				_ => None,
			})
			.unwrap_or_default();

		let alignment_width = typesetting.max_width.map(|w| w as f32).unwrap_or_else(|| layout.full_width());
		let last_line_correction = typesetting.align.last_line_correction();

		let mut path_builder = PathBuilder::new(per_glyph_items, layout.scale() as f64, text_frame_size, first_glyph_offset);

		for line in layout.lines() {
			let range = line.text_range();
			// Parley always includes a hard-break `\n` as the last byte of the preceding line's range, so the line
			// is at the end of a paragraph if it's the very last line of the buffer or its text ends with `\n`.
			let is_last_para_line = range.end == text.len() || text.get(range.clone()).is_some_and(|s| s.ends_with('\n'));

			let (x_offset, space_extra) = if let (true, Some(correction)) = (is_last_para_line, last_line_correction) {
				let metrics = line.metrics();
				let content_advance = metrics.advance - metrics.trailing_whitespace;
				let free_space = alignment_width - content_advance;

				match correction {
					parley::Alignment::Center => (free_space * 0.5, 0.),
					parley::Alignment::Right => (free_space, 0.),
					parley::Alignment::Justify => {
						// Exclude trailing-whitespace clusters from the divisor so the redistribution stretches only the internal spaces.
						// Parley's `trailing_whitespace` is in advance units, not bytes, so we re-derive the byte boundary here to filter cluster ranges.
						let line_text = text.get(range.clone()).unwrap_or("");
						let trailing_len = line_text.len() - line_text.trim_end().len();
						let visible_end_index = range.end - trailing_len;

						let space_count: usize = line
							.runs()
							.map(|run| run.clusters().filter(|c| c.is_space_or_nbsp() && c.text_range().start < visible_end_index).count())
							.sum();
						let extra = if space_count > 0 { free_space / space_count as f32 } else { 0. };
						(0., extra)
					}
					_ => (0., 0.),
				}
			} else {
				(0., 0.)
			};

			for item in line.items() {
				if let PositionedLayoutItem::GlyphRun(glyph_run) = item
					&& typesetting.max_height.filter(|&max_height| glyph_run.baseline() > max_height as f32).is_none()
				{
					path_builder.render_glyph_run(&glyph_run, typesetting.tilt, per_glyph_items, x_offset, space_extra);
				}
			}
		}

		path_builder.finalize()
	}

	/// Calculate the bounding box of text using the specified font and typesetting configuration
	pub fn bounding_box(&mut self, text: &str, font: &Resource, typesetting: TypesettingConfig, for_clipping_test: bool) -> DVec2 {
		let Some(layout) = self.layout_text(text, font, typesetting) else {
			return DVec2::ZERO;
		};

		let layout_width = layout.full_width() as f64;
		let layout_height = layout.height() as f64;

		if for_clipping_test {
			return DVec2::new(layout_width, layout_height);
		}

		let width = typesetting.max_width.unwrap_or(layout_width);
		let height = typesetting.max_height.unwrap_or(layout_height);

		DVec2::new(width, height)
	}

	/// Check if text lines are being clipped due to height constraints
	pub fn lines_clipping(&mut self, text: &str, font: &Resource, typesetting: TypesettingConfig) -> bool {
		let Some(max_height) = typesetting.max_height else { return false };
		let bounds = self.bounding_box(text, font, typesetting, true);
		max_height < bounds.y
	}
}
