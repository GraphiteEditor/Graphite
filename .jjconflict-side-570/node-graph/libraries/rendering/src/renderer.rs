use crate::render_ext::{PaintTarget, RenderExt};
use crate::to_peniko::{BlendModeExt, ToPenikoColor};
use core_types::CacheHash;
use core_types::blending::{BlendMode, apply_blend_mode};
use core_types::bounds::BoundingBox;
use core_types::bounds::RenderBoundingBox;
use core_types::color::Color;
use core_types::color::SRGBA8;
use core_types::consts::DEFAULT_FONT_SIZE;
use core_types::list::ATTR_APPEARANCE;
use core_types::list::{Item, List, NodeIdPath};
use core_types::math::quad::Quad;
use core_types::render_complexity::RenderComplexity;
use core_types::transform::Footprint;
use core_types::uuid::{NodeId, generate_uuid};
use core_types::{
	ATTR_BACKGROUND, ATTR_BLEND_MODE, ATTR_CLIP, ATTR_CLIPPING_MASK, ATTR_DIMENSIONS, ATTR_EDITOR_CLICK_TARGET, ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_EDITOR_TEXT_FRAME, ATTR_FONT,
	ATTR_FONT_SIZE, ATTR_GRADIENT_FORM, ATTR_LETTER_SPACING, ATTR_LETTER_TILT, ATTR_LINE_HEIGHT, ATTR_LOCATION, ATTR_MAX_HEIGHT, ATTR_MAX_WIDTH, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TEXT_ALIGN,
	ATTR_TRANSFORM,
};
use dyn_any::DynAny;
use glam::{DAffine2, DMat2, DVec2};
use graphene_hash::CacheHashWrapper;
use graphene_resource::Resource;
use graphic_types::raster_types::{BitmapMut, CPU, GPU, Image, Raster, Texture};
use graphic_types::vector_types::gradient::{Gradient, GradientForm};
use graphic_types::vector_types::vector::click_target::{ClickTarget, FreePoint};
use graphic_types::vector_types::vector::misc::dvec2_to_point;
use graphic_types::vector_types::vector::style::{RenderMode, StrokeAlign, StrokeCap, StrokeJoin};
use graphic_types::{Appearance, Artboard, Cover, Coverage, FillAndStroke, Graphic, Vector};
use kurbo::{Affine, BezPath, Cap, Join, PathEl, Shape, StrokeOpts};
use num_traits::Zero;
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::FontRef as SkrifaFontRef;
use skrifa::{GlyphId, MetadataProvider};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::{Arc, LazyLock};
use vector_types::gradient::{GradientSettings, GradientSpread};
use vello::*;

/// A borrowed view of one item of ranked content: one index of a `List<T>`'s attributes, or a lone `Item<T>` reading its own envelope.
/// Lets the per-item render logic serve both the list impls and the `Graphic` leaf variants without cloning.
pub(crate) enum ItemRef<'a, T> {
	ListItem(&'a List<T>, usize),
	Item(&'a Item<T>),
}

impl<T> Copy for ItemRef<'_, T> {}
impl<T> Clone for ItemRef<'_, T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<'a, T> ItemRef<'a, T> {
	pub(crate) fn element(self) -> Option<&'a T> {
		match self {
			ItemRef::ListItem(list, index) => list.element(index),
			ItemRef::Item(item) => Some(item.element()),
		}
	}

	pub(crate) fn attribute<A: 'static>(self, key: &str) -> Option<&'a A> {
		match self {
			ItemRef::ListItem(list, index) => list.attribute(key, index),
			ItemRef::Item(item) => item.attribute(key),
		}
	}

	pub(crate) fn attribute_cloned_or<A: Clone + 'static>(self, key: &str, fallback: A) -> A {
		match self {
			ItemRef::ListItem(list, index) => list.attribute_cloned_or(key, index, fallback),
			ItemRef::Item(item) => item.attribute_cloned_or(key, fallback),
		}
	}

	pub(crate) fn attribute_cloned_or_default<A: Clone + Default + 'static>(self, key: &str) -> A {
		match self {
			ItemRef::ListItem(list, index) => list.attribute_cloned_or_default(key, index),
			ItemRef::Item(item) => item.attribute_cloned_or_default(key),
		}
	}

	/// The alpha multiplier this item's opacity attributes apply when it serves as a paint.
	/// Fill opacity fades a paint just as opacity does, but a masker drops it so it cannot reach the content clipped to it.
	pub(crate) fn paint_opacity(self, for_mask: bool) -> f32 {
		let opacity_fill = if for_mask { 1. } else { self.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.) };

		(self.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) * opacity_fill) as f32
	}

	pub(crate) fn clone_item_attributes(self) -> core_types::list::ItemAttributeValues {
		match self {
			ItemRef::ListItem(list, index) => list.clone_item_attributes(index),
			ItemRef::Item(item) => item.attributes().clone(),
		}
	}

	/// The last layer ID of the item's `editor:layer_path` tag, if any.
	fn layer(self) -> Option<NodeId> {
		self.attribute::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH).and_then(|path| path.0.iter_element_values().next_back().copied())
	}
}

/// The color one paint item contributes, faded by its opacity attributes.
pub(crate) fn faded_paint_color(item: ItemRef<'_, Color>, for_mask: bool) -> Option<Color> {
	let color = item.element()?;

	Some(color.with_alpha(color.a() * item.paint_opacity(for_mask)))
}

/// Composites one paint color over the stack beneath it, mixing by the blend mode and then source-over in straight alpha.
fn composite_paint_over(over: Color, under: Color, blend_mode: BlendMode) -> Color {
	let (over_alpha, under_alpha) = (over.a(), under.a());

	// These modes only move the backdrop's alpha, leaving its color alone
	match blend_mode {
		BlendMode::Erase => return under.with_alpha((under_alpha - over_alpha).clamp(0., 1.)),
		BlendMode::Restore => return under.with_alpha((under_alpha + over_alpha).clamp(0., 1.)),
		BlendMode::MultiplyAlpha => return under.with_alpha(under_alpha * over_alpha),
		_ => {}
	}

	let result_alpha = over_alpha + under_alpha * (1. - over_alpha);
	if result_alpha <= 0. {
		return Color::TRANSPARENT;
	}

	// The blend formulas read their backdrop premultiplied
	let premultiplied_under = Color::from_rgbaf32_unchecked(under.r() * under_alpha, under.g() * under_alpha, under.b() * under_alpha, under_alpha);
	let mixed = apply_blend_mode(over, premultiplied_under, blend_mode);

	// The mode only mixes where the backdrop has coverage, so its alpha interpolates each source channel from the raw color to the mixed color
	let source_channel = |over_channel: f32, mixed_channel: f32| over_channel * (1. - under_alpha) + mixed_channel * under_alpha;

	let channel =
		|mixed_channel: f32, over_channel: f32, under_channel: f32| (source_channel(over_channel, mixed_channel) * over_alpha + under_channel * under_alpha * (1. - over_alpha)) / result_alpha;

	Color::from_rgbaf32_unchecked(
		channel(mixed.r(), over.r(), under.r()),
		channel(mixed.g(), over.g(), under.g()),
		channel(mixed.b(), over.b(), under.b()),
		result_alpha,
	)
}

