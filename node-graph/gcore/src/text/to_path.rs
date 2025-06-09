use crate::vector::PointId;
use bezier_rs::{ManipulatorGroup, Subpath};
use dyn_any::DynAny;
use glam::DVec2;
use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};
use rustybuzz::{GlyphBuffer, GlyphPosition, UnicodeBuffer};

struct Builder {
	current_subpath: Subpath<PointId>,
	other_subpaths: Vec<Subpath<PointId>>,
	text_cursor: DVec2,
	offset: DVec2,
	ascender: f64,
	scale: f64,
	id: PointId,
}

impl Builder {
	fn point(&self, x: f32, y: f32) -> DVec2 {
		self.text_cursor + self.offset + DVec2::new(x as f64, self.ascender - y as f64) * self.scale
	}
}

impl OutlineBuilder for Builder {
	fn move_to(&mut self, x: f32, y: f32) {
		if !self.current_subpath.is_empty() {
			self.other_subpaths.push(std::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
		}
		self.current_subpath.push_manipulator_group(ManipulatorGroup::new_anchor_with_id(self.point(x, y), self.id.next_id()));
	}

	fn line_to(&mut self, x: f32, y: f32) {
		self.current_subpath.push_manipulator_group(ManipulatorGroup::new_anchor_with_id(self.point(x, y), self.id.next_id()));
	}

	fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
		let [handle, anchor] = [self.point(x1, y1), self.point(x2, y2)];
		self.current_subpath.last_manipulator_group_mut().unwrap().out_handle = Some(handle);
		self.current_subpath.push_manipulator_group(ManipulatorGroup::new_with_id(anchor, None, None, self.id.next_id()));
	}

	fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
		let [handle1, handle2, anchor] = [self.point(x1, y1), self.point(x2, y2), self.point(x3, y3)];
		self.current_subpath.last_manipulator_group_mut().unwrap().out_handle = Some(handle1);
		self.current_subpath
			.push_manipulator_group(ManipulatorGroup::new_with_id(anchor, Some(handle2), None, self.id.next_id()));
	}

	fn close(&mut self) {
		self.current_subpath.set_closed(true);
		self.other_subpaths.push(std::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
	}
}

fn font_properties(buzz_face: &rustybuzz::Face, font_size: f64, line_height_ratio: f64) -> (f64, f64, UnicodeBuffer) {
	let scale = (buzz_face.units_per_em() as f64).recip() * font_size;
	let line_height = font_size * line_height_ratio;
	let buffer = UnicodeBuffer::new();
	(scale, line_height, buffer)
}

fn push_str(buffer: &mut UnicodeBuffer, word: &str) {
	buffer.push_str(word);
}

fn wrap_word(max_width: Option<f64>, glyph_buffer: &GlyphBuffer, font_size: f64, character_spacing: f64, x_pos: f64, space_glyph: Option<GlyphId>) -> bool {
	if let Some(max_width) = max_width {
		// We don't word wrap spaces (to match the browser)
		let all_glyphs = glyph_buffer.glyph_positions().iter().zip(glyph_buffer.glyph_infos());
		let non_space_glyphs = all_glyphs.take_while(|(_, info)| space_glyph != Some(GlyphId(info.glyph_id as u16)));
		let word_length: f64 = non_space_glyphs.map(|(pos, _)| pos.x_advance as f64 * character_spacing).sum();
		let scaled_word_length = word_length * font_size;

		if scaled_word_length + x_pos > max_width {
			return true;
		}
	}
	false
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, specta::Type, node_macro::ChoiceType, DynAny)]
#[widget(Radio)]
pub enum TextAlignment {
	#[default]
	Left,
	Center,
	Right,
}

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TypesettingConfig {
	pub font_size: f64,
	pub line_height_ratio: f64,
	pub character_spacing: f64,
	pub max_width: Option<f64>,
	pub max_height: Option<f64>,
	pub text_alignment: TextAlignment,
}

impl Default for TypesettingConfig {
	fn default() -> Self {
		Self {
			font_size: 24.,
			line_height_ratio: 1.2,
			character_spacing: 1.,
			max_width: None,
			max_height: None,
			text_alignment: TextAlignment::Left,
		}
	}
}

#[derive(Default, Debug)]
struct GlyphRow {
	glyphs: Vec<(GlyphId, GlyphPosition)>,
	width: f64,
}

