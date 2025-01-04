use crate::vector::PointId;

use bezier_rs::{ManipulatorGroup, Subpath};

use glam::DVec2;
use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};
use rustybuzz::{GlyphBuffer, UnicodeBuffer};

struct Builder {
	current_subpath: Subpath<PointId>,
	other_subpaths: Vec<Subpath<PointId>>,
	pos: DVec2,
	offset: DVec2,
	ascender: f64,
	scale: f64,
	id: PointId,
}

impl Builder {
	fn point(&self, x: f32, y: f32) -> DVec2 {
		self.pos + self.offset + DVec2::new(x as f64, self.ascender - y as f64) * self.scale
	}
}

impl OutlineBuilder for Builder {
	fn move_to(&mut self, x: f32, y: f32) {
		if !self.current_subpath.is_empty() {
			self.other_subpaths.push(core::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
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
		self.other_subpaths.push(core::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
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

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TypesettingConfig {
	pub font_size: f64,
	pub line_height_ratio: f64,
	pub character_spacing: f64,
	pub max_width: Option<f64>,
	pub max_height: Option<f64>,
}

impl Default for TypesettingConfig {
	fn default() -> Self {
		Self {
			font_size: 24.,
			line_height_ratio: 1.2,
			character_spacing: 1.,
			max_width: None,
			max_height: None,
		}
	}
}

pub fn to_path(str: &str, buzz_face: Option<rustybuzz::Face>, typesetting: TypesettingConfig) -> Vec<Subpath<PointId>> {
	let buzz_face = match buzz_face {
		Some(face) => face,
		// Show blank layer if font has not loaded
		None => return vec![],
	};
	let space_glyph = buzz_face.glyph_index(' ');

	let (scale, line_height, mut buffer) = font_properties(&buzz_face, typesetting.font_size, typesetting.line_height_ratio);

	let mut builder = Builder {
		current_subpath: Subpath::new(Vec::new(), false),
		other_subpaths: Vec::new(),
		pos: DVec2::ZERO,
		offset: DVec2::ZERO,
		ascender: (buzz_face.ascender() as f64 / buzz_face.height() as f64) * typesetting.font_size / scale,
		scale,
		id: PointId::ZERO,
	};

	for line in str.split('\n') {
		for (index, word) in SplitWordsIncludingSpaces::new(line).enumerate() {
			push_str(&mut buffer, word);
			let glyph_buffer = rustybuzz::shape(&buzz_face, &[], buffer);

			// Don't wrap the first word
			if index != 0 && wrap_word(typesetting.max_width, &glyph_buffer, scale, typesetting.character_spacing, builder.pos.x, space_glyph) {
				builder.pos = DVec2::new(0., builder.pos.y + line_height);
			}

			for (glyph_position, glyph_info) in glyph_buffer.glyph_positions().iter().zip(glyph_buffer.glyph_infos()) {
				let glyph_id = GlyphId(glyph_info.glyph_id as u16);
				if let Some(max_width) = typesetting.max_width {
					if space_glyph != Some(glyph_id) && builder.pos.x + (glyph_position.x_advance as f64 * builder.scale * typesetting.character_spacing) >= max_width {
						builder.pos = DVec2::new(0., builder.pos.y + line_height);
					}
				}
				// Clip when the height is exceeded
				if typesetting.max_height.is_some_and(|max_height| builder.pos.y > max_height) {
					return builder.other_subpaths;
				}

				builder.offset = DVec2::new(glyph_position.x_offset as f64, glyph_position.y_offset as f64) * builder.scale;
				buzz_face.outline_glyph(glyph_id, &mut builder);
				if !builder.current_subpath.is_empty() {
					builder.other_subpaths.push(core::mem::replace(&mut builder.current_subpath, Subpath::new(Vec::new(), false)));
				}

				builder.pos += DVec2::new(glyph_position.x_advance as f64 * typesetting.character_spacing, glyph_position.y_advance as f64) * builder.scale;
			}

			buffer = glyph_buffer.clear();
		}

		builder.pos = DVec2::new(0., builder.pos.y + line_height);
	}

	builder.other_subpaths
}

pub fn bounding_box(str: &str, buzz_face: Option<rustybuzz::Face>, typesetting: TypesettingConfig) -> DVec2 {
	let buzz_face = match buzz_face {
		Some(face) => face,
		// Show blank layer if font has not loaded
		None => return DVec2::ZERO,
	};
	let space_glyph = buzz_face.glyph_index(' ');

	let (scale, line_height, mut buffer) = font_properties(&buzz_face, typesetting.font_size, typesetting.line_height_ratio);

	let mut pos = DVec2::ZERO;
	let mut bounds = DVec2::ZERO;

	for line in str.split('\n') {
		for (index, word) in SplitWordsIncludingSpaces::new(line).enumerate() {
			push_str(&mut buffer, word);

			let glyph_buffer = rustybuzz::shape(&buzz_face, &[], buffer);

			// Don't wrap the first word
			if index != 0 && wrap_word(typesetting.max_width, &glyph_buffer, scale, typesetting.character_spacing, pos.x, space_glyph) {
				pos = DVec2::new(0., pos.y + line_height);
			}

			for (glyph_position, glyph_info) in glyph_buffer.glyph_positions().iter().zip(glyph_buffer.glyph_infos()) {
				let glyph_id = GlyphId(glyph_info.glyph_id as u16);
				if let Some(max_width) = typesetting.max_width {
					if space_glyph != Some(glyph_id) && pos.x + (glyph_position.x_advance as f64 * scale * typesetting.character_spacing) >= max_width {
						pos = DVec2::new(0., pos.y + line_height);
					}
				}
				pos += DVec2::new(glyph_position.x_advance as f64 * typesetting.character_spacing, glyph_position.y_advance as f64) * scale;
				bounds = bounds.max(pos + DVec2::new(0., line_height));
			}

			buffer = glyph_buffer.clear();
		}
		pos = DVec2::new(0., pos.y + line_height);
		bounds = bounds.max(pos);
	}

	if let Some(max_width) = typesetting.max_width {
		bounds.x = max_width;
	}
	if let Some(max_height) = typesetting.max_height {
		bounds.y = max_height;
	}

	bounds
}

pub fn load_face(data: &[u8]) -> rustybuzz::Face {
	rustybuzz::Face::from_slice(data, 0).expect("Loading font failed")
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

#[test]
fn split_words_including_spaces() {
	let mut split_words = SplitWordsIncludingSpaces::new("hello  world     .");
	assert_eq!(split_words.next(), Some("hello  "));
	assert_eq!(split_words.next(), Some("world     "));
	assert_eq!(split_words.next(), Some("."));
	assert_eq!(split_words.next(), None);
}