/// Flattens a rank-1 color paint into the single color the fast path emits, stacking the items in paint order.
pub(crate) fn composite_paint_colors(list: &List<Color>, for_mask: bool) -> Option<Color> {
	let mut composited = None;

	for index in 0..list.len() {
		let item = ItemRef::ListItem(list, index);
		let Some(faded) = faded_paint_color(item, for_mask) else { continue };

		composited = Some(match composited {
			// The lowest paint has nothing beneath it, so its blend mode has nothing to act on
			None => faded,
			Some(under) => composite_paint_over(faded, under, item.attribute_cloned_or_default(ATTR_BLEND_MODE)),
		});
	}

	composited
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum MaskType {
	Clip,
	Mask,
}

impl MaskType {
	fn to_attribute(self) -> String {
		match self {
			Self::Mask => "mask".to_string(),
			Self::Clip => "clip-path".to_string(),
		}
	}

	fn write_to_defs(self, svg_defs: &mut String, uuid: u64, svg_string: String) {
		let id = format!("mask-{uuid}");
		match self {
			Self::Clip => write!(svg_defs, r##"<clipPath id="{id}">{svg_string}</clipPath>"##).unwrap(),
			Self::Mask => write!(svg_defs, r##"<mask id="{id}" mask-type="alpha">{svg_string}</mask>"##).unwrap(),
		}
	}
}

/// Mutable state used whilst rendering to an SVG
pub struct SvgRender {
	pub svg: Vec<SvgSegment>,
	pub svg_defs: String,
	pub transform: DAffine2,
	pub image_data: HashMap<CacheHashWrapper<Image<Color>>, u64>,
	indent: usize,
}

impl SvgRender {
	pub fn new() -> Self {
		Self {
			svg: Vec::default(),
			svg_defs: String::new(),
			transform: DAffine2::IDENTITY,
			image_data: HashMap::new(),
			indent: 0,
		}
	}

	pub fn indent(&mut self) {
		self.svg.push("\n".into());
		self.svg.push("\t".repeat(self.indent).into());
	}

	/// Add an outer `<svg>...</svg>` tag with a `viewBox` and the `<defs />`
	pub fn format_svg(&mut self, bounds_min: DVec2, bounds_max: DVec2) {
		let (x, y) = bounds_min.into();
		let (size_x, size_y) = (bounds_max - bounds_min).into();
		let svg_header = format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:graphite="https://graphite.art" viewBox="{x} {y} {size_x} {size_y}"><defs>{defs}</defs>"#,
			defs = &self.svg_defs
		);
		self.svg_defs = String::new();
		self.svg.insert(0, svg_header.into());
		self.svg.push("</svg>".into());
	}

	/// Wraps the SVG with `<svg><g transform="...">...</g></svg>`, which allows for rotation
	pub fn wrap_with_transform(&mut self, transform: DAffine2, size: Option<DVec2>) {
		let view_box = size
			.map(|size| format!("viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\"", size.x, size.y, size.x, size.y))
			.unwrap_or_default();

		let matrix = format_transform_matrix(transform);
		let transform = if matrix.is_empty() { String::new() } else { format!(r#" transform="{matrix}""#) };

		let svg_header = format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:graphite="https://graphite.art" {view_box}><defs>{defs}</defs><g{transform}>"#,
			defs = &self.svg_defs
		);
		self.svg_defs = String::new();
		self.svg.insert(0, svg_header.into());
		self.svg.push("</g></svg>".into());
	}

	pub fn leaf_tag(&mut self, name: impl Into<SvgSegment>, attributes: impl FnOnce(&mut SvgRenderAttrs)) {
		self.indent();

		self.svg.push("<".into());
		self.svg.push(name.into());

		attributes(&mut SvgRenderAttrs(self));

		self.svg.push("/>".into());
	}

	pub fn leaf_node(&mut self, content: impl Into<SvgSegment>) {
		self.indent();
		self.svg.push(content.into());
	}

	pub fn parent_tag(&mut self, name: impl Into<SvgSegment>, attributes: impl FnOnce(&mut SvgRenderAttrs), inner: impl FnOnce(&mut Self)) {
		let name = name.into();
		self.indent();
		self.svg.push("<".into());
		self.svg.push(name.clone());
		// Wraps `self` in a newtype (1-tuple) which is then mutated by the `attributes` closure
		attributes(&mut SvgRenderAttrs(self));
		self.svg.push(">".into());
		let length = self.svg.len();
		self.indent += 1;
		inner(self);
		self.indent -= 1;
		if self.svg.len() != length {
			self.indent();
			self.svg.push("</".into());
			self.svg.push(name);
			self.svg.push(">".into());
		} else {
			self.svg.pop();
			self.svg.push("/>".into());
		}
	}
}

pub struct SvgRenderOutput {
	pub svg: String,
	pub svg_defs: String,
	pub image_data: HashMap<CacheHashWrapper<Image<Color>>, u64>,
}

impl From<&SvgRenderOutput> for SvgRender {
	fn from(value: &SvgRenderOutput) -> Self {
		Self {
			svg: vec![value.svg.clone().into()],
			svg_defs: value.svg_defs.clone(),
			transform: DAffine2::IDENTITY,
			image_data: value.image_data.clone(),
			indent: 0,
		}
	}
}

impl From<SvgRender> for SvgRenderOutput {
	fn from(val: SvgRender) -> Self {
		Self {
			svg: val.svg.to_svg_string(),
			svg_defs: val.svg_defs,
			image_data: val.image_data,
		}
	}
}

impl Default for SvgRender {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Clone, Debug, Default)]
pub struct RenderContext {
	pub resource_overrides: Vec<(peniko::ImageBrush, Texture)>,
}

#[derive(Default, Clone, Copy, Hash, graphene_hash::CacheHash)]
pub enum RenderOutputType {
	#[default]
	Svg,
	Vello,
}

/// Static state used whilst rendering
#[derive(Default, Clone, CacheHash)]
pub struct RenderParams {
	pub render_mode: RenderMode,
	pub footprint: Footprint,
	#[cache_hash(skip)]
	pub scale: f64,
	pub render_output_type: RenderOutputType,
	pub thumbnail: bool,
	/// Are we exporting
	pub for_export: bool,
	/// Are we generating a mask in this render pass? Used to see if fill should be multiplied with alpha.
	pub for_mask: bool,
	/// Are we generating a mask for alignment? Used to prevent unnecessary transforms in masks
	pub alignment_parent_transform: Option<DAffine2>,
	pub aligned_strokes: bool,
	/// Paint the stroke below the fill within the same SVG path element
	pub stroke_below: bool,
	/// Are we rendering for a pattern content
	pub inside_pattern: bool,
	pub artboard_background: Option<Color>,
	/// Viewport zoom level (document-space scale). Used to compute constant viewport-pixel stroke widths in Outline mode.
	pub viewport_zoom: f64,
	/// The nearest ancestor's appearance, cascading to items that lack their own.
	pub inherited_appearance: Option<Appearance>,
}

impl RenderParams {
	pub fn for_clipper(&self) -> Self {
		Self { for_mask: true, ..self.clone() }
	}

	pub fn for_alignment(&self, transform: DAffine2) -> Self {
		Self {
			alignment_parent_transform: Some(transform),
			..self.clone()
		}
	}

	pub fn for_pattern(&self) -> Self {
		// A paint subtree supplies its own styling, so the painted element's appearance must not cascade into it
		Self {
			inside_pattern: true,
			inherited_appearance: None,
			..self.clone()
		}
	}

	/// Params for rendering a child item, cascading this item's appearance to descendants lacking their own.
	/// Callers only build these when the item carries a declared appearance, so an item without one clones nothing.
	pub fn for_child_item(&self, item_appearance: &Appearance) -> Self {
		Self {
			inherited_appearance: Some(item_appearance.clone()),
			..self.clone()
		}
	}

	pub fn to_canvas(&self) -> bool {
		!self.for_export && !self.thumbnail && !self.for_mask && !self.inside_pattern
	}
}

pub fn format_transform_matrix(transform: DAffine2) -> String {
	if transform == DAffine2::IDENTITY {
		return String::new();
	}

	transform.to_cols_array().iter().enumerate().fold("matrix(".to_string(), |val, (i, num)| {
		let num = if num.abs() < 1_000_000_000. { (num * 1_000_000_000.).round() / 1_000_000_000. } else { *num };
		let num = if num.is_zero() { "0".to_string() } else { num.to_string() };
		let comma = if i == 5 { "" } else { "," };
		val + &(num + comma)
	}) + ")"
}

/// `(max, min)` factors by which a unit vector is stretched under `transform`'s linear part — the
/// principal and minor singular values, equal to the semi-axes of the ellipse a unit circle maps to.
/// Equivalent to `(max(sx, sy), min(sx, sy))` for axis-aligned scales, but accounts for shear.
fn singular_values(transform: DAffine2) -> (f64, f64) {
	let m = transform.matrix2;
	let a = m.x_axis.x;
	let b = m.x_axis.y;
	let c = m.y_axis.x;
	let d = m.y_axis.y;
	// Eigenvalues of MᵀM via the closed form for a 2×2, both are non-negative
	let trace = a * a + b * b + c * c + d * d;
	let det = a * d - b * c;
	let discriminant = (trace * trace - 4. * det * det).max(0.).sqrt();
	let largest_eigenvalue = (trace + discriminant) * 0.5;
	let smallest_eigenvalue = ((trace - discriminant) * 0.5).max(0.);
	(largest_eigenvalue.sqrt(), smallest_eigenvalue.sqrt())
}

pub fn black_or_white_for_best_contrast(background: Option<Color>) -> Color {
	let Some(bg) = background else { return core_types::consts::LAYER_OUTLINE_STROKE_COLOR };

	let alpha = bg.a();

	// Un-premultiply, then encode to gamma sRGB to do the composite in display space.
	let (gamma_r, gamma_g, gamma_b) = if alpha > f32::EPSILON {
		let [r, g, b, _] = Color::from_rgbaf32_unchecked(bg.r() / alpha, bg.g() / alpha, bg.b() / alpha, alpha).to_gamma_srgb_channels();
		(r, g, b)
	} else {
		(0., 0., 0.)
	};

	// Composite over black in sRGB space (premultiplied by alpha), then decode to linear for the luminance test.
	let composited = Color::from_gamma_srgb_channels(gamma_r * alpha, gamma_g * alpha, gamma_b * alpha, 1.);

	let threshold = (1.05 * 0.05f32).sqrt() - 0.05;

	if composited.luminance_rec_709() > threshold { Color::BLACK } else { Color::WHITE }
}

pub fn to_transform(transform: DAffine2) -> usvg::Transform {
	let cols = transform.to_cols_array();
	usvg::Transform::from_row(cols[0] as f32, cols[1] as f32, cols[2] as f32, cols[3] as f32, cols[4] as f32, cols[5] as f32)
}

fn to_point(p: DVec2) -> kurbo::Point {
	kurbo::Point::new(p.x, p.y)
}

fn get_outline_styles(render_params: &RenderParams) -> (kurbo::Stroke, peniko::Color) {
	use core_types::consts::LAYER_OUTLINE_STROKE_WEIGHT;

	let outline_stroke = kurbo::Stroke {
		width: LAYER_OUTLINE_STROKE_WEIGHT / if render_params.viewport_zoom > 0. { render_params.viewport_zoom } else { 1. },
		miter_limit: 4.,
		join: Join::Miter,
		start_cap: Cap::Butt,
		end_cap: Cap::Butt,
		dash_pattern: Default::default(),
		dash_offset: 0.,
	};

	let outline_color = black_or_white_for_best_contrast(render_params.artboard_background);
	let outline_color_peniko = SRGBA8::from(outline_color).to_peniko_color();

	(outline_stroke, outline_color_peniko)
}

fn draw_raster_outline(scene: &mut Scene, outline_transform: &DAffine2, render_params: &RenderParams) {
	let (outline_stroke, outline_color_peniko) = get_outline_styles(render_params);

	let mut outline_path = rectangle_path(DVec2::ZERO, DVec2::ONE);
	outline_path.apply_affine(Affine::new(outline_transform.to_cols_array()));

	scene.stroke(&outline_stroke, Affine::IDENTITY, outline_color_peniko, None, &outline_path);
}

/// Emits an SVG `<path>` element with the resolved fill attribute corresponding to the given fill_graphic.
#[allow(clippy::too_many_arguments)]
fn emit_svg_fill_path(
	render: &mut SvgRender,
	d: String,
	fill_paint: Option<&Graphic>,
	item_transform: DAffine2,
	element_transform: DAffine2,
	applied_stroke_transform: DAffine2,
	bounds_matrix: DAffine2,
	render_params: &RenderParams,
) {
	render.leaf_tag("path", |attributes| {
		attributes.push("d", d);
		let matrix = format_transform_matrix(element_transform);
		if !matrix.is_empty() {
			attributes.push(ATTR_TRANSFORM, matrix);
		}
		let defs = &mut attributes.0.svg_defs;
		let fill_attribute = fill_paint
			.map(|paint| paint.render(defs, item_transform, element_transform, applied_stroke_transform, bounds_matrix, render_params, PaintTarget::Fill))
			.unwrap_or_else(|| r#" fill="none""#.to_string());
		attributes.push_val(fill_attribute);
	});
}

/// The whole-ramp settings a gradient item carries beside its element, defaulting each absent one.
pub(crate) fn gradient_settings_from_item(item: ItemRef<'_, Gradient>) -> GradientSettings {
	match item {
		ItemRef::ListItem(list, index) => GradientSettings::from_list_row_attributes(list, index),
		ItemRef::Item(item) => GradientSettings::from_item_attributes(item),
	}
}

/// Whether the affine transform inverts to a finite matrix (a zero, subnormal, or NaN determinant does not).
pub(crate) fn transform_is_invertible(transform: DAffine2) -> bool {
	transform.matrix2.determinant().recip().is_finite()
}

/// Maps a gradient's `transform` into the frame handed to the renderer: radial keeps the full matrix (so a
/// non-uniform transform makes an ellipse), while linear is reduced to the equivalent non-sheared gradient line (the
/// axis projected onto the band normal) so the iso-color bands keep following a sheared transform, which Vello can
/// represent since it stores only two endpoints.
pub(crate) fn gradient_placement(transform: DAffine2, gradient_form: GradientForm) -> DAffine2 {
	match gradient_form {
		GradientForm::Radial => transform,
		GradientForm::Linear => {
			let axis = transform.matrix2.x_axis;
			let band_normal = transform.matrix2.y_axis.perp();
			let line = if band_normal.length_squared() > 0. { axis.project_onto(band_normal) } else { axis };
			DAffine2 {
				matrix2: DMat2::from_cols(line, line.perp()),
				translation: transform.translation,
			}
		}
	}
}

/// Texel count of the baked gradient ramp Vello samples stops through (`N_SAMPLES`/`GRADIENT_WIDTH` in vello_encoding).
const VELLO_GRADIENT_RAMP_TEXELS: f64 = 512.;

/// Renderable gradient samples of `(position, color, original midpoint)`, as produced by [`Gradient::interpolated_samples`].
type GradientSamples = Vec<(f64, Color, Option<f64>)>;

/// Where a renderer needs the transparent guard stops that emulate the `Clear` spread, which neither SVG nor Vello supports natively.
#[derive(Copy, Clone, PartialEq)]
pub(crate) enum ClearGuardPlacement {
	/// Guards share the range ends' exact offsets, resolved against the visible colors by stop order alone.
	SvgStopOrder,
	/// Guards own the outermost ramp texel at each cleared end, since Vello's pad extension samples those texels for
	/// everything beyond the ends and its ramp bake would tie-break a shared-offset guard away. The visible range
	/// compresses inward by one texel per cleared end, costing about 0.4% of the ramp's color resolution.
	VelloRampTexels,
}

/// The gradient's renderable samples plus the gradient-space span `(start, end)` the renderer's 0 to 1 offset range must cover, normally the unit interval with the samples unchanged.
///
/// The `Clear` spread brackets the samples with transparent guard stops placed per `guards`: the pad extension then
/// paints transparency outward while hard stops cut the paint off exactly at the unit range's boundaries. A radial
/// gradient's span still starts at zero, since its sampling distance never goes below the center.
pub(crate) fn spread_adjusted_samples(gradient: &Gradient, settings: GradientSettings, gradient_form: GradientForm, guards: ClearGuardPlacement) -> (GradientSamples, (f64, f64)) {
	let samples = gradient.interpolated_samples(settings);
	if settings.spread != GradientSpread::Clear {
		return (samples, (0., 1.));
	}

	// The remapped offsets where the visible range's ends land, with the guards owning whatever lies outside them
	let texel = 1. / (VELLO_GRADIENT_RAMP_TEXELS - 1.);
	let (start_offset, end_offset) = match (guards, gradient_form) {
		(ClearGuardPlacement::SvgStopOrder, _) => (0., 1.),
		(ClearGuardPlacement::VelloRampTexels, GradientForm::Linear) => (texel, 1. - texel),
		(ClearGuardPlacement::VelloRampTexels, GradientForm::Radial) => (0., 1. - texel),
	};
	let remap = |position: f64| (1. - position) * start_offset + position * end_offset;

	// The geometric span grows to compensate for the compression, keeping the visible range at the unit interval
	let scale = 1. / (end_offset - start_offset);
	let span = (-start_offset * scale, (1. - start_offset) * scale);

	// A stopless gradient paints solid black, matching `Gradient::evaluate`
	let first_color = samples.first().map_or(Color::BLACK, |&(_, color, _)| color);
	let last_color = samples.last().map_or(Color::BLACK, |&(_, color, _)| color);
	let needs_start_anchor = samples.first().is_none_or(|&(position, ..)| position > 0.);
	let needs_end_anchor = samples.last().is_none_or(|&(position, ..)| position < 1.);

	let mut adjusted = Vec::with_capacity(samples.len() + 4);

	// Lead with the transparent guard (linear only, a radial's center is already the sampling minimum), then anchor the visible range's start color
	if gradient_form == GradientForm::Linear {
		adjusted.push((0., Color::TRANSPARENT, None));
	}
	if needs_start_anchor {
		adjusted.push((remap(0.), first_color, None));
	}

	adjusted.extend(samples.into_iter().map(|(position, color, midpoint)| (remap(position), color, midpoint)));

	// Anchor the visible range's end color, then cut to the trailing transparent guard
	if needs_end_anchor {
		adjusted.push((remap(1.), last_color, None));
	}
	adjusted.push((1., Color::TRANSPARENT, None));

	(adjusted, span)
}

/// Converts a gradient's renderer samples to peniko color stops, duplicating an off-zero first stop at position 0 since Vello ignores the first stop's position and always treats it as 0.
fn peniko_color_stops(samples: &[(f64, Color, Option<f64>)]) -> peniko::ColorStops {
	let mut peniko_stops = peniko::ColorStops::new();

	for &(position, color, _) in samples {
		let color = peniko::color::DynamicColor::from_alpha_color(SRGBA8::from(color).to_peniko_color());

		if peniko_stops.is_empty() && position > 0. {
			peniko_stops.push(peniko::ColorStop { offset: 0., color });
		}

		peniko_stops.push(peniko::ColorStop { offset: position as f32, color });
	}

	// A gradient with no stops paints as solid black, matching `Gradient::evaluate`
	if peniko_stops.is_empty() {
		peniko_stops.push(peniko::ColorStop {
			offset: 0.,
			color: peniko::color::DynamicColor::from_alpha_color(SRGBA8::from(Color::BLACK).to_peniko_color()),
		});
	}

	peniko_stops
}

/// The peniko extend mode for a spread; `Clear` rides pad, with the transparent guard stops from `spread_adjusted_samples` doing the clearing.
fn peniko_extend(gradient_spread: GradientSpread) -> peniko::Extend {
	match gradient_spread {
		GradientSpread::Pad | GradientSpread::Clear => peniko::Extend::Pad,
		GradientSpread::Reflect => peniko::Extend::Reflect,
		GradientSpread::Repeat => peniko::Extend::Repeat,
	}
}

/// The Vello brush for one gradient item, paired with its placement transform.
/// `for_mask` keeps the fill opacity at full, as [`ItemRef::paint_opacity`] explains.
fn create_peniko_gradient_brush(gradient_item: ItemRef<'_, Gradient>, multiplied_transform: &DAffine2, for_mask: bool) -> Option<(peniko::Brush, DAffine2)> {
	let stops = gradient_item.element()?;

	let gradient_form: GradientForm = gradient_item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
	let gradient_transform: DAffine2 = gradient_item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let settings = gradient_settings_from_item(gradient_item);

	let (mut samples, span) = spread_adjusted_samples(stops, settings, gradient_form, ClearGuardPlacement::VelloRampTexels);

	let paint_opacity = gradient_item.paint_opacity(for_mask);
	if paint_opacity < 1. {
		// A stopless ramp gets its black stop downstream, too late to be faded, so it needs one here instead
		if samples.is_empty() {
			samples.push((0., Color::BLACK, None));
		}

		for (_, color, _) in &mut samples {
			*color = color.with_alpha(color.a() * paint_opacity);
		}
	}

	let peniko_stops = peniko_color_stops(&samples);

	// The unit gradient is placed by the desheared frame so a non-uniform transform produces the intended ellipse
	let (start, end, gradient_to_device) = (DVec2::X * span.0, DVec2::X * span.1, gradient_placement(multiplied_transform * gradient_transform, gradient_form));

	let brush = peniko::Brush::Gradient(peniko::Gradient {
		kind: match gradient_form {
			GradientForm::Linear => peniko::LinearGradientPosition {
				start: to_point(start),
				end: to_point(end),
			}
			.into(),
			GradientForm::Radial => peniko::RadialGradientPosition {
				start_center: to_point(start),
				start_radius: 0.,
				end_center: to_point(start),
				end_radius: start.distance(end) as f32,
			}
			.into(),
		},
		extend: peniko_extend(settings.spread),
		stops: peniko_stops,
		// Straight alpha, keeping parity with the SVG renderer's stop interpolation
		interpolation_alpha_space: peniko::InterpolationAlphaSpace::Unpremultiplied,
		..Default::default()
	});

	Some((brush, gradient_to_device))
}

// TODO: Click targets can be removed from the render output, since the vector data is available in the vector modify data from Monitor nodes.
// This will require that the transform for child layers into that layer space be calculated, or it could be returned from the RenderOutput instead of click targets.
#[derive(Debug, Default, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RenderMetadata {
	pub upstream_footprints: HashMap<NodeId, Footprint>,
	pub local_transforms: HashMap<NodeId, DAffine2>,
	pub first_element_source_id: HashMap<NodeId, Option<NodeId>>,
	pub click_targets: HashMap<NodeId, Vec<Arc<ClickTarget>>>,
	/// Source-geometry outlines for hover/selection overlays, separate from `click_targets` so
	/// nodes with an `editor:click_target` override still outline the precise geometry.
	pub outlines: HashMap<NodeId, Vec<Arc<ClickTarget>>>,
	/// Per-layer text frame from item 0's `editor:text_frame` attribute.
	/// The Text tool composes this with `transform_to_viewport(layer)` to position its drag cage.
	pub text_frames: HashMap<NodeId, DAffine2>,
	pub clip_targets: HashSet<NodeId>,
	pub vector_data: HashMap<NodeId, Arc<Vector>>,
	/// Per-layer `ATTR_APPEARANCE` item attribute, exposed so message handlers can read it.
	#[cfg_attr(feature = "serde", serde(skip))]
	pub appearance_attributes: HashMap<NodeId, Arc<Appearance>>,
	pub backgrounds: Vec<Background>,
}

impl RenderMetadata {
	pub fn apply_transform(&mut self, transform: DAffine2) {
		for value in self.upstream_footprints.values_mut() {
			value.transform = transform * value.transform;
		}
	}

	/// Merge another RenderMetadata into this one.
	/// Values from `other` take precedence for duplicate keys.
	pub fn merge(&mut self, other: &RenderMetadata) {
		// Destructure Self to get errors when new fields are added to the struct
		let RenderMetadata {
			upstream_footprints,
			local_transforms,
			first_element_source_id,
			click_targets,
			outlines,
			text_frames,
			clip_targets,
			vector_data,
			appearance_attributes,
			backgrounds,
		} = self;
		upstream_footprints.extend(other.upstream_footprints.iter());
		local_transforms.extend(other.local_transforms.iter());
		first_element_source_id.extend(other.first_element_source_id.iter());
		click_targets.extend(other.click_targets.iter().map(|(k, v)| (*k, v.clone())));
		outlines.extend(other.outlines.iter().map(|(k, v)| (*k, v.clone())));
		text_frames.extend(other.text_frames.iter());
		clip_targets.extend(other.clip_targets.iter());
		vector_data.extend(other.vector_data.iter().map(|(id, data)| (*id, data.clone())));
		appearance_attributes.extend(other.appearance_attributes.iter().map(|(id, data)| (*id, data.clone())));

		// TODO: Find a better non O(n^2) way to merge backgrounds
		for background in &other.backgrounds {
			if !backgrounds.contains(background) {
				backgrounds.push(background.clone());
			}
		}
	}
}

#[derive(Debug, Default, Clone, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct Background {
	pub location: DVec2,
	pub dimensions: DVec2,
}

// TODO: Rename to "Graphical"
pub trait Render: BoundingBox + RenderComplexity {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams);

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, _render_params: &RenderParams);

	/// The upstream click targets for each layer are collected during the render so that they do not have to be calculated for each click detection.
	/// `inherited_appearance` is the nearest ancestor's appearance, cascading to items that lack their own, mirroring the render cascade.
	fn add_upstream_click_targets(&self, _click_targets: &mut Vec<ClickTarget>, _inherited_appearance: Option<&Appearance>) {}

	/// Like `add_upstream_click_targets` but for visual outlines. `List<Vector>` overrides this to ignore `editor:click_target` so outlines reflect the actual geometry.
	fn add_upstream_outline_targets(&self, outlines: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
		self.add_upstream_click_targets(outlines, inherited_appearance);
	}

	// TODO: Store all click targets in a vec which contains the AABB, click target, and path
	// fn add_click_targets(&self, click_targets: &mut Vec<([DVec2; 2], ClickTarget, Vec<NodeId>)>, current_path: Option<NodeId>) {}

	/// Recursively iterate over data in the render (including nested layer stacks upstream of a vector node, in the case of a boolean operation) to collect the footprints, click targets, and vector modify.
	fn collect_metadata(&self, _metadata: &mut RenderMetadata, _footprint: Footprint, _element_id: Option<NodeId>, _inherited_appearance: Option<&Appearance>) {}

	fn contains_artboard(&self) -> bool {
		false
	}

	fn new_ids_from_hash(&mut self, _reference: Option<NodeId>) {}
}

/// Emits one item of graphic content as SVG, wrapped in a group carrying the item's transform, opacity, and blend mode.
/// `mask_state` carries the sibling clipping run between a list's items; a lone item has no siblings, so both mask inputs stay inert.
fn render_graphic_item_svg(item: ItemRef<'_, Graphic>, next_clips: bool, mask_state: &mut Option<(u64, MaskType)>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(element) = item.element() else { return };
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	// This item's declared appearance (if any) cascades to descendants lacking their own
	let child_render_params = item
		.attribute::<Appearance>(ATTR_APPEARANCE)
		.and_then(Appearance::declared)
		.map(|appearance| render_params.for_child_item(appearance));
	let render_params = child_render_params.as_ref().unwrap_or(render_params);

	let matrix = format_transform_matrix(transform);
	let mut masked_by = None;

	if next_clips && mask_state.is_none() {
		let uuid = generate_uuid();
		let mask_type = if element.can_reduce_to_clip_path() { MaskType::Clip } else { MaskType::Mask };

		let mut svg = SvgRender::new();
		element.render_svg(&mut svg, &render_params.for_clipper());

		// The def is resolved in this list's space, so the masker's own transform has to be baked into it
		let masker = match matrix.is_empty() {
			true => svg.svg.to_svg_string(),
			false => format!(r##"<g transform="{matrix}">{}</g>"##, svg.svg.to_svg_string()),
		};

		render.svg_defs.push_str(&svg.svg_defs);
		mask_type.write_to_defs(&mut render.svg_defs, uuid, masker);

		*mask_state = Some((uuid, mask_type));
	} else if let Some((uuid, mask_type)) = *mask_state {
		if !next_clips {
			*mask_state = None;
		}

		masked_by = Some((mask_type.to_attribute(), format!("url(#mask-{uuid})")));
	}

	let render_item = |render: &mut SvgRender| {
		render.parent_tag(
			"g",
			|attributes| {
				if !matrix.is_empty() {
					attributes.push(ATTR_TRANSFORM, matrix.clone());
				}

				let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}

				if blend_mode != BlendMode::default() {
					attributes.push("style", blend_mode.render());
				}
			},
			|render| element.render_svg(render, render_params),
		);
	};

	// The mask rides an untransformed wrapper so it resolves in this list's space rather than the item's own
	match masked_by {
		Some((attribute, selector)) => render.parent_tag("g", |attributes| attributes.push(attribute, selector), render_item),
		None => render_item(render),
	}
}