impl GlyphRow {
	fn append(&mut self, glyph_id: GlyphId, glyph_position: GlyphPosition, advance: f64) {
		self.width += advance;
		self.glyphs.push((glyph_id, glyph_position));
	}

	fn pop_trailing_space(&mut self, space_glyph: Option<GlyphId>, scale: f64, spacing: f64) {
		if let Some((last_glyph_id, _)) = self.glyphs.last() {
			if space_glyph == Some(*last_glyph_id) {
				self.width -= self.glyphs.last().map_or(0., |(_, pos)| pos.x_advance as f64 * scale * spacing);
				self.glyphs.pop();
			}
		}
	}
}

fn precompute_shapes(input: &str, buzz_face: &rustybuzz::Face, typesetting: TypesettingConfig) -> Vec<GlyphRow> {
	let space_glyph = buzz_face.glyph_index(' ');
	let mut shaped_lines = Vec::new();
	let (scale, line_height, mut buffer) = font_properties(buzz_face, typesetting.font_size, typesetting.line_height_ratio);
	for line in input.lines() {
		let mut current_line = GlyphRow::default();
		for word in SplitWordsIncludingSpaces::new(line) {
			push_str(&mut buffer, word);
			let glyph_buffer = rustybuzz::shape(buzz_face, &[], buffer);

			// Don't wrap the first word
			if !current_line.glyphs.is_empty() && wrap_word(typesetting.max_width, &glyph_buffer, scale, typesetting.character_spacing, current_line.width, space_glyph) {
				// use a trailing space only for wrapping and do no account for len
				current_line.pop_trailing_space(space_glyph, scale, typesetting.character_spacing);
				shaped_lines.push(core::mem::take(&mut current_line));
			}

			for (glyph_position, glyph_info) in glyph_buffer.glyph_positions().iter().zip(glyph_buffer.glyph_infos()) {
				let advance = glyph_position.x_advance as f64 * scale * typesetting.character_spacing;
				let glyph_id = GlyphId(glyph_info.glyph_id as u16);
				if typesetting
					.max_width
					.is_some_and(|max_width| space_glyph != Some(glyph_id) && current_line.width + advance >= max_width)
				{
					shaped_lines.push(core::mem::take(&mut current_line));
				}
				current_line.append(glyph_id, *glyph_position, advance);

				// Clip when the height is exceeded
				if typesetting.max_height.is_some_and(|max_height| shaped_lines.len() as f64 * line_height > max_height - line_height) {
					return shaped_lines;
				}
			}
			buffer = glyph_buffer.clear();
		}
		// use a trailing space only for wrapping and do no account for len
		current_line.pop_trailing_space(space_glyph, scale, typesetting.character_spacing);
		shaped_lines.push(core::mem::take(&mut current_line));
	}
	shaped_lines
}

fn render_shapes(shaped_lines: Vec<GlyphRow>, typesetting: TypesettingConfig, buzz_face: &rustybuzz::Face) -> Vec<Subpath<PointId>> {
	let overall_width = typesetting
		.max_width
		.unwrap_or_else(|| shaped_lines.iter().max_by_key(|line| line.width as u64).map_or(0., |x| x.width));
	let (scale, line_height, _) = font_properties(buzz_face, typesetting.font_size, typesetting.line_height_ratio);

	let mut builder = Builder {
		current_subpath: Subpath::new(Vec::new(), false),
		other_subpaths: Vec::new(),
		text_cursor: DVec2::ZERO,
		offset: DVec2::ZERO,
		ascender: (buzz_face.ascender() as f64 / buzz_face.height() as f64) * typesetting.font_size / scale,
		scale,
		id: PointId::ZERO,
	};

	for (line_number, glyph_line) in shaped_lines.into_iter().enumerate() {
		let x_offset = alignment_offset(typesetting.text_alignment, glyph_line.width, overall_width);
		builder.text_cursor = DVec2::new(x_offset, line_number as f64 * line_height);
		for (glyph_id, glyph_position) in glyph_line.glyphs {
			builder.offset = DVec2::new(glyph_position.x_offset as f64, glyph_position.y_offset as f64) * builder.scale;
			buzz_face.outline_glyph(glyph_id, &mut builder);

			if !builder.current_subpath.is_empty() {
				builder.other_subpaths.push(std::mem::replace(&mut builder.current_subpath, Subpath::new(Vec::new(), false)));
			}

			builder.text_cursor += DVec2::new(glyph_position.x_advance as f64 * typesetting.character_spacing, glyph_position.y_advance as f64) * builder.scale;
		}
	}
	builder.other_subpaths
}

