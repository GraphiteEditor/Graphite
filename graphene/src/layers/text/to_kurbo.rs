use glam::DVec2;
use kurbo::{BezPath, Point, Vec2};
use rustybuzz::{GlyphBuffer, UnicodeBuffer};
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

pub fn to_kurbo(str: &str, buzz_face: rustybuzz::Face, font_size: f64, line_width: Option<f64>) -> BezPath {
	let (scale, line_height, mut buffer) = font_properties(&buzz_face, font_size);

	let mut builder = Builder {
		path: BezPath::new(),
		pos: Point::ZERO,
		offset: Vec2::ZERO,
		ascender: buzz_face.ascender() as f64,
		scale,
	};

	for line in str.split('\n') {
		let length = line.split(' ').count();
		for (index, word) in line.split(' ').enumerate() {
			push_str(&mut buffer, word, index != length - 1);

			let glyph_buffer = rustybuzz::shape(&buzz_face, &[], buffer);

			if wrap_word(line_width, &glyph_buffer, scale, builder.pos.x) {
				builder.pos = Point::new(0., builder.pos.y + line_height);
			}

			for (glyph_position, glyph_info) in glyph_buffer.glyph_positions().iter().zip(glyph_buffer.glyph_infos()) {
				if let Some(line_width) = line_width {
					if builder.pos.x + (glyph_position.x_advance as f64 * builder.scale) >= line_width {
						builder.pos = Point::new(0., builder.pos.y + line_height);
					}
				}
				builder.offset = Vec2::new(glyph_position.x_offset as f64, glyph_position.y_offset as f64) * builder.scale;
				buzz_face.outline_glyph(GlyphId(glyph_info.glyph_id as u16), &mut builder);
				builder.pos += Vec2::new(glyph_position.x_advance as f64, glyph_position.y_advance as f64) * builder.scale;
			}

			buffer = glyph_buffer.clear();
		}
		builder.pos = Point::new(0., builder.pos.y + line_height);
	}
	builder.path
}

pub fn bounding_box(str: &str, buzz_face: rustybuzz::Face, font_size: f64, line_width: Option<f64>) -> DVec2 {
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
	let svg = to_kurbo(text, buzz_face, 20., Some(400.)).to_svg();

	let mut file = File::create("src/layers/text/SourceSansPro/font_text.svg").unwrap();
	write!(
		&mut file,
		r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><path d="{}" /></svg>"#,
		svg
	)
	.unwrap();
}
