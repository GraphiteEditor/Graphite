use crate::text::to_path::cosmic_text::Edit;
use crate::{uuid::ManipulatorGroupId, vector::VectorData};
use bezier_rs::{ManipulatorGroup, Subpath};
use glam::{DAffine2, DVec2, Vec2};
pub extern crate cosmic_text;
use super::{FontCache, RichText, TextSpan};
use core::cmp;
use unicode_segmentation::UnicodeSegmentation;

/// Builds subpaths out of a glyph outline
struct Builder {
	current_subpath: Subpath<ManipulatorGroupId>,
	other_subpaths: Vec<Subpath<ManipulatorGroupId>>,
	transform: DAffine2,
	ascender: f64,
	scale: f64,
	font_size: f64,
	bold: Option<f64>,
	italic: Option<f64>,
	id: ManipulatorGroupId,
}

impl Builder {
	fn convert_point(&self, x: f32, y: f32) -> DVec2 {
		self.transform
			.transform_point2(DVec2::new(x as f64 + y as f64 * self.font_size * self.italic.unwrap_or(0.) / self.ascender, 0. - y as f64) * self.scale)
	}
}

impl cosmic_text::rustybuzz::ttf_parser::OutlineBuilder for Builder {
	fn move_to(&mut self, x: f32, y: f32) {
		if !self.current_subpath.is_empty() {
			self.other_subpaths.push(core::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
		}
		self.current_subpath
			.push_manipulator_group(ManipulatorGroup::new_anchor_with_id(self.convert_point(x, y), self.id.next_id()));
	}

	fn line_to(&mut self, x: f32, y: f32) {
		self.current_subpath
			.push_manipulator_group(ManipulatorGroup::new_anchor_with_id(self.convert_point(x, y), self.id.next_id()));
	}

	fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
		let [handle, anchor] = [self.convert_point(x1, y1), self.convert_point(x2, y2)];
		self.current_subpath.last_manipulator_group_mut().unwrap().out_handle = Some(handle);
		self.current_subpath.push_manipulator_group(ManipulatorGroup::new_anchor_with_id(anchor, self.id.next_id()));
	}

	fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
		let [handle1, handle2, anchor] = [self.convert_point(x1, y1), self.convert_point(x2, y2), self.convert_point(x3, y3)];
		self.current_subpath.last_manipulator_group_mut().unwrap().out_handle = Some(handle1);
		self.current_subpath
			.push_manipulator_group(ManipulatorGroup::new_with_id(anchor, Some(handle2), None, self.id.next_id()));
	}

	fn close(&mut self) {
		self.current_subpath.set_closed(true);
		if let Some(bold) = self.bold {
			self.current_subpath = self.current_subpath.offset(-bold * self.scale * self.font_size, bezier_rs::Join::Miter(None));
		}
		self.other_subpaths.push(core::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
	}
}

#[must_use]
pub fn rich_text_to_path(text: &RichText, line_length: f64, path: &VectorData, font_cache: &FontCache) -> Vec<Subpath<ManipulatorGroupId>> {
	let Some(mut font_system) = font_cache.get_system() else { return Vec::new() };
	let mut buffer = construct_buffer(text, &mut font_system);

	create_buffer(&mut buffer, &mut font_system, text, font_cache, line_length);

	buffer_to_path(&buffer, &mut font_system, &text.spans, path)
}

