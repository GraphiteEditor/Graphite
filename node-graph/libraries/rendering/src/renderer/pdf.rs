//! Draws graphic content into a PDF document with krilla, keeping vector geometry, gradients, and text as such.

use super::*;
use crate::deferred_image::DeferredImagePlacement;
use graphic_types::vector_types::gradient::GradientSpread;
use graphic_types::vector_types::vector::style::Stroke as VectorStroke;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Size, Transform};
use krilla::mask::{Mask, MaskType};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{FillRule, LineCap, LineJoin, LinearGradient, Paint, RadialGradient, SpreadMethod, Stop, StrokeDash};
use krilla::stream::Stream;
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph};
use std::collections::BTreeSet;

/// The resolution, in pixels per inch, of the images the GPU renders of content the PDF can't draw itself, at an export scale of 1.
pub const DEFERRED_IMAGE_DPI: f64 = 300.;

const POINTS_PER_INCH: f64 = 72.;

/// How the content of a PDF goes onto its page.
#[derive(Clone, Copy, Debug)]
pub struct PdfPage {
	/// The page's size in points.
	pub size: DVec2,
	/// Places the content on the page.
	pub transform: DAffine2,
	/// The pixels per point of the images the GPU renders of content the PDF can't draw itself, like effects.
	pub raster_scale: f64,
}

impl PdfPage {
	/// A page of `size` points with `transform` placing the content on it, whose images of deferred content have [`DEFERRED_IMAGE_DPI`] times `export_scale` pixels per inch.
	pub fn new(size: DVec2, transform: DAffine2, export_scale: f64) -> Self {
		Self {
			size,
			transform,
			raster_scale: export_scale * DEFERRED_IMAGE_DPI / POINTS_PER_INCH,
		}
	}

	/// The view the GPU renders the images of deferred content in, mapping the page into their pixels.
	pub fn deferred_image_view(&self) -> Footprint {
		Footprint {
			transform: DAffine2::from_scale(DVec2::splat(self.raster_scale)),
			resolution: (self.size * self.raster_scale).ceil().max(DVec2::ONE).as_uvec2(),
			..Default::default()
		}
	}
}

/// The images the GPU rendered of the content a PDF can't draw itself, with where each goes, by the IDs of their [`DeferredImage`]s.
pub type PdfRasters = HashMap<u64, (DeferredImagePlacement, Raster<CPU>)>;

/// Renders `content` into a PDF document of one page, drawing the `rasters` in place of the content they're images of.
/// Also gives back the content the PDF can't draw itself that `rasters` lacks images of, for the GPU to render before rendering the document again.
/// Content is met in the same order each time, so the IDs of that content name the same content then.
pub fn render_pdf_document(content: &(impl Render + ?Sized), page: &PdfPage, render_params: &RenderParams, rasters: PdfRasters) -> Result<(Vec<u8>, Vec<DeferredImage>), String> {
	let Some(page_size) = Size::from_wh(page.size.x as f32, page.size.y as f32) else {
		return Err(format!("A PDF page can't be {} by {} points", page.size.x, page.size.y));
	};

	let mut document = krilla::Document::new();
	let mut pdf_page = document.start_page_with(PageSettings::new(page_size));
	let mut surface = pdf_page.surface();

	let mut state = DocumentState {
		fonts: HashMap::new(),
		left_out: BTreeSet::new(),
		rasters,
		deferred: Vec::new(),
		next_deferred_id: 0,
	};
	let mut canvas = PdfCanvas {
		surface: &mut surface,
		document: &mut state,
		page_size: page.size,
		base_transform: DAffine2::IDENTITY,
		pushed: Vec::new(),
	};
	canvas.push_transform(page.transform);
	content.render_pdf(&mut canvas, render_params);
	canvas.pop();

	if !state.left_out.is_empty() {
		let left_out: Vec<_> = state.left_out.iter().copied().collect();
		log::warn!("The PDF leaves out content it can't draw yet: {}", left_out.join(", "));
	}

	surface.finish();
	pdf_page.finish();
	let pdf = document.finish().map_err(|error| format!("The PDF could not be written: {error:?}"))?;

	// The content still without an image is what the GPU renders before the document is drawn again
	let missing = state.deferred.into_iter().filter(|deferred| !state.rasters.contains_key(&deferred.id)).collect();
	Ok((pdf, missing))
}

/// What one [`PdfCanvas::pop`] ends.
enum Pushed {
	/// A transform, placing the content within the space before it.
	Transform(DAffine2),
	/// A number of clips, opacities, blend modes, and isolated groups pushed together.
	States(usize),
}

/// What drawing every part of a document shares, including the streams drawn for its masks.
struct DocumentState {
	/// The fonts the document embeds, by the IDs of their data, or `None` for one krilla can't read.
	fonts: HashMap<u64, Option<Font>>,
	/// The kinds of content left out because the PDF can't draw them yet, reported once the document is done.
	left_out: BTreeSet<&'static str>,
	rasters: PdfRasters,
	/// The content drawn as images the GPU renders, in the order it's met, which numbers it the same each time the document is drawn.
	deferred: Vec<DeferredImage>,
	next_deferred_id: u64,
}

