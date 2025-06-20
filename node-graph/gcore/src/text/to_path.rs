use core::cell::RefCell;
use std::sync::Arc;

use crate::vector::PointId;
use bezier_rs::{ManipulatorGroup, Subpath};
use glam::DVec2;
use parley::{Alignment, AlignmentOptions, FontContext, GlyphRun, Layout, LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty, fontique::Blob};
use skrifa::{
	GlyphId, MetadataProvider, OutlineGlyph,
	instance::{LocationRef, NormalizedCoord, Size},
	outline::{DrawSettings, OutlinePen},
	raw::FontRef as ReadFontsRef,
};

thread_local! {
	static FONT_CONTEXT: RefCell<FontContext> = RefCell::new(FontContext::new());
	static LAYOUT_CONTEXT: RefCell<LayoutContext<()>> = RefCell::new(LayoutContext::new());
}

struct PathBuilder {
	current_subpath: Subpath<PointId>,
	other_subpaths: Vec<Subpath<PointId>>,
	x: f32,
	y: f32,
	scale: f64,
	id: PointId,
}

impl PathBuilder {
	fn point(&self, x: f32, y: f32) -> DVec2 {
		DVec2::new((self.x + x) as f64, (self.y - y) as f64) * self.scale
	}

	fn set_origin(&mut self, x: f32, y: f32) {
		self.x = x;
		self.y = y;
	}