/// Draws one item of graphic content into the Vello scene, layering for the item's opacity, blend mode, and sibling clipping.
/// `mask_element_and_transform` carries the clipping run between a list's items; a lone item passes inert mask inputs.
#[allow(clippy::too_many_arguments)]
fn render_graphic_item_to_vello<'a>(
	item: ItemRef<'a, Graphic>,
	next_clips: bool,
	mask_element_and_transform: &mut Option<(&'a Graphic, DAffine2)>,
	scene: &mut Scene,
	transform: DAffine2,
	context: &mut RenderContext,
	render_params: &RenderParams,
) {
	let Some(element) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let transform = transform * item_transform;
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	// This item's declared appearance (if any) cascades to descendants lacking their own
	let child_render_params = item
		.attribute::<Appearance>(ATTR_APPEARANCE)
		.and_then(Appearance::declared)
		.map(|appearance| render_params.for_child_item(appearance));
	let render_params = child_render_params.as_ref().unwrap_or(render_params);

	let mut layer = false;

	let blend_mode = match render_params.render_mode {
		RenderMode::Outline => peniko::Mix::Normal,
		_ => blend_mode_attr.to_peniko(),
	};
	let mut bounds = RenderBoundingBox::None;

	let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
	if opacity < 1. || (render_params.render_mode != RenderMode::Outline && blend_mode_attr != BlendMode::default()) {
		bounds = element.bounding_box(transform, true);

		if let RenderBoundingBox::Rectangle(bounds) = bounds {
			scene.push_layer(
				peniko::Fill::NonZero,
				peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver),
				opacity,
				kurbo::Affine::IDENTITY,
				&kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y),
			);
			layer = true;
		}
	}

	if next_clips && mask_element_and_transform.is_none() {
		*mask_element_and_transform = Some((element, transform));

		element.render_to_vello(scene, transform, context, render_params);
	} else if let Some((mask_element, transform_mask)) = *mask_element_and_transform {
		if !next_clips {
			*mask_element_and_transform = None;
		}
		if !layer {
			bounds = element.bounding_box(transform, true);
		}

		if let RenderBoundingBox::Rectangle(bounds) = bounds {
			let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);

			scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., kurbo::Affine::IDENTITY, &rect);
			mask_element.render_to_vello(scene, transform_mask, context, &render_params.for_clipper());
			scene.push_layer(
				peniko::Fill::NonZero,
				peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcIn),
				1.,
				kurbo::Affine::IDENTITY,
				&rect,
			);
		}

		element.render_to_vello(scene, transform, context, render_params);

		if matches!(bounds, RenderBoundingBox::Rectangle(_)) {
			scene.pop_layer();
			scene.pop_layer();
		}
	} else {
		element.render_to_vello(scene, transform, context, render_params);
	}

	if layer {
		scene.pop_layer();
	}
}

/// Recurses one item of graphic content for metadata, composing the item's transform into the footprint and cascading its appearance.
fn collect_graphic_item_metadata(item: ItemRef<'_, Graphic>, metadata: &mut RenderMetadata, footprint: Footprint, inherited_appearance: Option<&Appearance>) {
	let Some(element) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	// This item's appearance (if any) cascades to descendants lacking their own
	let child_appearance = Appearance::cascade(item.attribute::<Appearance>(ATTR_APPEARANCE), inherited_appearance);

	let mut footprint = footprint;
	footprint.transform *= item_transform;

	// An anonymous wrapper item (no layer tag) still recurses to reach nested content with "editor:layer_path" attributes
	element.collect_metadata(metadata, footprint, item.layer(), child_appearance);
}

/// Collects one graphic item's click and outline targets, baked through the item's transform.
fn collect_graphic_item_targets(item: ItemRef<'_, Graphic>, inherited_appearance: Option<&Appearance>, click_targets: &mut Vec<ClickTarget>, outlines: &mut Vec<ClickTarget>) {
	let Some(element) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let child_appearance = Appearance::cascade(item.attribute::<Appearance>(ATTR_APPEARANCE), inherited_appearance);

	let mut new_click_targets = Vec::new();
	element.add_upstream_click_targets(&mut new_click_targets, child_appearance);
	for click_target in new_click_targets.iter_mut() {
		click_target.apply_transform(item_transform)
	}
	click_targets.extend(new_click_targets);

	let mut new_outlines = Vec::new();
	element.add_upstream_outline_targets(&mut new_outlines, child_appearance);
	for outline in new_outlines.iter_mut() {
		outline.apply_transform(item_transform)
	}
	outlines.extend(new_outlines);
}

/// The full metadata pass over a run of graphic items: per-item recursion, then the aggregated targets when an `element_id` names the run.
fn collect_graphic_items_metadata<'a>(
	items: impl Iterator<Item = ItemRef<'a, Graphic>> + Clone,
	metadata: &mut RenderMetadata,
	footprint: Footprint,
	element_id: Option<NodeId>,
	inherited_appearance: Option<&Appearance>,
) {
	for item in items.clone() {
		collect_graphic_item_metadata(item, metadata, footprint, inherited_appearance);
	}

	if let Some(element_id) = element_id {
		let mut all_upstream_click_targets = Vec::new();
		let mut all_upstream_outlines = Vec::new();

		for item in items {
			collect_graphic_item_targets(item, inherited_appearance, &mut all_upstream_click_targets, &mut all_upstream_outlines);
		}

		metadata.click_targets.insert(element_id, all_upstream_click_targets.into_iter().map(|x| x.into()).collect());
		metadata.outlines.insert(element_id, all_upstream_outlines.into_iter().map(|x| x.into()).collect());
	}
}

/// Collects one graphic item's click targets into the caller's list, baked through the item's transform.
fn add_graphic_item_click_targets(item: ItemRef<'_, Graphic>, click_targets: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
	let Some(element) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let child_appearance = Appearance::cascade(item.attribute::<Appearance>(ATTR_APPEARANCE), inherited_appearance);
	let mut new_click_targets = Vec::new();

	element.add_upstream_click_targets(&mut new_click_targets, child_appearance);

	for click_target in new_click_targets.iter_mut() {
		click_target.apply_transform(item_transform)
	}

	click_targets.extend(new_click_targets);
}

/// Collects one graphic item's outline targets into the caller's list, baked through the item's transform.
fn add_graphic_item_outline_targets(item: ItemRef<'_, Graphic>, outlines: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
	let Some(element) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let child_appearance = Appearance::cascade(item.attribute::<Appearance>(ATTR_APPEARANCE), inherited_appearance);
	let mut new_outlines = Vec::new();

	element.add_upstream_outline_targets(&mut new_outlines, child_appearance);

	for outline in new_outlines.iter_mut() {
		outline.apply_transform(item_transform)
	}

	outlines.extend(new_outlines);
}

impl Render for Graphic {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		match self {
			Graphic::None(_) | Graphic::NoneList(_) => (),
			Graphic::Graphic(item) => render_graphic_item_svg(ItemRef::Item(item), false, &mut None, render, render_params),
			Graphic::Vector(item) => render_vector_item_svg(ItemRef::Item(item), false, &mut None, render, render_params),
			Graphic::RasterCPU(item) => render_raster_cpu_item_svg(ItemRef::Item(item), render, render_params),
			Graphic::RasterGPU(_) => (),
			Graphic::Color(item) => render_color_item_svg(ItemRef::Item(item), render, render_params),
			Graphic::Gradient(item) => render_gradient_item_svg(ItemRef::Item(item), render, render_params),
			Graphic::Text(item) => render_text_item_svg(ItemRef::Item(item), render, render_params),
			Graphic::GraphicList(list) => list.render_svg(render, render_params),
			Graphic::VectorList(list) => list.render_svg(render, render_params),
			Graphic::RasterCPUList(list) => list.render_svg(render, render_params),
			Graphic::RasterGPUList(_) => (),
			Graphic::ColorList(list) => list.render_svg(render, render_params),
			Graphic::GradientList(list) => list.render_svg(render, render_params),
			Graphic::TextList(list) => list.render_svg(render, render_params),
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		match self {
			Graphic::None(_) | Graphic::NoneList(_) => (),
			Graphic::Graphic(item) => render_graphic_item_to_vello(ItemRef::Item(item), false, &mut None, scene, transform, context, render_params),
			Graphic::Vector(item) => {
				// A paint subtree supplies its own styling, so an element's appearance must not cascade into it
				let paint_render_params = RenderParams {
					inherited_appearance: None,
					..render_params.clone()
				};
				render_vector_item_to_vello(ItemRef::Item(item), false, &mut None, scene, transform, context, render_params, &paint_render_params);
			}
			Graphic::RasterCPU(item) => render_raster_cpu_item_to_vello(ItemRef::Item(item), scene, transform, render_params),
			Graphic::RasterGPU(item) => render_raster_gpu_item_to_vello(ItemRef::Item(item), scene, transform, context, render_params),
			Graphic::Color(item) => render_color_item_to_vello(ItemRef::Item(item), scene, render_params),
			Graphic::Gradient(item) => render_gradient_item_to_vello(ItemRef::Item(item), scene, transform, render_params),
			Graphic::Text(item) => render_text_item_to_vello(ItemRef::Item(item), scene, transform, render_params),
			Graphic::GraphicList(list) => list.render_to_vello(scene, transform, context, render_params),
			Graphic::VectorList(list) => list.render_to_vello(scene, transform, context, render_params),
			Graphic::RasterCPUList(list) => list.render_to_vello(scene, transform, context, render_params),
			Graphic::RasterGPUList(list) => list.render_to_vello(scene, transform, context, render_params),
			Graphic::ColorList(list) => list.render_to_vello(scene, transform, context, render_params),
			Graphic::GradientList(list) => list.render_to_vello(scene, transform, context, render_params),
			Graphic::TextList(list) => list.render_to_vello(scene, transform, context, render_params),
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>, inherited_appearance: Option<&Appearance>) {
		if let Some(element_id) = element_id {
			// The footprint always lands; the transform (and for vectors the source layer) comes from the first item when one exists
			let first_item_inserts = |metadata: &mut RenderMetadata, transform: DAffine2| {
				metadata.upstream_footprints.insert(element_id, footprint);
				metadata.local_transforms.insert(element_id, transform);
			};

			match self {
				Graphic::None(_) | Graphic::NoneList(_) => {}
				Graphic::Graphic(_) | Graphic::GraphicList(_) => {
					metadata.upstream_footprints.insert(element_id, footprint);
				}
				Graphic::Vector(item) => {
					first_item_inserts(metadata, item.attribute_cloned_or_default(ATTR_TRANSFORM));
					metadata.first_element_source_id.insert(element_id, ItemRef::Item(item).layer());
				}
				Graphic::VectorList(list) => {
					metadata.upstream_footprints.insert(element_id, footprint);
					// TODO: Find a way to handle more than the first item
					if !list.is_empty() {
						let transform: DAffine2 = list.attribute_cloned_or_default(ATTR_TRANSFORM, 0);

						metadata.first_element_source_id.insert(element_id, ItemRef::ListItem(list, 0).layer());
						metadata.local_transforms.insert(element_id, transform);
					}
				}
				Graphic::RasterCPU(item) => first_item_inserts(metadata, item.attribute_cloned_or_default(ATTR_TRANSFORM)),
				Graphic::RasterGPU(item) => first_item_inserts(metadata, item.attribute_cloned_or_default(ATTR_TRANSFORM)),
				Graphic::Color(item) => first_item_inserts(metadata, item.attribute_cloned_or_default(ATTR_TRANSFORM)),
				Graphic::Gradient(item) => first_item_inserts(metadata, item.attribute_cloned_or_default(ATTR_TRANSFORM)),
				Graphic::Text(item) => first_item_inserts(metadata, item.attribute_cloned_or_default(ATTR_TRANSFORM)),
				Graphic::RasterCPUList(list) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first item
					if !list.is_empty() {
						metadata.local_transforms.insert(element_id, list.attribute_cloned_or_default(ATTR_TRANSFORM, 0));
					}
				}
				Graphic::RasterGPUList(list) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first item
					if !list.is_empty() {
						metadata.local_transforms.insert(element_id, list.attribute_cloned_or_default(ATTR_TRANSFORM, 0));
					}
				}
				Graphic::ColorList(list) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first item
					if !list.is_empty() {
						metadata.local_transforms.insert(element_id, list.attribute_cloned_or_default(ATTR_TRANSFORM, 0));
					}
				}
				Graphic::GradientList(list) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first item
					if !list.is_empty() {
						metadata.local_transforms.insert(element_id, list.attribute_cloned_or_default(ATTR_TRANSFORM, 0));
					}
				}
				Graphic::TextList(list) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first item
					if !list.is_empty() {
						metadata.local_transforms.insert(element_id, list.attribute_cloned_or_default(ATTR_TRANSFORM, 0));
					}
				}
			}
		}

		match self {
			Graphic::None(_) | Graphic::NoneList(_) => (),
			Graphic::Graphic(item) => collect_graphic_items_metadata(std::iter::once(ItemRef::Item(item.as_ref())), metadata, footprint, element_id, inherited_appearance),
			Graphic::Vector(item) => collect_vector_items_metadata(std::iter::once(ItemRef::Item(item.as_ref())), metadata, footprint, element_id, inherited_appearance),
			Graphic::RasterCPU(item) => collect_raster_metadata(Some(ItemRef::Item(item)), metadata, footprint, element_id),
			Graphic::RasterGPU(item) => collect_raster_metadata(Some(ItemRef::Item(item)), metadata, footprint, element_id),
			Graphic::Color(_) => (),
			Graphic::Gradient(item) => collect_gradient_items_metadata(std::iter::once(ItemRef::Item(item)), metadata, element_id),
			Graphic::Text(item) => collect_text_items_metadata(std::iter::once(ItemRef::Item(item)), metadata, footprint, element_id),
			Graphic::GraphicList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::VectorList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::RasterCPUList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::RasterGPUList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::ColorList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::GradientList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::TextList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
		match self {
			Graphic::None(_) | Graphic::NoneList(_) => (),
			Graphic::Graphic(item) => add_graphic_item_click_targets(ItemRef::Item(item), click_targets, inherited_appearance),
			Graphic::Vector(item) => add_vector_item_click_targets(ItemRef::Item(item), click_targets, inherited_appearance),
			Graphic::RasterCPU(item) => add_unit_square_click_target(item.attribute_cloned_or_default(ATTR_TRANSFORM), click_targets),
			Graphic::RasterGPU(item) => add_unit_square_click_target(item.attribute_cloned_or_default(ATTR_TRANSFORM), click_targets),
			Graphic::Color(_) => (),
			Graphic::Gradient(item) => add_gradient_item_click_targets(ItemRef::Item(item), click_targets),
			Graphic::Text(item) => add_text_item_click_targets(ItemRef::Item(item), click_targets),
			Graphic::GraphicList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::VectorList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::RasterCPUList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::RasterGPUList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::ColorList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::GradientList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::TextList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
		}
	}

	fn add_upstream_outline_targets(&self, outlines: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
		match self {
			Graphic::None(_) | Graphic::NoneList(_) => (),
			Graphic::Graphic(item) => add_graphic_item_outline_targets(ItemRef::Item(item), outlines, inherited_appearance),
			Graphic::Vector(item) => add_vector_item_outline_targets(ItemRef::Item(item), outlines, inherited_appearance),
			Graphic::RasterCPU(item) => add_unit_square_click_target(item.attribute_cloned_or_default(ATTR_TRANSFORM), outlines),
			Graphic::RasterGPU(item) => add_unit_square_click_target(item.attribute_cloned_or_default(ATTR_TRANSFORM), outlines),
			Graphic::Color(_) => (),
			Graphic::Gradient(item) => add_gradient_item_outline_targets(ItemRef::Item(item), outlines),
			Graphic::Text(item) => add_text_item_click_targets(ItemRef::Item(item), outlines),
			Graphic::GraphicList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::VectorList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::RasterCPUList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::RasterGPUList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::ColorList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::GradientList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::TextList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
		}
	}

	fn contains_artboard(&self) -> bool {
		match self {
			Graphic::Graphic(item) => item.element().contains_artboard(),
			Graphic::GraphicList(list) => list.contains_artboard(),
			_ => false,
		}
	}

	fn new_ids_from_hash(&mut self, reference: Option<NodeId>) {
		match self {
			Graphic::Graphic(item) => {
				let layer = ItemRef::Item(item).layer();
				item.element_mut().new_ids_from_hash(layer);
			}
			Graphic::Vector(item) => item.element_mut().vector_new_ids_from_hash(reference.map(|id| id.0).unwrap_or_default()),
			Graphic::GraphicList(list) => list.new_ids_from_hash(reference),
			Graphic::VectorList(list) => list.new_ids_from_hash(reference),
			_ => (),
		}
	}
}

/// Reads the artboard metadata for the item at `index` from a `List<Artboard>`.
fn read_artboard_attributes(list: &List<Artboard>, index: usize) -> (DVec2, DVec2, Color, bool) {
	let location: DVec2 = list.attribute_cloned_or_default(ATTR_LOCATION, index);
	let dimensions: DVec2 = list.attribute_cloned_or_default(ATTR_DIMENSIONS, index);
	let background: Color = list.attribute_cloned_or_default(ATTR_BACKGROUND, index);
	let clip: bool = list.attribute_cloned_or_default(ATTR_CLIP, index);
	(location, dimensions, background, clip)
}

impl Render for List<Artboard> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for index in 0..self.len() {
			let Some(content) = self.element(index).map(Artboard::as_graphic_list) else { continue };
			let (location, dimensions, background, clip) = read_artboard_attributes(self, index);

			let x = location.x.min(location.x + dimensions.x);
			let y = location.y.min(location.y + dimensions.y);
			let width = dimensions.x.abs();
			let height = dimensions.y.abs();

