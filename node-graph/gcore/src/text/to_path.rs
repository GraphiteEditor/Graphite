use crate::vector::PointId;
use bezier_rs::{ManipulatorGroup, Subpath};
use core::cell::RefCell;
use glam::{DAffine2, DVec2};
use parley::fontique::Blob;
use parley::{Alignment, AlignmentOptions, FontContext, GlyphRun, Layout, LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty};
use skrifa::GlyphId;
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::FontRef as ReadFontsRef;
use skrifa::{MetadataProvider, OutlineGlyph};
use std::sync::Arc;

// Thread-local storage avoids expensive re-initialization of font and layout contexts
// across multiple text rendering operations within the same thread
thread_local! {
	static FONT_CONTEXT: RefCell<FontContext> = RefCell::new(FontContext::new());
	static LAYOUT_CONTEXT: RefCell<LayoutContext<()>> = RefCell::new(LayoutContext::new());
}

struct PathBuilder {
	current_subpath: Subpath<PointId>,
	glyph_subpaths: Vec<Subpath<PointId>>,
	other_subpaths: Vec<Subpath<PointId>>,
	origin: DVec2,
	scale: f64,
	id: PointId,
}

impl PathBuilder {
	fn point(&self, x: f32, y: f32) -> DVec2 {
		// Y-axis inversion converts from font coordinate system (Y-up) to graphics coordinate system (Y-down)
		DVec2::new(self.origin.x + x as f64, self.origin.y - y as f64) * self.scale
	}

	fn set_origin(&mut self, x: f64, y: f64) {
		self.origin = DVec2::new(x, y);
	}

	fn draw_glyph(&mut self, glyph: &OutlineGlyph<'_>, size: f32, normalized_coords: &[NormalizedCoord], style_skew: Option<DAffine2>, skew: DAffine2) {
		let location_ref = LocationRef::new(normalized_coords);
		let settings = DrawSettings::unhinted(Size::new(size), location_ref);
		glyph.draw(settings, self).unwrap();

		// Apply transforms in correct order: style-based skew first, then user-requested skew
		// This ensures font synthesis (italic) is applied before user transformations
		for glyph_subpath in &mut self.glyph_subpaths {
			if let Some(style_skew) = style_skew {
				glyph_subpath.apply_transform(style_skew);
			}

			glyph_subpath.apply_transform(skew);
		}

		if !self.glyph_subpaths.is_empty() {
			self.other_subpaths.extend(core::mem::take(&mut self.glyph_subpaths));
		}
	}
}

impl OutlinePen for PathBuilder {
	fn move_to(&mut self, x: f32, y: f32) {
		if !self.current_subpath.is_empty() {
			self.glyph_subpaths.push(std::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
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
		self.glyph_subpaths.push(std::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
	}
}

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TypesettingConfig {
	pub font_size: f64,
	pub line_height_ratio: f64,
	pub character_spacing: f64,
	pub max_width: Option<f64>,
	pub max_height: Option<f64>,
	pub tilt: f64,
}

impl Default for TypesettingConfig {
	fn default() -> Self {
		Self {
			font_size: 24.,
			line_height_ratio: 1.2,
			character_spacing: 0.,
			max_width: None,
			max_height: None,
			tilt: 0.,
		}
	}
}

fn render_glyph_run(glyph_run: &GlyphRun<'_, ()>, path_builder: &mut PathBuilder, tilt: f64) {
	let mut run_x = glyph_run.offset();
	let run_y = glyph_run.baseline();

	let run = glyph_run.run();

	// User-requested tilt applied around baseline to avoid vertical displacement
	// Translation ensures rotation point is at the baseline, not origin
	let skew = DAffine2::from_translation(DVec2::new(0., run_y as f64))
		* DAffine2::from_cols_array(&[1., 0., -tilt.to_radians().tan(), 1., 0., 0.])
		* DAffine2::from_translation(DVec2::new(0., -run_y as f64));

	let synthesis = run.synthesis();

	// Font synthesis (e.g., synthetic italic) applied separately from user transforms
	// This preserves the distinction between font styling and user transformations
	let style_skew = synthesis.skew().map(|angle| {
		DAffine2::from_translation(DVec2::new(0., run_y as f64))
			* DAffine2::from_cols_array(&[1., 0., -angle.to_radians().tan() as f64, 1., 0., 0.])
			* DAffine2::from_translation(DVec2::new(0., -run_y as f64))
	});

	let font = run.font();
	let font_size = run.font_size();

	let normalized_coords = run.normalized_coords().iter().map(|coord| NormalizedCoord::from_bits(*coord)).collect::<Vec<_>>();

	// TODO: This can be cached for better performance
	let font_collection_ref = font.data.as_ref();
	let font_ref = ReadFontsRef::from_index(font_collection_ref, font.index).unwrap();
	let outlines = font_ref.outline_glyphs();

	for glyph in glyph_run.glyphs() {
		let glyph_x = run_x + glyph.x;
		let glyph_y = run_y - glyph.y;
		run_x += glyph.advance;

		let glyph_id = GlyphId::from(glyph.id);
		if let Some(glyph_outline) = outlines.get(glyph_id) {
			path_builder.set_origin(glyph_x as f64, glyph_y as f64);
			path_builder.draw_glyph(&glyph_outline, font_size, &normalized_coords, style_skew, skew);
		}
	}
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
			.first()
			.and_then(|(family_id, _)| font_cx.collection.family_name(*family_id).map(String::from))
	})?;

	const DISPLAY_SCALE: f32 = 1.;
	let mut builder = layout_cx.ranged_builder(&mut font_cx, str, DISPLAY_SCALE, true);

	builder.push_default(StyleProperty::FontSize(typesetting.font_size as f32));
	builder.push_default(StyleProperty::LetterSpacing(typesetting.character_spacing as f32));
	builder.push_default(StyleProperty::FontStack(parley::FontStack::Single(parley::FontFamily::Named(std::borrow::Cow::Owned(font_family)))));
	builder.push_default(LineHeight::FontSizeRelative(typesetting.line_height_ratio as f32));

	let mut layout: Layout<()> = builder.build(str);

	layout.break_all_lines(typesetting.max_width.map(|mw| mw as f32));
	layout.align(typesetting.max_width.map(|max_w| max_w as f32), Alignment::Left, AlignmentOptions::default());

	Some(layout)
}

pub fn to_path(str: &str, font_data: Option<Blob<u8>>, typesetting: TypesettingConfig) -> Vec<Subpath<PointId>> {
	let Some(layout) = layout_text(str, font_data, typesetting) else { return Vec::new() };

	let mut path_builder = PathBuilder {
		current_subpath: Subpath::new(Vec::new(), false),
		glyph_subpaths: Vec::new(),
		other_subpaths: Vec::new(),
		origin: DVec2::ZERO,
		scale: layout.scale() as f64,
		id: PointId::ZERO,
	};

	for line in layout.lines() {
		for item in line.items() {
			if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
				render_glyph_run(&glyph_run, &mut path_builder, typesetting.tilt);
			}
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

	let Some(layout) = layout_text(str, font_data, typesetting) else { return DVec2::ZERO };

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