/// The page being drawn, which tracks its transforms so content filling the page knows where its edges are.
pub struct PdfCanvas<'a, 'b> {
	surface: &'a mut Surface<'b>,
	document: &'a mut DocumentState,
	page_size: DVec2,
	/// The transform from the space the surface starts in to the page, which a stream drawn for part of the page starts in.
	base_transform: DAffine2,
	pushed: Vec<Pushed>,
}

impl PdfCanvas<'_, '_> {
	/// Notes that some kind of content is left out of the document.
	pub(super) fn leave_out(&mut self, kind: &'static str) {
		self.document.left_out.insert(kind);
	}

	/// The transform from the space content is drawn in to the page.
	fn current_transform(&self) -> DAffine2 {
		(self.pushed.iter())
			.filter_map(|pushed| match pushed {
				Pushed::Transform(transform) => Some(*transform),
				Pushed::States(_) => None,
			})
			.fold(self.base_transform, |outer, inner| outer * inner)
	}

	/// Draws what `draw` draws into a stream of its own, in the current space, like the content of a mask.
	fn draw_stream(&mut self, draw: impl FnOnce(&mut PdfCanvas)) -> Stream {
		let base_transform = self.current_transform();

		let mut builder = self.surface.stream_builder();
		let mut surface = builder.surface();
		let mut stream_canvas = PdfCanvas {
			surface: &mut surface,
			document: &mut *self.document,
			page_size: self.page_size,
			base_transform,
			pushed: Vec::new(),
		};
		draw(&mut stream_canvas);

		surface.finish();
		builder.finish()
	}

	/// Draws the image the GPU rendered of `content`, which covers `bounds` in the current space, or notes it for the GPU to render if there isn't one yet.
	/// The image takes the content's `opacity` and `blend_mode`, which `content` leaves out.
	fn draw_deferred(&mut self, content: impl FnOnce() -> Graphic, bounds: [DVec2; 2], opacity: f32, blend_mode: BlendMode, render_params: &RenderParams) {
		let id = self.document.next_deferred_id;
		self.document.next_deferred_id += 1;

		let user_to_document = self.current_transform();
		self.document.deferred.push(DeferredImage {
			id,
			content: content(),
			render_params: render_params.clone(),
			user_to_document,
			bounds,
			margin: 0.,
			opacity,
			blend_mode,
		});

		let Some((placement, raster)) = self.document.rasters.get(&id) else { return };
		let (pixels, width, height) = raster.to_flat_u8();
		let image = krilla::image::Image::from_rgba8(pixels, width, height);
		let Some(unit_size) = Size::from_wh(1., 1.) else { return };
		let placement = placement.transform;

		self.push_compositing(opacity, blend_mode);
		self.push_transform(placement);
		self.surface.draw_image(image, unit_size);
		self.pop();
		self.pop();
	}

	/// Shows the content drawn until the matching [`Self::pop`] only where `mask`, drawn in the current space, has alpha.
	fn push_alpha_mask(&mut self, mask: Stream) {
		self.surface.push_mask(Mask::new(mask, MaskType::Alpha));
		self.pushed.push(Pushed::States(1));
	}

	/// Places the content drawn until the matching [`Self::pop`] by `transform`, within the current space.
	fn push_transform(&mut self, transform: DAffine2) {
		self.surface.push_transform(&to_krilla_transform(transform));
		self.pushed.push(Pushed::Transform(transform));
	}

	/// Clips the content drawn until the matching [`Self::pop`] to `path`, of which an empty one shows nothing.
	fn push_clip(&mut self, path: &BezPath, fill_rule: FillRule) {
		match to_krilla_path(path) {
			Some(path) => self.surface.push_clip_path(&path, &fill_rule),
			// A path without an area to clip to hides everything, as a fully transparent group does
			None => self.surface.push_opacity(NormalizedF32::ZERO),
		}
		self.pushed.push(Pushed::States(1));
	}

	/// Composites the content drawn until the matching [`Self::pop`] as a group, faded to `opacity` and blended with `blend_mode`.
	fn push_compositing(&mut self, opacity: f32, blend_mode: BlendMode) {
		let mut pushes = 0;

		if blend_mode != BlendMode::Normal {
			match to_krilla_blend_mode(blend_mode) {
				Some(blend_mode) => {
					// Isolated, so the group blends with what's below as a whole rather than each of its parts on their own
					self.surface.push_blend_mode(blend_mode);
					self.surface.push_isolated();
					pushes += 2;
				}
				None => self.leave_out("blend modes PDF lacks"),
			}
		}
		if opacity < 1. {
			self.surface.push_opacity(normalized(opacity));
			pushes += 1;
		}

		self.pushed.push(Pushed::States(pushes));
	}

	/// Ends the most recent transform, clip, or compositing group.
	fn pop(&mut self) {
		let pushes = match self.pushed.pop() {
			Some(Pushed::Transform(_)) => 1,
			Some(Pushed::States(pushes)) => pushes,
			None => {
				log::error!("The PDF ends a group it never began");
				0
			}
		};
		for _ in 0..pushes {
			self.surface.pop();
		}
	}

	/// The font a glyph run draws with, embedding it the first time.
	fn font(&mut self, font: &peniko::FontData) -> Option<Font> {
		let blob = font.data.clone();
		let index = font.index;
		let entry = self.document.fonts.entry(blob.id()).or_insert_with(|| {
			let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(blob);
			Font::new(data.into(), index)
		});
		entry.clone()
	}