			// Background
			render.leaf_tag("rect", |attributes| {
				attributes.push("fill", format!("#{}", SRGBA8::from(background).to_rgb_hex()));
				if background.a() < 1. {
					attributes.push("fill-opacity", ((background.a() * 1000.).round() / 1000.).to_string());
				}
				attributes.push("x", x.to_string());
				attributes.push("y", y.to_string());
				attributes.push("width", width.to_string());
				attributes.push("height", height.to_string());
			});

			// Artwork
			render.parent_tag(
				// SVG group tag
				"g",
				// Group tag attributes
				|attributes| {
					let matrix = format_transform_matrix(DAffine2::from_translation(location));
					if !matrix.is_empty() {
						attributes.push(ATTR_TRANSFORM, matrix);
					}

					if clip {
						let id = format!("artboard-{}", generate_uuid());
						let selector = format!("url(#{id})");

						write!(
							&mut attributes.0.svg_defs,
							r##"<clipPath id="{id}"><rect x="0" y="0" width="{}" height="{}" /></clipPath>"##,
							dimensions.x, dimensions.y,
						)
						.unwrap();
						attributes.push("clip-path", selector);
					}
				},
				// Artwork content
				|render| {
					let mut render_params = render_params.clone();
					render_params.artboard_background = Some(background);
					content.render_svg(render, &render_params);
				},
			);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		use vello::peniko;

		for index in 0..self.len() {
			let Some(content) = self.element(index).map(Artboard::as_graphic_list) else { continue };
			let (location, dimensions, background, clip) = read_artboard_attributes(self, index);

			let [a, b] = [location, location + dimensions];
			let rect = kurbo::Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));

			let artboard_transform = kurbo::Affine::new(transform.to_cols_array());

			let color = SRGBA8::from(background).to_peniko_color();
			scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., artboard_transform, &rect);
			scene.fill(peniko::Fill::NonZero, artboard_transform, color, None, &rect);
			scene.pop_layer();

			if clip {
				scene.push_clip_layer(peniko::Fill::NonZero, kurbo::Affine::new(transform.to_cols_array()), &rect);
			}

			// Since the content's transform is right multiplied in when rendering the content, we just need to right multiply by the artboard offset here.
			let child_transform = transform * DAffine2::from_translation(location);
			let mut render_params = render_params.clone();
			render_params.artboard_background = Some(background);
			content.render_to_vello(scene, child_transform, context, &render_params);
			if clip {
				scene.pop_layer();
			}
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, _element_id: Option<NodeId>, inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			let Some(content) = self.element(index).map(Artboard::as_graphic_list) else { continue };
			let (location, dimensions, _background, clip) = read_artboard_attributes(self, index);

			let layer_path: List<NodeId> = self.attribute_cloned_or_default::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH, index).0;
			let element_id = layer_path.iter_element_values().next_back().copied();

			if let Some(element_id) = element_id {
				metadata
					.click_targets
					.insert(element_id, vec![ClickTarget::new_with_path(rectangle_path(DVec2::ZERO, dimensions), 0.).into()]);
				metadata.upstream_footprints.insert(element_id, footprint);
				metadata.local_transforms.insert(element_id, DAffine2::from_translation(location));
				if clip {
					metadata.clip_targets.insert(element_id);
				}
			}

			metadata.backgrounds.push(Background { location, dimensions });

			let mut child_footprint = footprint;
			child_footprint.transform *= DAffine2::from_translation(location);
			content.collect_metadata(metadata, child_footprint, None, inherited_appearance);
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>, _inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			let dimensions: DVec2 = self.attribute_cloned_or_default(ATTR_DIMENSIONS, index);
			click_targets.push(ClickTarget::new_with_path(rectangle_path(DVec2::ZERO, dimensions), 0.));
		}
	}

	fn contains_artboard(&self) -> bool {
		!self.is_empty()
	}
}

impl Render for List<Graphic> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		let mut mask_state = None;

		for index in 0..self.len() {
			let next_clips = index + 1 < self.len() && self.element(index + 1).unwrap().had_clip_enabled();
			render_graphic_item_svg(ItemRef::ListItem(self, index), next_clips, &mut mask_state, render, render_params);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		let mut mask_element_and_transform = None;

		for index in 0..self.len() {
			let next_clips = index + 1 < self.len() && self.element(index + 1).unwrap().had_clip_enabled();
			render_graphic_item_to_vello(ItemRef::ListItem(self, index), next_clips, &mut mask_element_and_transform, scene, transform, context, render_params);
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>, inherited_appearance: Option<&Appearance>) {
		collect_graphic_items_metadata((0..self.len()).map(|index| ItemRef::ListItem(self, index)), metadata, footprint, element_id, inherited_appearance);
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			add_graphic_item_click_targets(ItemRef::ListItem(self, index), click_targets, inherited_appearance);
		}
	}

	fn add_upstream_outline_targets(&self, outlines: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			add_graphic_item_outline_targets(ItemRef::ListItem(self, index), outlines, inherited_appearance);
		}
	}

	fn contains_artboard(&self) -> bool {
		self.iter_element_values().any(|element| element.contains_artboard())
	}

	fn new_ids_from_hash(&mut self, _reference: Option<NodeId>) {
		let (elements, layers) = self.element_and_attribute_slices_mut::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH);
		for (element, layer) in elements.iter_mut().zip(layers.iter()) {
			element.new_ids_from_hash(layer.0.iter_element_values().next_back().copied());
		}
	}
}