#[must_use]
pub fn buffer_to_path(buffer: &cosmic_text::Buffer, font_system: &mut cosmic_text::FontSystem, spans: &[TextSpan], path: &VectorData) -> Vec<Subpath<ManipulatorGroupId>> {
	let mut builder = Builder {
		current_subpath: Subpath::new(Vec::new(), false),
		other_subpaths: Vec::new(),
		transform: DAffine2::IDENTITY,
		ascender: 0.,
		scale: 1.,
		font_size: 1.,
		bold: None,
		italic: None,
		id: ManipulatorGroupId::ZERO,
	};
	let subpath = path
		.stroke_bezier_paths()
		.map(|mut subpath| {
			subpath.apply_transform(path.transform);

			(subpath.iter().map(|bezier| bezier.length(None)).collect::<Vec<f64>>(), subpath)
		})
		.find(|(_, subpath)| !subpath.is_empty());

	// Inspect the output runs
	let mut offset;
	for run in buffer.layout_runs() {
		offset = DVec2::ZERO;
		for glyph_position in run.glyphs.iter() {
			let Some(font) = font_system.get_font(glyph_position.font_id) else { continue };
			let buzz_face = font.rustybuzz();
			builder.scale = glyph_position.font_size as f64 / buzz_face.units_per_em() as f64;
			builder.font_size = glyph_position.font_size as f64;
			builder.ascender = (buzz_face.ascender() as f64 / buzz_face.height() as f64) * glyph_position.font_size as f64 / builder.scale;
			let span = &spans[glyph_position.metadata];
			builder.bold = span.bold.map(|x| x as f64);
			builder.italic = span.italic.map(|x| x as f64);

			let glyph_offset = DVec2::new(glyph_position.x_offset as f64, glyph_position.y_offset as f64) + span.kerning.as_dvec2() + offset;
			if let Some((lengths, subpath)) = &subpath {
				let total_length: f64 = lengths.iter().sum();
				let eval_euclidean = |dist: f64| {
					let (segment_index, segment_t_euclidean) = subpath.global_euclidean_to_local_euclidean(dist / total_length, lengths.as_slice(), total_length);
					let segment = subpath.get_segment(segment_index).unwrap();
					let segment_t_parametric = segment.euclidean_to_parametric_with_total_length(segment_t_euclidean, 0., lengths[segment_index]);
					segment.evaluate(bezier_rs::TValue::Parametric(segment_t_parametric))
				};

				// Text on path based on https://svgwg.org/svg2-draft/text.html#TextpathLayoutRules
				let left_x = glyph_position.x as f64 + glyph_offset.x;
				let right_x = left_x + glyph_position.w as f64;
				let centre_x = (left_x + right_x) / 2.;
				if right_x >= total_length {
					break;
				}
				let left = eval_euclidean(left_x);
				let right = eval_euclidean(right_x);
				let centre = eval_euclidean(centre_x);
				let angle = DVec2::X.angle_between(right - left);
				let angle = if angle.is_finite() { angle } else { 0. };
				builder.transform = DAffine2::from_translation(centre) * DAffine2::from_angle(angle) * DAffine2::from_translation(DVec2::X * (left_x - centre_x));
			} else {
				let pos = DVec2::new(glyph_position.x as f64, glyph_position.y as f64 + run.line_y as f64);
				builder.transform = DAffine2::from_translation(pos + glyph_offset);
			}
			buzz_face.outline_glyph(cosmic_text::rustybuzz::ttf_parser::GlyphId(glyph_position.glyph_id), &mut builder);

			if &run.text[glyph_position.start..glyph_position.end] == " " {
				offset.x += span.word_spacing as f64;
			}
			offset.x += span.letter_spacing as f64;
		}
	}

	builder.other_subpaths
}

#[must_use]
pub fn has_hit_text_bounds(buffer: &cosmic_text::Buffer, spans: &[TextSpan], pos: Vec2) -> bool {
	let mut offset;
	let mut min = Vec2::MAX;
	let mut max = Vec2::MIN;
	let line_height = buffer.metrics().line_height;
	for run in buffer.layout_runs() {
		offset = Vec2::ZERO;
		for glyph_position in run.glyphs.iter() {
			let span = &spans[glyph_position.metadata];

			let glyph_offset = Vec2::new(glyph_position.x_offset, glyph_position.y_offset) + span.kerning + offset;
			let glyph_pos = Vec2::new(glyph_position.x, glyph_position.y + run.line_y) + glyph_offset;

			let space = &run.text[glyph_position.start..glyph_position.end] == " ";
			let spacing = span.letter_spacing + if space { span.word_spacing } else { 0. };
			min = min.min(Vec2::new(glyph_pos.x, glyph_pos.y - line_height));
			max = max.max(Vec2::new(glyph_pos.x + spacing + glyph_position.w, glyph_pos.y));

			offset.x += spacing;
		}
	}
	min.x <= pos.x && min.y <= pos.y && pos.x <= max.x && pos.y <= max.y
}

