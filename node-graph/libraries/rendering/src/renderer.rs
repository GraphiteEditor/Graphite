use crate::render_ext::{PaintTarget, RenderExt};
use crate::to_peniko::{BlendModeExt, ToPenikoColor};
use crate::vello_scene_builder::VelloSceneBuilder;
use core_types::CacheHash;
use core_types::FallibleVec2Operations;
use core_types::blending::{BlendMode, blend_colors};
use core_types::bounds::BoundingBox;
use core_types::bounds::RenderBoundingBox;
use core_types::color::Color;
use core_types::color::SRGBA8;
use core_types::consts::DEFAULT_FONT_SIZE;
use core_types::list::ATTR_APPEARANCE;
use core_types::list::{ATTR_GEOMETRY_CACHE, ATTR_PATHS};
use core_types::list::{Item, List, NodeIdPath};
use core_types::math::quad::Quad;
use core_types::transform::Footprint;
use core_types::uuid::{NodeId, generate_uuid};
use core_types::{
	ATTR_BACKGROUND, ATTR_BLEND_MODE, ATTR_CLIP, ATTR_CLIPPING_MASK, ATTR_DIMENSIONS, ATTR_EDITOR_CLICK_TARGET, ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_EDITOR_TEXT_FRAME,
	ATTR_EFFECTS, ATTR_FONT, ATTR_FONT_SIZE, ATTR_GRADIENT_FORM, ATTR_LETTER_SPACING, ATTR_LETTER_TILT, ATTR_LINE_HEIGHT, ATTR_LOCATION, ATTR_MAX_HEIGHT, ATTR_MAX_WIDTH, ATTR_OPACITY,
	ATTR_OPACITY_FILL, ATTR_TEXT_ALIGN, ATTR_TRANSFORM,
};
use dyn_any::DynAny;
use glam::{DAffine2, DMat2, DVec2};
use graphene_resource::Resource;
use graphic_types::generator::GeneratorPattern;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::vector_types::gradient::{Gradient, GradientForm};
use graphic_types::vector_types::vector::click_target::{ClickTarget, FreePoint, TargetPath};
use graphic_types::vector_types::vector::misc::dvec2_to_point;
use graphic_types::vector_types::vector::style::{RenderMode, StrokeAlign, StrokeCap, StrokeJoin};
use graphic_types::vector_types::vector::{
	GeometryCache, Path, PlacedContours, contour_bezpaths, contours_bounding_box, contours_stroke_inclusive_bounding_box, edit_item_geometry, edit_list_row_geometry, no_paths,
};
use graphic_types::{Adjustment, Appearance, Artboard, BevelStyle, Cover, Coverage, EffectOperation, Effects, FillAndStroke, Generator, Graphic, Vector};
use kurbo::{Affine, BezPath, Cap, Join, PathEl, Shape};
use num_traits::Zero;
use raster_kernels::raster_effects::adjusted_raster;
use raster_kernels::value_effects::{adjusted_color, adjusted_gradient, adjustments};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::FontRef as SkrifaFontRef;
use skrifa::{GlyphId, MetadataProvider};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::hash::Hash;
use std::sync::Arc;
use vector_types::gradient::{GradientSettings, GradientSpread};
use vello_gpu::peniko;

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

	pub(crate) fn attribute_any(self, key: &str) -> Option<&'a dyn std::any::Any> {
		match self {
			ItemRef::ListItem(list, index) => list.attribute_any(key, index),
			ItemRef::Item(item) => item.attributes().get_any(key),
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

	/// How the item's opacity attributes and effects apply as it renders.
	fn compositing(self, render_params: &RenderParams) -> ItemCompositing<'a> {
		let opacity = self.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) as f32;
		let fill = if render_params.for_mask {
			1.
		} else {
			self.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.) as f32
		};

		// Outline mode shows only geometry
		let effects = self
			.attribute::<Effects>(ATTR_EFFECTS)
			.filter(|effects| !effects.is_empty() && render_params.render_mode != RenderMode::Outline);

		let pixel_scale = render_params.viewport_zoom.unwrap_or(1.) * render_params.scale;
		let pixel_scale = if pixel_scale.is_normal() && pixel_scale > 0. { pixel_scale } else { 1. };

		match effects {
			Some(effects) => ItemCompositing {
				opacity,
				effects: Some((effects, fill)),
				pixel_scale,
			},
			None => ItemCompositing {
				opacity: opacity * fill,
				effects: None,
				pixel_scale,
			},
		}
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

/// Geometry derived from a vector item, extracted once per geometry when the item carries a geometry cache.
impl<'a> ItemRef<'a, Vector> {
	fn paths(self) -> &'a List<Path> {
		self.attribute::<List<Path>>(ATTR_PATHS).unwrap_or_else(|| no_paths())
	}

	fn contours(self) -> Arc<[BezPath]> {
		let Some(vector) = self.element() else { return Arc::default() };
		match self.attribute::<GeometryCache>(ATTR_GEOMETRY_CACHE) {
			Some(cache) => cache.contours(vector, self.paths()),
			None => contour_bezpaths(&vector.0, self.paths()).into(),
		}
	}

	fn faces(self) -> Arc<[BezPath]> {
		let Some(vector) = self.element() else { return Arc::default() };
		match self.attribute::<GeometryCache>(ATTR_GEOMETRY_CACHE) {
			Some(cache) => cache.faces(vector, self.paths()),
			None => vector.construct_faces(self.paths()).into(),
		}
	}

	/// The contours combined into one click target path, covering what the fill paints when `filled`.
	fn target_path(self, filled: bool) -> Option<Arc<TargetPath>> {
		let vector = self.element()?;
		match self.attribute::<GeometryCache>(ATTR_GEOMETRY_CACHE) {
			Some(cache) => cache.target_path(vector, self.paths(), filled),
			None => GeometryCache::default().target_path(vector, self.paths(), filled),
		}
	}

	/// The item's geometry cache, for a copy of the item drawing the same geometry, or a fresh one if it has none.
	fn geometry_cache(self) -> GeometryCache {
		self.attribute::<GeometryCache>(ATTR_GEOMETRY_CACHE).cloned().unwrap_or_default()
	}

	/// The bounding box of the contours in the item's own space.
	fn contours_bounding_box(self) -> Option<[DVec2; 2]> {
		let vector = self.element()?;
		match self.attribute::<GeometryCache>(ATTR_GEOMETRY_CACHE) {
			Some(cache) => cache.bounding_box(vector, self.paths()),
			None => contours_bounding_box(&self.contours(), DAffine2::IDENTITY),
		}
	}

	/// The contours placed in the stroke space given by `transform`, with the outlines of the strokes drawn around them.
	/// Copies of the item drawn in the same stroke space share these, so their strokes are expanded once.
	fn placed_contours(self, transform: DAffine2) -> Option<Arc<PlacedContours>> {
		Some(self.geometry_cache().placed_contours(self.element()?, self.paths(), transform))
	}
}

/// How an item composites: the alpha of the layer it draws through, and the effects its content renders through inside that layer.
struct ItemCompositing<'a> {
	opacity: f32,
	/// With the fill opacity, which the effects group applies to the content alone so the shadows and glows cast from it stay at full strength.
	effects: Option<(&'a Effects, f32)>,
	/// How many output pixels span a document unit, which SVG lighting measures the bevels' slopes across.
	pixel_scale: f64,
}

impl ItemCompositing<'_> {
	/// How far the effects reach past the content, in document pixels.
	fn reach(&self) -> f64 {
		self.effects.map_or(0., |(effects, _)| effects.reach())
	}

	/// The adjustments at the start of the effects, which can be carried out on the content's own colors ahead of the rest.
	/// A fill fades the content before the effects adjust it, so with one, only adjustments that keep alpha commute with it.
	fn leading_adjustments(&self) -> Vec<Adjustment> {
		let Some((effects, fill)) = self.effects else { return Vec::new() };

		effects
			.operations()
			.map_while(|operation| match operation {
				EffectOperation::Adjustment(adjustment) if fill == 1. || raster_kernels::adjustments::preserves_alpha(&adjustment) => Some(adjustment),
				_ => None,
			})
			.collect()
	}

	/// The leading adjustments for the content to carry out on its own colors, and the compositing left for it, holding the effects after them in `remaining`.
	fn split_leading_adjustments<'b>(&self, remaining: &'b mut Effects) -> (Vec<Adjustment>, ItemCompositing<'b>)
	where
		Self: 'b,
	{
		let leading_adjustments = self.leading_adjustments();
		let Some((effects, _)) = self.effects.filter(|_| !leading_adjustments.is_empty()) else {
			return (leading_adjustments, ItemCompositing { ..*self });
		};

		*remaining = effects.after(leading_adjustments.len());
		(leading_adjustments, self.with_remaining_effects(remaining))
	}

	/// This compositing with only the `remaining` effects, those after the leading adjustments the content has taken.
	fn with_remaining_effects<'b>(&self, remaining: &'b Effects) -> ItemCompositing<'b> {
		let fill = self.effects.map_or(1., |(_, fill)| fill);

		match remaining.is_empty() {
			true => ItemCompositing {
				opacity: self.opacity * fill,
				effects: None,
				pixel_scale: self.pixel_scale,
			},
			false => ItemCompositing {
				opacity: self.opacity,
				effects: Some((remaining, fill)),
				pixel_scale: self.pixel_scale,
			},
		}
	}

	/// Draws the content `draw` draws through the effects group, given the content's bounds in the scene's space.
	/// Rendering a split scene calls `draw` again for each part.
	fn draw_with_effects(&self, scene: &mut VelloSceneBuilder, bounds: impl FnOnce() -> RenderBoundingBox, mut draw: impl FnMut(&mut VelloSceneBuilder)) {
		let Some((effects, fill)) = self.effects else { return draw(scene) };

		let bounds = match bounds() {
			RenderBoundingBox::Rectangle([min, max]) => Some(kurbo::Rect::new(min.x, min.y, max.x, max.y)),
			RenderBoundingBox::Infinite => None,
			RenderBoundingBox::None => return draw(scene),
		};
		scene.draw_effects(effects, fill, bounds, &mut draw);
	}

	/// The opacity for content drawn without the effects group, as SVG draws content its filter can't apply to.
	fn opacity_without_effects(&self) -> f32 {
		self.opacity * self.effects.map_or(1., |(_, fill)| fill)
	}

	/// A vector's `fill` and `stroke` paints with the leading adjustments carried out on their colors, if they're solid colors this is exact for,
	/// sparing the content an effects group or an SVG filter that might not express them. `strokes` is whether the stroke is drawn at all.
	fn fold_into_solid_paints(&self, fill: Option<&Graphic>, stroke: Option<&Graphic>, strokes: bool, for_mask: bool) -> Option<FoldedPaints> {
		// Antialiased edges are translucent, so an adjustment changing alpha would act differently on them than on the paint
		let adjustments: Vec<Adjustment> = self.leading_adjustments().into_iter().take_while(raster_kernels::adjustments::preserves_alpha).collect();
		let (effects, _) = self.effects.filter(|_| !adjustments.is_empty())?;

		// The color a solid paint draws, which is `None` for no paint, or no color at all for a paint that isn't solid
		let solid_color = |paint: Option<&Graphic>| match paint {
			None | Some(Graphic::None(_) | Graphic::NoneList(_)) => Some(None),
			Some(Graphic::Color(item)) => Some(faded_paint_color(ItemRef::Item(item), for_mask)),
			Some(Graphic::ColorList(list)) => Some(composite_paint_colors(list, for_mask)),
			Some(_) => None,
		};
		let fill_color = solid_color(fill)?;
		let stroke_color = if strokes { solid_color(stroke)? } else { None };

		// Where translucent paints overlap, the adjustment would act on their blend rather than on each
		if let (Some(fill_color), Some(stroke_color)) = (fill_color, stroke_color)
			&& (fill_color.a() < 1. || stroke_color.a() < 1.)
		{
			return None;
		}

		let adjusted = |color: Option<Color>| Some(Graphic::Color(Item::new_from_element(adjusted_color(color?, &adjustments))));
		Some(FoldedPaints {
			fill: adjusted(fill_color),
			stroke: adjusted(stroke_color),
			remaining_effects: effects.after(adjustments.len()),
		})
	}

	/// Writes the SVG filter carrying out the effects, if there are any to apply, given the content's bounds in the current user space.
	/// Returns the reference for the `filter` attribute of a group wrapping the content, which also takes the item's opacity and blend mode.
	fn svg_filter(&self, render: &mut SvgRender, bounds: impl FnOnce() -> RenderBoundingBox) -> Option<String> {
		let (effects, fill) = self.effects?;

		// Effects change nothing visible about content without edges, like a color or gradient layer filling everything
		let RenderBoundingBox::Rectangle(bounds) = bounds() else { return None };

		let id = write_svg_effects_filter(&mut render.svg_defs, effects, fill, bounds, render.transform, self.pixel_scale)?;
		Some(format!("url(#{id})"))
	}
}

/// A vector's solid paints with adjustments carried out on their colors, and the effects left after those adjustments.
struct FoldedPaints {
	fill: Option<Graphic>,
	stroke: Option<Graphic>,
	remaining_effects: Effects,
}

/// How far the effects of the items nested anywhere inside `graphic` reach past its bounds, in document pixels.
fn nested_effects_reach(graphic: &Graphic) -> f64 {
	fn item_reach<T>(item: &Item<T>) -> f64 {
		item.attribute::<Effects>(ATTR_EFFECTS).map_or(0., Effects::reach)
	}
	fn list_reach<T>(list: &List<T>, nested: impl Fn(&T) -> f64) -> f64 {
		(0..list.len())
			.map(|index| list.attribute::<Effects>(ATTR_EFFECTS, index).map_or(0., Effects::reach) + list.element(index).map_or(0., &nested))
			.fold(0., f64::max)
	}

	match graphic {
		Graphic::None(_) | Graphic::NoneList(_) | Graphic::StrokeList(_) => 0.,
		Graphic::Graphic(item) => item_reach(item) + nested_effects_reach(item.element()),
		Graphic::Vector(item) => item_reach(item),
		Graphic::RasterCPU(item) => item_reach(item),
		Graphic::RasterGPU(item) => item_reach(item),
		Graphic::Color(item) => item_reach(item),
		Graphic::Gradient(item) => item_reach(item),
		Graphic::Generator(item) => item_reach(item),
		Graphic::Text(item) => item_reach(item),
		Graphic::GraphicList(list) => list_reach(list, nested_effects_reach),
		Graphic::VectorList(list) => list_reach(list, |_| 0.),
		Graphic::RasterCPUList(list) => list_reach(list, |_| 0.),
		Graphic::RasterGPUList(list) => list_reach(list, |_| 0.),
		Graphic::ColorList(list) => list_reach(list, |_| 0.),
		Graphic::GradientList(list) => list_reach(list, |_| 0.),
		Graphic::GeneratorList(list) => list_reach(list, |_| 0.),
		Graphic::TextList(list) => list_reach(list, |_| 0.),
	}
}