fn alignment_offset(align: TextAlignment, line_width: f64, total_width: f64) -> f64 {
	match align {
		TextAlignment::Left => 0.,
		TextAlignment::Center => (total_width - line_width) / 2.,
		TextAlignment::Right => total_width - line_width,
	}
	.max(0.)
}

pub fn to_path(str: &str, buzz_face: Option<rustybuzz::Face>, typesetting: TypesettingConfig) -> Vec<Subpath<PointId>> {
	let Some(buzz_face) = buzz_face else { return vec![] };

	let all_shapes = precompute_shapes(str, &buzz_face, typesetting);
	render_shapes(all_shapes, typesetting, &buzz_face)
}

pub fn bounding_box(str: &str, buzz_face: Option<&rustybuzz::Face>, typesetting: TypesettingConfig, for_clipping_test: bool) -> DVec2 {
	// Show blank layer if font has not loaded
	let Some(buzz_face) = buzz_face else { return DVec2::ZERO };
	let space_glyph = buzz_face.glyph_index(' ');

	let (scale, line_height, mut buffer) = font_properties(buzz_face, typesetting.font_size, typesetting.line_height_ratio);

	let [mut text_cursor, mut bounds] = [DVec2::ZERO; 2];
	if !for_clipping_test {
		if let (Some(max_height), Some(max_width)) = (typesetting.max_height, typesetting.max_width) {
			return DVec2::new(max_width, max_height);
		}
	}

	for line in str.split('\n') {
		for (index, word) in SplitWordsIncludingSpaces::new(line).enumerate() {
			push_str(&mut buffer, word);

			let glyph_buffer = rustybuzz::shape(buzz_face, &[], buffer);

			// Don't wrap the first word
			if index != 0 && wrap_word(typesetting.max_width, &glyph_buffer, scale, typesetting.character_spacing, text_cursor.x, space_glyph) {
				text_cursor = DVec2::new(0., text_cursor.y + line_height);
			}

			for (glyph_position, glyph_info) in glyph_buffer.glyph_positions().iter().zip(glyph_buffer.glyph_infos()) {
				let glyph_id = GlyphId(glyph_info.glyph_id as u16);
				if let Some(max_width) = typesetting.max_width {
					if space_glyph != Some(glyph_id) && text_cursor.x + (glyph_position.x_advance as f64 * scale * typesetting.character_spacing) >= max_width {
						text_cursor = DVec2::new(0., text_cursor.y + line_height);
					}
				}
				text_cursor += DVec2::new(glyph_position.x_advance as f64 * typesetting.character_spacing, glyph_position.y_advance as f64) * scale;
				bounds = bounds.max(text_cursor + DVec2::new(0., line_height));
			}

			buffer = glyph_buffer.clear();
		}
		text_cursor = DVec2::new(0., text_cursor.y + line_height);
		bounds = bounds.max(text_cursor);
	}

	if !for_clipping_test {
		if let Some(max_width) = typesetting.max_width {
			bounds.x = max_width;
		}
		if let Some(max_height) = typesetting.max_height {
			bounds.y = max_height;
		}
	}

	bounds
}

pub fn load_face(data: &[u8]) -> rustybuzz::Face<'_> {
	rustybuzz::Face::from_slice(data, 0).expect("Loading font failed")
}

pub fn lines_clipping(str: &str, buzz_face: Option<rustybuzz::Face>, typesetting: TypesettingConfig) -> bool {
	let Some(max_height) = typesetting.max_height else { return false };
	let bounds = bounding_box(str, buzz_face.as_ref(), typesetting, true);
	max_height < bounds.y
}

struct SplitWordsIncludingSpaces<'a> {
	text: &'a str,
	start_byte: usize,
}

impl<'a> SplitWordsIncludingSpaces<'a> {
	pub fn new(text: &'a str) -> Self {
		Self { text, start_byte: 0 }
	}
}