/// Emits one vector shape as SVG, with no wrapping group of its own.
fn render_vector_shape_svg(item: ItemRef<'_, Vector>, vector: &Vector, render: &mut SvgRender, render_params: &RenderParams) {
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);

	// The item's own declared appearance wins over one cascading down from an ancestor
	let own_appearance = item.attribute::<Appearance>(ATTR_APPEARANCE).and_then(Appearance::declared);
	let appearance = own_appearance.or(render_params.inherited_appearance.as_ref());
	let FillAndStroke {
		stroke: stroke_params,
		fill_paint,
		stroke_paint,
		stroke_below: wants_stroke_below,
	} = appearance.map(Appearance::fill_and_stroke).unwrap_or_default();

	// Only consider strokes with non-zero weight, since default strokes with zero weight would prevent assigning the correct stroke transform
	let has_real_stroke = stroke_params.as_ref().filter(|stroke| stroke.weight() > 0.);
	// A cascaded coverage records its stroke space in the ancestor's coordinates, so this item authors its own
	let set_stroke_transform = has_real_stroke
		.map(|stroke| if own_appearance.is_some() { stroke.transform } else { item_transform })
		.filter(|transform| transform_is_invertible(*transform));
	let applied_stroke_transform = set_stroke_transform.unwrap_or(item_transform);
	let applied_stroke_transform = render_params.alignment_parent_transform.unwrap_or(applied_stroke_transform);
	let element_transform = set_stroke_transform.map(|stroke_transform| item_transform * stroke_transform.inverse());
	let element_transform = element_transform.unwrap_or(DAffine2::IDENTITY);
	let layer_bounds = vector.bounding_box().unwrap_or_default();
	let transformed_bounds = vector.bounding_box_with_transform(applied_stroke_transform).unwrap_or_default();
	let stroke_layer_bounds = vector.stroke_inclusive_bounding_box_with_transform(DAffine2::IDENTITY, stroke_params.as_ref()).unwrap_or(layer_bounds);

	let bounds_matrix = DAffine2::from_scale_angle_translation(layer_bounds[1] - layer_bounds[0], 0., layer_bounds[0]);
	let stroke_bounds_matrix = DAffine2::from_scale_angle_translation(stroke_layer_bounds[1] - stroke_layer_bounds[0], 0., stroke_layer_bounds[0]);

	let mut path = String::new();

	for mut bezpath in vector.stroke_bezpath_iter() {
		bezpath.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));
		path.push_str(bezpath.to_svg().as_str());
	}

	let mask_type = if stroke_params.as_ref().map(|stroke| stroke.align) == Some(StrokeAlign::Inside) {
		MaskType::Clip
	} else {
		MaskType::Mask
	};

	let path_is_closed = vector.stroke_bezpath_iter().all(|path| matches!(path.elements().last(), Some(PathEl::ClosePath)));
	let can_draw_aligned_stroke = path_is_closed
		&& stroke_params.as_ref().is_some_and(|stroke| stroke.has_renderable_stroke() && stroke.align.is_not_centered())
		&& stroke_paint.is_some_and(|graphic| !graphic.is_guaranteed_fully_transparent());
	let can_use_paint_order = !(fill_paint.is_none_or(|graphic| !graphic.is_guaranteed_to_cover_opaquely()) || mask_type == MaskType::Clip);

	let needs_separate_alignment_fill = can_draw_aligned_stroke && !can_use_paint_order;
	let override_paint_order = can_draw_aligned_stroke && can_use_paint_order;
	let use_face_fill = vector.use_face_fill();

	if needs_separate_alignment_fill && !wants_stroke_below {
		emit_svg_fill_path(
			render,
			path.clone(),
			fill_paint,
			item_transform,
			element_transform,
			applied_stroke_transform,
			bounds_matrix,
			render_params,
		);
	}

	let push_id = needs_separate_alignment_fill.then_some({
		let id = format!("alignment-{}", generate_uuid());

		let cloned_vector = vector.clone();

		// The mask must draw at full alpha so the SVG `<mask>`/`<clipPath>` fully zeroes the path interior.
		// The wrapping SVG group (above) handles the user-set opacity.
		let mut mask_item = Item::new_from_element(cloned_vector).with_attribute(ATTR_TRANSFORM, item_transform);
		let black_fill = Graphic::ColorList(List::new_from_element(Color::BLACK));
		mask_item.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), black_fill));
		let vector_item = List::new_from_item(mask_item);

		(id, mask_type, vector_item)
	});

	if use_face_fill {
		for mut face_path in vector.construct_faces().filter(|face| face.area() >= 0.) {
			face_path.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));
			let face_d = face_path.to_svg();

			emit_svg_fill_path(render, face_d, fill_paint, item_transform, element_transform, applied_stroke_transform, bounds_matrix, render_params);
		}
	}

	render.leaf_tag("path", |attributes| {
		attributes.push("d", path.clone());
		let matrix = format_transform_matrix(element_transform);
		if !matrix.is_empty() {
			attributes.push(ATTR_TRANSFORM, matrix);
		}

		let defs = &mut attributes.0.svg_defs;
		if let Some((ref id, mask_type, ref vector_item)) = push_id {
			let mut svg = SvgRender::new();
			vector_item.render_svg(&mut svg, &render_params.for_alignment(applied_stroke_transform));
			// `push_id` is only `Some` when `can_draw_aligned_stroke`, which is gated on `path_is_closed`
			let (largest_scale, _) = singular_values(applied_stroke_transform);
			let inflation = stroke_params.as_ref().map(|stroke| stroke.max_aabb_inflation(true)).unwrap_or_default() * largest_scale;
			let quad = Quad::from_box(transformed_bounds).inflate(inflation);
			let (x, y) = quad.top_left().into();
			let (width, height) = (quad.bottom_right() - quad.top_left()).into();

			write!(defs, r##"{}"##, svg.svg_defs).unwrap();
			let rect = format!(r##"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="white" />"##);

			match mask_type {
				MaskType::Clip => write!(defs, r##"<clipPath id="{id}">{}</clipPath>"##, svg.svg.to_svg_string()).unwrap(),
				MaskType::Mask => write!(
					defs,
					r##"<mask id="{id}" maskUnits="userSpaceOnUse" maskContentUnits="userSpaceOnUse" x="{x}" y="{y}" width="{width}" height="{height}">{}{}</mask>"##,
					rect,
					svg.svg.to_svg_string()
				)
				.unwrap(),
			}
		}

		let mut render_params = render_params.clone();
		render_params.aligned_strokes = can_draw_aligned_stroke;
		render_params.stroke_below = override_paint_order || wants_stroke_below;

		let stroke_shape_attribute = stroke_params
			.as_ref()
			.map(|stroke| {
				if stroke_paint.is_some() {
					stroke.render(defs, item_transform, element_transform, applied_stroke_transform, bounds_matrix, &render_params, PaintTarget::Stroke)
				} else {
					String::new()
				}
			})
			.unwrap_or_default();

		// Need to avoid generating only paint attribute, otherwise SVG uses 1px width stroke as a fallback
		let stroke_visible = stroke_params.as_ref().is_some_and(|stroke| stroke.has_renderable_stroke()) && stroke_paint.is_some_and(|g| !g.is_guaranteed_fully_transparent());
		let stroke_attribute = if stroke_visible {
			stroke_paint
				.map(|paint| {
					// Gradient should align with the fill path bbox so that a shared gradient lines up across fill and stroke.
					// Only clipping-based paints need the stroke-inclusive bbox.
					let paint_bounds = match paint {
						Graphic::Color(_) | Graphic::Gradient(_) | Graphic::ColorList(_) | Graphic::GradientList(_) => bounds_matrix,
						_ => stroke_bounds_matrix,
					};
					paint.render(defs, item_transform, element_transform, applied_stroke_transform, paint_bounds, &render_params, PaintTarget::Stroke)
				})
				.unwrap_or_else(|| r#" stroke="none""#.to_string())
		} else {
			String::new()
		};

		let fill_attribute = if needs_separate_alignment_fill || use_face_fill {
			r#" fill="none""#.to_string()
		} else {
			fill_paint
				.map(|paint| paint.render(defs, item_transform, element_transform, applied_stroke_transform, bounds_matrix, &render_params, PaintTarget::Fill))
				.unwrap_or_else(|| r#" fill="none""#.to_string())
		};

		if let Some((id, mask_type, _)) = push_id {
			let selector = format!("url(#{id})");
			attributes.push(mask_type.to_attribute(), selector);
		}
		attributes.push_val(fill_attribute);
		attributes.push_val(stroke_shape_attribute);
		attributes.push_val(stroke_attribute);

		if vector.is_branching() && !use_face_fill {
			attributes.push("fill-rule", "evenodd");
		}

		let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
		if opacity < 1. {
			attributes.push("opacity", opacity.to_string());
		}

		if blend_mode_attr != BlendMode::default() {
			attributes.push("style", blend_mode_attr.render());
		}
	});

	// When splitting passes and stroke is below, draw the fill after the stroke.
	if needs_separate_alignment_fill && wants_stroke_below {
		emit_svg_fill_path(
			render,
			path.clone(),
			fill_paint,
			item_transform,
			element_transform,
			applied_stroke_transform,
			bounds_matrix,
			render_params,
		);
	}
}

/// Emits one item of vector content as SVG, handling the sibling clipping run carried in `clip_mask_state`.
/// A lone item has no siblings, so both mask inputs stay inert.
fn render_vector_item_svg(item: ItemRef<'_, Vector>, next_clips: bool, clip_mask_state: &mut Option<(u64, MaskType)>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(vector) = item.element() else { return };

	let mut masked_by = None;

	if next_clips && clip_mask_state.is_none() {
		let masker = Graphic::VectorList(List::new_from_item(Item::from_parts(vector.clone(), item.clone_item_attributes())));
		let mask_type = if masker.can_reduce_to_clip_path() { MaskType::Clip } else { MaskType::Mask };
		let uuid = generate_uuid();

		let mut masker_svg = SvgRender::new();
		masker.render_svg(&mut masker_svg, &render_params.for_clipper());
		render.svg_defs.push_str(&masker_svg.svg_defs);
		mask_type.write_to_defs(&mut render.svg_defs, uuid, masker_svg.svg.to_svg_string());

		*clip_mask_state = Some((uuid, mask_type));
	} else if let Some((uuid, mask_type)) = *clip_mask_state {
		if !next_clips {
			*clip_mask_state = None;
		}

		masked_by = Some((mask_type.to_attribute(), format!("url(#mask-{uuid})")));
	}

	// Item geometry is baked into the path data instead of a group transform, so mask coordinates line up
	match masked_by {
		Some((attribute, selector)) => render.parent_tag(
			"g",
			|attributes| attributes.push(attribute, selector),
			|render| render_vector_shape_svg(item, vector, render, render_params),
		),
		None => render_vector_shape_svg(item, vector, render, render_params),
	}
}

/// Draws one item of vector content into the Vello scene: fill and stroke paints, blend and opacity layering,
/// stroke alignment compositing, and the sibling clipping run carried in `clip_masker` (inert for a lone item).
#[allow(clippy::too_many_arguments)]
fn render_vector_item_to_vello(
	item: ItemRef<'_, Vector>,
	next_clips: bool,
	clip_masker: &mut Option<List<Vector>>,
	scene: &mut Scene,
	parent_transform: DAffine2,
	context: &mut RenderContext,
	render_params: &RenderParams,
	paint_render_params: &RenderParams,
) {
	let Some(element) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	let multiplied_transform = parent_transform * item_transform;

	// The item's own declared appearance wins over one cascading down from an ancestor
	let own_appearance = item.attribute::<Appearance>(ATTR_APPEARANCE).and_then(Appearance::declared);
	let appearance = own_appearance.or(render_params.inherited_appearance.as_ref());
	let FillAndStroke {
		stroke: stroke_params,
		fill_paint,
		stroke_paint,
		stroke_below: wants_stroke_below,
	} = appearance.map(Appearance::fill_and_stroke).unwrap_or_default();

	let has_real_stroke = stroke_params.as_ref().filter(|stroke| stroke.weight() > 0.);
	// A cascaded coverage records its stroke space in the ancestor's coordinates, so this item authors its own
	let set_stroke_transform = has_real_stroke
		.map(|stroke| if own_appearance.is_some() { stroke.transform } else { item_transform })
		.filter(|transform| transform_is_invertible(*transform));
	let mut applied_stroke_transform = set_stroke_transform.unwrap_or(multiplied_transform);
	let mut element_transform = set_stroke_transform
		.map(|stroke_transform| multiplied_transform * stroke_transform.inverse())
		.unwrap_or(DAffine2::IDENTITY);
	if let Some(alignment_transform) = render_params.alignment_parent_transform {
		applied_stroke_transform = alignment_transform;
		element_transform = if transform_is_invertible(alignment_transform) {
			multiplied_transform * alignment_transform.inverse()
		} else {
			multiplied_transform
		};
	}
	let layer_bounds = element.bounding_box().unwrap_or_default();

	let mut path = kurbo::BezPath::new();
	for mut bezpath in element.stroke_bezpath_iter() {
		bezpath.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));
		for element in bezpath {
			path.push(element);
		}
	}

	// If we're using opacity or a blend mode, we need to push a layer
	let blend_mode = match render_params.render_mode {
		RenderMode::Outline => peniko::Mix::Normal,
		_ => blend_mode_attr.to_peniko(),
	};
	let mut layer = false;

	// Whether the renderer will engage the stroke-alignment compositing trick (non-Center align on a fully closed path).
	// Used by both the blend-layer clip rect inflation below (as `max_aabb_inflation`'s `path_is_closed` arg, equivalent here since
	// the function ignores the arg for Center align) and the `SrcIn`/`SrcOut` aligned-stroke branch further down.
	let stroke = stroke_params.as_ref();
	let stroke_fully_transparent = stroke_paint.is_none_or(|paint| paint.is_guaranteed_fully_transparent());
	let can_draw_aligned_stroke = !stroke_fully_transparent
		&& stroke.is_some_and(|s| s.has_renderable_stroke() && s.align.is_not_centered())
		&& element.stroke_bezpath_iter().all(|p| matches!(p.elements().last(), Some(PathEl::ClosePath)));

	let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
	let needs_blend_layer = opacity < 1. || blend_mode_attr != BlendMode::default();

	// Shared by the blend and clipping layers below, so it is only worth deriving when one of them is pushed
	let layer_geometry = (needs_blend_layer || clip_masker.is_some()).then(|| {
		// `max_aabb_inflation` is in `applied_stroke_transform`-space; `layer_bounds` is path-local and `push_layer` re-applies `multiplied_transform`.
		// Divide by the smaller axial scale to cover the stroke in both axes after Vello's transform. Skip on a degenerate transform.
		let (_, smallest_scale) = singular_values(applied_stroke_transform);
		let stroke_inflation = stroke.map_or(0., |s| s.max_aabb_inflation(can_draw_aligned_stroke));
		let inflate_amount = if smallest_scale > 0. { stroke_inflation / smallest_scale } else { 0. };
		let bounds = Quad::from_box(layer_bounds).inflate(inflate_amount).bounding_box();

		(
			kurbo::Affine::new(multiplied_transform.to_cols_array()),
			kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y),
		)
	});

	if needs_blend_layer && let Some((layer_affine, layer_rect)) = layer_geometry {
		layer = true;
		scene.push_layer(peniko::Fill::NonZero, peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver), opacity, layer_affine, &layer_rect);
	}

	// Pushed inside the blend layer so the mask cuts this item's own paint rather than the composited result
	let mut clip_layers = false;
	if next_clips && clip_masker.is_none() {
		*clip_masker = Some(List::new_from_item(Item::from_parts(element.clone(), item.clone_item_attributes())));
	} else if let Some(masker) = clip_masker.as_ref() {
		if let Some((layer_affine, layer_rect)) = layer_geometry {
			scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., layer_affine, &layer_rect);
			masker.render_to_vello(scene, parent_transform, context, &render_params.for_clipper());
			scene.push_layer(
				peniko::Fill::NonZero,
				peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcIn),
				1.,
				layer_affine,
				&layer_rect,
			);
			clip_layers = true;
		}

		if !next_clips {
			*clip_masker = None;
		}
	}

	let use_layer = can_draw_aligned_stroke;

	let do_fill_path = |scene: &mut Scene, context: &mut RenderContext, path: &kurbo::BezPath, fill_rule: peniko::Fill| {
		let Some(paint) = fill_paint else { return };

		let solid_fill = |scene: &mut Scene, color: Option<Color>| {
			let Some(color) = color else { return };

			let fill = peniko::Brush::Solid(SRGBA8::from(color).to_peniko_color());
			scene.fill(fill_rule, kurbo::Affine::new(element_transform.to_cols_array()), &fill, None, path);
		};
		let gradient_fill = |scene: &mut Scene, gradient_item: ItemRef<'_, Gradient>| {
			let Some((brush, gradient_to_device)) = create_peniko_gradient_brush(gradient_item, &multiplied_transform, render_params.for_mask) else {
				return;
			};

			let inverse_element_transform = if transform_is_invertible(element_transform) {
				element_transform.inverse()
			} else {
				Default::default()
			};
			let brush_transform = kurbo::Affine::new((inverse_element_transform * gradient_to_device).to_cols_array());
			scene.fill(fill_rule, kurbo::Affine::new(element_transform.to_cols_array()), &brush, Some(brush_transform), path);
		};

		match paint {
			Graphic::None(_) | Graphic::NoneList(_) => (),
			Graphic::Color(item) => solid_fill(scene, faded_paint_color(ItemRef::Item(item), render_params.for_mask)),
			Graphic::ColorList(list) => solid_fill(scene, composite_paint_colors(list, render_params.for_mask)),
			Graphic::Gradient(item) => gradient_fill(scene, ItemRef::Item(item)),
			// Stacked gradients cannot be composited into one brush, so they fall through to the clipped texture path
			Graphic::GradientList(list) if list.len() <= 1 => gradient_fill(scene, ItemRef::ListItem(list, 0)),
			// Any other graphic content paints as a texture clipped to the path
			Graphic::GradientList(_)
			| Graphic::Graphic(_)
			| Graphic::Vector(_)
			| Graphic::RasterCPU(_)
			| Graphic::RasterGPU(_)
			| Graphic::Text(_)
			| Graphic::VectorList(_)
			| Graphic::RasterCPUList(_)
			| Graphic::RasterGPUList(_)
			| Graphic::GraphicList(_)
			| Graphic::TextList(_) => {
				scene.push_clip_layer(fill_rule, kurbo::Affine::new(element_transform.to_cols_array()), path);
				paint.render_to_vello(scene, multiplied_transform, context, paint_render_params);
				scene.pop_layer();
			}
		};
	};

	let use_face_fill = element.use_face_fill();
	let do_fill = |scene: &mut Scene, context: &mut RenderContext| {
		if use_face_fill {
			for mut face_path in element.construct_faces().filter(|face| face.area() >= 0.) {
				face_path.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));
				let mut kurbo_path = kurbo::BezPath::new();
				for element in face_path {
					kurbo_path.push(element);
				}
				do_fill_path(scene, context, &kurbo_path, peniko::Fill::NonZero);
			}
		} else if element.is_branching() {
			do_fill_path(scene, context, &path, peniko::Fill::EvenOdd);
		} else {
			do_fill_path(scene, context, &path, peniko::Fill::NonZero);
		}
	};

	let do_stroke = |scene: &mut Scene, width_scale: f64, context: &mut RenderContext| {
		let Some(paint) = stroke_paint else { return };
		let Some(stroke) = stroke else { return };

		let cap = match stroke.cap {
			StrokeCap::Butt => Cap::Butt,
			StrokeCap::Round => Cap::Round,
			StrokeCap::Square => Cap::Square,
		};
		let join = match stroke.join {
			StrokeJoin::Miter => Join::Miter,
			StrokeJoin::Bevel => Join::Bevel,
			StrokeJoin::Round => Join::Round,
		};
		let dash_pattern = stroke.dash_lengths.iter().map(|l| l.max(0.)).collect();
		let stroke = kurbo::Stroke {
			width: stroke.weight * width_scale,
			miter_limit: stroke.join_miter_limit,
			join,
			start_cap: cap,
			end_cap: cap,
			dash_pattern,
			dash_offset: stroke.dash_offset,
		};

		if stroke.width <= 0. {
			return;
		};

		let solid_stroke = |scene: &mut Scene, color: Option<Color>| {
			let Some(color) = color else { return };

			let brush = peniko::Brush::Solid(SRGBA8::from(color).to_peniko_color());

			scene.stroke(&stroke, kurbo::Affine::new(element_transform.to_cols_array()), &brush, None, &path);
		};
		let gradient_stroke = |scene: &mut Scene, gradient_item: ItemRef<'_, Gradient>| {
			let Some((brush, gradient_to_device)) = create_peniko_gradient_brush(gradient_item, &multiplied_transform, render_params.for_mask) else {
				return;
			};
			let inverse_element_transform = if transform_is_invertible(element_transform) {
				element_transform.inverse()
			} else {
				Default::default()
			};
			let brush_transform = kurbo::Affine::new((inverse_element_transform * gradient_to_device).to_cols_array());

			scene.stroke(&stroke, kurbo::Affine::new(element_transform.to_cols_array()), &brush, Some(brush_transform), &path);
		};

		match paint {
			Graphic::None(_) | Graphic::NoneList(_) => (),
			Graphic::Color(item) => solid_stroke(scene, faded_paint_color(ItemRef::Item(item), render_params.for_mask)),
			Graphic::ColorList(list) => solid_stroke(scene, composite_paint_colors(list, render_params.for_mask)),
			Graphic::Gradient(item) => gradient_stroke(scene, ItemRef::Item(item)),
			// Stacked gradients cannot be composited into one brush, so they fall through to the clipped texture path
			Graphic::GradientList(list) if list.len() <= 1 => gradient_stroke(scene, ItemRef::ListItem(list, 0)),
			// Any other graphic content paints as a texture clipped to the stroked region
			Graphic::GradientList(_)
			| Graphic::Graphic(_)
			| Graphic::Vector(_)
			| Graphic::RasterCPU(_)
			| Graphic::RasterGPU(_)
			| Graphic::Text(_)
			| Graphic::VectorList(_)
			| Graphic::RasterCPUList(_)
			| Graphic::RasterGPUList(_)
			| Graphic::GraphicList(_)
			| Graphic::TextList(_) => {
				let stroked = peniko::kurbo::stroke(path.iter(), &stroke, &StrokeOpts::default(), 0.01);

				scene.push_clip_layer(peniko::Fill::NonZero, kurbo::Affine::new(element_transform.to_cols_array()), &stroked);
				paint.render_to_vello(scene, multiplied_transform, context, paint_render_params);
				scene.pop_layer();
			}
		};
	};

	// Render the path
	match render_params.render_mode {
		RenderMode::Outline => {
			let (outline_stroke, outline_color_peniko) = get_outline_styles(render_params);

			scene.stroke(&outline_stroke, kurbo::Affine::new(element_transform.to_cols_array()), outline_color_peniko, None, &path);
		}
		_ => {
			if use_layer {
				let cloned_element = element.clone();

				// The mask must draw at full alpha so `SrcOut` fully zeroes the path interior.
				// The outer opacity/blend layer (above) handles the user-set opacity.
				let mut mask_item = Item::new_from_element(cloned_element).with_attribute(ATTR_TRANSFORM, item_transform);
				let black_fill = Graphic::ColorList(List::new_from_element(Color::BLACK));
				mask_item.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), black_fill));
				let vector_list = List::new_from_item(mask_item);

				let bounds = element.bounding_box_with_transform(multiplied_transform).unwrap_or(layer_bounds);
				// This branch is gated on `can_draw_aligned_stroke`, which already requires every subpath is closed
				let inflation = stroke.map_or(0., |stroke| stroke.max_aabb_inflation(true));
				let (largest_scale, _) = singular_values(applied_stroke_transform);
				let quad = Quad::from_box(bounds).inflate(inflation * largest_scale);
				let bounds = quad.bounding_box();
				let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);

				let compose = if stroke.is_some_and(|x| x.align == StrokeAlign::Outside) {
					peniko::Compose::SrcOut
				} else {
					peniko::Compose::SrcIn
				};

				if wants_stroke_below {
					scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., kurbo::Affine::IDENTITY, &rect);
					vector_list.render_to_vello(scene, parent_transform, context, &render_params.for_alignment(applied_stroke_transform));
					scene.push_layer(peniko::Fill::NonZero, peniko::BlendMode::new(peniko::Mix::Normal, compose), 1., kurbo::Affine::IDENTITY, &rect);

					do_stroke(scene, 2., context);

					scene.pop_layer();
					scene.pop_layer();

					do_fill(scene, context);
				} else {
					// Fill first (unclipped), then stroke (clipped) above
					do_fill(scene, context);

					scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., kurbo::Affine::IDENTITY, &rect);
					vector_list.render_to_vello(scene, parent_transform, context, &render_params.for_alignment(applied_stroke_transform));
					scene.push_layer(peniko::Fill::NonZero, peniko::BlendMode::new(peniko::Mix::Normal, compose), 1., kurbo::Affine::IDENTITY, &rect);

					do_stroke(scene, 2., context);

					scene.pop_layer();
					scene.pop_layer();
				}
			} else {
				// Non-aligned strokes or open paths: default order behavior
				enum Op {
					Fill,
					Stroke,
				}

				let order = match wants_stroke_below {
					true => [Op::Stroke, Op::Fill],
					false => [Op::Fill, Op::Stroke], // Default
				};

				for operation in &order {
					match operation {
						Op::Fill => do_fill(scene, context),
						Op::Stroke => do_stroke(scene, 1., context),
					}
				}
			}
		}
	}

	if clip_layers {
		scene.pop_layer();
		scene.pop_layer();
	}

	// If we pushed a layer for opacity or a blend mode, we need to pop it
	if layer {
		scene.pop_layer();
	}
}

/// The full metadata pass over a run of vector items.
/// Aggregates all items' targets per element_id so multi-item lists (e.g. the "Text to Vector Glyphs" node) produce hit areas for every glyph.
/// Targets are baked relative to the first item carrying each element_id, since that is the transform recorded as its `local_transforms` entry.
fn collect_vector_items_metadata<'a>(
	items: impl Iterator<Item = ItemRef<'a, Vector>>,
	metadata: &mut RenderMetadata,
	footprint: Footprint,
	caller_element_id: Option<NodeId>,
	inherited_appearance: Option<&Appearance>,
) {
	let mut reference_transforms: HashMap<NodeId, DAffine2> = HashMap::new();

	let mut accumulated_click_targets: HashMap<NodeId, Vec<Arc<ClickTarget>>> = HashMap::new();
	let mut accumulated_outlines: HashMap<NodeId, Vec<Arc<ClickTarget>>> = HashMap::new();

	for item in items {
		let Some(source) = item.element() else { continue };
		let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
		// The item's own appearance wins over one cascading down from an ancestor
		let appearance = Appearance::cascade(item.attribute::<Appearance>(ATTR_APPEARANCE), inherited_appearance);

		if let Some(element_id) = caller_element_id.or(item.layer()) {
			let reference_transform = *reference_transforms.entry(element_id).or_insert(transform);
			let reference_inverse = if transform_is_invertible(reference_transform) {
				reference_transform.inverse()
			} else {
				DAffine2::IDENTITY
			};

			// Use click-target override if the item provides one (e.g. 'Text' node's per-glyph bboxes)
			let click_target_vector = item.attribute::<Vector>(ATTR_EDITOR_CLICK_TARGET).unwrap_or(source);

			let item_relative_transform = reference_inverse * transform;

			let mut click_targets_unwrapped = Vec::new();
			extend_targets_from_vector(&mut click_targets_unwrapped, appearance, click_target_vector, item_relative_transform);
			accumulated_click_targets.entry(element_id).or_default().extend(click_targets_unwrapped.into_iter().map(Arc::new));

			// Outlines always use source geometry so the visual outline reflects actual letterforms
			let mut outlines_unwrapped = Vec::new();
			extend_targets_from_vector(&mut outlines_unwrapped, appearance, source, item_relative_transform);
			accumulated_outlines.entry(element_id).or_default().extend(outlines_unwrapped.into_iter().map(Arc::new));

			// Source geometry (not the click-target override) so editing tools work on letterforms.
			// Recorded together with `vector_data` from the same (first) item so stroke geometry stays consistent with the paint.
			// Only item 0 is recorded since editing tools can only target a single item currently.
			// If that item has no paint attribute, none is recorded.
			if let std::collections::hash_map::Entry::Vacant(e) = metadata.vector_data.entry(element_id) {
				e.insert(Arc::new(source.clone()));

				if let Some(appearance) = appearance {
					metadata.appearance_attributes.insert(element_id, Arc::new(appearance.clone()));
				}
			}

			// Surface `editor:text_frame` for the Text tool's drag cage
			if let Some(&frame) = item.attribute::<DAffine2>(ATTR_EDITOR_TEXT_FRAME) {
				metadata.text_frames.entry(element_id).or_insert(frame);
			}
		}

		// If this item carries a snapshot of upstream graphic content (e.g. it was produced by Boolean Operation,
		// Combine Paths, Morph, or any other destructive merge), recurse into that snapshot so the editor can
		// surface the original child layers' click targets.
		let upstream_nested_layers = item.attribute_cloned_or_default::<List<Graphic>>(ATTR_EDITOR_MERGED_LAYERS);
		if !upstream_nested_layers.is_empty() {
			let mut upstream_footprint = footprint;
			upstream_footprint.transform *= transform;
			// Snapshot layers carry their own styling, so the merged result's appearance must not cascade into them
			upstream_nested_layers.collect_metadata(metadata, upstream_footprint, None, None);
		}
	}

	// Overwrite with the full accumulated set (not just item 0's contribution)
	for (element_id, targets) in accumulated_click_targets {
		metadata.click_targets.insert(element_id, targets);
	}
	for (element_id, targets) in accumulated_outlines {
		metadata.outlines.insert(element_id, targets);
	}

	// Recovering element_id from `editor:layer_path` means `Graphic::collect_metadata` skipped this transform metadata.
	// It lands after the snapshot recursion above so each element keeps the pair its targets were baked against.
	if caller_element_id.is_none() {
		for (element_id, reference_transform) in reference_transforms {
			metadata.upstream_footprints.insert(element_id, footprint);
			metadata.local_transforms.insert(element_id, reference_transform);
		}
	}
}

/// Collects one vector item's click target into the caller's list, baked through the item's transform.
fn add_vector_item_click_targets(item: ItemRef<'_, Vector>, click_targets: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
	let Some(source) = item.element() else { return };
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let appearance = Appearance::cascade(item.attribute::<Appearance>(ATTR_APPEARANCE), inherited_appearance);

	// Use click-target override geometry if the item provides one (e.g. 'Text' node's per-glyph bounding boxes)
	let vector = item.attribute::<Vector>(ATTR_EDITOR_CLICK_TARGET).unwrap_or(source);

	extend_targets_from_vector(click_targets, appearance, vector, transform);
}