	/// Fills `path` with `paint`, or with nothing when it shows nothing.
	fn fill_path(&mut self, path: &BezPath, paint: &PdfPaint, fill_rule: FillRule) {
		let PdfPaint::Paint(paint, opacity) = paint else { return };
		let Some(path) = to_krilla_path(path) else { return };

		self.surface.set_stroke(None);
		self.surface.set_fill(Some(krilla::paint::Fill {
			paint: paint.clone(),
			opacity: *opacity,
			rule: fill_rule,
		}));
		self.surface.draw_path(&path);
	}

	/// Strokes `path` with `stroke` (measured in the current space) and `paint`, or with nothing when it shows nothing.
	fn stroke_path(&mut self, path: &BezPath, stroke: &VectorStroke, paint: &PdfPaint) {
		let PdfPaint::Paint(paint, opacity) = paint else { return };
		let Some(path) = to_krilla_path(path) else { return };

		self.surface.set_fill(None);
		self.surface.set_stroke(Some(to_krilla_stroke(stroke, paint.clone(), *opacity)));
		self.surface.draw_path(&path);
	}

	/// The page's area in the current space, which content filling the plane (like a color layer) paints.
	fn page_area(&self) -> BezPath {
		let page_to_current = self.current_transform().inverse();
		let page = Quad::from_box([DVec2::ZERO, self.page_size]);

		let mut path = BezPath::new();
		for (index, corner) in (page_to_current * page).0.iter().enumerate() {
			let point = kurbo::Point::new(corner.x, corner.y);
			if index == 0 { path.move_to(point) } else { path.line_to(point) }
		}
		path.close_path();
		path
	}
}

/// How a paint covers a filled or stroked area in the PDF.
enum PdfPaint {
	/// The paint shows nothing.
	Nothing,
	/// A color or gradient, with the opacity it's drawn at.
	Paint(Paint, NormalizedF32),
	/// Content that needs clipping to the area, which the PDF can't draw yet.
	Clipped,
}

/// The PDF paint for `paint` drawn on an element placed by `element_transform` within content placed by `item_transform`, as SVG places them.
fn pdf_paint(paint: Option<&Graphic>, item_transform: DAffine2, element_transform: DAffine2, for_mask: bool) -> PdfPaint {
	let Some(paint) = paint else { return PdfPaint::Nothing };
	match paint_kind(Some(paint)) {
		PaintKind::Nothing => return PdfPaint::Nothing,
		PaintKind::Clipped => return PdfPaint::Clipped,
		PaintKind::Brush => {}
	}

	let gradient = |item: ItemRef<'_, Gradient>| match pdf_gradient(item, item_transform, element_transform, for_mask) {
		Some(paint) => paint,
		None => PdfPaint::Nothing,
	};

	match paint {
		Graphic::Color(item) => solid_pdf_paint(faded_paint_color(ItemRef::Item(item), for_mask)),
		Graphic::ColorList(list) => solid_pdf_paint(composite_paint_colors(list, for_mask)),
		Graphic::Gradient(item) => gradient(ItemRef::Item(item)),
		Graphic::GradientList(list) => gradient(ItemRef::ListItem(list, 0)),
		// Every other paint was classified as drawing nothing or clipped above
		_ => PdfPaint::Nothing,
	}
}

fn solid_pdf_paint(color: Option<Color>) -> PdfPaint {
	let Some(color) = color else { return PdfPaint::Nothing };
	let SRGBA8 { red, green, blue, .. } = SRGBA8::from(color);

	PdfPaint::Paint(rgb::Color::new(red, green, blue).into(), normalized(color.a()))
}

/// A gradient paint placed like SVG's [`render_gradient_paint`](crate::render_ext) places it, from the same stops.
fn pdf_gradient(item: ItemRef<'_, Gradient>, item_transform: DAffine2, element_transform: DAffine2, for_mask: bool) -> Option<PdfPaint> {
	let (gradient, settings) = painted_gradient(item)?;
	let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
	let local_gradient_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let paint_opacity = item.paint_opacity(for_mask);

	let samples = spread_adjusted_samples(&gradient, settings, gradient_form);
	// A gradient with no stops paints as solid black, matching `Gradient::evaluate`
	if samples.is_empty() {
		return Some(solid_pdf_paint(Some(Color::BLACK.with_alpha(paint_opacity))));
	}

	let stops = (samples.into_iter())
		.map(|(position, color, _)| {
			let SRGBA8 { red, green, blue, .. } = SRGBA8::from(color);
			Stop {
				offset: normalized(position as f32),
				color: rgb::Color::new(red, green, blue).into(),
				opacity: normalized(color.a() * paint_opacity),
			}
		})
		.collect();

	// The element's transform is already applied to what the paint covers
	let element_transform_inverse = if transform_is_invertible(element_transform) {
		element_transform.inverse()
	} else {
		DAffine2::IDENTITY
	};
	let placement = to_krilla_transform(element_transform_inverse * gradient_placement(item_transform * local_gradient_transform, gradient_form));
	let spread_method = match settings.spread {
		GradientSpread::Reflect => SpreadMethod::Reflect,
		GradientSpread::Repeat => SpreadMethod::Repeat,
		// The samples of a clear spread end in transparent guards, which padding extends
		_ => SpreadMethod::Pad,
	};

	let paint: Paint = match gradient_form {
		GradientForm::Linear => LinearGradient {
			x1: 0.,
			y1: 0.,
			x2: 1.,
			y2: 0.,
			transform: placement,
			spread_method,
			stops,
			anti_alias: true,
		}
		.into(),
		GradientForm::Radial => RadialGradient {
			fx: 0.,
			fy: 0.,
			fr: 0.,
			cx: 0.,
			cy: 0.,
			cr: 1.,
			transform: placement,
			spread_method,
			stops,
			anti_alias: true,
		}
		.into(),
	};

	Some(PdfPaint::Paint(paint, NormalizedF32::ONE))
}