#[must_use]
pub fn find_line_wrap_handle(buffer: &cosmic_text::Buffer, spans: &[TextSpan]) -> DVec2 {
	if buffer.size().0 != f64::MAX as f32 {
		return DVec2::new(buffer.size().0 as f64, buffer.metrics().line_height as f64);
	}
	let mut offset = 0.;
	let mut line = 0;
	let mut max = 0_f32;
	for run in buffer.layout_runs() {
		if run.line_i != line {
			offset = 0.;
		}
		line = run.line_i;
		for glyph_position in run.glyphs.iter() {
			let span = &spans[glyph_position.metadata];

			max = max.max(glyph_position.x + glyph_position.w + offset + glyph_position.x_offset);

			if &run.text[glyph_position.start..glyph_position.end] == " " {
				offset += span.word_spacing;
			}
			offset += span.letter_spacing;
		}
	}

	DVec2::new(max as f64, buffer.metrics().line_height as f64)
}

fn get_cursor_in_run(cursor: &cosmic_text::Cursor, run: &cosmic_text::LayoutRun) -> Option<(usize, f32)> {
	if cursor.line != run.line_i {
		return None;
	}
	for (glyph_i, glyph) in run.glyphs.iter().enumerate() {
		if cursor.index == glyph.start {
			return Some((glyph_i, 0.0));
		} else if cursor.index > glyph.start && cursor.index < glyph.end {
			// Guess x offset based on characters
			let mut before = 0;
			let mut total = 0;

			let cluster = &run.text[glyph.start..glyph.end];
			for (i, _) in cluster.grapheme_indices(true) {
				if glyph.start + i < cursor.index {
					before += 1;
				}
				total += 1;
			}

			let offset_x = glyph.w * (before as f32) / (total as f32);
			return Some((glyph_i, offset_x));
		}
	}
	match run.glyphs.last() {
		Some(glyph) => {
			if cursor.index == glyph.end {
				return Some((run.glyphs.len(), 0.0));
			}
		}
		None => {
			return Some((0, 0.0));
		}
	}
	None
}