/// Like [`add_vector_item_click_targets`] but on source geometry only, ignoring `editor:click_target`, so outlines reflect actual letterforms.
fn add_vector_item_outline_targets(item: ItemRef<'_, Vector>, outlines: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
	let Some(source) = item.element() else { return };
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let appearance = Appearance::cascade(item.attribute::<Appearance>(ATTR_APPEARANCE), inherited_appearance);

	extend_targets_from_vector(outlines, appearance, source, transform);
}

impl Render for List<Vector> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		let mut clip_mask_state: Option<(u64, MaskType)> = None;

		for index in 0..self.len() {
			// A clip-flagged item is masked by its nearest preceding unflagged sibling, which a consecutive run shares
			let next_clips = index + 1 < self.len() && self.attribute_cloned_or_default::<bool>(ATTR_CLIPPING_MASK, index + 1);
			render_vector_item_svg(ItemRef::ListItem(self, index), next_clips, &mut clip_mask_state, render, render_params);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, parent_transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		let mut clip_masker: Option<List<Vector>> = None;

		// A paint subtree supplies its own styling, so an element's appearance must not cascade into it
		let paint_render_params = RenderParams {
			inherited_appearance: None,
			..render_params.clone()
		};

		for index in 0..self.len() {
			let next_clips = index + 1 < self.len() && self.attribute_cloned_or_default::<bool>(ATTR_CLIPPING_MASK, index + 1);
			render_vector_item_to_vello(
				ItemRef::ListItem(self, index),
				next_clips,
				&mut clip_masker,
				scene,
				parent_transform,
				context,
				render_params,
				&paint_render_params,
			);
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, caller_element_id: Option<NodeId>, inherited_appearance: Option<&Appearance>) {
		collect_vector_items_metadata(
			(0..self.len()).map(|index| ItemRef::ListItem(self, index)),
			metadata,
			footprint,
			caller_element_id,
			inherited_appearance,
		);
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			add_vector_item_click_targets(ItemRef::ListItem(self, index), click_targets, inherited_appearance);
		}
	}

	fn add_upstream_outline_targets(&self, outlines: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			add_vector_item_outline_targets(ItemRef::ListItem(self, index), outlines, inherited_appearance);
		}
	}

	fn new_ids_from_hash(&mut self, reference: Option<NodeId>) {
		for vector in self.iter_element_values_mut() {
			vector.vector_new_ids_from_hash(reference.map(|id| id.0).unwrap_or_default());
		}
	}
}

/// Build one multi-contour `Path` (non-zero fill rule, so holes like the inside of an "O" work
/// correctly) plus one `FreePoint` per disconnected anchor, apply the transform, and append.
fn extend_targets_from_vector(targets: &mut Vec<ClickTarget>, appearance: Option<&Appearance>, geometry: &Vector, transform: DAffine2) {
	// A coverage whose paint is `Graphic::None` exists but paints nothing, so it does not close subpaths for hit testing
	let filled = appearance.is_some_and(|appearance| appearance.has_painted_cover(Cover::Fill));

	let mut bezpaths: Vec<BezPath> = geometry.stroke_bezpath_iter().filter(|bezpath| !bezpath.elements().is_empty()).collect();
	let all_contours_closed = bezpaths.iter().all(|bezpath| matches!(bezpath.elements().last(), Some(PathEl::ClosePath)));

	// Inside/Outside-aligned strokes reach `weight` from the centerline rather than `weight / 2` per side,
	// so they need double the click inflation. Alignment is only honored by the renderer for fully-closed paths.
	let stroke_width = appearance.and_then(|appearance| appearance.first_coverage_of(Cover::Stroke)).map_or(0., |coverage| {
		let stroke = coverage.stroke_params();
		if stroke.align.is_not_centered() && all_contours_closed {
			stroke.weight * 2.
		} else {
			stroke.weight
		}
	});

	if filled {
		for bezpath in &mut bezpaths {
			if !matches!(bezpath.elements().last(), Some(PathEl::ClosePath)) {
				bezpath.close_path();
			}
		}
	}

	if !bezpaths.is_empty() {
		let mut combined_path = BezPath::new();
		for bezpath in bezpaths {
			combined_path.extend(bezpath);
		}

		let mut click_target = ClickTarget::new_with_path(combined_path, stroke_width);
		click_target.apply_transform(transform);
		targets.push(click_target);
	}

	for click_target in extend_free_point_targets(geometry, transform) {
		targets.push(click_target);
	}
}

fn extend_free_point_targets(vector: &Vector, transform: DAffine2) -> impl Iterator<Item = ClickTarget> + '_ {
	// Mark every point index touched by a segment endpoint in one `O(points + segments)` pass, avoiding a per-point `any_connected` scan
	let mut connected = vec![false; vector.point_domain.len()];
	for &point_index in vector.segment_domain.start_point().iter().chain(vector.segment_domain.end_point()) {
		connected[point_index] = true;
	}

	vector.point_domain.ids().iter().enumerate().filter_map(move |(point_index, &point_id)| {
		if connected[point_index] {
			return None;
		}

		let anchor = vector.point_domain.position_from_id(point_id).unwrap_or_default();
		let mut click_target = ClickTarget::new_with_free_point(FreePoint::new(point_id, anchor));
		click_target.apply_transform(transform);
		Some(click_target)
	})
}

/// Emits one item of CPU raster content as SVG, as a canvas placeholder or an embedded base64 image.
fn render_raster_cpu_item_svg(item: ItemRef<'_, Raster<CPU>>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(image) = item.element() else { return };

	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);

	if image.data.is_empty() {
		return;
	}

	if render_params.to_canvas() {
		let mut image_copy = image.clone();
		image_copy.data_mut().map_pixels(|p| p.to_unassociated_alpha());
		let id = *render.image_data.entry(CacheHashWrapper(image_copy.into_data())).or_insert_with(generate_uuid);

		render.parent_tag(
			"foreignObject",
			|attributes| {
				let size = DVec2::new(image.width as f64, image.height as f64);

				let matrix = transform * DAffine2::from_scale(1. / size);
				let matrix = format_transform_matrix(matrix);
				if !matrix.is_empty() {
					attributes.push(ATTR_TRANSFORM, matrix);
				}

				attributes.push("width", size.x.to_string());
				attributes.push("height", size.y.to_string());

				let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}

				if blend_mode_attr != BlendMode::default() {
					attributes.push("style", blend_mode_attr.render());
				}
			},
			|render| {
				render.leaf_tag(
					"img", // Must be a self-closing (void element) tag, so we can't use `div` or `span`, for example
					|attributes| {
						attributes.push("data-canvas-placeholder", id.to_string());
					},
				)
			},
		);
	} else {
		let base64_string = image.base64_string.clone().unwrap_or_else(|| {
			use base64::Engine;

			let output = image.to_png();
			let preamble = "data:image/png;base64,";
			let mut base64_string = String::with_capacity(preamble.len() + output.len() * 4);
			base64_string.push_str(preamble);
			base64::engine::general_purpose::STANDARD.encode_string(output, &mut base64_string);
			base64_string
		});

		render.leaf_tag("image", |attributes| {
			attributes.push("width", "1");
			attributes.push("height", "1");
			attributes.push("preserveAspectRatio", "none");
			attributes.push("href", base64_string);
			let matrix = format_transform_matrix(transform);
			if !matrix.is_empty() {
				attributes.push(ATTR_TRANSFORM, matrix);
			}

			let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
			if opacity < 1. {
				attributes.push("opacity", opacity.to_string());
			}
			if blend_mode_attr != BlendMode::default() {
				attributes.push("style", blend_mode_attr.render());
			}
		});
	}
}

/// Draws one item of CPU raster content into the Vello scene.
fn render_raster_cpu_item_to_vello(item: ItemRef<'_, Raster<CPU>>, scene: &mut Scene, transform: DAffine2, render_params: &RenderParams) {
	let Some(image) = item.element() else { return };
	if image.data.is_empty() {
		return;
	}

	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	let blend_mode = blend_mode_attr.to_peniko();

	let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
	let mut layer = false;

	let whole_bounds = || match item {
		ItemRef::ListItem(list, _) => list.bounding_box(transform, false),
		ItemRef::Item(item) => item.bounding_box(transform, false),
	};
	if (opacity < 1. || (render_params.render_mode != RenderMode::Outline && blend_mode_attr != BlendMode::default()))
		&& let RenderBoundingBox::Rectangle(bounds) = whole_bounds()
	{
		let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
		let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);
		scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &rect);
		layer = true;
	}

	let transform_attribute: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);

	if let RenderMode::Outline = render_params.render_mode {
		let outline_transform: DAffine2 = transform * transform_attribute;
		draw_raster_outline(scene, &outline_transform, render_params);

		if layer {
			scene.pop_layer();
		}

		return;
	}

	let image_transform = transform * transform_attribute * DAffine2::from_scale(1. / DVec2::new(image.width as f64, image.height as f64));

	let image_brush = peniko::ImageBrush::new(peniko::ImageData {
		data: image.to_flat_u8().0.into(),
		format: peniko::ImageFormat::Rgba8,
		width: image.width,
		height: image.height,
		alpha_type: peniko::ImageAlphaType::Alpha,
	})
	.with_extend(peniko::Extend::Repeat);

	scene.draw_image(&image_brush, kurbo::Affine::new(image_transform.to_cols_array()));

	if layer {
		scene.pop_layer();
	}
}

/// The metadata a raster contributes under an `element_id`: a unit-square click target,
/// plus the first item's transform and any merged-layers snapshot when a first item exists.
fn collect_raster_metadata<T>(first_row: Option<ItemRef<'_, T>>, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
	let Some(element_id) = element_id else { return };
	metadata
		.click_targets
		.insert(element_id, vec![ClickTarget::new_with_path(rectangle_path(DVec2::ZERO, DVec2::ONE), 0.).into()]);
	metadata.upstream_footprints.insert(element_id, footprint);
	// TODO: Find a way to handle more than one item of the `List<Raster<...>>`
	if let Some(item) = first_row {
		let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
		metadata.local_transforms.insert(element_id, transform);

		// If this raster carries a snapshot of upstream graphic content (e.g. it was produced by Rasterize,
		// which destructively merges its inputs into pixels), recurse into that snapshot so the editor can
		// surface the original child layers' click targets (the same mechanism Boolean Operation uses).
		// The snapshot was captured before Rasterize shifted its input transforms to align with the rasterization
		// area, so the children are already in the coordinate space matching `footprint` here, meaning we must NOT
		// multiply in `transform` (which is the rasterization area, not a layer-stack transform).
		let upstream_nested_layers = item.attribute_cloned_or_default::<List<Graphic>>(ATTR_EDITOR_MERGED_LAYERS);
		if !upstream_nested_layers.is_empty() {
			upstream_nested_layers.collect_metadata(metadata, footprint, None, None);
		}
	}
}

/// Adds the unit-square click target every raster item presents, placed by the item's transform.
fn add_unit_square_click_target(transform: DAffine2, click_targets: &mut Vec<ClickTarget>) {
	// The unit square is the raster's own space, so its placement only exists in the item transform
	let mut path = rectangle_path(DVec2::ZERO, DVec2::ONE);
	path.apply_affine(Affine::new(transform.to_cols_array()));

	click_targets.push(ClickTarget::new_with_path(path, 0.));
}

impl Render for List<Raster<CPU>> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_raster_cpu_item_svg(ItemRef::ListItem(self, index), render, render_params);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, _: &mut RenderContext, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_raster_cpu_item_to_vello(ItemRef::ListItem(self, index), scene, transform, render_params);
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>, _inherited_appearance: Option<&Appearance>) {
		collect_raster_metadata((!self.is_empty()).then_some(ItemRef::ListItem(self, 0)), metadata, footprint, element_id);
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>, _inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			add_unit_square_click_target(self.attribute_cloned_or_default(ATTR_TRANSFORM, index), click_targets);
		}
	}
}

static LAZY_ARC_VEC_ZERO_U8: LazyLock<Arc<Vec<u8>>> = LazyLock::new(|| Arc::new(Vec::new()));

impl Render for List<Raster<GPU>> {
	fn render_svg(&self, _render: &mut SvgRender, _render_params: &RenderParams) {
		log::warn!("tried to render texture as an svg");
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_raster_gpu_item_to_vello(ItemRef::ListItem(self, index), scene, transform, context, render_params);
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>, _inherited_appearance: Option<&Appearance>) {
		collect_raster_metadata((!self.is_empty()).then_some(ItemRef::ListItem(self, 0)), metadata, footprint, element_id);
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>, _inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			add_unit_square_click_target(self.attribute_cloned_or_default(ATTR_TRANSFORM, index), click_targets);
		}
	}
}

/// Draws one item of GPU raster content into the Vello scene as a placeholder image, registering the texture override.
fn render_raster_gpu_item_to_vello(item: ItemRef<'_, Raster<GPU>>, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
	let Some(raster) = item.element() else { return };
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	let clip_attr: bool = item.attribute_cloned_or_default(ATTR_CLIPPING_MASK);
	let blend_mode = match render_params.render_mode {
		RenderMode::Outline => peniko::Mix::Normal,
		_ => blend_mode_attr.to_peniko(),
	};

	let mut layer = false;

	let whole_bounds = || match item {
		ItemRef::ListItem(list, _) => list.bounding_box(transform, true),
		ItemRef::Item(item) => item.bounding_box(transform, true),
	};
	let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
	let any_nondefault = blend_mode_attr != BlendMode::default() || opacity < 1. || clip_attr;
	if (render_params.render_mode != RenderMode::Outline && any_nondefault)
		&& let RenderBoundingBox::Rectangle(bounds) = whole_bounds()
	{
		let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
		let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);
		scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &rect);
		layer = true;
	}

	let transform_attribute: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);

	if let RenderMode::Outline = render_params.render_mode {
		let outline_transform = transform * transform_attribute;
		draw_raster_outline(scene, &outline_transform, render_params);

		if layer {
			scene.pop_layer();
		}

		return;
	}

	let width = raster.data().width();
	let height = raster.data().height();
	let image = peniko::ImageBrush::new(peniko::ImageData {
		data: peniko::Blob::new(LAZY_ARC_VEC_ZERO_U8.deref().clone()),
		format: peniko::ImageFormat::Rgba8,
		width,
		height,
		alpha_type: peniko::ImageAlphaType::Alpha,
	})
	.with_extend(peniko::Extend::Repeat);
	let image_transform = transform * transform_attribute * DAffine2::from_scale(1. / DVec2::new(width as f64, height as f64));
	scene.draw_image(&image, kurbo::Affine::new(image_transform.to_cols_array()));
	context.resource_overrides.push((image, raster.texture.clone()));

	if layer {
		scene.pop_layer()
	}
}

// Since colors and gradients are technically infinitely big, we have to implement
// workarounds for rendering them correctly in a way which still allows us
// to cache the intermediate render data (SVG string/Vello scene).
// For SVG, this is is achived by creating a truly giant rectangle.
// For Vello, we create a layer with a placeholder transform which we
// later replace with the current viewport transform before each render.
impl Render for List<Color> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_color_item_svg(ItemRef::ListItem(self, index), render, render_params);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, _parent_transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_color_item_to_vello(ItemRef::ListItem(self, index), scene, render_params);
		}
	}
}

/// Emits one item of color content as SVG, painting a stand-in for an infinite background.
fn render_color_item_svg(item: ItemRef<'_, Color>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(color) = item.element() else { return };
	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	render.leaf_tag("polyline", |attributes| {
		// Stand-in for an infinite background. Chrome's SVG renderer keeps internal coordinates in f32 and loses
		// precision past ~2^24 (~16.7 million), causing tile-boundary artifacts that pop in and out during panning.
		// 1e7 stays under that limit while still being far larger than any practical document extent.
		const MAX: f64 = 1e7;
		attributes.push("points", format!("{MAX},{MAX} -{MAX},{MAX} -{MAX},-{MAX} {MAX},-{MAX}"));

		attributes.push("fill", format!("#{}", SRGBA8::from(*color).to_rgb_hex()));
		if color.a() < 1. {
			attributes.push("fill-opacity", ((color.a() * 1000.).round() / 1000.).to_string());
		}

		let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
		if opacity < 1. {
			attributes.push("opacity", opacity.to_string());
		}

		if blend_mode != BlendMode::default() {
			attributes.push("style", blend_mode.render());
		}
	});
}

/// Draws one item of color content into the Vello scene under the viewport-replaced infinite transform.
fn render_color_item_to_vello(item: ItemRef<'_, Color>, scene: &mut Scene, render_params: &RenderParams) {
	use vello::peniko;

	let Some(color) = item.element() else { return };
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	let blend_mode = blend_mode_attr.to_peniko();
	let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;

	let vello_color = SRGBA8::from(*color).to_peniko_color();

	let rect = kurbo::Rect::from_origin_size(kurbo::Point::ZERO, kurbo::Size::new(1., 1.));

	let mut layer = false;
	if opacity < 1. || blend_mode_attr != BlendMode::default() {
		let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
		scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::scale(f64::INFINITY), &rect);
		layer = true;
	}

	scene.fill(peniko::Fill::NonZero, kurbo::Affine::scale(f64::INFINITY), vello_color, None, &rect);

	if layer {
		scene.pop_layer();
	}
}

/// The closed rectangular path spanning the two opposite corners, used for the box-shaped click targets.
fn rectangle_path(corner1: DVec2, corner2: DVec2) -> BezPath {
	kurbo::Rect::from_points(dvec2_to_point(corner1), dvec2_to_point(corner2)).to_path(kurbo::DEFAULT_ACCURACY)
}