impl<'a> Iterator for SplitWordsIncludingSpaces<'a> {
	type Item = &'a str;
	fn next(&mut self) -> Option<Self::Item> {
		let mut eaten_chars = self.text[self.start_byte..].char_indices().skip_while(|(_, c)| *c != ' ').skip_while(|(_, c)| *c == ' ');
		let start_byte = self.start_byte;
		self.start_byte = eaten_chars.next().map_or(self.text.len(), |(offset, _)| self.start_byte + offset);
		(self.start_byte > start_byte).then(|| self.text.get(start_byte..self.start_byte)).flatten()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn split_words_including_spaces() {
		let mut split_words = SplitWordsIncludingSpaces::new("hello  world     .");
		assert_eq!(split_words.next(), Some("hello  "));
		assert_eq!(split_words.next(), Some("world     "));
		assert_eq!(split_words.next(), Some("."));
		assert_eq!(split_words.next(), None);
	}

	#[cfg(test)]
	fn test_font_face() -> rustybuzz::Face<'static> {
		// Simple font just consisting of rectangles
		let data = include_bytes!("./TestBoxFont.ttf") as &[u8];
		rustybuzz::Face::from_slice(data, 0).expect("Failed to load test font")
	}

	#[cfg(test)]
	fn height_diff(paths: &[Subpath<PointId>], idx1: usize, idx2: usize) -> f64 {
		let y1 = paths[idx1].manipulator_groups().first().unwrap().anchor.y;
		let y2 = paths[idx2].manipulator_groups().first().unwrap().anchor.y;
		(y2 - y1).abs()
	}

	#[test]
	fn test_empty_string_returns_no_paths() {
		let buzz_face = Some(test_font_face());
		let config = TypesettingConfig::default();

		let result = to_path("", buzz_face, config);
		assert!(result.is_empty());
	}

	#[test]
	fn test_simple_text_create_some_paths() {
		let buzz_face = Some(test_font_face());
		let config = TypesettingConfig::default();

		let result = to_path("foobar\nspam", buzz_face.clone(), config);
		assert_eq!(result.len(), 10, "Expected paths to be rendered for non-empty text");
		assert!(
			(height_diff(&result, 0, 6) - 28.8).abs() < 1e-6,
			"Expected line height to be 28.8 (default font size 24 * default line height ratio 1.2)"
		);
		assert_eq!(
			result[6].manipulator_groups().first().unwrap().anchor.x,
			0.0,
			"Expected first character to start at x=0 (default to flush left)"
		);
	}

	#[test]
	fn test_line_wrapping_on_max_width() {
		let buzz_face = Some(test_font_face());
		let config = TypesettingConfig {
			max_width: Some(20.0),
			..Default::default()
		};

		let result = to_path("AA AAA AAAAAA", buzz_face, config);
		let line_count = result
			.iter()
			.flat_map(|subpath| subpath.manipulator_groups().first())
			.map(|p| p.anchor.y as u32)
			.collect::<std::collections::HashSet<_>>();

		assert_eq!(line_count.len(), 4, "Expected 4 lines due to wrapping");
		assert_ne!(
			result[1].manipulator_groups().first().unwrap().anchor.y,
			result[2].manipulator_groups().first().unwrap().anchor.y,
			"Expected line wrapping to split lines between second and third character"
		);
		assert_ne!(
			result[4].manipulator_groups().first().unwrap().anchor.y,
			result[5].manipulator_groups().first().unwrap().anchor.y,
			"Expected line wrapping to split lines between fifth and sixth character"
		);
	}

	#[test]
	fn test_character_spacing() {
		for i in 0..10 {
			let character_spacing = 1. + i as f64 * 0.1;
			let buzz_face = Some(test_font_face());
			let config = TypesettingConfig {
				character_spacing,
				..Default::default()
			};
			let test_str = "fffff\nAAA";
			let result = to_path(test_str, buzz_face, config);
			let f_width = 6.;
			let a_width = 4.8;

			assert_eq!(result.len(), 8, "Expected 8 subpaths for '{}'", test_str);
			for (i, subpath) in result.iter().enumerate().take(5) {
				let expected_x = f_width * i as f64 * character_spacing;
				let actual_x = subpath.manipulator_groups().first().unwrap().anchor.x;
				let diff_ok = (actual_x - expected_x).abs() < 1e-6;
				assert!(
					diff_ok,
					"Expected x position of character {} to be {:.2}, but got {:.2} with character spacing {:.1}",
					i, expected_x, actual_x, character_spacing
				);
			}
			for (i, subpath) in result.iter().enumerate().skip(5) {
				let i = i - 5; // Adjust index for the second line
				let expected_x = a_width * i as f64 * character_spacing;
				let actual_x = subpath.manipulator_groups().first().unwrap().anchor.x;
				let diff_ok = (actual_x - expected_x).abs() < 1e-6;
				assert!(
					diff_ok,
					"Expected x position of character {} to be {:.2}, but got {:.2} with character spacing {:.1}",
					i, expected_x, actual_x, character_spacing
				);
			}
		}
	}