pub fn selection_shape(editor: &cosmic_text::Editor, text: &RichText) -> Vec<Subpath<ManipulatorGroupId>> {
	let Some(select) = editor.select_opt() else { return Vec::new() };
	let (start, end) = match select.line.cmp(&editor.cursor().line) {
		cmp::Ordering::Greater => (editor.cursor(), select),
		cmp::Ordering::Less => (select, editor.cursor()),
		cmp::Ordering::Equal => {
			if select.index < editor.cursor().index {
				(select, editor.cursor())
			} else {
				(editor.cursor(), select)
			}
		}
	};
	let mut result = Vec::new();
	let line_height = editor.buffer().metrics().line_height as f64;
	let mut offset_spacing;
	for run in editor.buffer().layout_runs() {
		offset_spacing = DVec2::ZERO;

		let run: cosmic_text::LayoutRun = run;
		if run.line_i >= start.line && run.line_i <= end.line {
			let mut range_opt: Option<(f32, f32)> = None;
			for glyph in run.glyphs {
				let spans = &text.spans[glyph.metadata];

				// Guess x offset based on characters
				let cluster = &run.text[glyph.start..glyph.end];
				let total = cluster.grapheme_indices(true).count();
				let mut c_x = glyph.x;
				let c_w = glyph.w / total as f32;
				let spacing = if &run.text[glyph.start..glyph.end] == " " { spans.word_spacing as f64 } else { 0. } + spans.letter_spacing as f64;
				for (i, c) in cluster.grapheme_indices(true) {
					let c_start = glyph.start + i;
					let c_end = glyph.start + i + c.len();
					if (start.line != run.line_i || c_end > start.index) && (end.line != run.line_i || c_start < end.index) {
						range_opt = match range_opt.take() {
							Some((min, max)) => Some((min.min(c_x + offset_spacing.x as f32), max.max(c_x + c_w + offset_spacing.x as f32 + spacing as f32))),
							None => Some((c_x + offset_spacing.x as f32, (c_x + c_w + offset_spacing.x as f32 + spacing as f32))),
						};
					} else if let Some((min, max)) = range_opt.take() {
						result.push(Subpath::new_rect(
							DVec2::new(min as f64, run.line_top as f64),
							DVec2::new(max as f64, run.line_top as f64 + line_height),
						));
					}
					c_x += c_w;
				}
				offset_spacing.x += spacing;
			}

			if let Some((mut min, mut max)) = range_opt.take() {
				if end.line > run.line_i {
					// Draw to end of line
					if run.rtl {
						min = 0.;
					} else {
						max = if editor.buffer().size().0 == f32::MAX { max } else { editor.buffer().size().0 };
					}
				}
				result.push(Subpath::new_rect(
					DVec2::new(min as f64, run.line_top as f64),
					DVec2::new(max as f64, run.line_top as f64 + line_height),
				));
			}
		}
	}
	result
}

pub fn cursor_rectangle(editor: &cosmic_text::Editor, text: &RichText) -> Option<[DVec2; 2]> {
	let line_height = editor.buffer().metrics().line_height as f64;
	for run in editor.buffer().layout_runs() {
		let Some((cursor_glyph, cursor_glyph_offset)) = get_cursor_in_run(&editor.cursor(), &run) else {
			continue;
		};
		let letter_spacing: f32 = run.glyphs.iter().take(cursor_glyph).map(|glyph| text.spans[glyph.metadata].letter_spacing).sum();
		let spaces = run.glyphs.iter().take(cursor_glyph).filter(|glyph| &run.text[glyph.start..glyph.end] == " ");
		let word_spacing: f32 = spaces.map(|glyph| text.spans[glyph.metadata].word_spacing).sum();
		let x = match run.glyphs.get(cursor_glyph) {
			Some(glyph) => {
				// Start of detected glyph
				if glyph.level.is_rtl() {
					glyph.x + glyph.w - cursor_glyph_offset
				} else {
					glyph.x + cursor_glyph_offset
				}
			}
			None => match run.glyphs.last() {
				Some(glyph) => {
					// End of last glyph
					if glyph.level.is_rtl() {
						glyph.x
					} else {
						glyph.x + glyph.w
					}
				} // Start of empty line
				None => 0.,
			},
		};
		return Some([
			DVec2::new(x as f64 + letter_spacing as f64 + word_spacing as f64, run.line_top as f64),
			DVec2::new(x as f64 + 1. + letter_spacing as f64 + word_spacing as f64, run.line_top as f64 + line_height),
		]);
	}
	None
}

pub fn cursor_shape(editor: &cosmic_text::Editor, text: &RichText) -> Vec<Subpath<ManipulatorGroupId>> {
	if let Some(cursor) = cursor_rectangle(editor, text) {
		vec![Subpath::new_rect(cursor[0], cursor[1])]
	} else {
		Vec::new()
	}
}