/// A gradient's control geometry in its local space: the unit circle a radial gradient's transform carries to its drawn ellipse, or the (0,0) to (1,0) gradient line for a linear one.
fn gradient_control_outline(gradient_form: GradientForm) -> BezPath {
	match gradient_form {
		GradientForm::Linear => BezPath::from_path_segments(std::iter::once(kurbo::PathSeg::Line(kurbo::Line::new(dvec2_to_point(DVec2::ZERO), dvec2_to_point(DVec2::X))))),
		GradientForm::Radial => {
			// Four-cubic kappa circle with anchors on the axes, so the tight bounding box is exactly the unit square
			// <https://en.wikipedia.org/wiki/Composite_B%C3%A9zier_curve#Using_four_curves>
			const KAPPA: f64 = 4. / 3. * (std::f64::consts::SQRT_2 - 1.);
			let mut path = BezPath::new();
			path.move_to((1., 0.));
			path.curve_to((1., KAPPA), (KAPPA, 1.), (0., 1.));
			path.curve_to((-KAPPA, 1.), (-1., KAPPA), (-1., 0.));
			path.curve_to((-1., -KAPPA), (-KAPPA, -1.), (0., -1.));
			path.curve_to((KAPPA, -1.), (1., -KAPPA), (1., 0.));
			path.close_path();
			path
		}
	}
}

/// Whether the control geometry's interior is a draggable click area: a radial's main ellipse acts as the layer's handle regardless of spread, while a linear's control line has no interior.
fn gradient_control_interior_is_clickable(gradient_form: GradientForm) -> bool {
	gradient_form == GradientForm::Radial
}

/// For thumbnails the gradient fills a finite rect at the footprint's document space bounds, with a 1-unit margin to cover the `as u32` truncation of `Footprint::resolution`.
/// The viewBox crops the overshoot. Canvas rendering keeps the polyline path since Chrome rejects rects larger than ~20 million.
fn gradient_thumbnail_rect(render_params: &RenderParams) -> Option<(DVec2, DVec2)> {
	if render_params.thumbnail {
		let truncated_size = render_params.footprint.resolution.as_dvec2();
		let margin = DVec2::ONE;
		Some((render_params.footprint.transform.translation - margin / 2., truncated_size + margin))
	} else {
		None
	}
}

/// Emits one item of gradient content as SVG.
fn render_gradient_item_svg(item: ItemRef<'_, Gradient>, render: &mut SvgRender, render_params: &RenderParams) {
	render_gradient_item_svg_with_thumbnail_rect(item, gradient_thumbnail_rect(render_params), render, render_params);
}

impl Render for List<Gradient> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		let thumbnail_rect = gradient_thumbnail_rect(render_params);

		for index in 0..self.len() {
			render_gradient_item_svg_with_thumbnail_rect(ItemRef::ListItem(self, index), thumbnail_rect, render, render_params);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, parent_transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_gradient_item_to_vello(ItemRef::ListItem(self, index), scene, parent_transform, render_params);
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, _footprint: Footprint, element_id: Option<NodeId>, _inherited_appearance: Option<&Appearance>) {
		collect_gradient_items_metadata((0..self.len()).map(|index| ItemRef::ListItem(self, index)), metadata, element_id);
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>, _inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			add_gradient_item_click_targets(ItemRef::ListItem(self, index), click_targets);
		}
	}

	fn add_upstream_outline_targets(&self, outlines: &mut Vec<ClickTarget>, _inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			add_gradient_item_outline_targets(ItemRef::ListItem(self, index), outlines);
		}
	}
}

/// Emits one item of gradient content as SVG, painting the thumbnail rect or an infinite-background stand-in.
fn render_gradient_item_svg_with_thumbnail_rect(item: ItemRef<'_, Gradient>, thumbnail_rect: Option<(DVec2, DVec2)>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(gradient) = item.element() else { return };
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
	let settings = gradient_settings_from_item(item);
	let tag = if thumbnail_rect.is_some() { "rect" } else { "polyline" };
	render.leaf_tag(tag, |attributes| {
		if let Some((min, size)) = thumbnail_rect {
			attributes.push("x", min.x.to_string());
			attributes.push("y", min.y.to_string());
			attributes.push("width", size.x.to_string());
			attributes.push("height", size.y.to_string());
		} else {
			// Stand-in for an infinite background. Chrome's SVG renderer keeps internal coordinates in f32 and loses
			// precision past ~2^24 (~16.7 million), causing tile-boundary artifacts that pop in and out during panning.
			// 1e7 stays under that limit while still being far larger than any practical document extent.
			const MAX: f64 = 1e7;
			attributes.push("points", format!("{MAX},{MAX} -{MAX},{MAX} -{MAX},-{MAX} {MAX},-{MAX}"));
		}

		let (samples, _) = spread_adjusted_samples(gradient, settings, gradient_form, ClearGuardPlacement::SvgStopOrder);

		let mut stop_string = String::new();
		for (position, color, original_midpoint) in samples {
			let _ = write!(stop_string, r##"<stop offset="{}" stop-color="#{}""##, position, SRGBA8::from(color).to_rgb_hex());
			if color.a() < 1. {
				let _ = write!(stop_string, r#" stop-opacity="{}""#, color.a());
			}
			if let Some(midpoint) = original_midpoint {
				let _ = write!(stop_string, r#" graphite:midpoint="{}""#, (midpoint * 1000.).round() / 1000.);
			}
			stop_string.push_str(" />");
		}

		// render_thumbnail already added the footprint transform
		let gradient_transform = if render_params.thumbnail { transform } else { render_params.footprint.transform * transform };
		let gradient_transform_matrix = format_transform_matrix(gradient_transform);
		let gradient_transform_attribute = if gradient_transform_matrix.is_empty() {
			String::new()
		} else {
			format!(r#" gradientTransform="{gradient_transform_matrix}""#)
		};

		let gradient_id = generate_uuid();
		let gradient_spread_attribute = if matches!(settings.spread, GradientSpread::Pad | GradientSpread::Clear) {
			String::new()
		} else {
			format!(r#" spreadMethod="{}""#, settings.spread.svg_name())
		};

		// The unit gradient line is the +X unit vector in local space, before the item's transform is applied
		match gradient_form {
			GradientForm::Linear => {
				let _ = write!(
					&mut attributes.0.svg_defs,
					r#"<linearGradient id="{gradient_id}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="1" y2="0"{gradient_spread_attribute}{gradient_transform_attribute}>{stop_string}</linearGradient>"#
				);
			}
			GradientForm::Radial => {
				let _ = write!(
					&mut attributes.0.svg_defs,
					r#"<radialGradient id="{gradient_id}" gradientUnits="userSpaceOnUse" cx="0" cy="0" r="1"{gradient_spread_attribute}{gradient_transform_attribute}>{stop_string}</radialGradient>"#
				);
			}
		}

		attributes.push("fill", format!("url('#{gradient_id}')"));

		let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
		if opacity < 1. {
			attributes.push("opacity", opacity.to_string());
		}

		if blend_mode != BlendMode::default() {
			attributes.push("style", blend_mode.render());
		}
	});
}

/// Draws one item of gradient content into the Vello scene under the viewport-replaced infinite transform.
fn render_gradient_item_to_vello(item: ItemRef<'_, Gradient>, scene: &mut Scene, parent_transform: DAffine2, render_params: &RenderParams) {
	use vello::peniko;

	if let RenderMode::Outline = render_params.render_mode {
		return;
	}

	{
		let Some(gradient) = item.element() else { return };
		let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
		let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
		let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
		let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
		let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
		let gradient_transform = parent_transform * transform;

		let blend_mode = blend_mode_attr.to_peniko();
		let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;

		let settings = gradient_settings_from_item(item);
		let (samples, span) = spread_adjusted_samples(gradient, settings, gradient_form, ClearGuardPlacement::VelloRampTexels);

		let stops = peniko_color_stops(&samples);

		let extend = peniko_extend(settings.spread);

		// The unit gradient line is the +X unit vector in local space, before the item's transform is applied.
		// For radial, the unit-radius circle at the origin scales out to the line's length once the brush transform applies.
		let kind = match gradient_form {
			GradientForm::Linear => peniko::LinearGradientPosition {
				start: to_point(DVec2::X * span.0),
				end: to_point(DVec2::X * span.1),
			}
			.into(),
			GradientForm::Radial => peniko::RadialGradientPosition {
				start_center: to_point(DVec2::ZERO),
				start_radius: 0.,
				end_center: to_point(DVec2::ZERO),
				end_radius: span.1 as f32,
			}
			.into(),
		};

		let fill = peniko::Brush::Gradient(peniko::Gradient {
			kind,
			stops,
			extend,
			interpolation_alpha_space: peniko::InterpolationAlphaSpace::Unpremultiplied,
			..Default::default()
		});
		let brush_transform = kurbo::Affine::new(gradient_placement(gradient_transform, gradient_form).to_cols_array());
		let rect = kurbo::Rect::from_origin_size(kurbo::Point::ZERO, kurbo::Size::new(1., 1.));

		let mut layer = false;
		if opacity < 1. || blend_mode_attr != BlendMode::default() {
			let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
			// See implementation in `List<Color>` for more detail
			scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::scale(f64::INFINITY), &rect);
			layer = true;
		}

		// Encode shape and brush manually instead of Scene.fill(), which would multiply brush_transform by the path transform
		scene.encoding_mut().encode_transform(vello_encoding::Transform::from_kurbo(&kurbo::Affine::scale(f64::INFINITY)));
		scene.encoding_mut().encode_fill_style(peniko::Fill::NonZero);
		scene.encoding_mut().encode_shape(&rect, true);

		scene.encoding_mut().encode_transform(vello_encoding::Transform::from_kurbo(&brush_transform));
		scene.encoding_mut().swap_last_path_tags();
		scene.encoding_mut().encode_brush(&fill, 1.);

		if layer {
			scene.pop_layer();
		}
	}
}

/// The metadata pass over a run of gradient items: each contributes its control geometry as targets under the
/// run's `element_id`, baked relative to the first item's transform (recorded as its `local_transforms` entry).
fn collect_gradient_items_metadata<'a>(items: impl Iterator<Item = ItemRef<'a, Gradient>>, metadata: &mut RenderMetadata, element_id: Option<NodeId>) {
	let Some(element_id) = element_id else { return };

	let mut item_zero_inverse = None;
	let mut outline_targets = Vec::new();
	let mut click_targets = Vec::new();
	for item in items {
		let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
		let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);

		// The first item's transform is the reference all targets bake against
		let item_zero_inverse = *item_zero_inverse.get_or_insert_with(|| if transform_is_invertible(item_transform) { item_transform.inverse() } else { DAffine2::IDENTITY });

		let mut target = ClickTarget::new_with_path(gradient_control_outline(gradient_form), 0.);
		target.apply_transform(item_zero_inverse * item_transform);
		let target = Arc::new(target);

		if gradient_control_interior_is_clickable(gradient_form) {
			click_targets.push(target.clone());
		}
		outline_targets.push(target);
	}

	if outline_targets.is_empty() {
		return;
	}

	metadata.outlines.insert(element_id, outline_targets);
	if !click_targets.is_empty() {
		metadata.click_targets.insert(element_id, click_targets);
	}
}

/// Collects one gradient item's control geometry as a click target when its interior is draggable.
fn add_gradient_item_click_targets(item: ItemRef<'_, Gradient>, click_targets: &mut Vec<ClickTarget>) {
	let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
	if !gradient_control_interior_is_clickable(gradient_form) {
		return;
	}

	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let mut target = ClickTarget::new_with_path(gradient_control_outline(gradient_form), 0.);
	target.apply_transform(transform);
	click_targets.push(target);
}

/// Collects one gradient item's control geometry as an outline target.
fn add_gradient_item_outline_targets(item: ItemRef<'_, Gradient>, outlines: &mut Vec<ClickTarget>) {
	let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);

	let mut target = ClickTarget::new_with_path(gradient_control_outline(gradient_form), 0.);
	target.apply_transform(transform);
	outlines.push(target);
}

/// Builds a `kurbo::BezPath` from a glyph outline, baking in the glyph origin (`ox`, `oy`) and faux-italic shear (`tilt_tan`).
struct GlyphOutlinePen<'a> {
	path: &'a mut BezPath,
	ox: f64,
	oy: f64,
	tilt_tan: f64,
}

impl GlyphOutlinePen<'_> {
	#[inline]
	fn px(&self, x: f32, y: f32) -> f64 {
		self.ox + x as f64 + (y as f64 * self.tilt_tan)
	}

	#[inline]
	fn py(&self, y: f32) -> f64 {
		self.oy - y as f64
	}
}

impl OutlinePen for GlyphOutlinePen<'_> {
	fn move_to(&mut self, x: f32, y: f32) {
		self.path.move_to((self.px(x, y), self.py(y)));
	}
	fn line_to(&mut self, x: f32, y: f32) {
		self.path.line_to((self.px(x, y), self.py(y)));
	}
	fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
		self.path.quad_to((self.px(cx, cy), self.py(cy)), (self.px(x, y), self.py(y)));
	}
	fn curve_to(&mut self, cx1: f32, cy1: f32, cx2: f32, cy2: f32, x: f32, y: f32) {
		self.path.curve_to((self.px(cx1, cy1), self.py(cy1)), (self.px(cx2, cy2), self.py(cy2)), (self.px(x, y), self.py(y)));
	}
	fn close(&mut self) {
		self.path.close_path();
	}
}

/// Draws each glyph of `glyph_run` into a `BezPath` (with the run's position and faux-italic `tilt_tan` baked in)
/// and calls `emit` for each non-empty glyph. Zero-geometry glyphs advance by `space_extra` for justified spacing.
fn draw_glyph_run_to_bezpaths(glyph_run: &parley::GlyphRun<'_, ()>, x_offset: f32, space_extra: f32, tilt_tan: f64, mut emit: impl FnMut(&BezPath)) {
	let mut run_x = glyph_run.offset() + x_offset;
	let run_y = glyph_run.baseline();
	let run = glyph_run.run();
	let font = run.font();
	let font_size_pts = run.font_size();
	let normalized_coords: Vec<NormalizedCoord> = run.normalized_coords().iter().map(|c| NormalizedCoord::from_bits(*c)).collect();

	let Ok(font_ref) = SkrifaFontRef::from_index(font.data.as_ref(), font.index) else { return };
	let outlines = font_ref.outline_glyphs();

	let mut bez_path = BezPath::new();
	for glyph in glyph_run.glyphs() {
		let ox = (run_x + glyph.x) as f64;
		let oy = (run_y - glyph.y) as f64;
		run_x += glyph.advance;

		let Some(outline) = outlines.get(GlyphId::from(glyph.id)) else { continue };
		let settings = DrawSettings::unhinted(Size::new(font_size_pts), LocationRef::new(&normalized_coords));

		bez_path.truncate(0);
		let path = &mut bez_path;
		let mut pen = GlyphOutlinePen { path, ox, oy, tilt_tan };
		if outline.draw(settings, &mut pen).is_ok() && !bez_path.elements().is_empty() {
			emit(&bez_path);
		} else if space_extra != 0. && glyph.advance > 0. {
			run_x += space_extra;
		}
	}
}

/// Lays out one text item and returns its local size and transform. The `BoundingBox` trait can't do
/// this since a bare `String` carries no typography, so click-target and bounding-box computation share this. Falls back to an em
/// square if the font isn't registered yet.
fn text_item_size_and_transform(item: ItemRef<'_, String>) -> Option<(DVec2, DAffine2)> {
	let text = item.element()?;
	let font: Resource = {
		let f: Resource = item.attribute_cloned_or_default(ATTR_FONT);
		if f.is_empty() { text_nodes::FALLBACK_FONT_RESOURCE.clone() } else { f }
	};
	let font_size: f64 = item.attribute_cloned_or(ATTR_FONT_SIZE, DEFAULT_FONT_SIZE);
	let line_height: f64 = item.attribute_cloned_or(ATTR_LINE_HEIGHT, 1.2);
	let letter_spacing: f64 = item.attribute_cloned_or(ATTR_LETTER_SPACING, 0.);
	let max_width: Option<f64> = item.attribute_cloned_or(ATTR_MAX_WIDTH, None);
	let max_height: Option<f64> = item.attribute_cloned_or(ATTR_MAX_HEIGHT, None);
	let align: text_nodes::TextAlign = item.attribute_cloned_or_default(ATTR_TEXT_ALIGN);
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);

	let typesetting = text_nodes::TypesettingConfig {
		font_size,
		line_height_ratio: line_height,
		letter_spacing,
		letter_tilt: 0.,
		max_width,
		max_height,
		align,
	};

	let (width, height) = text_nodes::TextContext::with_thread_local(|ctx| {
		ctx.layout_text(text, &font, typesetting).map(|layout| {
			let w = max_width.unwrap_or_else(|| layout.width() as f64);
			let h = max_height.unwrap_or_else(|| layout.height() as f64);
			(w, h)
		})
	})
	.unwrap_or((font_size, font_size));

	Some((DVec2::new(width, height), transform))
}

/// Union bounding box of a styled `List<String>`, laid out per item. The `BoundingBox` trait returns `None` for `List<String>`
/// (a bare `String` has no extent), so text-layer thumbnails and bounds use this instead. Each item is laid out under `outer_transform`.
pub fn text_list_bounding_box(list: &List<String>, outer_transform: DAffine2) -> RenderBoundingBox {
	let mut bounds: Option<[DVec2; 2]> = None;
	for index in 0..list.len() {
		accumulate_text_item_bounds(ItemRef::ListItem(list, index), outer_transform, &mut bounds);
	}
	match bounds {
		Some(bounds) => RenderBoundingBox::Rectangle(bounds),
		None => RenderBoundingBox::None,
	}
}

/// Folds one laid-out text item's corner points into the running bounds.
fn accumulate_text_item_bounds(item: ItemRef<'_, String>, outer_transform: DAffine2, bounds: &mut Option<[DVec2; 2]>) {
	let Some((size, transform)) = text_item_size_and_transform(item) else { return };
	let full_transform = outer_transform * transform;
	for corner in [DVec2::ZERO, DVec2::new(size.x, 0.), DVec2::new(0., size.y), size] {
		let point = full_transform.transform_point2(corner);
		*bounds = Some(match *bounds {
			Some([min, max]) => [min.min(point), max.max(point)],
			None => [point, point],
		});
	}
}

