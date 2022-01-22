use std::str::Chars;

use kurbo::{BezPath, Point, Vec2};
use rustybuzz::{GlyphPosition, UnicodeBuffer};
use ttf_parser::{GlyphId, OutlineBuilder};

struct Builder {
	path: BezPath,
	pos: Point,
	offset: Vec2,
	ascender: f64,
	scale: f64,
}

impl OutlineBuilder for Builder {
	fn move_to(&mut self, x: f32, y: f32) {
		self.path.move_to(self.pos + self.offset + Vec2::new(x as f64, self.ascender - y as f64) * self.scale);
	}

	fn line_to(&mut self, x: f32, y: f32) {
		self.path.line_to(self.pos + self.offset + Vec2::new(x as f64, self.ascender - y as f64) * self.scale);
	}

	fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
		self.path.quad_to(
			self.pos + self.offset + Vec2::new(x1 as f64, self.ascender - y1 as f64) * self.scale,
			self.pos + self.offset + Vec2::new(x2 as f64, self.ascender - y2 as f64) * self.scale,
		);
	}

	fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
		self.path.curve_to(
			self.pos + self.offset + Vec2::new(x1 as f64, self.ascender - y1 as f64) * self.scale,
			self.pos + self.offset + Vec2::new(x2 as f64, self.ascender - y2 as f64) * self.scale,
			self.pos + self.offset + Vec2::new(x3 as f64, self.ascender - y3 as f64) * self.scale,
		);
	}

	fn close(&mut self) {
		self.path.close_path();
	}
}

fn word_over_length(mut str: Chars, positions: &[GlyphPosition], mut index: usize, mut pos: f64, scale: f64, line_width: f64) -> bool {
	assert_eq!(str.nth(index), Some(' '));
	pos += positions[index].x_advance as f64 * scale;

	for c in str {
		index += 1;

		if c == ' ' || c == '\n' {
			return false;
		}

		pos += positions[index].x_advance as f64 * scale;

		if pos > line_width {
			return true;
		}
	}
	false
}

pub fn to_kurbo(str: &str, buzz_face: rustybuzz::Face, font_size: f64, line_width: f64) -> BezPath {
	let scale = (buzz_face.units_per_em() as f64).recip() * font_size;
	let line_hight = font_size;

	let mut buffer = UnicodeBuffer::new();
	buffer.push_str(str);
	let glyph_buffer = rustybuzz::shape(&buzz_face, &[], buffer);

	let mut builder = Builder {
		path: BezPath::new(),
		pos: Point::ZERO,
		offset: Vec2::ZERO,
		ascender: buzz_face.ascender() as f64,
		scale,
	};

	let positions = glyph_buffer.glyph_positions();

	for (index, ((char, pos), info)) in str.chars().zip(positions).zip(glyph_buffer.glyph_infos()).enumerate() {
		if char == '\n' || (char == ' ' && word_over_length(str.chars(), positions, index, builder.pos.x, scale, line_width)) {
			builder.pos = Point::new(0., builder.pos.y + line_hight);
		} else {
			if builder.pos.x + (pos.x_advance as f64 * scale) >= line_width {
				builder.pos = Point::new(0., builder.pos.y + line_hight);
			}
			builder.offset = Vec2::new(pos.x_offset as f64, pos.y_offset as f64) * scale;
			buzz_face.outline_glyph(GlyphId(info.glyph_id as u16), &mut builder);
			builder.pos += Vec2::new(pos.x_advance as f64, pos.y_advance as f64) * scale;
		}
	}
	builder.path
}

#[test]
fn test() {
	use std::fs::File;
	use std::io::Write;

	let buzz_face = rustybuzz::Face::from_slice(include_bytes!("SourceSansPro/SourceSansPro-Regular.ttf"), 0).unwrap();

	let text = r#"The quick brown
fox jumped over the lazy cat.
In publishing and graphic design, Lorem ipsum is a placeholder text commonly used to demonstrate the visual form of a document or a typeface without relying on meaningful content. Lorem ipsum may be used as a placeholder before the final copy is available. It is also used to temporarily replace text in a process called greeking, which allows designers to consider the form of a webpage or publication, without the meaning of the text influencing the design.

Lorem ipsum is typically a corrupted version of De finibus bonorum et malorum, a 1st-century BC text by the Roman statesman and philosopher Cicero, with words altered, added, and removed to make it nonsensical and improper Latin.

Test for really long word: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"#;
	let svg = to_kurbo(text, buzz_face, 20., 400.).to_svg();

	let mut file = File::create("src/layers/text/SourceSansPro/font_text.svg").unwrap();
	write!(
		&mut file,
		r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><path d="{}" /></svg>"#,
		svg
	)
	.unwrap();
}