/// Draws the item of a group, with its transform, compositing, and appearance cascading into its content.
pub(super) fn draw_graphic_item(item: ItemRef<'_, Graphic>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let Some(element) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);

	let compositing = item.compositing(render_params, item_transform);
	if compositing.effects.is_some() {
		if let Some(bounds) = deferred_bounds(canvas, drawn_bounding_box(element, item_transform), compositing.reach()) {
			let content = || Graphic::Graphic(Box::new(without_opacity_and_blend_mode(item, element.clone())));
			canvas.draw_deferred(content, bounds, compositing.opacity, blend_mode, render_params);
		}
		return;
	}

	// This item's declared appearance cascades to descendants lacking their own
	let child_render_params = (item.attribute::<Appearance>(ATTR_APPEARANCE))
		.and_then(Appearance::declared)
		.map(|appearance| render_params.for_child_item(appearance));
	let render_params = child_render_params.as_ref().unwrap_or(render_params);

	canvas.push_transform(item_transform);
	canvas.push_compositing(compositing.opacity, blend_mode);
	element.render_pdf(canvas, render_params);
	canvas.pop();
	canvas.pop();
}

/// Draws `count` items of a list in order, those that `clips` clipped to the alpha of the item before the first of their run, as Vello and SVG clip them.
fn draw_items_with_clipping(canvas: &mut PdfCanvas, count: usize, clips: impl Fn(usize) -> bool, mut draw_item: impl FnMut(&mut PdfCanvas, usize), mut draw_masker: impl FnMut(&mut PdfCanvas, usize)) {
	let mut masker = None;

	for index in 0..count {
		let next_clips = index + 1 < count && clips(index + 1);

		match masker {
			// The item the run after it is clipped to draws as itself too
			None if next_clips => {
				masker = Some(index);
				draw_item(canvas, index);
			}
			None => draw_item(canvas, index),
			Some(masker_index) => {
				if !next_clips {
					masker = None;
				}

				let mask = canvas.draw_stream(|mask_canvas| draw_masker(mask_canvas, masker_index));
				canvas.push_alpha_mask(mask);
				draw_item(canvas, index);
				canvas.pop();
			}
		}
	}
}

/// Draws a group of items, each over the ones before it.
pub(super) fn draw_graphic_list(list: &List<Graphic>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let clipper_render_params = render_params.for_clipper();

	draw_items_with_clipping(
		canvas,
		list.len(),
		|index| list.element(index).is_some_and(Graphic::had_clip_enabled),
		|canvas, index| draw_graphic_item(ItemRef::ListItem(list, index), canvas, render_params),
		// The masker's content clips at its placement, without the compositing it draws with itself
		|canvas, index| {
			let Some(element) = list.element(index) else { return };
			canvas.push_transform(list.attribute_cloned_or_default(ATTR_TRANSFORM, index));
			element.render_pdf(canvas, &clipper_render_params);
			canvas.pop();
		},
	);
}

/// Draws a list of vector items, each over the ones before it.
pub(super) fn draw_vector_list(list: &List<Vector>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let clipper_render_params = render_params.for_clipper();

	draw_items_with_clipping(
		canvas,
		list.len(),
		|index| list.attribute_cloned_or_default::<bool>(ATTR_CLIPPING_MASK, index),
		|canvas, index| draw_vector_item(ItemRef::ListItem(list, index), canvas, render_params),
		// The shadows and glows the masker casts don't widen the area its clipped content shows in
		|canvas, index| {
			let Some(element) = list.element(index) else { return };
			let mut masker = Item::from_parts(element.clone(), list.clone_item_attributes(index));
			masker.remove_attribute::<Effects>(ATTR_EFFECTS);
			draw_vector_item(ItemRef::Item(&masker), canvas, &clipper_render_params);
		},
	);
}

/// Draws each artboard's background and content, clipped to its area if it clips.
pub(super) fn draw_artboards(list: &List<Artboard>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	for index in 0..list.len() {
		let Some(content) = list.element(index).map(Artboard::as_graphic_list) else { continue };
		let (location, dimensions, background, clip) = read_artboard_attributes(list, index);
		let area = rectangle_path(location, location + dimensions);

		canvas.fill_path(&area, &solid_pdf_paint(Some(background)), FillRule::NonZero);

		if clip {
			canvas.push_clip(&area, FillRule::NonZero);
		}
		canvas.push_transform(DAffine2::from_translation(location));

		let render_params = RenderParams {
			artboard_background: Some(background),
			..render_params.clone()
		};
		content.render_pdf(canvas, &render_params);

		canvas.pop();
		if clip {
			canvas.pop();
		}
	}
}