pub fn compute_cursor_position(buffer: &cosmic_text::Buffer, text: &RichText, mut pos: DVec2) -> Option<cosmic_text::Cursor> {
	// Adjust for letter and word spacing
	let cosmic_text::Metrics { font_size, line_height } = buffer.metrics();
	let length = buffer.layout_runs().size_hint().0;
	for (index, run) in buffer.layout_runs().enumerate() {
		let first = index == 0;
		let last = index + 1 == length;
		let line_y = run.line_y;
		if !((pos.y as f32 >= line_y - font_size || first) && ((pos.y as f32) < line_y - font_size + line_height || last)) {
			continue;
		}
		for glyph in run.glyphs {
			let span = &text.spans[glyph.metadata];

			let space = &run.text[glyph.start..glyph.end] == " ";
			let spacing = span.letter_spacing + if space { span.word_spacing } else { 0. };

			if glyph.x <= pos.x as f32 && glyph.x + glyph.w + spacing >= pos.x as f32 {
				pos.x = glyph.x as f64 + ((pos.x - glyph.x as f64) / (glyph.w + spacing) as f64).clamp(0., 1.) * glyph.w as f64;
			} else if glyph.x + glyph.w + spacing <= pos.x as f32 {
				pos.x -= spacing as f64;
			}
		}
	}

	// Compute on the underlying text with the updated position
	buffer.hit(pos.x as f32, pos.y as f32)
}

pub fn create_buffer(buffer: &mut cosmic_text::Buffer, font_system: &mut cosmic_text::FontSystem, text: &RichText, font_cache: &FontCache, line_length: f64) {
	// Set a size for the text buffer, in pixels
	buffer.set_size(font_system, line_length as f32, f32::MAX);
	buffer.set_metrics(font_system, create_metrics(text));

	// Add our rich text spans
	let mut start = 0;
	let mut next_spans = text.spans.iter().skip(1);
	let spans = text.spans.iter().enumerate().filter_map(|(metadata, span)| {
		start += span.offset;
		start = start.min(text.text.len());
		let text = next_spans.next().map_or(&text.text[start..], |next| &text.text[start..(start + next.offset).min(text.text.len())]);

		create_cosmic_attrs(font_cache, span, metadata).map(|attrs| (text, attrs))
	});
	buffer.set_rich_text(font_system, spans, cosmic_text::Shaping::Advanced);

	// Trailing new line
	if text.text.as_bytes().last().is_some_and(|&c| c == b'\n') {
		let span_index = text.spans.len() - 1;
		if let Some(attrs) = create_cosmic_attrs(font_cache, &text.spans[span_index], span_index) {
			buffer.lines.push(cosmic_text::BufferLine::new("", cosmic_text::AttrsList::new(attrs), cosmic_text::Shaping::Advanced));
		}
	}

	// Perform shaping as desired
	buffer.shape_until_scroll(font_system);
}
#[must_use]
fn create_cosmic_attrs<'a>(font_cache: &'a FontCache, span: &TextSpan, metadata: usize) -> Option<cosmic_text::Attrs<'a>> {
	font_cache
		.font_attrs
		.get(&*span.font)
		.or_else(|| font_cache.default_font.as_ref().and_then(|font| font_cache.font_attrs.get(font)))
		.map(|attrs| attrs.as_attrs().metadata(metadata))
}

#[must_use]
fn construct_buffer(text: &RichText, font_system: &mut cosmic_text::FontSystem) -> cosmic_text::Buffer {
	let metrics = create_metrics(text);

	// A Buffer provides shaping and layout for a UTF-8 string, create one per text widget
	cosmic_text::Buffer::new(font_system, metrics)
}

#[must_use]
fn create_metrics(text: &RichText) -> cosmic_text::Metrics {
	let max_font_size = text.spans.iter().map(|span| span.font_size).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(24.);
	let max_line = text.spans.iter().map(|span| span.font_size * span.line_spacing).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(24.);
	cosmic_text::Metrics::new(max_font_size, max_line)
}

#[must_use]
pub fn create_cosmic_editor(text: &RichText, font_cache: &FontCache, line_length: f64) -> Option<cosmic_text::Editor> {
	let mut font_system = font_cache.get_system()?;
	let mut buffer = construct_buffer(text, &mut font_system);

	create_buffer(&mut buffer, &mut font_system, text, font_cache, line_length);
	Some(cosmic_text::Editor::new(buffer))
}