/// Like `List<Graphic>::thumbnail_bounding_box`, but lays out `Graphic::TextList` items, which the `BoundingBox` trait reports as `None`.
/// Used for layer thumbnails so text layers (whose content is a `List<Graphic>` wrapping the text) frame their content.
pub fn graphic_list_bounding_box(list: &List<Graphic>, transform: DAffine2) -> RenderBoundingBox {
	let mut combined: Option<[DVec2; 2]> = None;
	let mut any_infinite = false;

	for index in 0..list.len() {
		let item_transform = transform * list.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, index);
		let Some(graphic) = list.element(index) else { continue };
		match graphic_thumbnail_bounding_box(graphic, item_transform) {
			RenderBoundingBox::None => {}
			RenderBoundingBox::Infinite => any_infinite = true,
			RenderBoundingBox::Rectangle([min, max]) => {
				combined = Some(match combined {
					Some([existing_min, existing_max]) => [existing_min.min(min), existing_max.max(max)],
					None => [min, max],
				})
			}
		}
	}

	match (combined, any_infinite) {
		(Some(bounds), _) => RenderBoundingBox::Rectangle(bounds),
		(None, true) => RenderBoundingBox::Infinite,
		(None, false) => RenderBoundingBox::None,
	}
}

/// One graphic's thumbnail bounds, laying out text (which the `BoundingBox` trait reports as `None`) and recursing into groups.
fn graphic_thumbnail_bounding_box(graphic: &Graphic, item_transform: DAffine2) -> RenderBoundingBox {
	match graphic {
		Graphic::Text(item) => {
			let mut bounds = None;
			accumulate_text_item_bounds(ItemRef::Item(item), item_transform, &mut bounds);
			match bounds {
				Some(bounds) => RenderBoundingBox::Rectangle(bounds),
				None => RenderBoundingBox::None,
			}
		}
		Graphic::TextList(text_list) => text_list_bounding_box(text_list, item_transform),
		// A lone graphic recurses like a one-item group, composing its envelope transform
		Graphic::Graphic(item) => {
			let inner_transform = item_transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
			graphic_thumbnail_bounding_box(item.element(), inner_transform)
		}
		Graphic::GraphicList(sub_list) => graphic_list_bounding_box(sub_list, item_transform),
		other => other.thumbnail_bounding_box(item_transform, true),
	}
}

/// Emits one item of text content as SVG, laying out its glyphs and wrapping them in a styled group.
fn render_text_item_svg(item: ItemRef<'_, String>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(text) = item.element() else { return };
	if text.is_empty() {
		return;
	}

	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let font: Resource = {
		let f: Resource = item.attribute_cloned_or_default(ATTR_FONT);
		if f.is_empty() { text_nodes::FALLBACK_FONT_RESOURCE.clone() } else { f }
	};
	let font_size: f64 = item.attribute_cloned_or(ATTR_FONT_SIZE, DEFAULT_FONT_SIZE);
	let line_height: f64 = item.attribute_cloned_or(ATTR_LINE_HEIGHT, 1.2);
	let letter_spacing: f64 = item.attribute_cloned_or(ATTR_LETTER_SPACING, 0.);
	let max_width: Option<f64> = item.attribute_cloned_or(ATTR_MAX_WIDTH, None);
	let max_height: Option<f64> = item.attribute_cloned_or(ATTR_MAX_HEIGHT, None);
	let letter_tilt: f64 = item.attribute_cloned_or(ATTR_LETTER_TILT, 0.);
	let align: text_nodes::TextAlign = item.attribute_cloned_or_default(ATTR_TEXT_ALIGN);
	let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;

	let typesetting = text_nodes::TypesettingConfig {
		font_size,
		line_height_ratio: line_height,
		letter_spacing,
		letter_tilt,
		max_width,
		max_height,
		align,
	};

	let mut glyph_paths: Vec<String> = Vec::new();

	text_nodes::TextContext::with_thread_local(|ctx| {
		let Some(layout) = ctx.layout_text(text, &font, typesetting) else { return };
		let tilt_tan = letter_tilt.to_radians().tan();

		text_nodes::for_each_styled_glyph_run(&layout, text, typesetting, |glyph_run, x_offset, space_extra| {
			draw_glyph_run_to_bezpaths(glyph_run, x_offset, space_extra, tilt_tan, |bez_path| {
				glyph_paths.push(bez_path.to_svg());
			});
		});
	});

	if glyph_paths.is_empty() {
		return;
	}

	// Wrap all glyph <path> elements in a <g> with the item's transform/opacity/blend-mode.
	render.parent_tag(
		"g",
		|attributes| {
			let matrix = format_transform_matrix(transform);
			if !matrix.is_empty() {
				attributes.push("transform", matrix);
			}
			if opacity < 1. {
				attributes.push("opacity", opacity.to_string());
			}
			if blend_mode_attr != BlendMode::default() {
				attributes.push("style", blend_mode_attr.render());
			}
		},
		|render| {
			for path_d in glyph_paths {
				render.leaf_tag("path", |attributes| {
					attributes.push("d", path_d);
					if let RenderMode::Outline = render_params.render_mode {
						attributes.push("fill", "none");
						attributes.push("stroke", "black");
						attributes.push("stroke-width", "1");
					} else {
						attributes.push("fill", "black");
						attributes.push("fill-rule", "nonzero");
					}
				});
			}
		},
	);
}

/// Draws one item of text content into the Vello scene, laying out its glyphs under the item's styling.
fn render_text_item_to_vello(item: ItemRef<'_, String>, scene: &mut Scene, transform: DAffine2, render_params: &RenderParams) {
	let Some(text) = item.element() else { return };
	if text.is_empty() {
		return;
	}

	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let font: Resource = {
		let f: Resource = item.attribute_cloned_or_default(ATTR_FONT);
		if f.is_empty() { text_nodes::FALLBACK_FONT_RESOURCE.clone() } else { f }
	};
	let font_size: f64 = item.attribute_cloned_or(ATTR_FONT_SIZE, DEFAULT_FONT_SIZE);
	let line_height: f64 = item.attribute_cloned_or(ATTR_LINE_HEIGHT, 1.2);
	let letter_spacing: f64 = item.attribute_cloned_or(ATTR_LETTER_SPACING, 0.);
	let max_width: Option<f64> = item.attribute_cloned_or(ATTR_MAX_WIDTH, None);
	let max_height: Option<f64> = item.attribute_cloned_or(ATTR_MAX_HEIGHT, None);
	let letter_tilt: f64 = item.attribute_cloned_or(ATTR_LETTER_TILT, 0.);
	let align: text_nodes::TextAlign = item.attribute_cloned_or_default(ATTR_TEXT_ALIGN);
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;

	let typesetting = text_nodes::TypesettingConfig {
		font_size,
		line_height_ratio: line_height,
		letter_spacing,
		letter_tilt,
		max_width,
		max_height,
		align,
	};

	let affine = Affine::new((transform * item_transform).to_cols_array());

	text_nodes::TextContext::with_thread_local(|ctx| {
		let Some(layout) = ctx.layout_text(text, &font, typesetting) else { return };

		let needs_layer = opacity < 1. || blend_mode_attr != BlendMode::default();
		if needs_layer {
			let alignment_width = max_width.map(|w| w as f32).unwrap_or_else(|| layout.full_width());
			let blending = peniko::BlendMode::new(blend_mode_attr.to_peniko(), peniko::Compose::SrcOver);
			let padding = font_size;
			let bounds = kurbo::Rect::new(-padding, -padding, alignment_width as f64 + padding, layout.height() as f64 + padding);
			let transformed_bounds = affine.transform_rect_bbox(bounds);
			scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &transformed_bounds);
		}

		let tilt_tan = letter_tilt.to_radians().tan();

		text_nodes::for_each_styled_glyph_run(&layout, text, typesetting, |glyph_run, x_offset, space_extra| {
			draw_glyph_run_to_bezpaths(glyph_run, x_offset, space_extra, tilt_tan, |bez_path| {
				if let RenderMode::Outline = render_params.render_mode {
					let (outline_stroke, outline_color) = get_outline_styles(render_params);
					scene.stroke(&outline_stroke, affine, outline_color, None, bez_path);
				} else {
					scene.fill(peniko::Fill::NonZero, affine, peniko::Color::BLACK, None, bez_path);
				}
			});
		});

		if needs_layer {
			scene.pop_layer();
		}
	});
}

/// The metadata pass over a run of text items. Click targets are baked relative to the first item's transform,
/// which `Graphic::collect_metadata` records as `local_transforms[element_id]`.
fn collect_text_items_metadata<'a>(items: impl Iterator<Item = ItemRef<'a, String>>, metadata: &mut RenderMetadata, footprint: Footprint, caller_element_id: Option<NodeId>) {
	let mut item_zero_transform = None;
	let mut item_zero_inverse = DAffine2::IDENTITY;

	let mut accumulated_click_targets: HashMap<NodeId, Vec<Arc<ClickTarget>>> = HashMap::new();

	for item in items {
		// The first item's transform is the reference all targets bake against
		let item_zero_transform = *item_zero_transform.get_or_insert_with(|| {
			let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
			item_zero_inverse = if transform.matrix2.determinant() != 0. { transform.inverse() } else { DAffine2::IDENTITY };
			transform
		});

		let Some(element_id) = caller_element_id.or(item.layer()) else { continue };

		// When recovering element_id from the item's tag (caller passed None), also store the transform metadata.
		if caller_element_id.is_none() {
			metadata.upstream_footprints.entry(element_id).or_insert(footprint);
			metadata.local_transforms.entry(element_id).or_insert(item_zero_transform);
		}

		let Some((size, item_transform)) = text_item_size_and_transform(item) else { continue };
		let mut target = ClickTarget::new_with_path(rectangle_path(DVec2::ZERO, size), 0.);
		target.apply_transform(item_zero_inverse * item_transform);
		accumulated_click_targets.entry(element_id).or_default().push(Arc::new(target));
	}

	// One rectangle per text item, reused for the selection outline (there's no letterform geometry to outline at this stage).
	for (element_id, targets) in accumulated_click_targets {
		metadata.outlines.insert(element_id, targets.clone());
		metadata.click_targets.insert(element_id, targets);
	}
}

/// Collects one text item's laid-out rectangle as a click target.
fn add_text_item_click_targets(item: ItemRef<'_, String>, click_targets: &mut Vec<ClickTarget>) {
	let Some((size, transform)) = text_item_size_and_transform(item) else { return };
	let mut target = ClickTarget::new_with_path(rectangle_path(DVec2::ZERO, size), 0.);
	target.apply_transform(transform);
	click_targets.push(target);
}

impl Render for List<String> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_text_item_svg(ItemRef::ListItem(self, index), render, render_params);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_text_item_to_vello(ItemRef::ListItem(self, index), scene, transform, render_params);
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, caller_element_id: Option<NodeId>, _inherited_appearance: Option<&Appearance>) {
		collect_text_items_metadata((0..self.len()).map(|index| ItemRef::ListItem(self, index)), metadata, footprint, caller_element_id);
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>, _inherited_appearance: Option<&Appearance>) {
		for index in 0..self.len() {
			add_text_item_click_targets(ItemRef::ListItem(self, index), click_targets);
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgSegment {
	Slice(&'static str),
	String(String),
}

impl From<String> for SvgSegment {
	fn from(value: String) -> Self {
		Self::String(value)
	}
}

impl From<&'static str> for SvgSegment {
	fn from(value: &'static str) -> Self {
		Self::Slice(value)
	}
}

pub trait RenderSvgSegmentList {
	fn to_svg_string(&self) -> String;
}

impl RenderSvgSegmentList for Vec<SvgSegment> {
	fn to_svg_string(&self) -> String {
		let mut result = String::new();
		for segment in self.iter() {
			result.push_str(match segment {
				SvgSegment::Slice(x) => x,
				SvgSegment::String(x) => x,
			});
		}
		result
	}
}

pub struct SvgRenderAttrs<'a>(&'a mut SvgRender);

impl SvgRenderAttrs<'_> {
	pub fn push_complex(&mut self, name: impl Into<SvgSegment>, value: impl FnOnce(&mut SvgRender)) {
		self.0.svg.push(" ".into());
		self.0.svg.push(name.into());
		self.0.svg.push("=\"".into());
		value(self.0);
		self.0.svg.push("\"".into());
	}
	pub fn push(&mut self, name: impl Into<SvgSegment>, value: impl Into<SvgSegment>) {
		self.push_complex(name, move |renderer| renderer.svg.push(value.into()));
	}
	pub fn push_val(&mut self, value: impl Into<SvgSegment>) {
		self.0.svg.push(value.into());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use vector_types::gradient::GradientSpace;

	#[test]
	fn stacked_paint_colors_composite_in_straight_alpha() {
		// A half-transparent red over an opaque blue lands halfway between the two
		let mut list = List::new();
		list.push(Item::new_from_element(Color::from_rgbaf32_unchecked(0., 0., 1., 1.)));
		list.push(Item::new_from_element(Color::from_rgbaf32_unchecked(1., 0., 0., 0.5)));

		let composited = composite_paint_colors(&list, false).expect("a non-empty paint list composites to a color");

		assert!((composited.r() - 0.5).abs() < 1e-5, "red was {}", composited.r());
		assert!((composited.g() - 0.).abs() < 1e-5, "green was {}", composited.g());
		assert!((composited.b() - 0.5).abs() < 1e-5, "blue was {}", composited.b());
		assert!((composited.a() - 1.).abs() < 1e-5, "alpha was {}", composited.a());
	}

	#[test]
	fn stacked_paint_blending_interpolates_by_backdrop_coverage() {
		// Multiply over half-covering black only half-multiplies the red
		let mut list = List::new();
		list.push(Item::new_from_element(Color::from_rgbaf32_unchecked(0., 0., 0., 0.5)));
		list.push(Item::new_from_element(Color::from_rgbaf32_unchecked(1., 0., 0., 1.)).with_attribute(ATTR_BLEND_MODE, BlendMode::Multiply));

		let composited = composite_paint_colors(&list, false).expect("a non-empty paint list composites to a color");

		assert!((composited.r() - 0.5).abs() < 1e-5, "red was {}", composited.r());
		assert!((composited.a() - 1.).abs() < 1e-5, "alpha was {}", composited.a());

		// Multiply over no backdrop at all leaves the source color untouched
		let mut list = List::new();
		list.push(Item::new_from_element(Color::TRANSPARENT));
		list.push(Item::new_from_element(Color::from_rgbaf32_unchecked(1., 0., 0., 1.)).with_attribute(ATTR_BLEND_MODE, BlendMode::Multiply));

		let composited = composite_paint_colors(&list, false).expect("a non-empty paint list composites to a color");

		assert!((composited.r() - 1.).abs() < 1e-5, "red was {}", composited.r());
		assert!((composited.a() - 1.).abs() < 1e-5, "alpha was {}", composited.a());
	}

	#[test]
	fn spread_adjusted_samples_wraps_clear_in_transparent_guards() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		let (samples, span) = spread_adjusted_samples(
			&gradient,
			GradientSettings {
				spread: GradientSpread::Repeat,
				space: GradientSpace::RgbGamma,
				..Default::default()
			},
			GradientForm::Linear,
			ClearGuardPlacement::SvgStopOrder,
		);
		assert_eq!(span, (0., 1.));
		assert_eq!(
			samples,
			gradient.interpolated_samples(GradientSettings {
				space: GradientSpace::RgbGamma,
				..Default::default()
			})
		);

		// SVG guards share the range ends' exact offsets, ordered so the pad extension resolves to the transparent outer stops
		let (samples, span) = spread_adjusted_samples(
			&gradient,
			GradientSettings {
				spread: GradientSpread::Clear,
				space: GradientSpace::RgbGamma,
				..Default::default()
			},
			GradientForm::Linear,
			ClearGuardPlacement::SvgStopOrder,
		);
		assert_eq!(span, (0., 1.));
		assert_eq!(
			samples,
			vec![(0., Color::TRANSPARENT, None), (0., Color::BLACK, None), (1., Color::WHITE, None), (1., Color::TRANSPARENT, None)]
		);

		// Vello guards own the outermost ramp texels, with the visible range compressed inward to make room
		let texel = 1. / (VELLO_GRADIENT_RAMP_TEXELS - 1.);
		let (samples, span) = spread_adjusted_samples(
			&gradient,
			GradientSettings {
				spread: GradientSpread::Clear,
				space: GradientSpace::RgbGamma,
				..Default::default()
			},
			GradientForm::Linear,
			ClearGuardPlacement::VelloRampTexels,
		);
		assert_eq!(
			samples,
			vec![
				(0., Color::TRANSPARENT, None),
				(texel, Color::BLACK, None),
				(1. - texel, Color::WHITE, None),
				(1., Color::TRANSPARENT, None)
			]
		);
		assert!(span.0 < 0. && span.1 > 1., "the geometry must stretch to compensate for the compressed stops: {span:?}");

		// A radial keeps its stops and span anchored at zero, with no guard below the center
		let (samples, span) = spread_adjusted_samples(
			&gradient,
			GradientSettings {
				spread: GradientSpread::Clear,
				space: GradientSpace::RgbGamma,
				..Default::default()
			},
			GradientForm::Radial,
			ClearGuardPlacement::VelloRampTexels,
		);
		assert_eq!(span.0, 0.);
		assert_eq!(samples.first().unwrap(), &(0., Color::BLACK, None));
		assert_eq!(samples.last().unwrap(), &(1., Color::TRANSPARENT, None));
	}

	#[test]
	fn spread_adjusted_samples_keeps_a_stopless_clear_gradient_black_inside_the_range() {
		let (samples, _) = spread_adjusted_samples(
			&Gradient::from(Vec::new()),
			GradientSettings {
				spread: GradientSpread::Clear,
				space: GradientSpace::RgbGamma,
				..Default::default()
			},
			GradientForm::Linear,
			ClearGuardPlacement::SvgStopOrder,
		);
		let colors: Vec<Color> = samples.iter().map(|&(_, color, _)| color).collect();
		assert_eq!(colors, vec![Color::TRANSPARENT, Color::BLACK, Color::BLACK, Color::TRANSPARENT]);
	}
}