/// Draws an item of vector content with its fill and stroke, placing its geometry in its stroke's space as SVG does.
pub(super) fn draw_vector_item(item: ItemRef<'_, Vector>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let Some(vector) = item.element() else { return };
	let paths = item.attribute::<List<Path>>(ATTR_PATHS).unwrap_or_else(|| no_paths());
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);

	// The item's own declared appearance wins over one cascading down from an ancestor
	let own_appearance = item.attribute::<Appearance>(ATTR_APPEARANCE).and_then(Appearance::declared);
	let appearance = own_appearance.or(render_params.inherited_appearance.as_ref());
	let FillAndStroke {
		stroke,
		fill_paint,
		stroke_paint,
		stroke_below,
	} = appearance.map(Appearance::fill_and_stroke).unwrap_or_default();
	let stroke = stroke.filter(|stroke| stroke.has_renderable_stroke());

	// Solid paints take leading adjustments into their colors, and the effects left over can't be drawn yet
	let compositing = item.compositing(render_params, item_transform);
	let folded = compositing.fold_into_solid_paints(fill_paint, stroke_paint, stroke.is_some(), render_params.for_mask);
	let (fill_paint, stroke_paint, compositing) = match &folded {
		Some(folded) => (folded.fill.as_ref(), folded.stroke.as_ref(), compositing.with_remaining_effects(&folded.remaining_effects)),
		None => (fill_paint, stroke_paint, compositing),
	};
	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	if compositing.effects.is_some() {
		let bounds = (vector.stroke_inclusive_bounding_box_with_transform(paths, item_transform, stroke.as_ref())).map_or(RenderBoundingBox::None, RenderBoundingBox::Rectangle);
		if let Some(bounds) = deferred_bounds(canvas, bounds, compositing.reach()) {
			let content = || Graphic::Vector(Box::new(without_opacity_and_blend_mode(item, vector.clone())));
			canvas.draw_deferred(content, bounds, compositing.opacity, blend_mode, render_params);
		}
		return;
	}

	// A cascaded coverage records its stroke space in the ancestor's coordinates, so this item authors its own
	let stroke_transform = (stroke.as_ref())
		.map(|stroke| if own_appearance.is_some() { stroke.transform } else { item_transform })
		.filter(|transform| transform_is_invertible(*transform));
	let applied_stroke_transform = stroke_transform.unwrap_or(item_transform);
	let element_transform = stroke_transform.map_or(DAffine2::IDENTITY, |stroke_transform| item_transform * stroke_transform.inverse());

	let Some(placed) = item.placed_contours(applied_stroke_transform) else { return };
	let stroke_affine = Affine::new(applied_stroke_transform.to_cols_array());
	let fill = pdf_paint(fill_paint, item_transform, element_transform, render_params.for_mask);
	let stroke_pdf_paint = pdf_paint(stroke_paint, item_transform, element_transform, render_params.for_mask);

	// A paint subtree supplies its own styling, so an element's appearance must not cascade into it
	let paint_render_params = RenderParams {
		inherited_appearance: None,
		..render_params.clone()
	};
	// Paint content that's clipped to an area draws in the item's space, which the geometry's stroke space reaches by its transform
	let draw_clipped_paint = |canvas: &mut PdfCanvas, area: &BezPath, paint: &Graphic| {
		canvas.push_clip(area, FillRule::NonZero);
		canvas.push_transform(applied_stroke_transform);
		paint.render_pdf(canvas, &paint_render_params);
		canvas.pop();
		canvas.pop();
	};

	let fill_area = |canvas: &mut PdfCanvas, area: &BezPath| match (&fill, fill_paint) {
		(PdfPaint::Clipped, Some(paint)) => draw_clipped_paint(canvas, area, paint),
		_ => canvas.fill_path(area, &fill, FillRule::NonZero),
	};
	let draw_fill = |canvas: &mut PdfCanvas| {
		if vector.use_face_fill(paths) {
			for face in item.faces().iter() {
				fill_area(canvas, &(stroke_affine * face));
			}
		} else {
			fill_area(canvas, placed.path());
		}
	};
	let draw_stroke = |canvas: &mut PdfCanvas| {
		let Some(stroke) = &stroke else { return };
		draw_aligned_stroke(canvas, placed.path(), stroke, &mut |canvas, stroke| match (&stroke_pdf_paint, stroke_paint) {
			(PdfPaint::Clipped, Some(paint)) => {
				let outline = kurbo::stroke(placed.path().iter(), &to_kurbo_stroke(stroke), &kurbo::StrokeOpts::default(), 0.01);
				draw_clipped_paint(canvas, &outline, paint);
			}
			_ => canvas.stroke_path(placed.path(), stroke, &stroke_pdf_paint),
		});
	};

	canvas.push_compositing(compositing.opacity, blend_mode);
	canvas.push_transform(element_transform);
	if stroke_below {
		draw_stroke(canvas);
		draw_fill(canvas);
	} else {
		draw_fill(canvas);
		draw_stroke(canvas);
	}
	canvas.pop();
	canvas.pop();
}