	#[test]
	fn test_test_line_height() {
		for i in 0..10 {
			for j in 20..30 {
				let line_height_ratio = 1. + i as f64 * 0.1;
				let buzz_face = Some(test_font_face());
				let config = TypesettingConfig {
					line_height_ratio,
					font_size: j as f64,
					..Default::default()
				};
				let test_str = "first line\nsecond line\nthird_line";
				let expected_line_height = config.font_size * config.line_height_ratio;
				let result = to_path(test_str, buzz_face, config);
				assert!((height_diff(&result, 8, 9) - expected_line_height).abs() < 1e-6, "Expected line height to be {expected_line_height}");
				assert!((height_diff(&result, 18, 19) - expected_line_height).abs() < 1e-6, "Expected line height to be {expected_line_height}");
			}
		}
	}

	#[test]
	fn test_text_alignment() {
		fn test_specific_alignment(text_alignment: TextAlignment) {
			let buzz_face = Some(test_font_face());
			let config = TypesettingConfig { text_alignment, ..Default::default() };
			let test_str = "short\nlongest line\nshort";
			let result = to_path(test_str, buzz_face, config);

			match text_alignment {
				TextAlignment::Left => {
					assert_eq!(result[0].manipulator_groups().first().unwrap().anchor.x, 0.0, "Expected left alignment to start at x=0");
					assert_eq!(result[5].manipulator_groups().first().unwrap().anchor.x, 0.0, "Expected left alignment to start at x=0");
					assert_eq!(result[16].manipulator_groups().first().unwrap().anchor.x, 0.0, "Expected left alignment to start at x=0");
				}
				TextAlignment::Center => {
					let longest_x = result[15].manipulator_groups().iter().skip(2).next().unwrap().anchor.x;

					let first_line_left = result[0].manipulator_groups().first().unwrap().anchor.x;
					let first_line_right = result[4].manipulator_groups().iter().skip(2).next().unwrap().anchor.x;
					let diff_first_right = longest_x - first_line_right;
					assert!(((diff_first_right - first_line_left).abs() < 1e-6), "Expected center alignment to have equal x for first two lines");

					let last_line_left = result[16].manipulator_groups().first().unwrap().anchor.x;
					let last_line_right = result.last().and_then(|g| g.manipulator_groups().iter().skip(2).next()).unwrap().anchor.x;
					let diff_first_right = longest_x - last_line_right;
					assert!(((diff_first_right - last_line_left).abs() < 1e-6), "Expected center alignment to have equal x for first two lines");
				}
				TextAlignment::Right => {
					let longest_x = result[15].manipulator_groups().iter().skip(2).next().unwrap().anchor.x;

					let first_line_x = result[4].manipulator_groups().iter().skip(2).next().unwrap().anchor.x;
					assert!((longest_x - first_line_x).abs() < 1e-6, "Expected right alignment to have equal x for first two lines");

					let last_x = result.last().and_then(|g| g.manipulator_groups().iter().skip(2).next()).unwrap().anchor.x;
					assert!((longest_x - last_x).abs() < 1e-6, "Expected right alignment to have equal x for first two lines");
				}
			}
		}

		test_specific_alignment(TextAlignment::Left);
		test_specific_alignment(TextAlignment::Center);
		test_specific_alignment(TextAlignment::Right);
	}

	#[test]
	fn test_alignment_offsets_unit() {
		let left = alignment_offset(TextAlignment::Left, 100.0, 500.0);
		let center = alignment_offset(TextAlignment::Center, 100.0, 500.0);
		let right = alignment_offset(TextAlignment::Right, 100.0, 500.0);

		assert_eq!(left, 0.0);
		assert_eq!(center, 200.0);
		assert_eq!(right, 400.0);
	}

	#[test]
	fn test_height_clipping() {
		let buzz_face = Some(test_font_face());
		let config = TypesettingConfig {
			max_height: Some(30.0),
			..Default::default()
		};

		let result = to_path("Line1\nLine2\nLine3\nLine4", buzz_face, config);
		let unique_lines = result
			.iter()
			.flat_map(|s| s.manipulator_groups().first())
			.map(|p| p.anchor.y as u32)
			.collect::<std::collections::HashSet<_>>();
		assert_eq!(unique_lines.len(), 3, "Too many lines rendered, max_height not respected. Expected last line to be clipped.");
	}
}