	fn draw_glyph(&mut self, glyph: &OutlineGlyph<'_>, size: f32, normalized_coords: &[NormalizedCoord]) {
		let location_ref = LocationRef::new(normalized_coords);
		let settings = DrawSettings::unhinted(Size::new(size), location_ref);
		glyph.draw(settings, self).unwrap();

		if !self.current_subpath.is_empty() {
			self.other_subpaths.push(core::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
		}
	}
}

impl OutlinePen for PathBuilder {
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

// fn render_decoration(path_builder: &mut PathBuilder<'_>, glyph_run: &GlyphRun<'_, ()>, brush: ColorBrush, offset: f32, width: f32, padding: u32) {
// 	let y = glyph_run.baseline() - offset + padding as f32;
// 	let x = glyph_run.offset() + padding as f32;
// 	path_builder.set_color(brush.color);
// 	path_builder.set_origin(x, y);
// 	path_builder.fill_rect(glyph_run.advance(), width);
// }

fn render_glyph_run(glyph_run: &GlyphRun<'_, ()>, path_builder: &mut PathBuilder) {
	// Resolve properties of the GlyphRun
	let mut run_x = glyph_run.offset();
	let run_y = glyph_run.baseline();
	let style = glyph_run.style();
	let brush = style.brush;

	// Get the "Run" from the "GlyphRun"
	let run = glyph_run.run();

	// Resolve properties of the Run
	let font = run.font();
	let font_size = run.font_size();

	let normalized_coords = run.normalized_coords().iter().map(|coord| NormalizedCoord::from_bits(*coord)).collect::<Vec<_>>();

	// Get glyph outlines using Skrifa. This can be cached in production code.
	let font_collection_ref = font.data.as_ref();
	let font_ref = ReadFontsRef::from_index(font_collection_ref, font.index).unwrap();
	let outlines = font_ref.outline_glyphs();

	// Iterates over the glyphs in the GlyphRun
	for glyph in glyph_run.glyphs() {
		let glyph_x = run_x + glyph.x;
		let glyph_y = run_y - glyph.y;
		run_x += glyph.advance;

		let glyph_id = GlyphId::from(glyph.id);
		if let Some(glyph_outline) = outlines.get(glyph_id) {
			path_builder.set_origin(glyph_x, glyph_y);
			// path_builder.set_color(brush.color);
			path_builder.draw_glyph(&glyph_outline, font_size, &normalized_coords);
		}
	}

	// Draw decorations: underline & strikethrough
	// let style = glyph_run.style();
	// let run_metrics = run.metrics();
	// if let Some(decoration) = &style.underline {
	// 	let offset = decoration.offset.unwrap_or(run_metrics.underline_offset);
	// 	let size = decoration.size.unwrap_or(run_metrics.underline_size);
	// 	render_decoration(path_builder, glyph_run, decoration.brush, offset, size);
	// }
	// if let Some(decoration) = &style.strikethrough {
	// 	let offset = decoration.offset.unwrap_or(run_metrics.strikethrough_offset);
	// 	let size = decoration.size.unwrap_or(run_metrics.strikethrough_size);
	// 	render_decoration(path_builder, glyph_run, decoration.brush, offset, size);
	// }
}

fn layout_text(str: &str, font_data: Option<Blob<u8>>, typesetting: TypesettingConfig) -> Option<Layout<()>> {
	let font_cx = FONT_CONTEXT.with(Clone::clone);
	let mut font_cx = font_cx.borrow_mut();
	let layout_cx = LAYOUT_CONTEXT.with(Clone::clone);
	let mut layout_cx = layout_cx.borrow_mut();

	let font_family = font_data.and_then(|font_data| {
		font_cx
			.collection
			.register_fonts(font_data, None)
			.get(0)
			.and_then(|(family_id, _)| font_cx.collection.family_name(*family_id).map(String::from))
	})?;

	const DISPLAY_SCALE: f32 = 1.0;
	let mut builder = layout_cx.ranged_builder(&mut font_cx, str, DISPLAY_SCALE, true);

	builder.push_default(StyleProperty::FontSize(typesetting.font_size as f32));
	builder.push_default(StyleProperty::LetterSpacing(typesetting.character_spacing as f32));
	debug!("{font_family}");
	builder.push_default(StyleProperty::FontStack(parley::FontStack::Single(parley::FontFamily::Named(std::borrow::Cow::Owned(font_family)))));
	builder.push_default(LineHeight::FontSizeRelative(typesetting.line_height_ratio as f32));

	let mut layout: Layout<()> = builder.build(str);

	layout.break_all_lines(typesetting.max_width.map(|mw| mw as f32));
	layout.align(typesetting.max_width.map(|mw| mw as f32), Alignment::Left, AlignmentOptions::default());

	Some(layout)
}

pub fn to_path(str: &str, font_data: Option<Blob<u8>>, typesetting: TypesettingConfig) -> Vec<Subpath<PointId>> {
	let Some(layout) = layout_text(str, font_data, typesetting) else {
		return vec![];
	};

	let mut path_builder = PathBuilder {
		current_subpath: Subpath::new(Vec::new(), false),
		other_subpaths: Vec::new(),
		x: 0.,
		y: 0.,
		scale: layout.scale() as f64,
		id: PointId::ZERO,
	};

	for line in layout.lines() {
		for item in line.items() {
			match item {
				PositionedLayoutItem::GlyphRun(glyph_run) => {
					render_glyph_run(&glyph_run, &mut path_builder);
				}
				PositionedLayoutItem::InlineBox(_inline_box) => {
					// Render the inline box
				}
			};
		}
	}

	path_builder.other_subpaths
}

pub fn bounding_box(str: &str, font_data: Option<Blob<u8>>, typesetting: TypesettingConfig, for_clipping_test: bool) -> DVec2 {
	if !for_clipping_test {
		if let (Some(max_height), Some(max_width)) = (typesetting.max_height, typesetting.max_width) {
			return DVec2::new(max_width, max_height);
		}
	}

	let Some(layout) = layout_text(str, font_data, typesetting) else {
		return DVec2::ZERO;
	};

	DVec2::new(layout.full_width() as f64, layout.height() as f64)
}

pub fn load_font(data: &[u8]) -> Blob<u8> {
	Blob::new(Arc::new(data.to_vec()))
}

pub fn lines_clipping(str: &str, font_data: Option<Blob<u8>>, typesetting: TypesettingConfig) -> bool {
	let Some(max_height) = typesetting.max_height else { return false };
	let bounds = bounding_box(str, font_data, typesetting, true);
	max_height < bounds.y
}
