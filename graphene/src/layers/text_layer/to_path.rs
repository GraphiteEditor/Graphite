use crate::layers::vector::constants::ManipulatorType;
use crate::layers::vector::manipulator_group::ManipulatorGroup;
use crate::layers::vector::manipulator_point::ManipulatorPoint;
use crate::layers::vector::subpath::Subpath;

use glam::DVec2;
use rustybuzz::{GlyphBuffer, UnicodeBuffer};
use ttf_parser::{GlyphId, OutlineBuilder};

struct Builder {
	path: Subpath,
	pos: DVec2,
	offset: DVec2,
	ascender: f64,
	scale: f64,
}

impl Builder {
	fn point(&self, x: f32, y: f32) -> DVec2 {
		self.pos + self.offset + DVec2::new(x as f64, self.ascender - y as f64) * self.scale
	}
}

impl OutlineBuilder for Builder {
	fn move_to(&mut self, x: f32, y: f32) {
		let anchor = self.point(x, y);
		if self.path.groups().last().filter(|el| el.points.iter().any(Option::is_some)).is_some() {
			self.path.groups_mut().push_end(ManipulatorGroup::closed());
		}
		self.path.groups_mut().push_end(ManipulatorGroup::new_with_anchor(anchor));
	}

	fn line_to(&mut self, x: f32, y: f32) {
		let anchor = self.point(x, y);
		self.path.groups_mut().push_end(ManipulatorGroup::new_with_anchor(anchor));
	}

	fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
		let [handle, anchor] = [self.point(x1, y1), self.point(x2, y2)];
		self.path.groups_mut().last_mut().unwrap().points[ManipulatorType::OutHandle] = Some(ManipulatorPoint::new(handle, ManipulatorType::OutHandle));
		self.path.groups_mut().push_end(ManipulatorGroup::new_with_anchor(anchor));
	}

	fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
		let [handle1, handle2, anchor] = [self.point(x1, y1), self.point(x2, y2), self.point(x3, y3)];
		self.path.groups_mut().last_mut().unwrap().points[ManipulatorType::OutHandle] = Some(ManipulatorPoint::new(handle1, ManipulatorType::OutHandle));
		self.path.groups_mut().push_end(ManipulatorGroup::new_with_anchor(anchor));
		self.path.groups_mut().last_mut().unwrap().points[ManipulatorType::InHandle] = Some(ManipulatorPoint::new(handle2, ManipulatorType::InHandle));
	}

	fn close(&mut self) {
		self.path.groups_mut().push_end(ManipulatorGroup::closed());
	}
}

fn font_properties(buzz_face: &rustybuzz::Face, font_size: f64) -> (f64, f64, UnicodeBuffer) {
	let scale = (buzz_face.units_per_em() as f64).recip() * font_size;
	let line_height = font_size;
	let buffer = UnicodeBuffer::new();
	(scale, line_height, buffer)
}

fn push_str(buffer: &mut UnicodeBuffer, word: &str, trailing_space: bool) {
	buffer.push_str(word);

	if trailing_space {
		buffer.push_str(" ");
	}
}

fn wrap_word(line_width: Option<f64>, glyph_buffer: &GlyphBuffer, scale: f64, x_pos: f64) -> bool {
	if let Some(line_width) = line_width {
		let word_length: i32 = glyph_buffer.glyph_positions().iter().map(|pos| pos.x_advance).sum();
		let scaled_word_length = word_length as f64 * scale;

		if scaled_word_length + x_pos > line_width {
			return true;
		}
	}
	false
}

pub fn to_path(str: &str, buzz_face: Option<rustybuzz::Face>, font_size: f64, line_width: Option<f64>) -> Subpath {
	let buzz_face = match buzz_face {
		Some(face) => face,
		// Show blank layer if font has not loaded
		None => return Subpath::default(),
	};

	let (scale, line_height, mut buffer) = font_properties(&buzz_face, font_size);

	let mut builder = Builder {
		path: Subpath::new(),
		pos: DVec2::ZERO,
		offset: DVec2::ZERO,
		ascender: (buzz_face.ascender() as f64 / buzz_face.height() as f64) * font_size / scale,
		scale,
	};

	for line in str.split('\n') {
		let length = line.split(' ').count();
		for (index, word) in line.split(' ').enumerate() {
			push_str(&mut buffer, word, index != length - 1);
			let glyph_buffer = rustybuzz::shape(&buzz_face, &[], buffer);

			if wrap_word(line_width, &glyph_buffer, scale, builder.pos.x) {
				builder.pos = DVec2::new(0., builder.pos.y + line_height);
			}

			for (glyph_position, glyph_info) in glyph_buffer.glyph_positions().iter().zip(glyph_buffer.glyph_infos()) {
				if let Some(line_width) = line_width {
					if builder.pos.x + (glyph_position.x_advance as f64 * builder.scale) >= line_width {
						builder.pos = DVec2::new(0., builder.pos.y + line_height);
					}
				}
				builder.offset = DVec2::new(glyph_position.x_offset as f64, glyph_position.y_offset as f64) * builder.scale;
				buzz_face.outline_glyph(GlyphId(glyph_info.glyph_id as u16), &mut builder);
				builder.pos += DVec2::new(glyph_position.x_advance as f64, glyph_position.y_advance as f64) * builder.scale;
			}

			buffer = glyph_buffer.clear();
		}
		builder.pos = DVec2::new(0., builder.pos.y + line_height);
	}
	builder.path
}

pub fn bounding_box(str: &str, buzz_face: Option<rustybuzz::Face>, font_size: f64, line_width: Option<f64>) -> DVec2 {
	let buzz_face = match buzz_face {
		Some(face) => face,
		// Show blank layer if font has not loaded
		None => return DVec2::ZERO,
	};

	let (scale, line_height, mut buffer) = font_properties(&buzz_face, font_size);

	let mut pos = DVec2::ZERO;
	let mut bounds = DVec2::ZERO;

	for line in str.split('\n') {
		let length = line.split(' ').count();
		for (index, word) in line.split(' ').enumerate() {
			push_str(&mut buffer, word, index != length - 1);

			let glyph_buffer = rustybuzz::shape(&buzz_face, &[], buffer);

			if wrap_word(line_width, &glyph_buffer, scale, pos.x) {
				pos = DVec2::new(0., pos.y + line_height);
			}

			for glyph_position in glyph_buffer.glyph_positions() {
				if let Some(line_width) = line_width {
					if pos.x + (glyph_position.x_advance as f64 * scale) >= line_width {
						pos = DVec2::new(0., pos.y + line_height);
					}
				}
				pos += DVec2::new(glyph_position.x_advance as f64, glyph_position.y_advance as f64) * scale;
			}
			bounds = bounds.max(pos + DVec2::new(0., line_height));

			buffer = glyph_buffer.clear();
		}
		pos = DVec2::new(0., pos.y + line_height);
	}

	bounds
}