/// Strokes `path` through `draw_stroke`, keeping an inside or outside aligned stroke to its side of the path by clipping a stroke twice as wide.
fn draw_aligned_stroke(canvas: &mut PdfCanvas, path: &BezPath, stroke: &VectorStroke, draw_stroke: &mut dyn FnMut(&mut PdfCanvas, &VectorStroke)) {
	let path_is_closed = path.elements().last().is_some_and(|element| matches!(element, PathEl::ClosePath));
	if !stroke.align.is_not_centered() || !path_is_closed {
		draw_stroke(canvas, stroke);
		return;
	}

	let doubled = VectorStroke {
		weight: stroke.weight * 2.,
		..stroke.clone()
	};
	match stroke.align {
		StrokeAlign::Inside => canvas.push_clip(path, FillRule::NonZero),
		// Everything but the path's interior, as an even-odd area between a rectangle around the stroke and the path
		_ => {
			let reach = stroke.max_aabb_inflation(true) * 2. + 1.;
			let bounds = path.bounding_box().inflate(reach, reach);
			let mut outside = bounds.to_path(kurbo::DEFAULT_ACCURACY);
			outside.extend(path.iter());
			canvas.push_clip(&outside, FillRule::EvenOdd);
		}
	}
	draw_stroke(canvas, &doubled);
	canvas.pop();
}