/// Wraps an item's SVG `content` in a group applying its effects `filter`, with the item's opacity and blend mode, if there is a filter.
fn render_svg_through_filter(render: &mut SvgRender, filter: Option<String>, opacity: f32, blend_mode: BlendMode, content: impl FnOnce(&mut SvgRender)) {
	let Some(filter) = filter else { return content(render) };

	render.parent_tag(
		"g",
		|attributes| {
			attributes.push("filter", filter);
			if opacity < 1. {
				attributes.push("opacity", opacity.to_string());
			}
			if blend_mode != BlendMode::default() {
				attributes.push("style", blend_mode.render());
			}
		},
		content,
	);
}

/// Builds the primitives of an SVG filter, naming each result so later primitives can take it as an input.
#[derive(Default)]
struct SvgFilterPrimitives {
	markup: String,
	count: usize,
}

impl SvgFilterPrimitives {
	/// Appends a primitive tag with `attributes` (and `children`, if any), returning the name of its result.
	fn push(&mut self, tag: &str, attributes: &str, children: &str) -> String {
		self.count += 1;
		let result = format!("effect-{}", self.count);

		match children.is_empty() {
			true => write!(self.markup, r#"<{tag} {attributes} result="{result}" />"#),
			false => write!(self.markup, r#"<{tag} {attributes} result="{result}">{children}</{tag}>"#),
		}
		.unwrap();

		result
	}

	/// The sum of two premultiplied images, for composites whose terms SVG's other operators can't express.
	fn add(&mut self, first: &str, second: &str) -> String {
		self.push("feComposite", &format!(r#"in="{first}" in2="{second}" operator="arithmetic" k2="1" k3="1""#), "")
	}

	/// The coverage of the content's silhouette (or of the area outside it, if `inverse`), cast by `offset`, grown by `spread` and softened by `blur`, all in user units.
	fn cast(&mut self, full_content: &str, offset: DVec2, blur: f64, spread: f64, inverse: bool) -> String {
		let mut coverage = match offset == DVec2::ZERO {
			true => full_content.to_string(),
			false => self.push("feOffset", &format!(r#"in="{full_content}" dx="{}" dy="{}""#, offset.x, offset.y), ""),
		};
		if inverse {
			let opaque = self.push("feFlood", r#"flood-opacity="1""#, "");
			coverage = self.push("feComposite", &format!(r#"in="{opaque}" in2="{coverage}" operator="out""#), "");
		}
		if spread >= 0.01 {
			coverage = self.push("feMorphology", &format!(r#"in="{coverage}" operator="dilate" radius="{spread}""#), "");
		}
		if blur >= 0.01 {
			coverage = self.push("feGaussianBlur", &format!(r#"in="{coverage}" stdDeviation="{blur}""#), "");
		}
		coverage
	}

	/// The coverage of the band reaching `width` user units from the silhouette's edge, outward or (if `inward`) inward.
	/// SVG measures no straight-line distances, so a blur thresholded where it falls to that of a straight edge's at the width rounds the corners instead.
	fn edge_band(&mut self, full_content: &str, width: f64, inward: bool) -> String {
		// At this distance past a straight edge, a Gaussian of half the width falls to Φ(-2), dropping by φ(2) / σ per unit
		const THRESHOLD: f64 = 0.022_750_13;
		const FALLOFF: f64 = 0.053_990_97;

		let silhouette = self.cast(full_content, DVec2::ZERO, width / 2., 0., inward);
		let slope = width / 2. / FALLOFF;
		let intercept = 0.5 - slope * THRESHOLD;
		self.push(
			"feComponentTransfer",
			&format!(r#"in="{silhouette}""#),
			&format!(r#"<feFuncA type="linear" slope="{slope}" intercept="{intercept}" />"#),
		)
	}

	/// A bevel's height in the alpha channel, shaped by the `style` over a slope `size` user units wide.
	/// SVG measures no distances from edges, so the silhouette's edge is moved to the slope's middle and blurred across the slope's width instead.
	fn bevel_height(&mut self, full_content: &str, size: f64, style: BevelStyle) -> String {
		let ramp = |primitives: &mut Self, operator: Option<&str>, width: f64| {
			let silhouette = match operator {
				Some(operator) => primitives.push("feMorphology", &format!(r#"in="{full_content}" operator="{operator}" radius="{}""#, width / 2.), ""),
				None => full_content.to_string(),
			};
			primitives.push("feGaussianBlur", &format!(r#"in="{silhouette}" stdDeviation="{}""#, width / 4.), "")
		};

		match style {
			BevelStyle::InnerBevel => ramp(self, Some("erode"), size),
			BevelStyle::OuterBevel => ramp(self, Some("dilate"), size),
			BevelStyle::Emboss => ramp(self, None, size),
			// Ramps up into the content and away from it, each half the slope wide, meeting low at the edge
			BevelStyle::PillowEmboss => {
				let rising_inward = ramp(self, Some("erode"), size / 2.);
				let falling_outward = ramp(self, Some("dilate"), size / 2.);
				self.push(
					"feComposite",
					&format!(r#"in="{rising_inward}" in2="{falling_outward}" operator="arithmetic" k2="1" k3="-1" k4="1""#),
					"",
				)
			}
		}
	}

	/// How far the `light` brightens the bevel's surface past a flat surface's brightness, or if `shadow`, how far it falls short, as coverage in the alpha channel.
	/// `slope` scales the height's gradient per output pixel into the surface's steepness, and is negative for a bevel pressed into the surface.
	fn bevel_shading(&mut self, height: &str, slope: f64, light: glam::DVec3, shadow: bool) -> String {
		let azimuth = light.y.atan2(light.x).to_degrees();
		let elevation = light.z.clamp(-1., 1.).asin().to_degrees();
		// SVG lighting's Sobel kernel measures twice the gradient
		let surface_scale = slope / 2.;
		let lit = self.push(
			"feDiffuseLighting",
			&format!(r#"in="{height}" surfaceScale="{surface_scale}" diffuseConstant="1" lighting-color="white""#),
			&format!(r#"<feDistantLight azimuth="{azimuth}" elevation="{elevation}" />"#),
		);

		// The lit brightness in red becomes coverage in alpha, past the flat brightness for a highlight or short of it for a shadow
		let flat = light.z;
		let (scale, offset) = match shadow {
			true => (-1. / flat.max(1e-4), 1.),
			false => (1. / (1. - flat).max(1e-4), -flat / (1. - flat).max(1e-4)),
		};
		self.push(
			"feColorMatrix",
			&format!(r#"in="{lit}" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 {scale} 0 0 0 {offset}""#),
			"",
		)
	}

	/// The coverage tinted `color`.
	fn tint(&mut self, coverage: &str, color: Color) -> String {
		let flood = self.push(
			"feFlood",
			&format!(r##"flood-color="#{}" flood-opacity="{}""##, SRGBA8::from(color.with_alpha(1.)).to_rgb_hex(), color.a()),
			"",
		);
		self.push("feComposite", &format!(r#"in="{flood}" in2="{coverage}" operator="in""#), "")
	}

	/// Stacks the coverage, tinted `color`, over the shadows and glows already `beneath` the content.
	fn shade_beneath(&mut self, beneath: Option<String>, coverage: &str, color: Color) -> String {
		let tinted = self.tint(coverage, color);
		match beneath {
			Some(beneath) => self.push("feComposite", &format!(r#"in="{tinted}" in2="{beneath}" operator="over""#), ""),
			None => tinted,
		}
	}

	/// Places the coverage, tinted `color`, over both the full-strength content and the faded fill, within the content so it stays visible over a faded fill.
	fn shade_within(&mut self, full_content: &mut String, filled: &mut Option<String>, coverage: &str, color: Color) {
		let tinted = self.tint(coverage, color);
		let within = self.push("feComposite", &format!(r#"in="{tinted}" in2="{full_content}" operator="in""#), "");

		let mut over = |image: &str| {
			let uncovered = self.push("feComposite", &format!(r#"in="{image}" in2="{tinted}" operator="out""#), "");
			self.add(&within, &uncovered)
		};
		*filled = filled.as_deref().map(&mut over);
		*full_content = over(full_content);
	}

	/// The image with the shadows and glows `beneath` it, knocked out by the full-strength content so a faded fill doesn't reveal them.
	fn composite_beneath(&mut self, image: &str, beneath: &str, full_content: &str) -> String {
		let knocked_out = self.push("feComposite", &format!(r#"in="{beneath}" in2="{full_content}" operator="out""#), "");
		self.add(image, &knocked_out)
	}

	/// The image with `adjustment` applied where SVG's color primitives can express it, or left as it is where they can't.
	fn adjust(&mut self, image: &str, adjustment: &graphic_types::Adjustment) -> String {
		use graphic_types::Adjustment;
		use graphic_types::adjustment::DesaturateMethod;

		// Enough entries that interpolating between them strays less than a level from the curve
		const TABLE_SIZE: usize = 256;

		let table = |values: &[f32]| values.iter().map(|value| format!("{:.4}", value.clamp(0., 1.))).collect::<Vec<_>>().join(" ");
		let functions = |kind: &str, [red, green, blue]: [&[f32]; 3]| {
			[("R", red), ("G", green), ("B", blue)]
				.map(|(channel, values)| format!(r#"<feFunc{channel} type="{kind}" tableValues="{}" />"#, table(values)))
				.concat()
		};
		let in_gamma = |input: &str| format!(r#"in="{input}" color-interpolation-filters="sRGB""#);
		// A matrix whose three color rows each hold `weights` (red, green, blue, constant), leaving alpha as it is
		let matrix = |rows: [[f32; 4]; 3]| {
			let row = |[red, green, blue, constant]: [f32; 4]| format!("{red} {green} {blue} 0 {constant}");
			format!(r#"type="matrix" values="{} {} {} 0 0 0 1 0""#, row(rows[0]), row(rows[1]), row(rows[2]))
		};

		if let Some([red, green, blue, mut alpha]) = raster_kernels::adjustments::channel_curves(adjustment, TABLE_SIZE) {
			// Fully transparent pixels keep no color to adjust, so they stay empty as on the GPU
			alpha[0] = 0.;
			let curves = functions("table", [&red, &green, &blue]) + &format!(r#"<feFuncA type="table" tableValues="{}" />"#, table(&alpha));
			return self.push("feComponentTransfer", &in_gamma(image), &curves);
		}

		match adjustment {
			Adjustment::Threshold { min_luminance, max_luminance } => {
				// The same fixed-point Rec. 601 luma the kernel compares, then white where it falls within the range
				let weight = [4915., 9667., 1802., 0.].map(|weight| weight / 16384.);
				let luma = self.push("feColorMatrix", &format!("{} {}", in_gamma(image), matrix([weight; 3])), "");
				let (min, max) = ((min_luminance / 100.) as f32, (max_luminance / 100.) as f32);
				let steps: Vec<f32> = (0..TABLE_SIZE)
					.map(|index| index as f32 / TABLE_SIZE as f32)
					.map(|luma| if luma >= min && luma <= max { 1. } else { 0. })
					.collect();
				self.push("feComponentTransfer", &in_gamma(&luma), &functions("discrete", [&steps, &steps, &steps]))
			}
			Adjustment::GradientMap { gradient, reverse } => {
				// The classic luma picks each color's position along the gradient, whose colors the tables hold
				let luma = self.push("feColorMatrix", &format!("{} {}", in_gamma(image), matrix([[0.3, 0.59, 0.11, 0.]; 3])), "");
				let evaluator = gradient.element().evaluator(GradientSettings::from_item_attributes(gradient));
				let colors: Vec<[f32; 4]> = (0..TABLE_SIZE)
					.map(|index| index as f64 / (TABLE_SIZE - 1) as f64)
					.map(|position| evaluator.evaluate(if *reverse { 1. - position } else { position }).to_gamma_srgb_channels())
					.collect();
				let channel = |index: usize| colors.iter().map(|color| color[index]).collect::<Vec<_>>();
				self.push("feComponentTransfer", &in_gamma(&luma), &functions("table", [&channel(0), &channel(1), &channel(2)]))
			}
			Adjustment::ChannelMixer(settings) => {
				// Weights are 10-bit fixed point truncated toward zero, as the kernel takes them
				let row = |row: graphic_types::adjustment::ChannelMixerRow| [row.red, row.green, row.blue, row.constant].map(|percent| (percent as f32 * 1024. / 100.).trunc() / 1024.);
				let rows = match settings.monochrome {
					true => [row(settings.monochrome_mix); 3],
					false => [row(settings.red), row(settings.green), row(settings.blue)],
				};
				self.push("feColorMatrix", &format!("{} {}", in_gamma(image), matrix(rows)), "")
			}
			Adjustment::Desaturate { method } => {
				let (weights, space) = match method {
					DesaturateMethod::LuminanceRec709 => ([0.2126, 0.7152, 0.0722, 0.], "linearRGB"),
					DesaturateMethod::ChannelsAverage => ([1. / 3., 1. / 3., 1. / 3., 0.], "linearRGB"),
					DesaturateMethod::LumaRec709 => ([0.2126, 0.7152, 0.0722, 0.], "sRGB"),
					DesaturateMethod::LumaRec601 => ([0.299, 0.587, 0.114, 0.], "sRGB"),
					// The rest take the extremes of the channels or a perceptual lightness, which a matrix can't express
					_ => return image.to_string(),
				};
				self.push("feColorMatrix", &format!(r#"in="{image}" color-interpolation-filters="{space}" {}"#, matrix([weights; 3])), "")
			}
			// TODO: Express the other cross-channel adjustments, which no SVG primitive covers exactly
			_ => image.to_string(),
		}
	}
}

/// Writes an SVG filter into `svg_defs` that carries out `effects` like the GPU passes do, returning its ID, or `None` if the user space is degenerate.
/// `bounds` is the content's extent in the filtered group's user space, which `user_to_document` maps into the document space the effects' lengths are measured in.
/// `pixel_scale` is how many output pixels span a document unit.
fn write_svg_effects_filter(svg_defs: &mut String, effects: &Effects, fill: f32, bounds: [DVec2; 2], user_to_document: DAffine2, pixel_scale: f64) -> Option<String> {
	use graphic_types::effects::{bevel_light, outline_extents};
	use graphic_types::{BevelDirection, EffectOperation, GlowSource};

	let determinant = user_to_document.matrix2.determinant();
	if !determinant.is_normal() {
		return None;
	}
	let document_to_user = user_to_document.matrix2.inverse();
	let length_scale = determinant.abs().sqrt().recip();

	let mut primitives = SvgFilterPrimitives::default();

	// The content at full strength, which casts every silhouette, and the content faded by the fill, which shows in the result
	let mut full = "SourceGraphic".to_string();
	let mut filled = (fill < 1.).then(|| primitives.push("feComponentTransfer", r#"in="SourceGraphic""#, &format!(r#"<feFuncA type="linear" slope="{fill}" />"#)));
	// The shadows and glows stacked beneath the content
	let mut beneath: Option<String> = None;

	for effect in effects.operations() {
		match effect {
			EffectOperation::Blur { radius, box_blur, gamma } => {
				let radius = radius * length_scale;
				if radius < 0.01 {
					continue;
				}

				// What lies beneath blurs together with the content
				if let Some(beneath) = beneath.take() {
					filled = filled.map(|filled| primitives.composite_beneath(&filled, &beneath, &full));
					full = primitives.composite_beneath(&full, &beneath, &full);
				}

				// SVG has no box blur, so a box becomes the Gaussian of the same variance
				let sigma = if box_blur { (radius * (radius + 1.) / 3.).sqrt() } else { radius };
				let space = if gamma { "sRGB" } else { "linearRGB" };
				let attributes = |input: &str| format!(r#"in="{input}" stdDeviation="{sigma}" color-interpolation-filters="{space}""#);

				filled = filled.map(|filled| primitives.push("feGaussianBlur", &attributes(&filled), ""));
				full = primitives.push("feGaussianBlur", &attributes(&full), "");
			}
			EffectOperation::DropShadow { color, offset, blur, spread } => {
				let coverage = primitives.cast(&full, document_to_user * offset, blur * length_scale, spread * length_scale, false);
				beneath = Some(primitives.shade_beneath(beneath, &coverage, color));
			}
			EffectOperation::OuterGlow { color, blur, spread } => {
				let coverage = primitives.cast(&full, DVec2::ZERO, blur * length_scale, spread * length_scale, false);
				beneath = Some(primitives.shade_beneath(beneath, &coverage, color));
			}
			EffectOperation::InnerShadow { color, offset, blur, spread } => {
				let coverage = primitives.cast(&full, document_to_user * offset, blur * length_scale, spread * length_scale, true);
				primitives.shade_within(&mut full, &mut filled, &coverage, color);
			}
			EffectOperation::InnerGlow { color, blur, choke, source } => {
				let mut coverage = primitives.cast(&full, DVec2::ZERO, blur * length_scale, choke * length_scale, true);
				if source == GlowSource::Center {
					coverage = primitives.push("feComponentTransfer", &format!(r#"in="{coverage}""#), r#"<feFuncA type="linear" slope="-1" intercept="1" />"#);
				}
				primitives.shade_within(&mut full, &mut filled, &coverage, color);
			}
			EffectOperation::Satin { color, offset, blur, invert } => {
				let offset = document_to_user * offset;
				let forward = primitives.cast(&full, offset, blur * length_scale, 0., false);
				let backward = primitives.cast(&full, -offset, blur * length_scale, 0., false);

				// The difference either way, each clamped at zero, adds up to its absolute value
				let difference = |primitives: &mut SvgFilterPrimitives, first: &str, second: &str| {
					primitives.push("feComposite", &format!(r#"in="{first}" in2="{second}" operator="arithmetic" k2="1" k3="-1""#), "")
				};
				let (ahead, behind) = (difference(&mut primitives, &forward, &backward), difference(&mut primitives, &backward, &forward));
				let mut coverage = primitives.add(&ahead, &behind);
				if invert {
					coverage = primitives.push("feComponentTransfer", &format!(r#"in="{coverage}""#), r#"<feFuncA type="linear" slope="-1" intercept="1" />"#);
				}
				primitives.shade_within(&mut full, &mut filled, &coverage, color);
			}
			EffectOperation::Outline { color, weight, align } => {
				let (outside, inside) = outline_extents(weight * length_scale, align);
				if outside >= 0.01 {
					let coverage = primitives.edge_band(&full, outside, false);
					beneath = Some(primitives.shade_beneath(beneath, &coverage, color));
				}
				if inside >= 0.01 {
					let coverage = primitives.edge_band(&full, inside, true);
					primitives.shade_within(&mut full, &mut filled, &coverage, color);
				}
			}
			EffectOperation::BevelAndEmboss {
				style,
				depth,
				direction,
				size,
				soften,
				angle,
				altitude,
				highlight_color,
				shadow_color,
			} => {
				let (size, soften) = (size * length_scale, soften * length_scale);
				if size < 0.01 {
					continue;
				}

				let height = primitives.bevel_height(&full, size, style);
				let slope = depth / 100. * size / length_scale * pixel_scale * if direction == BevelDirection::Down { -1. } else { 1. };
				let light = bevel_light(angle, altitude, document_to_user);
				for (shadow, color) in [(false, highlight_color), (true, shadow_color)] {
					let mut coverage = primitives.bevel_shading(&height, slope, light, shadow);
					if soften >= 0.01 {
						coverage = primitives.push("feGaussianBlur", &format!(r#"in="{coverage}" stdDeviation="{soften}""#), "");
					}

					beneath = Some(primitives.shade_beneath(beneath, &coverage, color));
					primitives.shade_within(&mut full, &mut filled, &coverage, color);
				}
			}
			EffectOperation::Adjustment(ref adjustment) => {
				filled = filled.map(|filled| primitives.adjust(&filled, adjustment));
				full = primitives.adjust(&full, adjustment);
			}
		}
	}

	// The filter's result is its last primitive's, so finish with the image shown
	let shown = filled.unwrap_or_else(|| full.clone());
	match beneath {
		Some(beneath) => {
			primitives.composite_beneath(&shown, &beneath, &full);
		}
		None => {
			primitives.push("feOffset", &format!(r#"in="{shown}""#), "");
		}
	}

	// The filter covers how far the effects reach past the content
	let reach = effects.reach() * length_scale;
	let (min, max) = (bounds[0] - reach, bounds[1] + reach);
	let id = format!("effects-{}", generate_uuid());
	write!(
		svg_defs,
		r#"<filter id="{id}" filterUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}" color-interpolation-filters="sRGB">{}</filter>"#,
		min.x,
		min.y,
		max.x - min.x,
		max.y - min.y,
		primitives.markup
	)
	.unwrap();

	Some(id)
}

/// Grows a layer's bounds in the scene's space by a length in document pixels.
fn inflate_bounds(bounds: [DVec2; 2], amount: f64) -> kurbo::Rect {
	kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y).inflate(amount, amount)
}

fn rect_bounds(rect: kurbo::Rect) -> RenderBoundingBox {
	RenderBoundingBox::Rectangle([DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)])
}

/// The color one paint item contributes, with the adjustments among its effects carried out and faded by its opacity attributes.
pub(crate) fn faded_paint_color(item: ItemRef<'_, Color>, for_mask: bool) -> Option<Color> {
	let mut color = *item.element()?;
	if let Some(effects) = item.attribute::<Effects>(ATTR_EFFECTS) {
		color = adjusted_color(color, &adjustments(effects));
	}

	Some(color.with_alpha(color.a() * item.paint_opacity(for_mask)))
}

/// The ramp one gradient paint item contributes, with the adjustments among its effects carried out along it, and the settings to read it with.
pub(crate) fn painted_gradient(item: ItemRef<'_, Gradient>) -> Option<(Cow<'_, Gradient>, GradientSettings)> {
	let gradient = item.element()?;
	let settings = gradient_settings_from_item(item);

	let adjusted = item.attribute::<Effects>(ATTR_EFFECTS).and_then(|effects| adjusted_gradient(gradient, settings, &adjustments(effects)));
	Some(match adjusted {
		Some((gradient, settings)) => (Cow::Owned(gradient), settings),
		None => (Cow::Borrowed(gradient), settings),
	})
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
			Some(under) => blend_colors(faded, under, item.attribute_cloned_or_default(ATTR_BLEND_MODE), 1.),
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
	/// Maps the user space of the content being rendered into the document's space, where the lengths of effects are measured.
	pub transform: DAffine2,
	/// The images drawn to canvases rather than embedded, by the canvas ID their placeholders name.
	pub image_data: HashMap<u64, CanvasImage>,
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
			defs = self.svg_defs
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
			defs = self.svg_defs
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
	pub image_data: HashMap<u64, CanvasImage>,
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
	pub viewport_zoom: Option<f64>,
	/// Screen pixels per document unit for sizing the images drawn to canvases, rounded up to a power of two so the render reruns only when it doubles or halves.
	/// Zero sends them at full resolution.
	pub canvas_image_scale: f64,
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

	// Composite over black in gamma sRGB space, then decode to linear for the luminance test.
	let [gamma_r, gamma_g, gamma_b, alpha] = bg.to_gamma_srgb_channels();
	let composited = Color::from_gamma_srgb_channels(gamma_r * alpha, gamma_g * alpha, gamma_b * alpha, 1.);

	let threshold = (1.05 * 0.05f32).sqrt() - 0.05;

	if composited.luminance_rec_709() > threshold { Color::BLACK } else { Color::WHITE }
}

pub fn to_transform(transform: DAffine2) -> usvg::Transform {
	let cols = transform.to_cols_array();
	usvg::Transform::from_row(cols[0] as f32, cols[1] as f32, cols[2] as f32, cols[3] as f32, cols[4] as f32, cols[5] as f32)
}

fn get_outline_styles(render_params: &RenderParams) -> (kurbo::Stroke, peniko::Color) {
	use core_types::consts::LAYER_OUTLINE_STROKE_WEIGHT;

	let zoom = render_params.viewport_zoom.unwrap_or(1.);
	let outline_stroke = kurbo::Stroke {
		width: LAYER_OUTLINE_STROKE_WEIGHT / if zoom > 0. { zoom } else { 1. },
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

fn draw_raster_outline(scene: &mut VelloSceneBuilder, outline_transform: &DAffine2, render_params: &RenderParams) {
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
			let line = axis.try_project_onto(band_normal).unwrap_or(axis);
			DAffine2 {
				matrix2: DMat2::from_cols(line, line.perp()),
				translation: transform.translation,
			}
		}
	}
}

/// Renderable gradient samples of `(position, color, original midpoint)`, as produced by [`Gradient::interpolated_samples`].
type GradientSamples = Vec<(f64, Color, Option<f64>)>;

/// The gradient's renderable samples, bracketed for the `Clear` spread (which neither SVG nor Vello supports natively) by transparent guard stops.
///
/// The guards share the unit range's end offsets, ordered so the pad extension paints transparency outward while the later
/// of each pair of coincident stops cuts the paint off exactly at the range's boundaries. A radial gradient needs no guard
/// at its start, since its sampling distance never goes below the center.
pub(crate) fn spread_adjusted_samples(gradient: &Gradient, settings: GradientSettings, gradient_form: GradientForm) -> GradientSamples {
	let samples = gradient.interpolated_samples(settings);
	if settings.spread != GradientSpread::Clear {
		return samples;
	}

	// A stopless gradient paints solid black, matching `Gradient::evaluate`
	let first_color = samples.first().map_or(Color::BLACK, |&(_, color, _)| color);
	let last_color = samples.last().map_or(Color::BLACK, |&(_, color, _)| color);
	let needs_start_anchor = samples.first().is_none_or(|&(position, ..)| position > 0.);
	let needs_end_anchor = samples.last().is_none_or(|&(position, ..)| position < 1.);

	let mut adjusted = Vec::with_capacity(samples.len() + 4);

	// Lead with the transparent guard, then anchor the visible range's start color
	if gradient_form == GradientForm::Linear {
		adjusted.push((0., Color::TRANSPARENT, None));
	}
	if needs_start_anchor {
		adjusted.push((0., first_color, None));
	}

	adjusted.extend(samples);

	// Anchor the visible range's end color, then cut to the trailing transparent guard
	if needs_end_anchor {
		adjusted.push((1., last_color, None));
	}
	adjusted.push((1., Color::TRANSPARENT, None));

	adjusted
}

/// Converts a gradient's renderer samples to peniko color stops, keeping each color's full precision.
fn peniko_color_stops(samples: &[(f64, Color, Option<f64>)]) -> peniko::ColorStops {
	let stop = |offset: f64, color: Color| peniko::ColorStop {
		offset: offset as f32,
		color: peniko::color::DynamicColor::from_alpha_color(peniko::Color::new(color.to_gamma_srgb_channels())),
	};

	// A gradient with no stops paints as solid black, matching `Gradient::evaluate`
	if samples.is_empty() {
		return peniko::ColorStops([stop(0., Color::BLACK)].into_iter().collect());
	}

	peniko::ColorStops(samples.iter().map(|&(position, color, _)| stop(position, color)).collect())
}

/// The gradient in its own space before the brush transform places it: the line from the origin along +X to 1 for linear, or the unit circle about the origin for radial.
fn unit_gradient_kind(gradient_form: GradientForm) -> peniko::GradientKind {
	match gradient_form {
		GradientForm::Linear => peniko::LinearGradientPosition {
			start: kurbo::Point::ORIGIN,
			end: kurbo::Point::new(1., 0.),
		}
		.into(),
		GradientForm::Radial => peniko::RadialGradientPosition {
			start_center: kurbo::Point::ORIGIN,
			start_radius: 0.,
			end_center: kurbo::Point::ORIGIN,
			end_radius: 1.,
		}
		.into(),
	}
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
	let (stops, settings) = painted_gradient(gradient_item)?;

	let gradient_form: GradientForm = gradient_item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
	let gradient_transform: DAffine2 = gradient_item.attribute_cloned_or_default(ATTR_TRANSFORM);

	let mut samples = spread_adjusted_samples(&stops, settings, gradient_form);

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
	let gradient_to_device = gradient_placement(multiplied_transform * gradient_transform, gradient_form);

	let brush = peniko::Brush::Gradient(peniko::Gradient {
		kind: unit_gradient_kind(gradient_form),
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
	/// A layer's own click targets in its space, which for a group leaves out its child layers.
	pub click_targets: HashMap<NodeId, Vec<Arc<ClickTarget>>>,
	/// Source-geometry outlines for hover/selection overlays, separate from `click_targets` so
	/// nodes with an `editor:click_target` override still outline the precise geometry.
	pub outlines: HashMap<NodeId, Vec<Arc<ClickTarget>>>,
	/// Group layers, whose child layers' targets the editor composes into theirs instead of `click_targets` holding them.
	pub group_layers: HashSet<NodeId>,
	/// Per-layer text frame from item 0's `editor:text_frame` attribute.
	/// The Text tool composes this with `transform_to_viewport(layer)` to position its drag cage.
	pub text_frames: HashMap<NodeId, DAffine2>,
	pub clip_targets: HashSet<NodeId>,
	/// Per-layer source geometry as the holder item, carrying the pool element and its paths attribute.
	#[cfg_attr(feature = "serde", serde(skip))]
	pub vector_data: HashMap<NodeId, Arc<Item<Vector>>>,
	/// Per-layer `ATTR_APPEARANCE` item attribute, exposed so message handlers can read it.
	#[cfg_attr(feature = "serde", serde(skip))]
	pub appearance_attributes: HashMap<NodeId, Arc<Appearance>>,
	/// Per-layer distance its effects paint past its content, in document pixels, for layers with effects.
	pub effects_reach: HashMap<NodeId, f64>,
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
			group_layers,
			text_frames,
			clip_targets,
			vector_data,
			appearance_attributes,
			effects_reach,
			backgrounds,
		} = self;
		upstream_footprints.extend(other.upstream_footprints.iter());
		local_transforms.extend(other.local_transforms.iter());
		first_element_source_id.extend(other.first_element_source_id.iter());
		click_targets.extend(other.click_targets.iter().map(|(k, v)| (*k, v.clone())));
		outlines.extend(other.outlines.iter().map(|(k, v)| (*k, v.clone())));
		group_layers.extend(other.group_layers.iter());
		text_frames.extend(other.text_frames.iter());
		clip_targets.extend(other.clip_targets.iter());
		vector_data.extend(other.vector_data.iter().map(|(id, data)| (*id, data.clone())));
		appearance_attributes.extend(other.appearance_attributes.iter().map(|(id, data)| (*id, data.clone())));
		effects_reach.extend(other.effects_reach.iter());

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
pub trait Render: BoundingBox {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams);

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, transform: DAffine2, _render_params: &RenderParams);

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

// SAFETY: The trait object borrows nothing, so it is already its own static type
unsafe impl dyn_any::StaticType for dyn Render + Send + Sync {
	type Static = dyn Render + Send + Sync;
}

/// Emits one item of graphic content as SVG, wrapped in a group carrying the item's transform, opacity, and blend mode.
/// `mask_state` carries the sibling clipping run between a list's items; a lone item has no siblings, so both mask inputs stay inert.
fn render_graphic_item_svg(item: ItemRef<'_, Graphic>, next_clips: bool, mask_state: &mut Option<(u64, MaskType)>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(element) = item.element() else { return };
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	// This item's declared appearance (if any) cascades to descendants lacking their own
	let child_render_params = item
		.attribute::<Appearance>(ATTR_APPEARANCE)
		.and_then(Appearance::declared)
		.map(|appearance| render_params.for_child_item(appearance));
	let render_params = child_render_params.as_ref().unwrap_or(render_params);
	let compositing = item.compositing(render_params);

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
		// The filter also covers how far the effects nested inside reach past the content, measured in this user space
		let document_scale = render.transform.matrix2.determinant().abs().sqrt();
		let filter = compositing.svg_filter(render, || match element.bounding_box(transform, true) {
			RenderBoundingBox::Rectangle([min, max]) => {
				let nested_reach = nested_effects_reach(element) / document_scale;
				RenderBoundingBox::Rectangle([min - nested_reach, max + nested_reach])
			}
			bounds => bounds,
		});
		let filtered = filter.is_some();

		render_svg_through_filter(render, filter, compositing.opacity, blend_mode, |render| {
			render.parent_tag(
				"g",
				|attributes| {
					if !matrix.is_empty() {
						attributes.push(ATTR_TRANSFORM, matrix.clone());
					}

					// A filter's wrapping group takes the opacity and blend mode instead
					if filtered {
						return;
					}

					let opacity = compositing.opacity_without_effects();
					if opacity < 1. {
						attributes.push("opacity", opacity.to_string());
					}

					if blend_mode != BlendMode::default() {
						attributes.push("style", blend_mode.render());
					}
				},
				|render| {
					let parent_transform = render.transform;
					render.transform = parent_transform * transform;
					element.render_svg(render, render_params);
					render.transform = parent_transform;
				},
			);
		});
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
	scene: &mut VelloSceneBuilder,
	transform: DAffine2,
	render_params: &RenderParams,
) {
	let Some(element) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let transform = transform * item_transform;
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	// This item's declared appearance (if any) cascades to descendants lacking their own
	let child_render_params = item
		.attribute::<Appearance>(ATTR_APPEARANCE)
		.and_then(Appearance::declared)
		.map(|appearance| render_params.for_child_item(appearance));
	let render_params = child_render_params.as_ref().unwrap_or(render_params);
	let compositing = item.compositing(render_params);

	let blend_mode = match render_params.render_mode {
		RenderMode::Outline => peniko::Mix::Normal,
		_ => blend_mode_attr.to_peniko(),
	};
	let needs_layer = compositing.opacity < 1. || (render_params.render_mode != RenderMode::Outline && blend_mode_attr != BlendMode::default());

	// The bounds are only worth finding for the layers or effects group that need them
	let bounds = if needs_layer || mask_element_and_transform.is_some() || compositing.effects.is_some() {
		element.bounding_box(transform, true)
	} else {
		RenderBoundingBox::None
	};
	let layer_rect = match bounds {
		RenderBoundingBox::Rectangle(bounds) => Some(inflate_bounds(bounds, compositing.reach())),
		_ => None,
	};

	let layer = if needs_layer && let Some(rect) = layer_rect {
		scene.push_layer(
			peniko::Fill::NonZero,
			peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver),
			compositing.opacity,
			kurbo::Affine::IDENTITY,
			&rect,
		);
		true
	} else {
		false
	};

	// The effects group also covers how far the effects nested inside reach past the content
	let effects_bounds = || match bounds {
		RenderBoundingBox::Rectangle(bounds) => rect_bounds(inflate_bounds(bounds, nested_effects_reach(element))),
		bounds => bounds,
	};
	let draw_content = |scene: &mut VelloSceneBuilder| {
		compositing.draw_with_effects(scene, effects_bounds, |scene| element.render_to_vello(scene, transform, render_params));
	};

	if next_clips && mask_element_and_transform.is_none() {
		*mask_element_and_transform = Some((element, transform));

		draw_content(scene);
	} else if let Some((mask_element, transform_mask)) = *mask_element_and_transform {
		if !next_clips {
			*mask_element_and_transform = None;
		}

		if let Some(rect) = layer_rect {
			scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., kurbo::Affine::IDENTITY, &rect);
			mask_element.render_to_vello(scene, transform_mask, &render_params.for_clipper());
			scene.push_layer(
				peniko::Fill::NonZero,
				peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcIn),
				1.,
				kurbo::Affine::IDENTITY,
				&rect,
			);
		}

		draw_content(scene);

		if layer_rect.is_some() {
			scene.pop_layer();
			scene.pop_layer();
		}
	} else {
		draw_content(scene);
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

	if let Some(layer) = item.layer() {
		let reach = item.attribute::<Effects>(ATTR_EFFECTS).map_or(0., Effects::reach) + nested_effects_reach(element);
		if reach > 0. {
			metadata.effects_reach.insert(layer, reach);
		}
	}

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
		let mut own_click_targets = Vec::new();
		let mut own_outlines = Vec::new();

		// Tagged items are child layers with targets of their own, which the editor composes into the group when queried
		for item in items.filter(|item| item.layer().is_none()) {
			collect_graphic_item_targets(item, inherited_appearance, &mut own_click_targets, &mut own_outlines);
		}

		metadata.click_targets.insert(element_id, own_click_targets.into_iter().map(|x| x.into()).collect());
		metadata.outlines.insert(element_id, own_outlines.into_iter().map(|x| x.into()).collect());
		metadata.group_layers.insert(element_id);
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
			Graphic::Generator(item) => render_generator_item_svg(ItemRef::Item(item), render, render_params),
			Graphic::Text(item) => render_text_item_svg(ItemRef::Item(item), render, render_params),
			Graphic::GraphicList(list) => list.render_svg(render, render_params),
			Graphic::VectorList(list) => list.render_svg(render, render_params),
			Graphic::RasterCPUList(list) => list.render_svg(render, render_params),
			Graphic::RasterGPUList(_) => (),
			Graphic::ColorList(list) => list.render_svg(render, render_params),
			Graphic::GradientList(list) => list.render_svg(render, render_params),
			Graphic::GeneratorList(list) => list.render_svg(render, render_params),
			Graphic::TextList(list) => list.render_svg(render, render_params),
			Graphic::StrokeList(_) => (),
		}
	}

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
		match self {
			Graphic::None(_) | Graphic::NoneList(_) => (),
			Graphic::Graphic(item) => render_graphic_item_to_vello(ItemRef::Item(item), false, &mut None, scene, transform, render_params),
			Graphic::Vector(item) => {
				// A paint subtree supplies its own styling, so an element's appearance must not cascade into it
				let paint_render_params = RenderParams {
					inherited_appearance: None,
					..render_params.clone()
				};
				render_vector_item_to_vello(ItemRef::Item(item), false, &mut None, scene, transform, render_params, &paint_render_params);
			}
			Graphic::RasterCPU(item) => render_raster_cpu_item_to_vello(ItemRef::Item(item), scene, transform, render_params),
			Graphic::RasterGPU(item) => render_raster_gpu_item_to_vello(ItemRef::Item(item), scene, transform, render_params),
			Graphic::Color(item) => render_color_item_to_vello(ItemRef::Item(item), scene, render_params),
			Graphic::Gradient(item) => render_gradient_item_to_vello(ItemRef::Item(item), scene, transform, render_params),
			Graphic::Generator(item) => render_generator_item_to_vello(ItemRef::Item(item), scene, transform, render_params),
			Graphic::Text(item) => render_text_item_to_vello(ItemRef::Item(item), scene, transform, render_params),
			Graphic::GraphicList(list) => list.render_to_vello(scene, transform, render_params),
			Graphic::VectorList(list) => list.render_to_vello(scene, transform, render_params),
			Graphic::RasterCPUList(list) => list.render_to_vello(scene, transform, render_params),
			Graphic::RasterGPUList(list) => list.render_to_vello(scene, transform, render_params),
			Graphic::ColorList(list) => list.render_to_vello(scene, transform, render_params),
			Graphic::GradientList(list) => list.render_to_vello(scene, transform, render_params),
			Graphic::GeneratorList(list) => list.render_to_vello(scene, transform, render_params),
			Graphic::TextList(list) => list.render_to_vello(scene, transform, render_params),
			Graphic::StrokeList(_) => (),
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
				Graphic::Generator(item) => first_item_inserts(metadata, item.attribute_cloned_or_default(ATTR_TRANSFORM)),
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
				Graphic::GeneratorList(list) => {
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
				Graphic::StrokeList(list) => {
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
			Graphic::Generator(_) => (),
			Graphic::Text(item) => collect_text_items_metadata(std::iter::once(ItemRef::Item(item)), metadata, footprint, element_id),
			Graphic::GraphicList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::VectorList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::RasterCPUList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::RasterGPUList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::ColorList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::GradientList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::GeneratorList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::TextList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
			Graphic::StrokeList(list) => list.collect_metadata(metadata, footprint, element_id, inherited_appearance),
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
			Graphic::Generator(_) => (),
			Graphic::Text(item) => add_text_item_click_targets(ItemRef::Item(item), click_targets),
			Graphic::GraphicList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::VectorList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::RasterCPUList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::RasterGPUList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::ColorList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::GradientList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::GeneratorList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::TextList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
			Graphic::StrokeList(list) => list.add_upstream_click_targets(click_targets, inherited_appearance),
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
			Graphic::Generator(_) => (),
			Graphic::Text(item) => add_text_item_click_targets(ItemRef::Item(item), outlines),
			Graphic::GraphicList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::VectorList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::RasterCPUList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::RasterGPUList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::ColorList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::GradientList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::GeneratorList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::TextList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
			Graphic::StrokeList(list) => list.add_upstream_outline_targets(outlines, inherited_appearance),
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
			Graphic::Vector(item) => {
				edit_item_geometry(item, |vector, paths| vector.vector_new_ids_from_hash(paths, reference.map(|id| id.0).unwrap_or_default()));
			}
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

					let parent_transform = render.transform;
					render.transform = parent_transform * DAffine2::from_translation(location);
					content.render_svg(render, &render_params);
					render.transform = parent_transform;
				},
			);
		}
	}

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
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
			content.render_to_vello(scene, child_transform, &render_params);
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

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
		let mut mask_element_and_transform = None;

		for index in 0..self.len() {
			let next_clips = index + 1 < self.len() && self.element(index + 1).unwrap().had_clip_enabled();
			render_graphic_item_to_vello(ItemRef::ListItem(self, index), next_clips, &mut mask_element_and_transform, scene, transform, render_params);
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
/// When `filtered`, the group applying the item's effects takes its opacity and blend mode instead.
fn render_vector_shape_svg(item: ItemRef<'_, Vector>, vector: &Vector, filtered: bool, folded: Option<&FoldedPaints>, render: &mut SvgRender, render_params: &RenderParams) {
	let paths = item.attribute::<List<Path>>(ATTR_PATHS).unwrap_or_else(|| no_paths());
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode_attr: BlendMode = if filtered { BlendMode::default() } else { item.attribute_cloned_or_default(ATTR_BLEND_MODE) };
	let opacity = if filtered { 1. } else { item.compositing(render_params).opacity_without_effects() };

	// The item's own declared appearance wins over one cascading down from an ancestor
	let own_appearance = item.attribute::<Appearance>(ATTR_APPEARANCE).and_then(Appearance::declared);
	let appearance = own_appearance.or(render_params.inherited_appearance.as_ref());
	let FillAndStroke {
		stroke: stroke_params,
		fill_paint,
		stroke_paint,
		stroke_below: wants_stroke_below,
	} = appearance.map(Appearance::fill_and_stroke).unwrap_or_default();
	let (fill_paint, stroke_paint) = match folded {
		Some(folded) => (folded.fill.as_ref(), folded.stroke.as_ref()),
		None => (fill_paint, stroke_paint),
	};

	// Only consider strokes with nonzero weight, since default strokes with zero weight would prevent assigning the correct stroke transform
	let has_real_stroke = stroke_params.as_ref().filter(|stroke| stroke.weight() > 0.);
	// A cascaded coverage records its stroke space in the ancestor's coordinates, so this item authors its own
	let set_stroke_transform = has_real_stroke
		.map(|stroke| if own_appearance.is_some() { stroke.transform } else { item_transform })
		.filter(|transform| transform_is_invertible(*transform));
	let applied_stroke_transform = set_stroke_transform.unwrap_or(item_transform);
	let applied_stroke_transform = render_params.alignment_parent_transform.unwrap_or(applied_stroke_transform);
	let element_transform = set_stroke_transform.map(|stroke_transform| item_transform * stroke_transform.inverse());
	let element_transform = element_transform.unwrap_or(DAffine2::IDENTITY);
	let contours = item.contours();
	let layer_bounds = contours_bounding_box(&contours, DAffine2::IDENTITY).unwrap_or_default();
	let transformed_bounds = contours_bounding_box(&contours, applied_stroke_transform).unwrap_or_default();
	let stroke_layer_bounds = contours_stroke_inclusive_bounding_box(&contours, DAffine2::IDENTITY, stroke_params.as_ref()).unwrap_or(layer_bounds);

	let bounds_matrix = DAffine2::from_scale_angle_translation(layer_bounds[1] - layer_bounds[0], 0., layer_bounds[0]);
	let stroke_bounds_matrix = DAffine2::from_scale_angle_translation(stroke_layer_bounds[1] - stroke_layer_bounds[0], 0., stroke_layer_bounds[0]);

	let mut path = String::new();

	let stroke_affine = Affine::new(applied_stroke_transform.to_cols_array());
	for bezpath in contours.iter() {
		path.push_str((stroke_affine * bezpath).to_svg().as_str());
	}

	let mask_type = if stroke_params.as_ref().map(|stroke| stroke.align) == Some(StrokeAlign::Inside) {
		MaskType::Clip
	} else {
		MaskType::Mask
	};

	let path_is_closed = contours.iter().all(|path| matches!(path.elements().last(), Some(PathEl::ClosePath)));
	let can_draw_aligned_stroke = path_is_closed
		&& stroke_params.as_ref().is_some_and(|stroke| stroke.has_renderable_stroke() && stroke.align.is_not_centered())
		&& stroke_paint.is_some_and(|graphic| !graphic.is_guaranteed_fully_transparent());
	let can_use_paint_order = !(fill_paint.is_none_or(|graphic| !graphic.is_guaranteed_to_cover_opaquely()) || mask_type == MaskType::Clip);

	let needs_separate_alignment_fill = can_draw_aligned_stroke && !can_use_paint_order;
	let override_paint_order = can_draw_aligned_stroke && can_use_paint_order;
	let use_face_fill = vector.use_face_fill(paths);

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
		let mut mask_item = Item::new_from_element(cloned_vector)
			.with_attribute(ATTR_PATHS, paths.clone())
			.with_attribute(ATTR_GEOMETRY_CACHE, item.geometry_cache())
			.with_attribute(ATTR_TRANSFORM, item_transform);
		let black_fill = Graphic::ColorList(List::new_from_element(Color::BLACK));
		mask_item.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), black_fill));
		let vector_item = List::new_from_item(mask_item);

		(id, mask_type, vector_item)
	});

	if use_face_fill {
		for face_path in item.faces().iter() {
			let face_d = (stroke_affine * face_path).to_svg();

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
		// The shadows and glows the masker casts don't widen the area its clipped content shows in
		let mut masker = Item::from_parts(vector.clone(), item.clone_item_attributes());
		masker.remove_attribute::<Effects>(ATTR_EFFECTS);
		let masker = Graphic::VectorList(List::new_from_item(masker));
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

	let compositing = item.compositing(render_params);
	let appearance = || {
		item.attribute::<Appearance>(ATTR_APPEARANCE)
			.and_then(Appearance::declared)
			.or(render_params.inherited_appearance.as_ref())
	};

	// Solid paints take leading adjustments into their colors, which SVG filters can't always express
	let folded = compositing.effects.and_then(|_| {
		let fill_and_stroke = appearance()?.fill_and_stroke();
		let strokes = fill_and_stroke.stroke.as_ref().is_some_and(|stroke| stroke.weight() > 0.);
		compositing.fold_into_solid_paints(fill_and_stroke.fill_paint, fill_and_stroke.stroke_paint, strokes, render_params.for_mask)
	});
	let compositing = match &folded {
		Some(folded) => compositing.with_remaining_effects(&folded.remaining_effects),
		None => compositing,
	};

	let render_shape = |render: &mut SvgRender| {
		let filter = compositing.svg_filter(render, || {
			let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
			let stroke = appearance().and_then(|appearance| appearance.fill_and_stroke().stroke);
			let paths = item.attribute::<List<Path>>(ATTR_PATHS).unwrap_or_else(|| no_paths());

			vector
				.stroke_inclusive_bounding_box_with_transform(paths, item_transform, stroke.as_ref())
				.map_or(RenderBoundingBox::None, RenderBoundingBox::Rectangle)
		});
		let filtered = filter.is_some();
		let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);

		render_svg_through_filter(render, filter, compositing.opacity, blend_mode, |render| {
			render_vector_shape_svg(item, vector, filtered, folded.as_ref(), render, render_params)
		});
	};

	// Item geometry is baked into the path data instead of a group transform, so mask coordinates line up
	match masked_by {
		Some((attribute, selector)) => render.parent_tag("g", |attributes| attributes.push(attribute, selector), render_shape),
		None => render_shape(render),
	}
}

/// Whether `bounds` placed by `transform` overlaps the scene once widened by `inflation`, measured in the space `inflation_transform` maps from, and by its effects' `reach`, measured in the scene's space.
fn overlaps_scene(scene: &VelloSceneBuilder, bounds: [DVec2; 2], transform: DAffine2, inflation: f64, inflation_transform: DAffine2, reach: f64) -> bool {
	let view = DAffine2::from_cols_array(&scene.view().as_coeffs());
	let (largest_scale, _) = singular_values(view * inflation_transform);
	let (largest_view_scale, _) = singular_values(view);
	let inflation = inflation * largest_scale + reach * largest_view_scale;
	let [min, max] = (view * transform * Quad::from_box(bounds)).bounding_box();

	min.x - inflation < scene.width() as f64 && max.x + inflation > 0. && min.y - inflation < scene.height() as f64 && max.y + inflation > 0.
}

/// Draws one item of vector content into the Vello scene: fill and stroke paints, blend and opacity layering,
/// stroke alignment compositing, and the sibling clipping run carried in `clip_masker` (inert for a lone item).
#[allow(clippy::too_many_arguments)]
fn render_vector_item_to_vello(
	item: ItemRef<'_, Vector>,
	next_clips: bool,
	clip_masker: &mut Option<List<Vector>>,
	scene: &mut VelloSceneBuilder,
	parent_transform: DAffine2,
	render_params: &RenderParams,
	paint_render_params: &RenderParams,
) {
	let Some(element) = item.element() else { return };
	let element_paths = item.attribute::<List<Path>>(ATTR_PATHS).unwrap_or_else(|| no_paths());
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let compositing = item.compositing(render_params);
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

	// Solid paints take leading adjustments into their colors rather than through an effects group
	let folded = compositing.fold_into_solid_paints(fill_paint, stroke_paint, has_real_stroke.is_some(), render_params.for_mask);
	let (fill_paint, stroke_paint, compositing) = match &folded {
		Some(folded) => (folded.fill.as_ref(), folded.stroke.as_ref(), compositing.with_remaining_effects(&folded.remaining_effects)),
		None => (fill_paint, stroke_paint, compositing),
	};

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
	let contours = item.contours();
	let layer_bounds = item.contours_bounding_box().unwrap_or_default();
	let stroke_affine = Affine::new(applied_stroke_transform.to_cols_array());
	let element_affine = Affine::new(element_transform.to_cols_array());

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
	let can_draw_aligned_stroke =
		!stroke_fully_transparent && stroke.is_some_and(|s| s.has_renderable_stroke() && s.align.is_not_centered()) && contours.iter().all(|p| matches!(p.elements().last(), Some(PathEl::ClosePath)));

	// Vello only culls after expanding strokes and flattening paths, so an item outside the scene is skipped before any of that work
	let stroke_inflation = stroke.map_or(0., |stroke| stroke.max_aabb_inflation(true)).max(get_outline_styles(render_params).0.width);
	if !overlaps_scene(scene, layer_bounds, multiplied_transform, stroke_inflation, element_transform, compositing.reach()) {
		if next_clips && clip_masker.is_none() {
			*clip_masker = Some(List::new_from_item(Item::from_parts(element.clone(), item.clone_item_attributes())));
		} else if clip_masker.is_some() && !next_clips {
			*clip_masker = None;
		}
		return;
	}

	let Some(placed) = item.placed_contours(applied_stroke_transform) else { return };
	let path = placed.path();

	let opacity = compositing.opacity;
	let needs_blend_layer = opacity < 1. || blend_mode_attr != BlendMode::default();

	// Shared by the layers and effects group below, so it is only worth deriving when one of them is pushed
	let layer_geometry = (needs_blend_layer || clip_masker.is_some() || compositing.effects.is_some()).then(|| {
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

	// The layers also cover what the effects paint past the content, which reaches a length in the scene's space
	let effects_inflation = match singular_values(multiplied_transform) {
		(_, smallest_scale) if smallest_scale > 0. => compositing.reach() / smallest_scale,
		_ => 0.,
	};
	let layer_rect_with_effects = |layer_rect: kurbo::Rect| layer_rect.inflate(effects_inflation, effects_inflation);

	if needs_blend_layer && let Some((layer_affine, layer_rect)) = layer_geometry {
		layer = true;
		scene.push_layer(
			peniko::Fill::NonZero,
			peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver),
			opacity,
			layer_affine,
			&layer_rect_with_effects(layer_rect),
		);
	}

	// Pushed inside the blend layer so the mask cuts this item's own paint rather than the composited result
	let mut clip_layers = false;
	if next_clips && clip_masker.is_none() {
		// The shadows and glows the masker casts don't widen the area its clipped content shows in
		let mut masker = Item::from_parts(element.clone(), item.clone_item_attributes());
		masker.remove_attribute::<Effects>(ATTR_EFFECTS);
		*clip_masker = Some(List::new_from_item(masker));
	} else if let Some(masker) = clip_masker.as_ref() {
		if let Some((layer_affine, layer_rect)) = layer_geometry {
			let layer_rect = layer_rect_with_effects(layer_rect);

			scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., layer_affine, &layer_rect);
			masker.render_to_vello(scene, parent_transform, &render_params.for_clipper());
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

	let do_fill_path = |scene: &mut VelloSceneBuilder, path: &kurbo::BezPath, fill_rule: peniko::Fill| {
		let Some(paint) = fill_paint else { return };

		let solid_fill = |scene: &mut VelloSceneBuilder, color: Option<Color>| {
			let Some(color) = color else { return };

			let fill = peniko::Brush::Solid(SRGBA8::from(color).to_peniko_color());
			scene.fill_path(fill_rule, element_affine, &fill, None, path);
		};
		let gradient_fill = |scene: &mut VelloSceneBuilder, gradient_item: ItemRef<'_, Gradient>| {
			let Some((brush, gradient_to_device)) = create_peniko_gradient_brush(gradient_item, &multiplied_transform, render_params.for_mask) else {
				return;
			};

			let inverse_element_transform = if transform_is_invertible(element_transform) {
				element_transform.inverse()
			} else {
				Default::default()
			};
			let brush_transform = kurbo::Affine::new((inverse_element_transform * gradient_to_device).to_cols_array());
			scene.fill_path(fill_rule, element_affine, &brush, Some(brush_transform), path);
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
			| Graphic::Generator(_)
			| Graphic::Text(_)
			| Graphic::VectorList(_)
			| Graphic::RasterCPUList(_)
			| Graphic::RasterGPUList(_)
			| Graphic::GeneratorList(_)
			| Graphic::GraphicList(_)
			| Graphic::TextList(_)
			| Graphic::StrokeList(_) => {
				scene.push_clip_layer(fill_rule, kurbo::Affine::new(element_transform.to_cols_array()), path);
				paint.render_to_vello(scene, multiplied_transform, paint_render_params);
				scene.pop_layer();
			}
		};
	};

	let use_face_fill = element.use_face_fill(element_paths);
	let do_fill = |scene: &mut VelloSceneBuilder| {
		if use_face_fill {
			for face_path in item.faces().iter() {
				do_fill_path(scene, &(stroke_affine * face_path), peniko::Fill::NonZero);
			}
		} else {
			do_fill_path(scene, path, peniko::Fill::NonZero);
		}
	};

	let do_stroke = |scene: &mut VelloSceneBuilder, width_scale: f64| {
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

		let solid_stroke = |scene: &mut VelloSceneBuilder, color: Option<Color>| {
			let Some(color) = color else { return };

			let brush = peniko::Brush::Solid(SRGBA8::from(color).to_peniko_color());

			let outline = placed.stroke_outline(&stroke, scene.stroke_tolerance(element_affine));
			scene.fill_path(peniko::Fill::NonZero, element_affine, &brush, None, &outline);
		};
		let gradient_stroke = |scene: &mut VelloSceneBuilder, gradient_item: ItemRef<'_, Gradient>| {
			let Some((brush, gradient_to_device)) = create_peniko_gradient_brush(gradient_item, &multiplied_transform, render_params.for_mask) else {
				return;
			};
			let inverse_element_transform = if transform_is_invertible(element_transform) {
				element_transform.inverse()
			} else {
				Default::default()
			};
			let brush_transform = kurbo::Affine::new((inverse_element_transform * gradient_to_device).to_cols_array());

			let outline = placed.stroke_outline(&stroke, scene.stroke_tolerance(element_affine));
			scene.fill_path(peniko::Fill::NonZero, element_affine, &brush, Some(brush_transform), &outline);
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
			| Graphic::Generator(_)
			| Graphic::Text(_)
			| Graphic::VectorList(_)
			| Graphic::RasterCPUList(_)
			| Graphic::RasterGPUList(_)
			| Graphic::GeneratorList(_)
			| Graphic::GraphicList(_)
			| Graphic::TextList(_)
			| Graphic::StrokeList(_) => {
				let stroked = placed.stroke_outline(&stroke, 0.01);

				scene.push_clip_layer(peniko::Fill::NonZero, element_affine, &*stroked);
				paint.render_to_vello(scene, multiplied_transform, paint_render_params);
				scene.pop_layer();
			}
		};
	};

	// Render the path
	match render_params.render_mode {
		RenderMode::Outline => {
			let (outline_stroke, outline_color_peniko) = get_outline_styles(render_params);

			let outline = placed.stroke_outline(&outline_stroke, scene.stroke_tolerance(element_affine));
			scene.fill_path(peniko::Fill::NonZero, element_affine, outline_color_peniko, None, &outline);
		}
		_ => {
			let effects_bounds = || layer_geometry.map_or(RenderBoundingBox::None, |(layer_affine, layer_rect)| rect_bounds(layer_affine.transform_rect_bbox(layer_rect)));
			compositing.draw_with_effects(scene, effects_bounds, |scene| {
				if use_layer {
					let cloned_element = element.clone();

					// The mask must draw at full alpha so `SrcOut` fully zeroes the path interior.
					// The outer opacity/blend layer (above) handles the user-set opacity.
					let mut mask_item = Item::new_from_element(cloned_element)
						.with_attribute(ATTR_PATHS, element_paths.clone())
						.with_attribute(ATTR_GEOMETRY_CACHE, item.geometry_cache())
						.with_attribute(ATTR_TRANSFORM, item_transform);
					let black_fill = Graphic::ColorList(List::new_from_element(Color::BLACK));
					mask_item.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), black_fill));
					let vector_list = List::new_from_item(mask_item);

					let bounds = contours_bounding_box(&contours, multiplied_transform).unwrap_or(layer_bounds);
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
						vector_list.render_to_vello(scene, parent_transform, &render_params.for_alignment(applied_stroke_transform));
						scene.push_layer(peniko::Fill::NonZero, peniko::BlendMode::new(peniko::Mix::Normal, compose), 1., kurbo::Affine::IDENTITY, &rect);

						do_stroke(scene, 2.);

						scene.pop_layer();
						scene.pop_layer();

						do_fill(scene);
					} else {
						// Fill first (unclipped), then stroke (clipped) above
						do_fill(scene);

						scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., kurbo::Affine::IDENTITY, &rect);
						vector_list.render_to_vello(scene, parent_transform, &render_params.for_alignment(applied_stroke_transform));
						scene.push_layer(peniko::Fill::NonZero, peniko::BlendMode::new(peniko::Mix::Normal, compose), 1., kurbo::Affine::IDENTITY, &rect);

						do_stroke(scene, 2.);

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
							Op::Fill => do_fill(scene),
							Op::Stroke => do_stroke(scene, 1.),
						}
					}
				}
			});
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

			let source_paths = item.attribute::<List<Path>>(ATTR_PATHS).unwrap_or_else(|| no_paths());
			let item_relative_transform = reference_inverse * transform;

			// Outlines trace the source geometry as drawn (actual letterforms, open paths left open)
			let mut outlines_unwrapped = Vec::new();
			extend_targets_from_vector(&mut outlines_unwrapped, appearance, item, item_relative_transform, false);
			let outlines: Vec<Arc<ClickTarget>> = outlines_unwrapped.into_iter().map(Arc::new).collect();

			// Use click-target override if the item provides one (e.g. 'Text' node's per-glyph bboxes), otherwise the click targets are the outlines, closed where filled
			let click_targets = match item.attribute::<BezPath>(ATTR_EDITOR_CLICK_TARGET) {
				Some(override_path) => {
					let mut click_targets_unwrapped = Vec::new();
					extend_targets_from_bezpaths(&mut click_targets_unwrapped, appearance, &split_bezpath_contours(override_path), item_relative_transform);
					click_targets_unwrapped.into_iter().map(Arc::new).collect()
				}
				None if is_filled_for_hit_testing(appearance) => {
					let mut click_targets_unwrapped = Vec::new();
					extend_targets_from_vector(&mut click_targets_unwrapped, appearance, item, item_relative_transform, true);
					click_targets_unwrapped.into_iter().map(Arc::new).collect()
				}
				None => outlines.clone(),
			};

			accumulated_click_targets.entry(element_id).or_default().extend(click_targets);
			accumulated_outlines.entry(element_id).or_default().extend(outlines);

			// Source geometry (not the click-target override) so editing tools work on letterforms.
			// Recorded together with `vector_data` from the same (first) item so stroke geometry stays consistent with the paint.
			// Only item 0 is recorded since editing tools can only target a single item currently.
			// If that item has no paint attribute, none is recorded.
			if let std::collections::hash_map::Entry::Vacant(e) = metadata.vector_data.entry(element_id) {
				e.insert(Arc::new(Item::new_from_element(source.clone()).with_attribute(ATTR_PATHS, source_paths.clone())));

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
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let appearance = Appearance::cascade(item.attribute::<Appearance>(ATTR_APPEARANCE), inherited_appearance);

	// Use click-target override geometry if the item provides one (e.g. 'Text' node's per-glyph bounding boxes)
	match item.attribute::<BezPath>(ATTR_EDITOR_CLICK_TARGET) {
		Some(override_path) => extend_targets_from_bezpaths(click_targets, appearance, &split_bezpath_contours(override_path), transform),
		None => extend_targets_from_vector(click_targets, appearance, item, transform, true),
	}
}

/// Like [`add_vector_item_click_targets`] but tracing the source geometry as drawn, without `editor:click_target` or closing open paths.
fn add_vector_item_outline_targets(item: ItemRef<'_, Vector>, outlines: &mut Vec<ClickTarget>, inherited_appearance: Option<&Appearance>) {
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let appearance = Appearance::cascade(item.attribute::<Appearance>(ATTR_APPEARANCE), inherited_appearance);

	extend_targets_from_vector(outlines, appearance, item, transform, false);
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

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, parent_transform: DAffine2, render_params: &RenderParams) {
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
		for index in 0..self.len() {
			edit_list_row_geometry(self, index, |vector, paths| vector.vector_new_ids_from_hash(paths, reference.map(|id| id.0).unwrap_or_default()));
		}
	}
}

/// Append one multi-contour target (nonzero fill rule, so holes like the inside of an "O" work correctly)
/// plus one `FreePoint` per disconnected anchor, placed by the transform.
/// The target's path is shared through the item's geometry cache with every copy drawing the same geometry.
fn extend_targets_from_vector(targets: &mut Vec<ClickTarget>, appearance: Option<&Appearance>, item: ItemRef<'_, Vector>, transform: DAffine2, close_filled_contours: bool) {
	// Click targets cover a filled layer's fill so a click anywhere on it hits
	let target_path = item.target_path(close_filled_contours && is_filled_for_hit_testing(appearance));
	push_path_target(targets, appearance, target_path, &item.contours(), transform);

	let Some(geometry) = item.element() else { return };
	for click_target in extend_free_point_targets(geometry, item.paths(), transform) {
		targets.push(click_target);
	}
}

/// The contour half of [`extend_targets_from_vector`] for `editor:click_target` overrides, which have no geometry cache.
fn extend_targets_from_bezpaths(targets: &mut Vec<ClickTarget>, appearance: Option<&Appearance>, bezpaths: &[BezPath], transform: DAffine2) {
	let target_path = TargetPath::from_contours(bezpaths, is_filled_for_hit_testing(appearance)).map(Arc::new);
	push_path_target(targets, appearance, target_path, bezpaths, transform);
}

/// A coverage whose paint is `Graphic::None` exists but paints nothing, so it does not close subpaths for hit testing.
fn is_filled_for_hit_testing(appearance: Option<&Appearance>) -> bool {
	appearance.is_some_and(|appearance| appearance.has_painted_cover(Cover::Fill))
}

/// Appends a target placing `target_path` (combined from `contours`) by `transform`, with the appearance's stroke as its click inflation.
fn push_path_target(targets: &mut Vec<ClickTarget>, appearance: Option<&Appearance>, target_path: Option<Arc<TargetPath>>, contours: &[BezPath], transform: DAffine2) {
	let Some(target_path) = target_path else { return };

	let all_contours_closed = contours
		.iter()
		.filter(|bezpath| !bezpath.elements().is_empty())
		.all(|bezpath| matches!(bezpath.elements().last(), Some(PathEl::ClosePath)));

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

	targets.push(ClickTarget::new_with_shared_path(target_path, transform, stroke_width));
}

/// Splits a compound bezpath into its contours, so per-contour closing behaves as with source geometry.
fn split_bezpath_contours(bezpath: &BezPath) -> Vec<BezPath> {
	let mut contours = Vec::new();

	for &element in bezpath.elements() {
		if matches!(element, PathEl::MoveTo(_)) || contours.is_empty() {
			contours.push(BezPath::new());
		}
		if let Some(contour) = contours.last_mut() {
			contour.push(element);
		}
	}

	contours
}

fn extend_free_point_targets<'a>(vector: &'a Vector, geometry_paths: &List<Path>, transform: DAffine2) -> impl Iterator<Item = ClickTarget> + 'a {
	// A pool point that no path references is a free point, since handles and anchors are all referenced
	let mut referenced = vec![false; vector.point_count()];
	for path in geometry_paths.iter_element_values() {
		for point in path.references() {
			if let Some(slot) = referenced.get_mut(point as usize) {
				*slot = true;
			}
		}
	}

	(0..vector.point_count() as u32).filter_map(move |point_index| {
		if referenced[point_index as usize] {
			return None;
		}

		let point_id = vector.point_id_at(point_index).unwrap_or_default();
		let anchor = vector.position(point_index as usize).unwrap_or_default();
		let mut click_target = ClickTarget::new_with_free_point(FreePoint::new(point_id, anchor));
		click_target.apply_transform(transform);
		Some(click_target)
	})
}

/// The most times an image drawn to a canvas is halved, which is when 4096 of its pixels fit on one screen pixel.
const MAX_CANVAS_HALVINGS: u32 = 12;

/// The most pixels an image may have for its adjustments to be carried out on a copy of its texture, past which an effects group carries them out at the render's resolution.
const MAX_ADJUSTED_TEXTURE_PIXELS: u64 = 4096 * 4096;

/// An image the frontend draws to a canvas rather than an SVG embedding it, at a resolution to suit how large it appears.
#[derive(Clone, Debug, PartialEq, CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CanvasImage {
	pub raster: Raster<CPU>,
	/// How many times the pixels are halved along each axis for display.
	pub halvings: u32,
}

/// Names the canvas for one resolution of an image's pixels, staying the same while the pixels do.
fn canvas_image_id(buffer_id: u64, halvings: u32) -> u64 {
	(buffer_id << 4) | halvings as u64
}

/// Emits one item of CPU raster content as SVG, as a canvas placeholder or an embedded base64 image.
fn render_raster_cpu_item_svg(item: ItemRef<'_, Raster<CPU>>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(image) = item.element() else { return };

	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let compositing = item.compositing(render_params);

	if image.data.is_empty() {
		return;
	}

	// SVG filters can't carry out every adjustment, so those leading the effects are carried out in the image's pixels
	let mut remaining_effects = Effects::default();
	let (leading_adjustments, compositing) = compositing.split_leading_adjustments(&mut remaining_effects);
	let adjusted_image;
	let image = match leading_adjustments.is_empty() {
		true => image,
		false => {
			adjusted_image = adjusted_raster(image, &leading_adjustments);
			&adjusted_image
		}
	};

	let filter = compositing.svg_filter(render, || RenderBoundingBox::Rectangle((transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box()));

	// A filter's wrapping group takes the opacity and blend mode instead
	let (opacity, blend_mode) = match filter {
		Some(_) => (1., BlendMode::default()),
		None => (compositing.opacity_without_effects(), blend_mode_attr),
	};

	render_svg_through_filter(render, filter, compositing.opacity, blend_mode_attr, |render| {
		render_raster_cpu_image_svg(image, transform, opacity, blend_mode, render, render_params)
	});
}

/// Emits a CPU raster's image as SVG, either as a placeholder for a canvas or as an embedded PNG.
fn render_raster_cpu_image_svg(image: &Raster<CPU>, transform: DAffine2, opacity: f32, blend_mode: BlendMode, render: &mut SvgRender, render_params: &RenderParams) {
	if render_params.to_canvas() {
		// An image drawn smaller than its pixels is sent at a resolution halved enough times to still cover each screen pixel
		let document_to_screen = DAffine2::from_scale(DVec2::splat(render_params.canvas_image_scale));
		let image_to_screen = document_to_screen * render.transform * transform * DAffine2::from_scale(1. / DVec2::new(image.width as f64, image.height as f64));
		let (largest_scale, _) = singular_values(image_to_screen);
		let halvings = if largest_scale > 0. {
			(1. / largest_scale).log2().floor().clamp(0., MAX_CANVAS_HALVINGS as f64) as u32
		} else {
			0
		};

		let id = canvas_image_id(image.buffer_id(), halvings);
		render.image_data.entry(id).or_insert_with(|| CanvasImage { raster: image.clone(), halvings });

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

				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}

				if blend_mode != BlendMode::default() {
					attributes.push("style", blend_mode.render());
				}
			},
			|render| {
				render.leaf_tag(
					"img", // Must be a self-closing (void element) tag, so we can't use `div` or `span`, for example
					|attributes| {
						// Prefixed to stay apart from the numbered thumbnail and viewport canvases on `window.imageCanvases`
						attributes.push("data-canvas-placeholder", format!("image-{id}"));
					},
				)
			},
		);
	} else {
		render.leaf_tag("image", |attributes| {
			attributes.push("width", "1");
			attributes.push("height", "1");
			attributes.push("preserveAspectRatio", "none");
			attributes.push("href", image.png_data_url().to_string());
			let matrix = format_transform_matrix(transform);
			if !matrix.is_empty() {
				attributes.push(ATTR_TRANSFORM, matrix);
			}

			if opacity < 1. {
				attributes.push("opacity", opacity.to_string());
			}
			if blend_mode != BlendMode::default() {
				attributes.push("style", blend_mode.render());
			}
		});
	}
}

/// Draws one item of CPU raster content into the Vello scene.
fn render_raster_cpu_item_to_vello(item: ItemRef<'_, Raster<CPU>>, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
	let Some(image) = item.element() else { return };
	if image.data.is_empty() {
		return;
	}

	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let compositing = item.compositing(render_params);
	let blend_mode = blend_mode_attr.to_peniko();

	// Leading adjustments are carried out on the image's texture as it's drawn, rather than on a render of it in an effects group
	let mut remaining_effects = Effects::default();
	let (leading_adjustments, split_compositing) = compositing.split_leading_adjustments(&mut remaining_effects);
	let changes_alpha = leading_adjustments.iter().any(|adjustment| !raster_kernels::adjustments::preserves_alpha(adjustment));
	let fits_texture_copy = image.width as u64 * image.height as u64 <= MAX_ADJUSTED_TEXTURE_PIXELS;
	let adjusted_image;
	let (image, adjustments, compositing) = if leading_adjustments.is_empty() || !(changes_alpha || fits_texture_copy) {
		(image, Vec::new(), compositing)
	} else if changes_alpha {
		// A premultiplied texture keeps no color where alpha is zero, so adjustments changing alpha are carried out on the CPU like in SVG
		adjusted_image = adjusted_raster(image, &leading_adjustments);
		(&adjusted_image, Vec::new(), split_compositing)
	} else {
		(image, leading_adjustments, split_compositing)
	};

	let opacity = compositing.opacity;
	let mut layer = false;

	let whole_bounds = || match item {
		ItemRef::ListItem(list, _) => list.bounding_box(transform, false),
		ItemRef::Item(item) => item.bounding_box(transform, false),
	};
	if (opacity < 1. || (render_params.render_mode != RenderMode::Outline && blend_mode_attr != BlendMode::default()))
		&& let RenderBoundingBox::Rectangle(bounds) = whole_bounds()
	{
		let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
		let rect = inflate_bounds(bounds, compositing.reach());
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
	let sampler = peniko::ImageSampler::new().with_extend(peniko::Extend::Pad);

	let item_bounds = || RenderBoundingBox::Rectangle((transform * transform_attribute * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box());
	compositing.draw_with_effects(scene, item_bounds, |scene| {
		scene.draw_raster(image.clone(), adjustments.clone(), sampler, kurbo::Affine::new(image_transform.to_cols_array()))
	});

	if layer {
		scene.pop_layer();
	}
}

/// The most pixels across or down an SVG's image of a generator may have.
const MAX_GENERATOR_IMAGE_PIXELS: f64 = 8192.;

/// The most pixels across or down a thumbnail's image of a generator may have, whatever the size of the area it shows.
const MAX_GENERATOR_THUMBNAIL_PIXELS: f64 = 256.;

/// Emits one item of generator content as SVG, as an image of the part in view at the view's resolution.
fn render_generator_item_svg(item: ItemRef<'_, Generator>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(&generator) = item.element() else { return };
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let Some((area, width, height)) = generator_area_in_view(transform, render, render_params) else {
		return;
	};

	// An image of the area, carrying the generator's attributes, stands in for it
	let pattern = GeneratorPattern::decode(generator, &|key| item.attribute_any(key));
	let pixels = match raster_kernels::generators::render_pattern(&pattern, area, width, height) {
		Ok(pixels) => pixels,
		Err(error) => {
			log::error!("Failed to render a generator as an SVG image: {error}");
			return;
		}
	};
	let mut image = Item::from_parts(Raster::new_cpu(pixels), item.clone_item_attributes());
	image.set_attribute(ATTR_TRANSFORM, transform * DAffine2::from_translation(area[0]) * DAffine2::from_scale(area[1] - area[0]));
	render_raster_cpu_item_svg(ItemRef::Item(&image), render, render_params);
}

/// The part of a generator's own space in view, which `transform` places, with the pixels across and down it covers in the view.
fn generator_area_in_view(transform: DAffine2, render: &SvgRender, render_params: &RenderParams) -> Option<([DVec2; 2], u32, u32)> {
	let footprint = render_params.footprint;

	// A thumbnail's footprint starts at the document area it shows, one pixel per unit, rather than mapping the document into a view
	let (document_to_view, max_pixels) = match render_params.thumbnail {
		true => (DAffine2::from_translation(-footprint.transform.translation), MAX_GENERATOR_THUMBNAIL_PIXELS),
		false => (footprint.transform, MAX_GENERATOR_IMAGE_PIXELS),
	};
	let local_to_view = document_to_view * render.transform * transform;
	if local_to_view.matrix2.determinant() == 0. || !local_to_view.is_finite() || footprint.resolution.min_element() <= 1 {
		return None;
	}

	let view = Quad::from_box([DVec2::ZERO, footprint.resolution.as_dvec2()]);
	let [min, max] = (local_to_view.inverse() * view).bounding_box();

	let pixels_per_unit = DVec2::new(local_to_view.matrix2.x_axis.length(), local_to_view.matrix2.y_axis.length());
	let pixels = ((max - min) * pixels_per_unit).ceil().clamp(DVec2::ONE, DVec2::splat(max_pixels));
	Some(([min, max], pixels.x as u32, pixels.y as u32))
}

/// Draws one item of generator content into the Vello scene, which renders its pattern for each render at the resolution it needs.
fn render_generator_item_to_vello(item: ItemRef<'_, Generator>, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
	let Some(&generator) = item.element() else { return };

	// Outline mode shows only geometry, and a generator filling the plane has none
	if let RenderMode::Outline = render_params.render_mode {
		return;
	}

	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let compositing = item.compositing(render_params);
	let local_to_scene = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
	let bounds = generator.bounding_box(local_to_scene, false);
	if bounds == RenderBoundingBox::None {
		return;
	}

	// Opacity and blend modes composite through a layer covering the whole render
	let mut layer = false;
	if compositing.opacity < 1. || blend_mode_attr != BlendMode::default() {
		let blending = peniko::BlendMode::new(blend_mode_attr.to_peniko(), peniko::Compose::SrcOver);
		scene.push_viewport_layer(blending, compositing.opacity);
		layer = true;
	}

	let pattern = GeneratorPattern::decode(generator, &|key| item.attribute_any(key));
	compositing.draw_with_effects(scene, || bounds, |scene| scene.draw_generator(&pattern, kurbo::Affine::new(local_to_scene.to_cols_array())));

	if layer {
		scene.pop_layer();
	}
}

impl Render for List<Generator> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_generator_item_svg(ItemRef::ListItem(self, index), render, render_params);
		}
	}

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_generator_item_to_vello(ItemRef::ListItem(self, index), scene, transform, render_params);
		}
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

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
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

impl Render for List<Raster<GPU>> {
	fn render_svg(&self, _render: &mut SvgRender, _render_params: &RenderParams) {
		log::warn!("tried to render texture as an svg");
	}

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_raster_gpu_item_to_vello(ItemRef::ListItem(self, index), scene, transform, render_params);
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

/// Draws one item of GPU raster content into the Vello scene.
fn render_raster_gpu_item_to_vello(item: ItemRef<'_, Raster<GPU>>, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
	let Some(raster) = item.element() else { return };
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let compositing = item.compositing(render_params);
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
	let opacity = compositing.opacity;
	let any_nondefault = blend_mode_attr != BlendMode::default() || opacity < 1. || clip_attr;
	if (render_params.render_mode != RenderMode::Outline && any_nondefault)
		&& let RenderBoundingBox::Rectangle(bounds) = whole_bounds()
	{
		let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
		let rect = inflate_bounds(bounds, compositing.reach());
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

	let sampler = peniko::ImageSampler::new().with_extend(peniko::Extend::Pad);
	let image_transform = transform * transform_attribute * DAffine2::from_scale(1. / DVec2::new(width as f64, height as f64));

	let item_bounds = || RenderBoundingBox::Rectangle((transform * transform_attribute * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box());
	compositing.draw_with_effects(scene, item_bounds, |scene| {
		scene.draw_texture(raster.texture.clone(), sampler, kurbo::Affine::new(image_transform.to_cols_array()))
	});

	if layer {
		scene.pop_layer()
	}
}

impl Render for List<brush_types::Stroke> {
	fn render_svg(&self, _render: &mut SvgRender, _render_params: &RenderParams) {}

	fn render_to_vello(&self, _scene: &mut VelloSceneBuilder, _transform: DAffine2, _render_params: &RenderParams) {}
}

// Since colors and gradients are technically infinitely big, we have to implement
// workarounds for rendering them correctly in a way which still allows us
// to cache the intermediate render data (SVG string/Vello scene).
// For SVG, this is is achived by creating a truly giant rectangle.
// For Vello, the scene records a viewport fill that covers whatever area each render draws.
impl Render for List<Color> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_color_item_svg(ItemRef::ListItem(self, index), render, render_params);
		}
	}

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, _parent_transform: DAffine2, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_color_item_to_vello(ItemRef::ListItem(self, index), scene, render_params);
		}
	}
}

/// Emits one item of color content as SVG, painting a stand-in for an infinite background.
fn render_color_item_svg(item: ItemRef<'_, Color>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(color) = item.element() else { return };
	// SVG filters change nothing visible about an endless color, so its adjustments are carried out on the color itself
	let color = item.attribute::<Effects>(ATTR_EFFECTS).map_or(*color, |effects| adjusted_color(*color, &adjustments(effects)));
	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	render.leaf_tag("polyline", |attributes| {
		// Stand-in for an infinite background. Chrome's SVG renderer keeps internal coordinates in f32 and loses
		// precision past ~2^24 (~16.7 million), causing tile-boundary artifacts that pop in and out during panning.
		// 1e7 stays under that limit while still being far larger than any practical document extent.
		const MAX: f64 = 1e7;
		attributes.push("points", format!("{MAX},{MAX} -{MAX},{MAX} -{MAX},-{MAX} {MAX},-{MAX}"));

		attributes.push("fill", format!("#{}", SRGBA8::from(color).to_rgb_hex()));
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

/// Draws one item of color content into the Vello scene, covering the whole render area.
fn render_color_item_to_vello(item: ItemRef<'_, Color>, scene: &mut VelloSceneBuilder, render_params: &RenderParams) {
	let Some(color) = item.element() else { return };
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let compositing = item.compositing(render_params);

	// The adjustments leading the effects are carried out on the color itself, leaving the rest to the effects group
	let mut remaining_effects = Effects::default();
	let (leading_adjustments, compositing) = compositing.split_leading_adjustments(&mut remaining_effects);
	let color = adjusted_color(*color, &leading_adjustments);

	let blend_mode = blend_mode_attr.to_peniko();
	let opacity = compositing.opacity;

	let vello_color = SRGBA8::from(color).to_peniko_color();

	let mut layer = false;
	if opacity < 1. || blend_mode_attr != BlendMode::default() {
		scene.push_viewport_layer(peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver), opacity);
		layer = true;
	}

	compositing.draw_with_effects(scene, || RenderBoundingBox::Infinite, |scene| scene.fill_viewport(vello_color, None));

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

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, parent_transform: DAffine2, render_params: &RenderParams) {
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
	// SVG filters change nothing visible about an endless gradient, so its adjustments are carried out along its ramp
	let Some((gradient, settings)) = painted_gradient(item) else { return };
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let opacity_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
	let opacity_fill_attr: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
	let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
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

		let samples = spread_adjusted_samples(&gradient, settings, gradient_form);

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

/// Draws one item of gradient content into the Vello scene, covering the whole render area.
fn render_gradient_item_to_vello(item: ItemRef<'_, Gradient>, scene: &mut VelloSceneBuilder, parent_transform: DAffine2, render_params: &RenderParams) {
	if let RenderMode::Outline = render_params.render_mode {
		return;
	}

	{
		let Some(gradient) = item.element() else { return };
		let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
		let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
		let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
		let compositing = item.compositing(render_params);
		let gradient_transform = parent_transform * transform;

		// The adjustments leading the effects are carried out along the ramp, leaving the rest to the effects group
		let mut remaining_effects = Effects::default();
		let (leading_adjustments, compositing) = compositing.split_leading_adjustments(&mut remaining_effects);
		let settings = gradient_settings_from_item(item);
		let (gradient, settings) = match adjusted_gradient(gradient, settings, &leading_adjustments) {
			Some((adjusted, settings)) => (Cow::Owned(adjusted), settings),
			None => (Cow::Borrowed(gradient), settings),
		};

		let blend_mode = blend_mode_attr.to_peniko();
		let opacity = compositing.opacity;

		let samples = spread_adjusted_samples(&gradient, settings, gradient_form);

		let stops = peniko_color_stops(&samples);

		let extend = peniko_extend(settings.spread);

		let kind = unit_gradient_kind(gradient_form);

		let fill = peniko::Brush::Gradient(peniko::Gradient {
			kind,
			stops,
			extend,
			interpolation_alpha_space: peniko::InterpolationAlphaSpace::Unpremultiplied,
			..Default::default()
		});
		let brush_transform = kurbo::Affine::new(gradient_placement(gradient_transform, gradient_form).to_cols_array());

		let mut layer = false;
		if opacity < 1. || blend_mode_attr != BlendMode::default() {
			scene.push_viewport_layer(peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver), opacity);
			layer = true;
		}

		compositing.draw_with_effects(scene, || RenderBoundingBox::Infinite, |scene| scene.fill_viewport(&fill, Some(brush_transform)));

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
	let max_width = item.attribute::<f64>(ATTR_MAX_WIDTH).copied().filter(|width| *width > 0.);
	let max_height = item.attribute::<f64>(ATTR_MAX_HEIGHT).copied().filter(|height| *height > 0.);
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
	let compositing = item.compositing(render_params);
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let font: Resource = {
		let f: Resource = item.attribute_cloned_or_default(ATTR_FONT);
		if f.is_empty() { text_nodes::FALLBACK_FONT_RESOURCE.clone() } else { f }
	};
	let font_size: f64 = item.attribute_cloned_or(ATTR_FONT_SIZE, DEFAULT_FONT_SIZE);
	let line_height: f64 = item.attribute_cloned_or(ATTR_LINE_HEIGHT, 1.2);
	let letter_spacing: f64 = item.attribute_cloned_or(ATTR_LETTER_SPACING, 0.);
	let max_width = item.attribute::<f64>(ATTR_MAX_WIDTH).copied().filter(|width| *width > 0.);
	let max_height = item.attribute::<f64>(ATTR_MAX_HEIGHT).copied().filter(|height| *height > 0.);
	let letter_tilt: f64 = item.attribute_cloned_or(ATTR_LETTER_TILT, 0.);
	let align: text_nodes::TextAlign = item.attribute_cloned_or_default(ATTR_TEXT_ALIGN);

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
	let mut layout_bounds = RenderBoundingBox::None;

	text_nodes::TextContext::with_thread_local(|ctx| {
		let Some(layout) = ctx.layout_text(text, &font, typesetting) else { return };
		let tilt_tan = letter_tilt.to_radians().tan();

		// Padded by the font size, since glyphs can overhang their layout box
		let alignment_width = max_width.unwrap_or(layout.full_width() as f64);
		let padded = Quad::from_box([DVec2::splat(-font_size), DVec2::new(alignment_width, layout.height() as f64) + font_size]);
		layout_bounds = RenderBoundingBox::Rectangle((transform * padded).bounding_box());

		text_nodes::for_each_styled_glyph_run(&layout, text, typesetting, |glyph_run, x_offset, space_extra| {
			draw_glyph_run_to_bezpaths(glyph_run, x_offset, space_extra, tilt_tan, |bez_path| {
				glyph_paths.push(bez_path.to_svg());
			});
		});
	});

	if glyph_paths.is_empty() {
		return;
	}

	let filter = compositing.svg_filter(render, || layout_bounds);
	let filtered = filter.is_some();

	// Wrap all glyph <path> elements in a <g> with the item's transform/opacity/blend-mode, which a filter's wrapping group takes instead
	render_svg_through_filter(render, filter, compositing.opacity, blend_mode_attr, |render| {
		render.parent_tag(
			"g",
			|attributes| {
				let matrix = format_transform_matrix(transform);
				if !matrix.is_empty() {
					attributes.push("transform", matrix);
				}
				if filtered {
					return;
				}

				let opacity = compositing.opacity_without_effects();
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
	});
}

/// Draws one item of text content into the Vello scene, laying out its glyphs under the item's styling.
fn render_text_item_to_vello(item: ItemRef<'_, String>, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
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
	let max_width = item.attribute::<f64>(ATTR_MAX_WIDTH).copied().filter(|width| *width > 0.);
	let max_height = item.attribute::<f64>(ATTR_MAX_HEIGHT).copied().filter(|height| *height > 0.);
	let letter_tilt: f64 = item.attribute_cloned_or(ATTR_LETTER_TILT, 0.);
	let align: text_nodes::TextAlign = item.attribute_cloned_or_default(ATTR_TEXT_ALIGN);
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	let compositing = item.compositing(render_params);
	let opacity = compositing.opacity;

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

		let transformed_bounds = || {
			let alignment_width = max_width.map(|w| w as f32).unwrap_or_else(|| layout.full_width());
			let padding = font_size;
			let bounds = kurbo::Rect::new(-padding, -padding, alignment_width as f64 + padding, layout.height() as f64 + padding);
			affine.transform_rect_bbox(bounds)
		};

		let needs_layer = opacity < 1. || blend_mode_attr != BlendMode::default();
		if needs_layer {
			let blending = peniko::BlendMode::new(blend_mode_attr.to_peniko(), peniko::Compose::SrcOver);
			let reach = compositing.reach();
			scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &transformed_bounds().inflate(reach, reach));
		}

		let tilt_tan = letter_tilt.to_radians().tan();

		compositing.draw_with_effects(
			scene,
			|| rect_bounds(transformed_bounds()),
			|scene| {
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
			},
		);

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

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, transform: DAffine2, render_params: &RenderParams) {
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
		let settings = |spread| GradientSettings {
			spread,
			space: GradientSpace::RgbGamma,
			..Default::default()
		};

		// Other spreads pass the samples through unchanged
		let samples = spread_adjusted_samples(&gradient, settings(GradientSpread::Repeat), GradientForm::Linear);
		assert_eq!(samples, gradient.interpolated_samples(settings(GradientSpread::Pad)));

		// The guards share the range ends' exact offsets, ordered so the pad extension resolves to the transparent outer stops
		let samples = spread_adjusted_samples(&gradient, settings(GradientSpread::Clear), GradientForm::Linear);
		assert_eq!(
			samples,
			vec![(0., Color::TRANSPARENT, None), (0., Color::BLACK, None), (1., Color::WHITE, None), (1., Color::TRANSPARENT, None)]
		);

		// A radial has no guard below the center
		let samples = spread_adjusted_samples(&gradient, settings(GradientSpread::Clear), GradientForm::Radial);
		assert_eq!(samples, vec![(0., Color::BLACK, None), (1., Color::WHITE, None), (1., Color::TRANSPARENT, None)]);
	}

	#[test]
	fn spread_adjusted_samples_keeps_a_stopless_clear_gradient_black_inside_the_range() {
		let samples = spread_adjusted_samples(
			&Gradient::from(Vec::new()),
			GradientSettings {
				spread: GradientSpread::Clear,
				space: GradientSpace::RgbGamma,
				..Default::default()
			},
			GradientForm::Linear,
		);
		let colors: Vec<Color> = samples.iter().map(|&(_, color, _)| color).collect();
		assert_eq!(colors, vec![Color::TRANSPARENT, Color::BLACK, Color::BLACK, Color::TRANSPARENT]);
	}

	#[test]
	fn canvas_images_are_halved_to_suit_the_canvas_image_scale() {
		use graphic_types::raster_types::Image;

		// An image placed at one pixel per document unit
		let image = Raster::new_cpu(Image::new(64, 64, Color::WHITE));
		let list = List::new_from_item(Item::new_from_element(image).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(64.))));

		let halvings = |canvas_image_scale| {
			let mut render = SvgRender::new();
			list.render_svg(
				&mut render,
				&RenderParams {
					canvas_image_scale,
					..Default::default()
				},
			);
			render.image_data.values().map(|canvas_image| canvas_image.halvings).collect::<Vec<_>>()
		};

		// Zoomed out to a quarter, two halvings still cover each screen pixel
		assert_eq!(halvings(0.25), [2]);
		// Zoomed in, or without a scale as in thumbnails, the full resolution is sent
		assert_eq!(halvings(4.), [0]);
		assert_eq!(halvings(0.), [0]);
	}

	#[test]
	fn svg_effects_filters_measure_lengths_in_the_document() {
		use graphic_types::EffectOperation;

		let shadow = EffectOperation::DropShadow {
			color: Color::BLACK,
			offset: DVec2::new(4., 0.),
			blur: 2.,
			spread: 0.,
		};
		let mut defs = String::new();

		// A user space drawn at twice the document's scale halves every length measured in it, so the shadow reaches 5 units past the content
		write_svg_effects_filter(
			&mut defs,
			&Effects::from_operations([shadow]),
			1.,
			[DVec2::ZERO, DVec2::splat(10.)],
			DAffine2::from_scale(DVec2::splat(2.)),
			1.,
		)
		.unwrap();

		assert!(defs.contains(r#"dx="2" dy="0""#), "{defs}");
		assert!(defs.contains(r#"stdDeviation="1""#), "{defs}");
		assert!(defs.contains(r#"x="-5" y="-5" width="20" height="20""#), "{defs}");
	}

	#[test]
	fn svg_effects_filters_end_with_the_image_shown() {
		use graphic_types::EffectOperation;

		let blur = EffectOperation::Blur {
			radius: 3.,
			box_blur: false,
			gamma: false,
		};
		let mut defs = String::new();
		write_svg_effects_filter(&mut defs, &Effects::from_operations([blur]), 0.5, [DVec2::ZERO, DVec2::ONE], DAffine2::IDENTITY, 1.).unwrap();

		// The fill fades a copy of the content, which is blurred in linear light and passed through as the filter's last result
		assert!(defs.contains(r#"<feFuncA type="linear" slope="0.5" />"#), "{defs}");
		assert!(defs.contains(r#"in="effect-1" stdDeviation="3" color-interpolation-filters="linearRGB""#), "{defs}");
		assert!(defs.ends_with(r#"<feOffset in="effect-2" result="effect-4" /></filter>"#), "{defs}");
	}

	#[test]
	fn svg_effects_filters_carry_out_inner_glows_satins_outlines_and_bevels() {
		use graphic_types::vector_types::vector::style::StrokeAlign;
		use graphic_types::{BevelDirection, EffectOperation, GlowSource};

		let filter = |operation| {
			let mut defs = String::new();
			write_svg_effects_filter(&mut defs, &Effects::from_operations([operation]), 1., [DVec2::ZERO, DVec2::splat(10.)], DAffine2::IDENTITY, 1.).unwrap();
			defs
		};
		let invert = r#"<feFuncA type="linear" slope="-1" intercept="1" />"#;

		// A glow from the center is the inverse of one from the edges, cast from the area outside the content
		let inner_glow = filter(EffectOperation::InnerGlow {
			color: Color::WHITE,
			blur: 2.,
			choke: 1.,
			source: GlowSource::Center,
		});
		assert!(
			inner_glow.contains(r#"operator="out""#) && inner_glow.contains(r#"operator="dilate" radius="1""#) && inner_glow.contains(invert),
			"{inner_glow}"
		);

		// Satin takes the difference both ways between two opposite casts
		let satin = filter(EffectOperation::Satin {
			color: Color::BLACK,
			offset: DVec2::new(3., 0.),
			blur: 1.,
			invert: false,
		});
		assert!(satin.contains(r#"dx="3" dy="0""#) && satin.contains(r#"dx="-3" dy="-0""#), "{satin}");
		assert_eq!(satin.matches(r#"k2="1" k3="-1""#).count(), 2, "{satin}");

		// A centered outline draws half its weight on each side, outside beneath the content and inside within it
		let outline = filter(EffectOperation::Outline {
			color: Color::BLACK,
			weight: 4.,
			align: StrokeAlign::Center,
		});
		assert_eq!(outline.matches(r#"stdDeviation="1""#).count(), 2, "{outline}");
		assert_eq!(outline.matches(r#"k2="1" k3="1""#).count(), 2, "placed within the content, then above what lies beneath it: {outline}");

		// An inner bevel's height ramps up inside the edge, lit once each for highlight and shadow, steepened by the pixel scale
		let bevel = EffectOperation::BevelAndEmboss {
			style: BevelStyle::InnerBevel,
			depth: 100.,
			direction: BevelDirection::Up,
			size: 4.,
			soften: 0.,
			angle: 120.,
			altitude: 30.,
			highlight_color: Color::WHITE,
			shadow_color: Color::BLACK,
		};
		let mut bevel_defs = String::new();
		write_svg_effects_filter(&mut bevel_defs, &Effects::from_operations([bevel]), 1., [DVec2::ZERO, DVec2::splat(10.)], DAffine2::IDENTITY, 2.).unwrap();
		assert!(bevel_defs.contains(r#"operator="erode" radius="2""#) && bevel_defs.contains(r#"stdDeviation="1""#), "{bevel_defs}");
		assert_eq!(bevel_defs.matches(r#"surfaceScale="4""#).count(), 2, "{bevel_defs}");
	}

	#[test]
	fn svg_effects_filters_express_adjustments_where_svg_can() {
		use graphic_types::adjustment::DesaturateMethod;
		use graphic_types::{Adjustment, EffectOperation};

		let filter = |adjustment| {
			let mut defs = String::new();
			write_svg_effects_filter(
				&mut defs,
				&Effects::from_operations([EffectOperation::Adjustment(adjustment)]),
				1.,
				[DVec2::ZERO, DVec2::ONE],
				DAffine2::IDENTITY,
				1.,
			)
			.unwrap();
			defs
		};

		// A per-channel adjustment becomes curves on the gamma-encoded channels, and Invert's run from white to black
		let invert = filter(Adjustment::Invert);
		assert!(invert.contains(r#"<feComponentTransfer in="SourceGraphic" color-interpolation-filters="sRGB""#), "{invert}");
		assert!(invert.contains(r#"<feFuncR type="table" tableValues="1.0000 0.9961"#), "{invert}");

		// Making the content opaque leaves the empty space around it empty
		let make_opaque = filter(Adjustment::MakeOpaque);
		assert!(make_opaque.contains(r#"<feFuncA type="table" tableValues="0.0000 1.0000"#), "{make_opaque}");

		// A luma in linear light is a matrix there
		let desaturate = filter(Adjustment::Desaturate {
			method: DesaturateMethod::LuminanceRec709,
		});
		assert!(
			desaturate.contains(r#"color-interpolation-filters="linearRGB" type="matrix" values="0.2126 0.7152 0.0722 0 0"#),
			"{desaturate}"
		);

		// One no primitive expresses passes the content through unchanged
		let vibrance = filter(Adjustment::Vibrance { vibrance: 50., saturation: 0. });
		assert!(!vibrance.contains("feComponentTransfer") && !vibrance.contains("feColorMatrix"), "{vibrance}");
	}

	#[test]
	fn image_adjustments_changing_alpha_are_carried_out_on_the_cpu_for_vello() {
		use crate::vello_scene_builder::{SceneImage, VelloSceneBuilder};
		use graphic_types::raster_types::Image;
		use graphic_types::{Effect, EffectOperation, stamp_effect};

		// A hidden color under full transparency, which only the CPU's straight colors keep for Make Opaque to reveal
		let hidden = Color::from_rgbaf32_unchecked(0.8, 0.4, 0.2, 0.);
		let mut item = Item::new_from_element(Raster::new_cpu(Image::new(2, 2, hidden)));
		stamp_effect(&mut item, Effect::new(&EffectOperation::Adjustment(Adjustment::MakeOpaque)));
		stamp_effect(&mut item, Effect::new(&EffectOperation::Adjustment(Adjustment::Invert)));

		let mut scene = VelloSceneBuilder::new(16, 16, kurbo::Affine::IDENTITY, 4096);
		List::new_from_item(item).render_to_vello(&mut scene, DAffine2::IDENTITY, &RenderParams::default());
		let (_, images) = scene.finish();
		let drawn = images.into_iter().find_map(|image| match image {
			SceneImage::Raster { raster, adjustments } => Some((raster, adjustments)),
			_ => None,
		});
		let (raster, adjustments) = drawn.expect("the image is drawn");
		assert!(adjustments.is_empty(), "the texture takes no adjustments of its own");
		assert_eq!(raster.data[0], raster_kernels::adjustments::adjust_color(&Adjustment::Invert, hidden.with_alpha(1.)));
	}

	#[test]
	fn solid_vector_paints_take_their_adjustments_into_their_colors() {
		use crate::vello_scene_builder::{EffectsRenderer, VelloSceneBuilder};
		use graphic_types::{Effect, EffectOperation, stamp_effect};
		use kurbo::Shape;

		let orange = Color::from_gamma_srgb_channels(1., 0.5, 0., 1.);
		let mut item = Vector::item_from_bezpath(&kurbo::Rect::new(0., 0., 10., 10.).to_path(0.1));
		item.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), Graphic::ColorList(List::new_from_element(orange))));
		stamp_effect(&mut item, Effect::new(&EffectOperation::Adjustment(Adjustment::Vibrance { vibrance: 0., saturation: -100. })));
		let vector = List::new_from_item(item);
		let is_gray = |[red, green, blue]: [u8; 3]| red.abs_diff(green) <= 1 && green.abs_diff(blue) <= 1;

		// SVG fills the shape gray rather than leaving Vibrance, which no filter primitive expresses, undone
		let mut render = SvgRender::new();
		vector.render_svg(&mut render, &RenderParams::default());
		let svg = render.svg.to_svg_string();
		assert!(!render.svg_defs.contains("<filter"), "{}", render.svg_defs);
		let hex = svg.split(r##"fill="#"##).nth(1).map(|rest| &rest[..6]).expect("a solid fill");
		let channel = |index: usize| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
		assert!(is_gray([channel(0), channel(1), channel(2)]), "expected a gray fill, got #{hex}");

		// Vello fills it without rendering it through an effects group
		struct NoEffectsGroups;
		impl EffectsRenderer for NoEffectsGroups {
			fn renders_effects(&self) -> bool {
				true
			}

			fn render_effects(&mut self, _: glam::UVec2, _: kurbo::Affine, _: &Effects, _: f32, _: DMat2, _: &mut dyn FnMut(&mut VelloSceneBuilder)) -> wgpu::Texture {
				panic!("the vector is drawn through an effects group");
			}
		}
		let mut effects_renderer = NoEffectsGroups;
		let mut scene = VelloSceneBuilder::new(16, 16, kurbo::Affine::IDENTITY, 4096).with_effects_renderer(&mut effects_renderer);
		vector.render_to_vello(&mut scene, DAffine2::IDENTITY, &RenderParams::default());
	}

	#[test]
	fn svg_draws_generators_as_images_of_the_part_in_view() {
		use glam::UVec2;

		let item = Item::new_from_element(Generator::Mandelbrot).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(100.)));
		let canvases = |render_params: &RenderParams| {
			let mut render = SvgRender::new();
			List::new_from_item(item.clone()).render_svg(&mut render, render_params);
			render.image_data.values().map(|canvas| (canvas.raster.width, canvas.raster.height)).collect::<Vec<_>>()
		};

		// Without a view to fit, a pattern filling the plane has no part to draw
		assert_eq!(canvases(&RenderParams::default()), Vec::new());

		// A 25 pixel view at half scale, drawn at the view's resolution
		let footprint = Footprint {
			transform: DAffine2::from_scale(DVec2::splat(0.5)),
			resolution: UVec2::splat(25),
			..Default::default()
		};
		assert_eq!(canvases(&RenderParams { footprint, ..Default::default() }), vec![(25, 25)]);
	}

	#[test]
	fn svg_images_carry_out_their_adjustments_in_their_pixels() {
		use graphic_types::raster_types::Image;
		use graphic_types::{Effect, EffectOperation, stamp_effect};

		// Vibrance, which no filter primitive expresses, followed by a blur, which one does
		let orange = Color::from_gamma_srgb_channels(1., 0.5, 0., 1.);
		let mut item = Item::new_from_element(Raster::new_cpu(Image::new(2, 2, orange)));
		stamp_effect(&mut item, Effect::new(&EffectOperation::Adjustment(Adjustment::Vibrance { vibrance: 0., saturation: -100. })));
		stamp_effect(
			&mut item,
			Effect::new(&EffectOperation::Blur {
				radius: 1.,
				box_blur: false,
				gamma: false,
			}),
		);
		let image = List::new_from_item(item);

		let render_canvas_images = || {
			let mut render = SvgRender::new();
			image.render_svg(&mut render, &RenderParams::default());
			assert!(render.svg_defs.contains("feGaussianBlur") && !render.svg_defs.contains("feComponentTransfer"), "{}", render.svg_defs);
			render.image_data
		};

		// The canvas shows gray pixels, and a second render reuses them rather than sending another image
		let first = render_canvas_images();
		let [canvas] = first.values().collect::<Vec<_>>()[..] else {
			panic!("expected one canvas, got {}", first.len())
		};
		let [red, green, blue, _] = canvas.raster.data[0].to_gamma_srgb_channels();
		assert!((red - green).abs() < 1e-3 && (green - blue).abs() < 1e-3, "expected gray, got {:?}", [red, green, blue]);
		assert_eq!(render_canvas_images().keys().collect::<Vec<_>>(), first.keys().collect::<Vec<_>>());
	}

	#[test]
	fn paints_carry_out_the_adjustments_stamped_on_them() {
		use graphic_types::{Effect, EffectOperation, stamp_effect};

		let invert = || Effect::new(&EffectOperation::Adjustment(Adjustment::Invert));

		let mut color = Item::new_from_element(Color::BLACK);
		stamp_effect(&mut color, invert());
		assert_eq!(faded_paint_color(ItemRef::Item(&color), false), Some(Color::WHITE));

		let mut gradient = Item::new_from_element(Gradient::from(vec![Color::BLACK, Color::WHITE]));
		stamp_effect(&mut gradient, invert());
		let (ramp, settings) = painted_gradient(ItemRef::Item(&gradient)).unwrap();
		let close = |actual: Color, expected: Color| (actual.to_vec4() - expected.to_vec4()).abs().max_element() < 1e-6;
		assert!(close(ramp.evaluate(0., settings), Color::WHITE), "the start turns white");
		assert!(close(ramp.evaluate(1., settings), Color::BLACK), "the end turns black");
	}
}