/// Draws a CPU raster's pixels across the unit square its transform places.
pub(super) fn draw_raster_item(item: ItemRef<'_, Raster<CPU>>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let Some(raster) = item.element() else { return };
	if raster.data.is_empty() {
		return;
	}

	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let compositing = item.compositing(render_params, item_transform);

	// Leading adjustments are carried out on the image's pixels, as SVG does
	let mut remaining_effects = Effects::default();
	let (leading_adjustments, compositing) = compositing.split_leading_adjustments(&mut remaining_effects);
	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	if compositing.effects.is_some() {
		let bounds = RenderBoundingBox::Rectangle((item_transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box());
		if let Some(bounds) = deferred_bounds(canvas, bounds, compositing.reach()) {
			let content = || Graphic::RasterCPU(Box::new(without_opacity_and_blend_mode(item, raster.clone())));
			canvas.draw_deferred(content, bounds, compositing.opacity, blend_mode, render_params);
		}
		return;
	}

	let adjusted;
	let raster = match leading_adjustments.is_empty() {
		true => raster,
		false => {
			adjusted = adjusted_raster(raster, &leading_adjustments);
			&adjusted
		}
	};
	let (pixels, width, height) = raster.to_flat_u8();
	let image = krilla::image::Image::from_rgba8(pixels, width, height);
	let Some(unit_size) = Size::from_wh(1., 1.) else { return };

	canvas.push_compositing(compositing.opacity, blend_mode);
	canvas.push_transform(item_transform);
	canvas.surface.draw_image(image, unit_size);
	canvas.pop();
	canvas.pop();
}

/// The area deferred content's image covers in the current space, its `bounds` grown by how far its effects `reach`, or the page for content without bounds.
fn deferred_bounds(canvas: &PdfCanvas, bounds: RenderBoundingBox, reach: DVec2) -> Option<[DVec2; 2]> {
	match bounds {
		RenderBoundingBox::Rectangle([min, max]) => Some([min - reach, max + reach]),
		RenderBoundingBox::Infinite => {
			let page = canvas.page_area().bounding_box();
			Some([DVec2::new(page.x0, page.y0), DVec2::new(page.x1, page.y1)])
		}
		RenderBoundingBox::None => None,
	}
}

/// Draws a generator's pattern, which the PDF has no way to describe, as an image the GPU renders of the page it covers.
pub(super) fn draw_generator_item(item: ItemRef<'_, Generator>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let Some(&generator) = item.element() else { return };
	let compositing = item.compositing(render_params, item.attribute_cloned_or_default(ATTR_TRANSFORM));

	let Some(bounds) = deferred_bounds(canvas, RenderBoundingBox::Infinite, DVec2::ZERO) else {
		return;
	};
	let content = || Graphic::Generator(without_opacity_and_blend_mode(item, generator));
	canvas.draw_deferred(content, bounds, compositing.opacity, item.attribute_cloned_or_default(ATTR_BLEND_MODE), render_params);
}

/// Draws a GPU raster, whose pixels only the GPU holds, as an image the GPU renders of it.
pub(super) fn draw_raster_gpu_item(item: ItemRef<'_, Raster<GPU>>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let Some(raster) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let compositing = item.compositing(render_params, item_transform);

	let bounds = RenderBoundingBox::Rectangle((item_transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box());
	let Some(bounds) = deferred_bounds(canvas, bounds, compositing.reach()) else { return };
	let content = || Graphic::RasterGPU(without_opacity_and_blend_mode(item, raster.clone()));
	canvas.draw_deferred(content, bounds, compositing.opacity, item.attribute_cloned_or_default(ATTR_BLEND_MODE), render_params);
}

/// Paints the whole page with a color item, as content filling the plane.
pub(super) fn draw_color_item(item: ItemRef<'_, Color>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let paint = solid_pdf_paint(faded_paint_color(item, render_params.for_mask));
	draw_page_filling_item(item.attribute_cloned_or_default(ATTR_BLEND_MODE), &paint, canvas);
}

/// Paints the whole page with a gradient item, placed by its own transform, as content filling the plane.
pub(super) fn draw_gradient_item(item: ItemRef<'_, Gradient>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let paint = pdf_gradient(item, DAffine2::IDENTITY, DAffine2::IDENTITY, render_params.for_mask).unwrap_or(PdfPaint::Nothing);
	draw_page_filling_item(item.attribute_cloned_or_default(ATTR_BLEND_MODE), &paint, canvas);
}

fn draw_page_filling_item(blend_mode: BlendMode, paint: &PdfPaint, canvas: &mut PdfCanvas) {
	let page_area = canvas.page_area();

	canvas.push_compositing(1., blend_mode);
	canvas.fill_path(&page_area, paint, FillRule::NonZero);
	canvas.pop();
}

/// One glyph run's glyphs, each with the text of its cluster so the PDF's text can be selected and searched.
struct PdfGlyphRun {
	font: peniko::FontData,
	font_size: f32,
	/// Where the run starts on its baseline.
	origin: DVec2,
	/// The tangent of the faux-italic skew around the baseline.
	tilt_tan: f64,
	glyphs: Vec<KrillaGlyph>,
}

impl PdfGlyphRun {
	/// The run's glyphs, or `None` if its font can't be embedded to draw it as laid out:
	/// an instance of a variable font, a synthesized bold, or a run drawing only part of its clusters.
	fn new(glyph_run: &parley::GlyphRun<'_, ()>, x_offset: f32, space_extra: f32, tilt_tan: f64) -> Option<Self> {
		let run = glyph_run.run();
		let default_instance = run.normalized_coords().iter().all(|coord| coord.to_bits() == 0);
		if !default_instance || run.synthesis().embolden() || (glyph_run.advance() - run.advance()).abs() > 1e-3 {
			return None;
		}

		// Offsets and advances are measured in ems, with offsets pointing up
		let font_size = run.font_size();
		let mut glyphs: Vec<KrillaGlyph> = Vec::new();
		for cluster in run.visual_clusters() {
			let text_range = cluster.text_range();
			let glyph_count = glyphs.len();

			for glyph in cluster.glyphs() {
				glyphs.push(KrillaGlyph::new(
					GlyphId::new(glyph.id),
					glyph.advance / font_size,
					glyph.x / font_size,
					glyph.y / font_size,
					0.,
					text_range.clone(),
					None,
				));
			}

			match glyphs.len() > glyph_count {
				// Justified lines widen their spaces after them
				true if space_extra != 0. && cluster.is_space_or_nbsp() => {
					if let Some(last) = glyphs.last_mut() {
						last.x_advance += space_extra / font_size;
					}
				}
				true => {}
				// A cluster without glyphs (like the rest of a ligature) belongs to the text of the glyph before it
				false => {
					if let Some(last) = glyphs.last_mut()
						&& last.text_range.end == text_range.start
					{
						last.text_range.end = text_range.end;
					}
				}
			}
		}

		Some(Self {
			font: run.font().clone(),
			font_size,
			origin: DVec2::new((glyph_run.offset() + x_offset) as f64, glyph_run.baseline() as f64),
			tilt_tan: tilt_tan + synthesized_tilt_tan(run),
			glyphs,
		})
	}
}

/// Draws an item of text as embedded glyph runs where they can draw its appearance exactly, else as its outlines.
pub(super) fn draw_text_item(item: ItemRef<'_, String>, canvas: &mut PdfCanvas, render_params: &RenderParams) {
	let Some(text) = item.element() else { return };
	if text.is_empty() {
		return;
	}
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);

	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let drawn = with_text_paints(item, render_params, item_transform, |paints| {
		if paints.compositing.effects.is_some() {
			let mut bounds = None;
			accumulate_text_item_bounds(item, DAffine2::IDENTITY, true, &mut bounds);
			if let Some(bounds) = deferred_bounds(canvas, bounds.map_or(RenderBoundingBox::None, RenderBoundingBox::Rectangle), paints.compositing.reach()) {
				let content = || Graphic::Text(without_opacity_and_blend_mode(item, text.clone()));
				canvas.draw_deferred(content, bounds, paints.compositing.opacity, blend_mode, render_params);
			}
			return true;
		}
		if !paints.draw_natively() {
			return false;
		}

		let (font, typesetting) = text_item_typesetting(item);
		let Some(runs) = text_nodes::TextContext::with_thread_local(|ctx| {
			let layout = ctx.layout_text(text, &font, typesetting)?;
			let tilt_tan = typesetting.letter_tilt.to_radians().tan();

			let mut runs = Some(Vec::new());
			text_nodes::for_each_styled_glyph_run(&layout, text, typesetting, |glyph_run, x_offset, space_extra| {
				let run = PdfGlyphRun::new(glyph_run, x_offset, space_extra, tilt_tan);
				runs = runs.take().zip(run).map(|(mut runs, run)| {
					runs.push(run);
					runs
				});
			});
			runs
		}) else {
			return false;
		};

		let fonts: Option<Vec<Font>> = runs.iter().map(|run| canvas.font(&run.font)).collect();
		let Some(fonts) = fonts else { return false };

		// The text sits in its layout space, which its paints are placed relative to
		let fill = pdf_paint(paints.fill_paint, item_transform, item_transform, render_params.for_mask);
		let stroke_paint = pdf_paint(paints.stroke_paint, item_transform, item_transform, render_params.for_mask);
		let local_stroke = paints.local_stroke();

		let fill = match fill {
			PdfPaint::Paint(paint, opacity) => Some(krilla::paint::Fill {
				paint,
				opacity,
				rule: FillRule::NonZero,
			}),
			_ => None,
		};
		let stroke = match (local_stroke, stroke_paint) {
			(Some(stroke), PdfPaint::Paint(paint, opacity)) => Some(to_krilla_stroke(&stroke, paint, opacity)),
			_ => None,
		};

		// Drawn as one text with both where the stroke paints above, else in two passes
		let passes = match (fill, stroke) {
			(fill, Some(stroke)) if paints.stroke_below => vec![(None, Some(stroke)), (fill, None)],
			(fill, stroke) => vec![(fill, stroke)],
		};

		canvas.push_compositing(paints.compositing.opacity, blend_mode);
		canvas.push_transform(item_transform);
		for (fill, stroke) in passes {
			canvas.surface.set_fill(fill);
			canvas.surface.set_stroke(stroke);

			for (run, font) in runs.iter().zip(&fonts) {
				// Skewed around the baseline, so glyph tops lean right as the outlines' faux italics do
				let skew = DAffine2::from_cols_array(&[1., 0., -run.tilt_tan, 1., run.tilt_tan * run.origin.y, 0.]);
				canvas.push_transform(skew);
				let origin = Point::from_xy(run.origin.x as f32, run.origin.y as f32);
				canvas.surface.draw_glyphs(origin, &run.glyphs, font.clone(), text, run.font_size, false);
				canvas.pop();
			}
		}
		canvas.pop();
		canvas.pop();

		true
	});

	if !drawn && let Some(vectors) = text_item_as_vectors(item, render_params) {
		vectors.render_pdf(canvas, render_params);
	}
}

fn normalized(value: f32) -> NormalizedF32 {
	NormalizedF32::new(value.clamp(0., 1.)).unwrap_or(NormalizedF32::ONE)
}

fn to_krilla_transform(transform: DAffine2) -> Transform {
	let [a, b, c, d, e, f] = transform.to_cols_array().map(|value| value as f32);
	Transform::from_row(a, b, c, d, e, f)
}

/// The krilla path for a kurbo path, or `None` for one without any segments.
fn to_krilla_path(path: &BezPath) -> Option<krilla::geom::Path> {
	let mut builder = PathBuilder::new();
	for element in path.elements() {
		match *element {
			PathEl::MoveTo(point) => builder.move_to(point.x as f32, point.y as f32),
			PathEl::LineTo(point) => builder.line_to(point.x as f32, point.y as f32),
			PathEl::QuadTo(control, point) => builder.quad_to(control.x as f32, control.y as f32, point.x as f32, point.y as f32),
			PathEl::CurveTo(control_1, control_2, point) => builder.cubic_to(control_1.x as f32, control_1.y as f32, control_2.x as f32, control_2.y as f32, point.x as f32, point.y as f32),
			PathEl::ClosePath => builder.close(),
		}
	}
	builder.finish()
}

fn to_krilla_stroke(stroke: &VectorStroke, paint: Paint, opacity: NormalizedF32) -> krilla::paint::Stroke {
	let line_cap = match stroke.cap {
		StrokeCap::Butt => LineCap::Butt,
		StrokeCap::Round => LineCap::Round,
		StrokeCap::Square => LineCap::Square,
	};
	let line_join = match stroke.join {
		StrokeJoin::Miter => LineJoin::Miter,
		StrokeJoin::Bevel => LineJoin::Bevel,
		StrokeJoin::Round => LineJoin::Round,
	};
	let dash = (!stroke.dash_lengths.is_empty()).then(|| StrokeDash {
		array: stroke.dash_lengths.iter().map(|length| length.max(0.) as f32).collect(),
		offset: stroke.dash_offset as f32,
	});

	krilla::paint::Stroke {
		paint,
		width: stroke.weight as f32,
		miter_limit: stroke.join_miter_limit as f32,
		line_cap,
		line_join,
		opacity,
		dash,
	}
}

/// The PDF blend mode for a blend mode, or `None` for one the PDF format lacks.
fn to_krilla_blend_mode(blend_mode: BlendMode) -> Option<krilla::blend::BlendMode> {
	use krilla::blend::BlendMode as PdfBlendMode;

	Some(match blend_mode {
		BlendMode::Normal => PdfBlendMode::Normal,
		BlendMode::Multiply => PdfBlendMode::Multiply,
		BlendMode::Screen => PdfBlendMode::Screen,
		BlendMode::Overlay => PdfBlendMode::Overlay,
		BlendMode::Darken => PdfBlendMode::Darken,
		BlendMode::Lighten => PdfBlendMode::Lighten,
		BlendMode::ColorDodge => PdfBlendMode::ColorDodge,
		BlendMode::ColorBurn => PdfBlendMode::ColorBurn,
		BlendMode::HardLight => PdfBlendMode::HardLight,
		BlendMode::SoftLight => PdfBlendMode::SoftLight,
		BlendMode::Difference => PdfBlendMode::Difference,
		BlendMode::Exclusion => PdfBlendMode::Exclusion,
		BlendMode::Hue => PdfBlendMode::Hue,
		BlendMode::Saturation => PdfBlendMode::Saturation,
		BlendMode::Color => PdfBlendMode::Color,
		BlendMode::Luminosity => PdfBlendMode::Luminosity,
		_ => return None,
	})
}
