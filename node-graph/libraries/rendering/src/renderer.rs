use crate::deferred_image::{self, DeferredImage, EffectMap};
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
use graphic_types::effect_layers::{EffectLayers, LayerPart, PiecePlace, is_pixel_filter, piece_blend_mode};
use graphic_types::effects::{EffectCapture, GlowFill, effect_space, spans_along_axes};
use graphic_types::generator::GeneratorPattern;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::vector_types::gradient::{Gradient, GradientForm};
use graphic_types::vector_types::vector::click_target::{ClickTarget, FreePoint, TargetPath};
use graphic_types::vector_types::vector::misc::dvec2_to_point;
use graphic_types::vector_types::vector::style::{RenderMode, StrokeAlign, StrokeCap, StrokeJoin};
use graphic_types::vector_types::vector::{
	GeometryCache, Path, PlacedContours, contour_bezpaths, contours_bounding_box, contours_stroke_inclusive_bounding_box, edit_item_geometry, edit_list_row_geometry, no_paths,
};
use graphic_types::{Adjustment, Appearance, Artboard, BevelStyle, Cover, Coverage, EffectOperation, Effects, FillAndStroke, Generator, Graphic, Vector, fitted_paint};
use kurbo::{Affine, BezPath, Cap, Join, PathEl, Shape};
use num_traits::Zero;
use raster_kernels::raster_effects::adjusted_raster;
use raster_kernels::value_effects::{adjusted_color, adjusted_gradient, adjustments, filled_color};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::FontRef as SkrifaFontRef;
use skrifa::{GlyphId, MetadataProvider};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::hash::Hash;
use std::ops::Range;
use std::sync::Arc;
use vector_types::gradient::{GradientSettings, GradientSpread};
use vello_gpu::peniko;

mod masks;
mod styles;

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

	/// How the item's opacity attributes and effects apply as it renders, with `content_transform` placing its content in the space it's drawn in.
	fn compositing(self, render_params: &RenderParams, content_transform: DAffine2) -> ItemCompositing<'a> {
		let opacity = self.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) as f32;
		let placement = self.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
		let fill = if render_params.for_mask {
			1.
		} else {
			self.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.) as f32
		};

		// Outline mode shows only geometry
		let effects = self
			.attribute::<Effects>(ATTR_EFFECTS)
			.filter(|effects| !effects.change_nothing() && render_params.render_mode != RenderMode::Outline);
		let blend_mode = self.attribute_cloned_or_default(ATTR_BLEND_MODE);

		match effects {
			Some(effects) => ItemCompositing {
				opacity,
				blend_mode,
				effects: Some((effects, fill)),
				layered: true,
				placement,
				content_transform,
			},
			None => ItemCompositing {
				opacity: opacity * fill,
				blend_mode,
				effects: None,
				layered: true,
				placement,
				content_transform,
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

/// How an item composites: the alpha and blend mode of the layer it draws through, and the effects its content renders through.
struct ItemCompositing<'a> {
	opacity: f32,
	blend_mode: BlendMode,
	/// With the fill opacity, which the effects group applies to the content alone so the shadows and glows cast from it stay at full strength.
	effects: Option<(&'a Effects, f32)>,
	/// Whether the effects draw as layers blending onto what lies beneath, rather than all flattened into the content's image as for content clipped by a mask.
	layered: bool,
	/// The item's own placement, which the graphics of effects that follow the content are placed by.
	placement: DAffine2,
	/// Places the content in the space it's drawn in, which the effects' spaces follow.
	content_transform: DAffine2,
}

impl ItemCompositing<'_> {
	/// How far the effects reach past the content along each axis of the space it's drawn in.
	fn reach(&self) -> DVec2 {
		self.effects.map_or(DVec2::ZERO, |(effects, _)| effects.reach_along_axes(self.content_transform))
	}

	/// How far the effects reach past the content along each axis of the content's own space.
	fn reach_in_content_space(&self) -> DVec2 {
		self.effects.map_or(DVec2::ZERO, |(effects, _)| effects.reach_along_axes(DAffine2::IDENTITY))
	}

	/// This compositing for content clipped by the content beneath it, if `clipped`, which flattens every effect into the content's image so the clip cuts the whole result.
	fn clipped(self, clipped: bool) -> Self {
		Self {
			layered: self.layered && !clipped,
			..self
		}
	}

	/// Whether the effects group draws the item's opacity and blend mode itself, as its layers blend onto what lies beneath, so the item pushes no layer of its own.
	fn effects_draw_compositing(&self) -> bool {
		self.effects.is_some() && self.layered
	}

	/// How the effects split into those flattened into the content's image and the layers drawn after them.
	fn layers(&self) -> Option<EffectLayers> {
		let (effects, fill) = self.effects?;
		Some(EffectLayers::plan_runs(effects, self.blend_mode, fill, self.layered))
	}

	/// The adjustments at the start of the effects, which can be carried out on the content's own colors ahead of the rest.
	/// A fill fades the content before the effects adjust it, so with one, only adjustments that keep alpha commute with it.
	/// A solid paint overlay paints over the content unfaded by the fill, so it mixes into the content's colors only at a full fill, and only where it blends with the content alone: flattened, or in the content's own layer.
	fn leading_adjustments(&self) -> Vec<Adjustment> {
		let (Some((effects, fill)), Some(layers)) = (self.effects, self.layers()) else {
			return Vec::new();
		};

		(effects.operations().enumerate())
			.map_while(|(index, operation)| {
				let flattened = index < layers.flattened;
				match operation {
					EffectOperation::Adjustment(adjustment @ Adjustment::ColorOverlay { .. }) => {
						let with_content = flattened || layers.layer_of(EffectCapture { index, plane: 0 }) == layers.content_layer();
						(fill == 1. && with_content).then_some(adjustment)
					}
					EffectOperation::Adjustment(adjustment) if flattened && (fill == 1. || raster_kernels::adjustments::preserves_alpha(&adjustment)) => Some(adjustment),
					_ => None,
				}
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

		match remaining.change_nothing() {
			true => ItemCompositing {
				opacity: self.opacity * fill,
				effects: None,
				..*self
			},
			false => ItemCompositing {
				effects: Some((remaining, fill)),
				..*self
			},
		}
	}

	/// Draws the content `draw` draws through the effects group, given the content's bounds in the scene's space. Rendering a split scene calls `draw` again for each part.
	/// `transform` maps the space the content's item is placed in, where the effects' drawn paints are placed too, into the scene's space.
	fn draw_with_effects(
		&self,
		scene: &mut VelloSceneBuilder,
		transform: DAffine2,
		render_params: &RenderParams,
		bounds: impl FnOnce() -> RenderBoundingBox,
		mut draw: impl FnMut(&mut VelloSceneBuilder),
	) {
		let Some((effects, fill)) = self.effects else { return draw(scene) };

		let bounds = match bounds() {
			RenderBoundingBox::Rectangle(bounds) => Some(bounds),
			RenderBoundingBox::Infinite => None,
			// The item pushed no layer of its own, so it still takes its opacity and blend mode
			RenderBoundingBox::None if self.effects_draw_compositing() && (self.opacity < 1. || self.blend_mode != BlendMode::Normal) => {
				scene.push_viewport_layer(peniko::BlendMode::new(self.blend_mode.to_peniko(), peniko::Compose::SrcOver), self.opacity);
				draw(scene);
				return scene.pop_layer();
			}
			RenderBoundingBox::None => return draw(scene),
		};

		// A paint fitted to the content's bounds is placed in the scene's space they're measured in, and one following the content by the content's placement
		let paint_params = render_params.for_pattern();
		let paints: Vec<(Graphic, DAffine2)> = (effects.runs().into_iter())
			.flat_map(|(stamped_transform, rows)| {
				let length_spans = spans_along_axes(effect_space(self.content_transform, stamped_transform).matrix2);
				effects.operations_in(rows).map(move |operation| (operation, length_spans))
			})
			.filter_map(|(operation, length_spans)| {
				let paint = operation.drawn_paint()?;
				Some(match bounds.and_then(|bounds| fitted_paint(paint, operation.drawn_paint_bounds(bounds, length_spans))) {
					Some(fitted) => (fitted, DAffine2::IDENTITY),
					None if operation.drawn_paint_follows_content() => (paint.clone(), transform * self.placement),
					None => (paint.clone(), transform),
				})
			})
			.collect();
		let draw_paint = |index: usize, scene: &mut VelloSceneBuilder| {
			if let Some((paint, transform)) = paints.get(index) {
				paint.render_to_vello(scene, *transform, &paint_params);
			}
		};

		// A capture names an effect by its place in the whole list, which falls in one run, so the runs after that one are left out of its map.
		// The share showing through the fill belongs to the outermost run, which draws the layers.
		let mut runs = effects.runs();
		let mut captures_inner_run = false;
		if let Some(capture) = scene.take_effect_capture() {
			let capture = match runs.iter().position(|(_, rows)| rows.contains(&capture.index)) {
				Some(capture_run) => {
					let run_start = runs[capture_run].1.start;
					captures_inner_run = capture_run + 1 < runs.len();
					runs.truncate(capture_run + 1);
					EffectCapture {
						index: capture.index - run_start,
						plane: capture.plane,
					}
				}
				None => capture,
			};
			scene.capture_next_effects(capture);
		}

		// Runs within the outermost one flatten, as does clipped content inside the layer the item pushes for its opacity and blend mode
		let compositing = match self.layered && !captures_inner_run {
			true => (self.opacity, self.blend_mode, true),
			false => (1., BlendMode::Normal, false),
		};
		let bounds = bounds.map(|[min, max]| kurbo::Rect::new(min.x, min.y, max.x, max.y));
		draw_effect_runs(scene, effects, &runs, fill, compositing, self.content_transform, bounds, &draw_paint, &mut draw);
	}

	/// The opacity for content drawn without the effects group, as SVG draws content its filter can't apply to.
	fn opacity_without_effects(&self) -> f32 {
		self.opacity * self.effects.map_or(1., |(_, fill)| fill)
	}

	/// How an endless item (a color or gradient filling everything) draws, alike in SVG and Vello, from the layers its effects plan.
	/// Its other effects, mostly casting from edges it lacks, are left undone.
	fn endless_paint(&self) -> EndlessPaint {
		let mut paint = EndlessPaint {
			adjustments: Vec::new(),
			fill: 1.,
			opacity: self.opacity,
			blend_mode: self.blend_mode,
			overlays: Vec::new(),
		};
		let (Some((effects, fill)), Some(layers)) = (self.effects, self.layers()) else { return paint };
		let content_layer = layers.content_layer();
		paint.fill = fill;
		paint.blend_mode = content_layer.map_or(self.blend_mode, |layer| layers.layers[layer].blend_mode);

		// Below a full fill, a blended overlay in the content's layer and every one after it paint over everything, which is exact for a paint covering everything
		let mut over_everything = false;
		for (index, operation) in effects.operations().enumerate() {
			let EffectOperation::Adjustment(adjustment) = operation else { continue };
			let capture = EffectCapture { index, plane: 0 };
			let in_content_layer = content_layer.is_some_and(|layer| layers.layers[layer].parts.iter().any(|part| matches!(part, LayerPart::Piece(piece) if piece.capture == capture)));
			let piece = layers.piece(capture).map(|(_, piece)| piece);
			over_everything |= fill < 1. && in_content_layer && piece.is_some_and(|piece| piece.blend_mode != BlendMode::Normal);

			match (adjustment, piece) {
				// A solid paint overlay in a layer of its own paints over everything beneath it
				(Adjustment::ColorOverlay { color, opacity, .. }, Some(piece)) if !in_content_layer || over_everything => {
					paint.overlays.push((color.with_alpha(color.a() * (opacity / 100.) as f32), piece.blend_mode));
				}
				// Flattened, or within the content's own share, an adjustment mixes into the paint
				(adjustment, _) => paint.adjustments.push(adjustment),
			}
		}
		paint
	}

	/// Whether SVG filters can carry out every one of the effects, rather than leaving some undone.
	fn svg_expresses_effects(&self) -> bool {
		self.effects.is_none_or(|(effects, _)| effects.operations().all(|operation| svg_expresses(&operation)))
	}

	/// A vector's solid `fill` and `stroke` paints with the leading adjustments carried out on their colors, sparing the content an effects group or an SVG filter
	/// that might not express them. Only the antialiased seam where an opaque fill meets an opaque stroke differs. `strokes` is whether the stroke is drawn at all.
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

	/// Writes the SVG filters and masks carrying out the effects, if there are any to apply, given the content's bounds in the current user space, where `content_transform` places it.
	/// Each run before the last is flattened into a group, and the last draws one group for each layer it plans, which blend onto what lies beneath.
	/// The item's `content` is what the GPU renders the maps of its effects' coverages from, given whether it keeps its opacity and blend mode, which a layered knockout depends on.
	fn svg_filters(&self, render: &mut SvgRender, render_params: &RenderParams, bounds: impl FnOnce() -> RenderBoundingBox, content: impl FnOnce(bool) -> Graphic) -> Option<SvgEffects> {
		self.effects?;

		// Effects change nothing visible about content without edges, like a color or gradient layer filling everything
		let RenderBoundingBox::Rectangle(bounds) = bounds() else { return None };

		self.svg_filters_within(render, render_params, bounds, Some(bounds), content)
	}

	/// Writes the SVG filters and masks as [`Self::svg_filters`] does, within `bounds` in the current user space.
	/// The drawn paints without a placement of their own are fitted to `paint_bounds`, or with none, as for endless content, left as they are.
	fn svg_filters_within(
		&self,
		render: &mut SvgRender,
		render_params: &RenderParams,
		bounds: [DVec2; 2],
		paint_bounds: Option<[DVec2; 2]>,
		content: impl FnOnce(bool) -> Graphic,
	) -> Option<SvgEffects> {
		let (effects, fill) = self.effects?;

		// Each run's filter measures in its effect space, which a group's transform sets up unless the content is only moved since the effects were stamped
		let runs = effects.runs();
		let spaces: Vec<DAffine2> = runs.iter().map(|(stamped_transform, _)| effect_space(self.content_transform, *stamped_transform)).collect();
		if spaces.iter().any(|space| !space.matrix2.determinant().is_normal()) {
			return None;
		}
		let transformed = spaces.len() > 1 || spaces.iter().any(|space| space.matrix2 != DMat2::IDENTITY);

		// Masks leading or trailing the other effects become SVG's own masks around the filters, which carry out those between.
		// SVG's own masks measure in the user space, so effects measuring in a space of their own leave every mask to the filters.
		let (leading_masks, trailing_masks) = if transformed { (0..0, effects.len()..effects.len()) } else { masks::native_masks(effects) };
		let filtered = leading_masks.end..trailing_masks.start;

		// The effects' drawn paints as markup in a run's effect space, where the paint bounds fit those without a placement and the content's placement places those following it.
		// The markup of SVG's own masks lies in the document, where the maps and images of its own effects reach it, unlike markup a filter embeds as an image.
		let paint_params = render_params.for_pattern();
		let live_params = RenderParams {
			inherited_appearance: None,
			embeds_images: true,
			..render_params.clone()
		};
		let in_document = !render_params.inside_pattern && !render.drawing_deferred_markup;
		let user_to_document = render.transform;
		let paints_in = |render: &mut SvgRender, rows: &Range<usize>, space: DAffine2, user_to_space: DAffine2| -> Vec<String> {
			let length_spans = spans_along_axes(space.matrix2);
			let space_to_document = spans_along_axes((user_to_document * space).matrix2).max_element();
			let mut paints = Vec::new();
			for (index, operation) in (rows.clone()).zip(effects.operations_in(rows.clone())) {
				let Some(paint) = svg_drawn_paint(&operation) else { continue };
				let paint = match paint_bounds.and_then(|paint_bounds| fitted_paint(paint, operation.drawn_paint_bounds(paint_bounds, length_spans))) {
					Some(fitted) => fitted,
					None if operation.drawn_paint_follows_content() => masks::placed(paint, self.placement),
					None => paint.clone(),
				};

				// The maps of a live mask graphic's own effects cover past the view as far as its feather, the effects after a leading one, and the groups around the content read it
				let live = in_document && (leading_masks.contains(&index) || trailing_masks.contains(&index));
				let read_after = if leading_masks.contains(&index) {
					effects.after(index + 1).read_reach(user_to_document * self.content_transform)
				} else {
					0.
				};
				let mut paint_render = SvgRender::new();
				paint_render.transform = user_to_document;
				paint_render.ancestors_reach = render.ancestors_reach + operation.reach() * space_to_document + read_after;
				paint.render_svg(&mut paint_render, if live { &live_params } else { &paint_params });
				if live {
					render.effect_maps.append(&mut paint_render.effect_maps);
					render.deferred_images.append(&mut paint_render.deferred_images);
				}

				let (defs, markup) = (paint_render.svg_defs, paint_render.svg.to_svg_string());
				paints.push(match format_transform_matrix(user_to_space) {
					matrix if matrix.is_empty() => format!("<defs>{defs}</defs>{markup}"),
					matrix => format!(r#"<defs>{defs}</defs><g transform="{matrix}">{markup}</g>"#),
				});
			}
			paints
		};

		let (mut build_content, mut built_content) = (Some(content), None);
		let (mut inner, mut layers) = (Vec::with_capacity(runs.len() - 1), Vec::new());
		let (mut leading_mask, mut trailing_mask) = (None, None);
		let (mut region, mut region_to_space) = (bounds, DAffine2::IDENTITY);
		for (index, ((_, rows), space)) in runs.iter().zip(&spaces).enumerate() {
			// The run's content is the content itself or the result of the runs before it, in its effect space
			let user_to_space = if transformed { space.inverse() } else { DAffine2::IDENTITY };
			let to_space = if index == 0 { user_to_space } else { user_to_space * region_to_space };
			let content_bounds = (to_space * Quad::from_box(region)).bounding_box();
			let run_fill = if index == 0 { fill } else { 1. };

			// Only the last run draws layers, in the item's opacity and blend mode, while those within it flatten into its content
			let run = effects.run(rows.clone());
			let last = index == runs.len() - 1;
			let (run_layers, opacity) = match last {
				true => (EffectLayers::plan(&run, self.blend_mode, run_fill, self.layered), self.opacity),
				false => (EffectLayers::plan(&run, BlendMode::Normal, run_fill, false), 1.),
			};
			let paints = paints_in(render, rows, *space, user_to_space);

			// The filters carry out the effects between the native masks, so with only masks at a full fill there are none
			let run_filtered = filtered.start.clamp(rows.start, rows.end) - rows.start..filtered.end.clamp(rows.start, rows.end) - rows.start;
			let needs_filter = !run_filtered.is_empty() || run_fill < 1.;
			let filters: Vec<Option<SvgEffectsFilter>> = (0..run_layers.layers.len())
				.map(|layer| match needs_filter {
					true => write_svg_effects_filter(
						&mut render.svg_defs,
						&run,
						&run_layers,
						layer,
						run_fill,
						opacity,
						content_bounds,
						rows.start,
						&paints,
						run_filtered.clone(),
					)
					.map(Some),
					false => Some(None),
				})
				.collect::<Option<_>>()?;
			if !transformed {
				[leading_mask, trailing_mask] = masks::write_native_masks(&mut render.svg_defs, effects, [leading_masks.clone(), trailing_masks.clone()], &paints, bounds);
			}
			let space_to_document = if transformed { render.transform * *space } else { render.transform };

			// A paint's markup is embedded as an image, where no map can reach its placeholders, and deferred content's image already covers its effects
			let maps: Vec<(SvgEffectMap, [DVec2; 2])> = (filters.iter().flatten()).flat_map(|filter| filter.maps.iter().map(|map| (map.clone(), filter.region))).collect();
			if !maps.is_empty() && in_document {
				if let Some(build_content) = build_content.take() {
					built_content = Some(build_content(self.layered));
				}
				for (map, map_region) in maps {
					// The effects after the map's (none, for the share showing through the fill), and the groups' around the content, read past the view
					let margin = effects.after(map.capture.index.saturating_add(1)).read_reach(render.transform * self.content_transform) + render.ancestors_reach;
					render.effect_maps.push(EffectMap {
						id: map.id,
						content: built_content.clone().unwrap_or_default(),
						render_params: render_params.clone(),
						capture: map.capture,
						result: map.result,
						user_to_document: space_to_document,
						content_to_document: render.transform,
						bounds: map_region,
						margin,
					});
				}
			}

			region = (filters.iter().flatten().map(|filter| filter.region))
				.reduce(|[min, max], [other_min, other_max]| [min.min(other_min), max.max(other_max)])
				.unwrap_or(content_bounds);
			region_to_space = if transformed { *space } else { DAffine2::IDENTITY };
			match last {
				true => {
					layers = (filters.into_iter().zip(&run_layers.layers))
						.map(|(filter, layer)| SvgLayer {
							filter: filter.as_ref().map(|filter| format!("url(#{})", filter.id)),
							region: filter.map(|filter| filter.region),
							blend_mode: layer.blend_mode,
							holds_content: layer.holds_content(),
						})
						.collect();
				}
				false => inner.extend(filters.into_iter().map(|filter| filter.map(|filter| format!("url(#{})", filter.id)))),
			}
		}

		Some(SvgEffects {
			inner,
			layers,
			spaces: transformed.then_some(spaces),
			leading_mask,
			trailing_mask,
		})
	}
}

/// What the SVG groups drawing an item's content take to carry out its effects.
struct SvgEffects {
	/// The filter references of the runs before the last (see [`Effects::runs`]), innermost first, each flattening its effects into the content of the run around it.
	inner: Vec<Option<String>>,
	/// The last run's layers, siblings that each blend onto what lies beneath the item.
	layers: Vec<SvgLayer>,
	/// Each run's effect space placed in the user space, innermost first, when groups have to transform into them.
	spaces: Option<Vec<DAffine2>>,
	/// The reference for the mask of a group wrapping the content's own markup, for the masks before the filters' effects.
	leading_mask: Option<String>,
	/// The reference for the layers' own `mask` attribute, which applies after their filters, for the masks after the filters' effects.
	trailing_mask: Option<String>,
}

/// One layer of an item's effects in SVG: a group filtering the content into the layer, blending by its mode.
struct SvgLayer {
	/// The reference for the group's `filter` attribute, if any effects need a filter.
	filter: Option<String>,
	/// The filter's region in the group's user space, if it has a filter.
	region: Option<[DVec2; 2]>,
	blend_mode: BlendMode,
	/// Whether the content's own markup goes in this layer's group, which the others reference.
	holds_content: bool,
}

/// The item with its own attributes as content for the GPU to render the maps of its effects from, keeping its opacity and blend mode if `keeps_compositing`, which a layered knockout depends on.
fn effects_content<T>(item: ItemRef<'_, T>, element: T, keeps_compositing: bool) -> Item<T> {
	match keeps_compositing {
		true => Item::from_parts(element, item.clone_item_attributes()),
		false => without_opacity_and_blend_mode(item, element),
	}
}

/// Draws the content `draw` draws through the last of the `runs` of `effects`, around the content drawn through the runs before it, each in its own effect space.
/// The innermost run fades the content by the `fill`, and each run around it takes the result within as its content, at full strength.
/// The outermost run draws in the `compositing` opacity and blend mode, layered if it says so, while those within it flatten into its content.
/// `draw_paint` draws each of the effects' drawn paints by its index among those in all of `effects`.
#[allow(clippy::too_many_arguments)]
fn draw_effect_runs(
	scene: &mut VelloSceneBuilder,
	effects: &Effects,
	runs: &[(DAffine2, Range<usize>)],
	fill: f32,
	(opacity, blend_mode, layered): (f32, BlendMode, bool),
	content_transform: DAffine2,
	bounds: Option<kurbo::Rect>,
	draw_paint: &dyn Fn(usize, &mut VelloSceneBuilder),
	draw: &mut dyn FnMut(&mut VelloSceneBuilder),
) {
	let Some(((stamped_transform, rows), earlier)) = runs.split_last() else { return draw(scene) };
	let space = effect_space(content_transform, *stamped_transform);
	let run = effects.run(rows.clone());

	// The run's drawn paints follow those of the rows before it
	let paints_before = effects.operations_in(0..rows.start).filter(|operation| operation.drawn_paint().is_some()).count();
	let mut draw_run_paint = |index: usize, scene: &mut VelloSceneBuilder| draw_paint(paints_before + index, scene);

	if earlier.is_empty() {
		return scene.draw_effects(&run, fill, opacity, blend_mode, layered, bounds, space, &mut draw_run_paint, draw);
	}

	// The runs before this one paint past the content, which this run then reaches from
	let earlier_reach: DVec2 = (earlier.iter())
		.map(|(stamped_transform, rows)| {
			let reach: f64 = effects.operations_in(rows.clone()).map(|operation| operation.painted_reach()).sum();
			reach * spans_along_axes(effect_space(content_transform, *stamped_transform).matrix2)
		})
		.sum();
	let earlier_bounds = bounds.map(|bounds| bounds.inflate(earlier_reach.x, earlier_reach.y));
	scene.draw_effects(&run, 1., opacity, blend_mode, layered, earlier_bounds, space, &mut draw_run_paint, &mut |scene| {
		let flattened = (1., BlendMode::Normal, false);
		draw_effect_runs(scene, effects, earlier, fill, flattened, content_transform, bounds, draw_paint, draw)
	});
}

/// The item as content of its own, without the opacity and blend mode that act on its effects' result as a whole, which an image of it takes instead.
fn without_opacity_and_blend_mode<T>(item: ItemRef<'_, T>, element: T) -> Item<T> {
	let mut content = Item::from_parts(element, item.clone_item_attributes());
	content.remove_attribute::<f64>(ATTR_OPACITY);
	content.remove_attribute::<BlendMode>(ATTR_BLEND_MODE);
	content
}

/// How an endless item (a color or gradient filling everything) draws: its paint with `adjustments` carried out and faded by `fill`, though not the paint overlays among them,
/// in a layer of `opacity` and `blend_mode`, then each solid paint overlay in a layer of its own, a color over everything in that opacity and its own mode.
struct EndlessPaint {
	adjustments: Vec<Adjustment>,
	fill: f32,
	opacity: f32,
	blend_mode: BlendMode,
	/// Each overlay's color with its strength in alpha, and its layer's mode. It paints within the full-strength paint, which an endless gradient is taken to cover everywhere.
	overlays: Vec<(Color, BlendMode)>,
}

impl EndlessPaint {
	/// The fill to carry out along a gradient's ramp and the opacity of its layer. With nothing to carry out, the fill fades the layer instead, sparing a resampled ramp.
	fn ramp_fill_and_opacity(&self) -> (f32, f32) {
		match self.adjustments.is_empty() {
			true => (1., self.opacity * self.fill),
			false => (self.fill, self.opacity),
		}
	}
}

/// A vector's solid paints with adjustments carried out on their colors, and the effects left after those adjustments.
struct FoldedPaints {
	fill: Option<Graphic>,
	stroke: Option<Graphic>,
	remaining_effects: Effects,
}

/// How far the effects of the items nested anywhere inside `graphic`, which `transform` places in the document, reach past its bounds along the document's axes,
/// measuring how far each effect reaches by `reach` (like [`EffectOperation::painted_reach`]).
fn nested_effects_reach(graphic: &Graphic, transform: DAffine2, reach: fn(&EffectOperation) -> f64) -> DVec2 {
	fn item_reach<T>(item: &Item<T>, transform: DAffine2, reach: fn(&EffectOperation) -> f64) -> DVec2 {
		let content_transform = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
		item.attribute::<Effects>(ATTR_EFFECTS)
			.map_or(DVec2::ZERO, |effects| effects.reach_along_axes_by(content_transform, reach))
	}
	fn list_reach<T>(list: &List<T>, transform: DAffine2, reach: fn(&EffectOperation) -> f64, nested: impl Fn(&T, DAffine2) -> DVec2) -> DVec2 {
		(0..list.len())
			.map(|index| {
				let content_transform = transform * list.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, index);
				let own = list
					.attribute::<Effects>(ATTR_EFFECTS, index)
					.map_or(DVec2::ZERO, |effects| effects.reach_along_axes_by(content_transform, reach));
				own + list.element(index).map_or(DVec2::ZERO, |element| nested(element, content_transform))
			})
			.fold(DVec2::ZERO, DVec2::max)
	}
	fn none<T>(_: &T, _: DAffine2) -> DVec2 {
		DVec2::ZERO
	}

	match graphic {
		Graphic::None(_) | Graphic::NoneList(_) | Graphic::StrokeList(_) => DVec2::ZERO,
		Graphic::Graphic(item) => {
			let content_transform = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
			item_reach(item, transform, reach) + nested_effects_reach(item.element(), content_transform, reach)
		}
		Graphic::Vector(item) => item_reach(item, transform, reach),
		Graphic::RasterCPU(item) => item_reach(item, transform, reach),
		Graphic::RasterGPU(item) => item_reach(item, transform, reach),
		Graphic::Color(item) => item_reach(item, transform, reach),
		Graphic::Gradient(item) => item_reach(item, transform, reach),
		Graphic::Generator(item) => item_reach(item, transform, reach),
		Graphic::Text(item) => item_reach(item, transform, reach),
		Graphic::GraphicList(list) => list_reach(list, transform, reach, |nested, content_transform| nested_effects_reach(nested, content_transform, reach)),
		Graphic::VectorList(list) => list_reach(list, transform, reach, none),
		Graphic::RasterCPUList(list) => list_reach(list, transform, reach, none),
		Graphic::RasterGPUList(list) => list_reach(list, transform, reach, none),
		Graphic::ColorList(list) => list_reach(list, transform, reach, none),
		Graphic::GradientList(list) => list_reach(list, transform, reach, none),
		Graphic::GeneratorList(list) => list_reach(list, transform, reach, none),
		Graphic::TextList(list) => list_reach(list, transform, reach, none),
	}
}

/// The bounds of the items in `list`, as [`graphic_list_bounding_box`] finds them, each grown by how far its effects and those nested anywhere inside it paint past it.
/// Without `maps_resolved`, as in an SVG with no GPU to render its maps and images, only the effects SVG then draws past the content count (see [`svg_drawn_reach`]).
pub fn graphic_list_painted_bounds(list: &List<Graphic>, maps_resolved: bool) -> RenderBoundingBox {
	let reach: fn(&EffectOperation) -> f64 = if maps_resolved { EffectOperation::painted_reach } else { svg_drawn_reach };

	let item_bounds = |index| {
		let element = list.element(index)?;
		let transform = list.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, index);
		let RenderBoundingBox::Rectangle([min, max]) = graphic_thumbnail_bounding_box(element, transform) else {
			return None;
		};

		let own_reach = list
			.attribute::<Effects>(ATTR_EFFECTS, index)
			.map_or(DVec2::ZERO, |effects| effects.reach_along_axes_by(transform, reach));
		let reach = own_reach + nested_effects_reach(element, transform, reach);
		Some([min - reach, max + reach])
	};

	match (0..list.len()).filter_map(item_bounds).reduce(Quad::combine_bounds) {
		Some(bounds) => RenderBoundingBox::Rectangle(bounds),
		None => graphic_list_bounding_box(list, DAffine2::IDENTITY),
	}
}

/// How far an effect that SVG draws without the GPU's maps paints past the content, in its effect space, which only the blurs do.
fn svg_drawn_reach(operation: &EffectOperation) -> f64 {
	match operation {
		EffectOperation::Blur { .. } => operation.painted_reach(),
		_ => 0.,
	}
}

/// Wraps an item's SVG `content` in its effects' groups, if it has effects SVG carries out: one for each layer the last run draws, applying the layer's filter, the item's opacity, and the layer's blend mode.
/// The layers are siblings so each blends onto what lies beneath the item, with the content (inside the groups of the runs before the last) in its own layer's group and referenced by the others.
/// With `repeats_content`, each layer holds the markup again instead, since the canvases that stand in for images in the viewport are left out of a `<use>` copy.
/// The leading masks wrap the content's markup itself, so every layer takes the masked content, and the trailing masks apply to each layer after its filter.
fn render_svg_through_filters(render: &mut SvgRender, effects: Option<SvgEffects>, opacity: f32, repeats_content: bool, mut content: impl FnMut(&mut SvgRender)) {
	let Some(SvgEffects {
		inner,
		layers,
		spaces,
		leading_mask,
		trailing_mask,
	}) = effects
	else {
		return content(render);
	};
	let mut masked_content = |render: &mut SvgRender| match &leading_mask {
		Some(mask) => render.parent_tag("g", |attributes| attributes.push("mask", mask.clone()), &mut content),
		None => content(render),
	};

	// Each layer transforms from the last run's effect space into the user space, and the content within draws through the groups of the runs before it
	let transform = spaces.as_ref().and_then(|spaces| spaces.last()).copied().unwrap_or_default();
	let matrix = format_transform_matrix(transform);
	let mut content_through_runs = |render: &mut SvgRender| render_svg_filter_groups(render, &inner, spaces.as_deref(), &mut masked_content);

	let content_id = (layers.len() > 1 && !repeats_content).then(|| format!("effects-content-{}", generate_uuid()));
	let mut emitted = false;
	for layer in layers {
		render.parent_tag(
			"g",
			|attributes| {
				if !matrix.is_empty() {
					attributes.push(ATTR_TRANSFORM, matrix.clone());
				}
				if let Some(filter) = layer.filter {
					attributes.push("filter", filter);
				}
				if let Some(mask) = &trailing_mask {
					attributes.push("mask", mask.clone());
				}
				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}
				if layer.blend_mode != BlendMode::default() {
					attributes.push("style", layer.blend_mode.render());
				}
			},
			|render| {
				let outside_transform = render.transform;
				render.transform = outside_transform * transform;
				match (&content_id, layer.holds_content) {
					(Some(id), true) => render.parent_tag("g", |attributes| attributes.push("id", id.clone()), &mut content_through_runs),
					(Some(id), false) => render.leaf_tag("use", |attributes| attributes.push("href", format!("#{id}"))),
					(None, _) if repeats_content || !emitted => {
						emitted = true;
						content_through_runs(render);
					}
					(None, _) => {}
				}

				// Browsers cull and cut a filtered group by its children's geometry, so an unpainted rectangle spans the filter's region
				if let Some([min, max]) = layer.region {
					render.leaf_tag("rect", |attributes| {
						attributes.push("x", min.x.to_string());
						attributes.push("y", min.y.to_string());
						attributes.push("width", (max.x - min.x).to_string());
						attributes.push("height", (max.y - min.y).to_string());
						attributes.push("fill", "none");
					});
				}
				render.transform = outside_transform;
			},
		);
	}
}

/// Wraps `content` in the group of the last of the `filters` (innermost first, and none for a run SVG's own masks carry out), around the groups of those before it,
/// each transforming into its run's effect space in `spaces`. Inside the innermost group, the content returns to the user space it's drawn in.
fn render_svg_filter_groups(render: &mut SvgRender, filters: &[Option<String>], spaces: Option<&[DAffine2]>, content: &mut dyn FnMut(&mut SvgRender)) {
	let Some((filter, inner_filters)) = filters.split_last() else {
		return match spaces.and_then(|spaces| spaces.first()) {
			Some(innermost_space) => render_svg_transformed(render, innermost_space.inverse(), content),
			None => content(render),
		};
	};

	// The group transforms from its run's effect space into the space of the run outside it
	let index = inner_filters.len();
	let transform = spaces.map(|spaces| match spaces.get(index + 1) {
		Some(outside_space) => outside_space.inverse() * spaces[index],
		None => spaces[index],
	});

	render.parent_tag(
		"g",
		|attributes| {
			let matrix = transform.map(format_transform_matrix).unwrap_or_default();
			if !matrix.is_empty() {
				attributes.push(ATTR_TRANSFORM, matrix);
			}
			if let Some(filter) = filter {
				attributes.push("filter", filter.clone());
			}
		},
		|render| {
			let outside_transform = render.transform;
			render.transform = outside_transform * transform.unwrap_or_default();
			render_svg_filter_groups(render, inner_filters, spaces, content);
			render.transform = outside_transform;
		},
	);
}

/// Wraps `content` in a group placing it by `transform`, keeping the render's transform into the document in step.
fn render_svg_transformed(render: &mut SvgRender, transform: DAffine2, content: &mut dyn FnMut(&mut SvgRender)) {
	let matrix = format_transform_matrix(transform);
	if matrix.is_empty() {
		return content(render);
	}

	render.parent_tag(
		"g",
		|attributes| attributes.push(ATTR_TRANSFORM, matrix),
		|render| {
			let outside_transform = render.transform;
			render.transform = outside_transform * transform;
			content(render);
			render.transform = outside_transform;
		},
	);
}

/// Draws an item's SVG `content` within a group taking its `mask` and blend mode, if it has a mask.
fn render_svg_within_mask(render: &mut SvgRender, mask: Option<String>, blend_mode: BlendMode, content: impl FnOnce(&mut SvgRender)) {
	let Some(mask) = mask else { return content(render) };

	render.parent_tag(
		"g",
		|attributes| {
			attributes.push("mask", mask);
			if blend_mode != BlendMode::default() {
				attributes.push("style", blend_mode.render());
			}
		},
		content,
	);
}

/// Markup standing in for an image the GPU renders of an item whose effects hold one that SVG filters can't carry out.
struct DeferredMarkup {
	id: u64,
	/// The content's extent with how far its effects reach past it, in the user space the markup is drawn in.
	bounds: [DVec2; 2],
}

/// Opens markup standing in for an image the GPU renders of an item, if SVG filters can't carry out all its effects, which [`finish_deferred_markup`] then closes.
/// `bounds` gives the content's extent in the current user space.
fn start_deferred_markup(render: &mut SvgRender, render_params: &RenderParams, compositing: &ItemCompositing, bounds: impl FnOnce() -> RenderBoundingBox) -> Option<DeferredMarkup> {
	// Masks and paints are nested markup no image can replace, and content inside deferred content is in its image already.
	// TODO: Paints (patterns) whose effects need the GPU's maps then draw wrongly or not at all, unlike Vello. Embed a GPU render of such a paint,
	// filled in by `svg_readback.rs` like its images, maps, and planned generators (see the values and renders plan's boundary work).
	let replaceable = !render.drawing_deferred_markup && !render_params.for_mask && !render_params.inside_pattern && !render_params.thumbnail;
	if !replaceable || compositing.svg_expresses_effects() || !render.transform.matrix2.determinant().is_normal() {
		return None;
	}
	let RenderBoundingBox::Rectangle([min, max]) = bounds() else { return None };

	// The image covers how far the effects reach past the content, measured in this user space
	let reach = compositing.reach();

	let id = generate_uuid();
	render.leaf_node(deferred_image::image_markers(id).0);
	render.drawing_deferred_markup = true;

	Some(DeferredMarkup {
		id,
		bounds: [min - reach, max + reach],
	})
}

/// Closes the markup [`start_deferred_markup`] opened, if it did, recording the item `content` makes without its opacity and blend mode for the GPU to render.
fn finish_deferred_markup(
	render: &mut SvgRender,
	markup: Option<DeferredMarkup>,
	render_params: &RenderParams,
	compositing: &ItemCompositing,
	blend_mode: BlendMode,
	content: impl FnOnce() -> Graphic,
) {
	let Some(DeferredMarkup { id, bounds }) = markup else { return };

	render.drawing_deferred_markup = false;
	render.leaf_node(deferred_image::image_markers(id).1);

	render.deferred_images.push(DeferredImage {
		id,
		content: content(),
		render_params: render_params.clone(),
		user_to_document: render.transform,
		bounds,
		margin: render.ancestors_reach,
		opacity: compositing.opacity,
		blend_mode,
	});
}

/// Whether SVG filter primitives can carry out the effect operation, rather than leaving it undone.
fn svg_expresses(operation: &EffectOperation) -> bool {
	match operation {
		EffectOperation::Adjustment(adjustment) => svg_expresses_adjustment(adjustment),
		_ => true,
	}
}

/// Whether SVG's color primitives can carry out the adjustment, which [`SvgFilterPrimitives::adjust`] otherwise leaves undone.
fn svg_expresses_adjustment(adjustment: &Adjustment) -> bool {
	use graphic_types::adjustment::DesaturateMethod;

	match adjustment {
		Adjustment::Threshold { .. } | Adjustment::GradientMap { .. } | Adjustment::ChannelMixer(_) | Adjustment::ColorOverlay { .. } => true,
		Adjustment::Desaturate { method } => matches!(
			method,
			DesaturateMethod::LuminanceRec709 | DesaturateMethod::ChannelsAverage | DesaturateMethod::LumaRec709 | DesaturateMethod::LumaRec601
		),
		_ => adjustment.is_per_channel(),
	}
}

/// Builds the primitives of an SVG filter, naming each result so later primitives can take it as an input.
#[derive(Default)]
struct SvgFilterPrimitives {
	markup: String,
	count: usize,
	maps: Vec<SvgEffectMap>,
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

	/// Places what lies `beneath` under the full-strength content and the faded fill, so a pixel filter after it (a blur, an adjustment, a mask, or a feather) processes them together.
	fn merge_beneath(&mut self, full: &mut String, filled: &mut Option<String>, beneath: Option<String>) {
		let Some(beneath) = beneath else { return };

		*filled = filled.take().map(|filled| self.composite_beneath(&filled, &beneath));
		*full = self.composite_beneath(full, &beneath);
	}

	/// A paint's markup, drawn in the user space, as an image filling the `region` of it from its minimum to its maximum corner.
	/// Filters take outside graphics only as images, which an embedded SVG document keeps portable to viewers that can't reference elements.
	fn paint_image(&mut self, markup: &str, [min, max]: [DVec2; 2]) -> String {
		use base64::Engine;

		let size = max - min;
		let document = format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:graphite="https://graphite.art" viewBox="{} {} {} {}" width="{}" height="{}" preserveAspectRatio="none">{markup}</svg>"#,
			min.x, min.y, size.x, size.y, size.x, size.y
		);
		let href = format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(document));

		let placement = format!(r#"x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none""#, min.x, min.y, size.x, size.y);
		self.push("feImage", &format!(r#"href="{href}" {placement}"#), "")
	}

	/// Opaque copies of the `image` and `paint` blended by the mode in the color `space` ("sRGB" or "linearRGB"), along with the image's opaque copy, or `None` for a mode that mixes no color.
	/// The modes SVG lacks are built from other primitives.
	fn opaque_blend(&mut self, image: &str, paint: &str, blend_mode: BlendMode, space: &str) -> Option<(String, String)> {
		use BlendMode::*;

		let space = format!(r#"color-interpolation-filters="{space}""#);
		let arithmetic = |primitives: &mut Self, first: &str, second: &str, [k1, k2, k3, k4]: [f64; 4]| {
			primitives.push(
				"feComposite",
				&format!(r#"in="{first}" in2="{second}" operator="arithmetic" k1="{k1}" k2="{k2}" k3="{k3}" k4="{k4}" {space}"#),
				"",
			)
		};
		let transfer = |primitives: &mut Self, input: &str, function: &str| {
			let functions = ["R", "G", "B"].map(|channel| format!("<feFunc{channel} {function} />")).concat();
			primitives.push("feComponentTransfer", &format!(r#"in="{input}" {space}"#), &functions)
		};
		let blend = |primitives: &mut Self, source: &str, backdrop: &str, mode: &str| primitives.push("feBlend", &format!(r#"in="{source}" in2="{backdrop}" mode="{mode}" {space}"#), "");
		let composite = |primitives: &mut Self, first: &str, second: &str, operator: &str| primitives.push("feComposite", &format!(r#"in="{first}" in2="{second}" operator="{operator}" {space}"#), "");

		// Straight colors made opaque, so every primitive below blends whole colors
		let opaque = |primitives: &mut Self, input: &str| primitives.push("feColorMatrix", &format!(r#"in="{input}" type="matrix" values="1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 0 1" {space}"#), "");
		let backdrop = opaque(self, image);
		let source = opaque(self, paint);
		let doubled = |primitives: &mut Self| transfer(primitives, &source, r#"type="linear" slope="2""#);
		let doubled_less_one = |primitives: &mut Self| transfer(primitives, &source, r#"type="linear" slope="2" intercept="-1""#);

		// Whichever of the backdrop or source has the lower mean channel, or the higher if `lighter`
		let pick_by_mean = |primitives: &mut Self, lighter: bool| {
			let mean = |primitives: &mut Self, input: &str| {
				primitives.push(
					"feColorMatrix",
					&format!(r#"in="{input}" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.3333 0.3333 0.3334 0 0" {space}"#),
					"",
				)
			};
			let (backdrop_mean, source_mean) = (mean(primitives, &backdrop), mean(primitives, &source));
			// Half the difference past a middle, in alpha, which a step turns into where the backdrop wins
			let difference = arithmetic(primitives, &backdrop_mean, &source_mean, [0., 0.5, -0.5, 0.5]);
			let steps = if lighter { "0 1" } else { "1 0" };
			let backdrop_wins = primitives.push(
				"feComponentTransfer",
				&format!(r#"in="{difference}" {space}"#),
				&format!(r#"<feFuncA type="discrete" tableValues="{steps}" />"#),
			);
			let kept = composite(primitives, &backdrop, &backdrop_wins, "in");
			let taken = composite(primitives, &source, &backdrop_wins, "out");
			arithmetic(primitives, &kept, &taken, [0., 1., 1., 0.])
		};

		let blended = match blend_mode {
			BlendMode::LinearBurn => arithmetic(self, &source, &backdrop, [0., 1., 1., -1.]),
			BlendMode::LinearDodge => arithmetic(self, &source, &backdrop, [0., 1., 1., 0.]),
			BlendMode::LinearLight => arithmetic(self, &source, &backdrop, [0., 2., 1., -1.]),
			// The backdrop less the source is one less the inverted backdrop plus the source, which clips at zero where it should
			BlendMode::Subtract => {
				let inverted = transfer(self, &backdrop, r#"type="table" tableValues="1 0""#);
				let sum = arithmetic(self, &source, &inverted, [0., 1., 1., 0.]);
				transfer(self, &sum, r#"type="table" tableValues="1 0""#)
			}
			BlendMode::PinLight => {
				let (doubled, doubled_less_one) = (doubled(self), doubled_less_one(self));
				let darkened = blend(self, &doubled, &backdrop, "darken");
				blend(self, &doubled_less_one, &darkened, "lighten")
			}
			// Linear Light thresholded at a half, which a third of the backdrop plus two thirds of the source reaches at the same point
			BlendMode::HardMix => {
				let mean = arithmetic(self, &source, &backdrop, [0., 2. / 3., 1. / 3., 0.]);
				transfer(self, &mean, r#"type="discrete" tableValues="0 1""#)
			}
			// Color Burn by twice the source where it's dark, and Color Dodge by twice it less one where it's light
			BlendMode::VividLight => {
				let (doubled, doubled_less_one) = (doubled(self), doubled_less_one(self));
				let burned = blend(self, &doubled, &backdrop, "color-burn");
				let dodged = blend(self, &doubled_less_one, &backdrop, "color-dodge");
				let light = transfer(self, &source, r#"type="discrete" tableValues="0 1""#);
				let dark = transfer(self, &source, r#"type="discrete" tableValues="1 0""#);
				let burned = arithmetic(self, &burned, &dark, [1., 0., 0., 0.]);
				let dodged = arithmetic(self, &dodged, &light, [1., 0., 0., 0.]);
				arithmetic(self, &burned, &dodged, [0., 1., 1., 0.])
			}
			BlendMode::DarkerColor => pick_by_mean(self, false),
			BlendMode::LighterColor => pick_by_mean(self, true),
			// Dividing by the paint is Color Dodge by its inverse, which keeps black backdrops black just as Divide does
			Divide => {
				let inverted = transfer(self, &source, r#"type="table" tableValues="1 0""#);
				blend(self, &inverted, &backdrop, "color-dodge")
			}
			_ => blend(self, &source, &backdrop, blend_mode.to_svg_style_name()?),
		};
		Some((backdrop, blended))
	}

	/// The `tinted` image placed over the `image` within the full-strength content, keeping the image's alpha.
	fn atop_within(&mut self, image: &str, tinted: &str, full_content: &str) -> String {
		let within = self.push("feComposite", &format!(r#"in="{tinted}" in2="{full_content}" operator="in""#), "");
		let uncovered = self.push("feComposite", &format!(r#"in="{image}" in2="{tinted}" operator="out""#), "");
		self.add(&within, &uncovered)
	}

	/// The image with the shadows and glows `beneath` it.
	fn composite_beneath(&mut self, image: &str, beneath: &str) -> String {
		self.push("feComposite", &format!(r#"in="{image}" in2="{beneath}" operator="over""#), "")
	}

	/// A placeholder for the map of an effect's coverage that `capture` names, which the GPU renders, returning the name of the result it stands for.
	fn map(&mut self, capture: EffectCapture) -> String {
		self.map_unresolved_as(capture, 0.)
	}

	/// Like [`Self::map`], with the placeholder covering everything to the `coverage` from 0 to 1 until the GPU resolves it, which leaves out an effect multiplying the content by its map at 1.
	fn map_unresolved_as(&mut self, capture: EffectCapture, coverage: f64) -> String {
		self.count += 1;
		let result = format!("effect-{}", self.count);
		let id = generate_uuid();

		self.markup.push_str(&deferred_image::map_placeholder(id, &result, coverage));
		self.maps.push(SvgEffectMap { id, capture, result: result.clone() });
		result
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

	/// The coverage tinted `color`, or with no color the already tinted coverage as it is.
	fn tinted(&mut self, coverage: &str, color: Option<Color>) -> String {
		match color {
			Some(color) => self.tint(coverage, color),
			None => coverage.to_string(),
		}
	}

	/// Stacks the coverage, tinted `color` (or with no color, already tinted), over the shadows and glows already `beneath` the content.
	fn shade_beneath(&mut self, beneath: Option<String>, coverage: &str, color: Option<Color>) -> String {
		let tinted = self.tinted(coverage, color);
		match beneath {
			Some(beneath) => self.push("feComposite", &format!(r#"in="{tinted}" in2="{beneath}" operator="over""#), ""),
			None => tinted,
		}
	}

	/// Places the coverage, tinted `color` (or with no color, already tinted), as a whole interior piece within the full-strength content's share of both it and the faded fill,
	/// mixed with each image's own colors by the blend mode in gamma.
	fn shade_within(&mut self, full_content: &mut String, filled: &mut Option<String>, coverage: &str, color: Option<Color>, blend_mode: BlendMode) {
		let tinted = self.tinted(coverage, color);
		let blend_mode = piece_blend_mode(color, blend_mode);
		*filled = filled.take().map(|filled| self.composite_in_share(&filled, &tinted, full_content, blend_mode, true));
		*full_content = self.composite_in_share(full_content, &tinted, full_content, blend_mode, true);
	}

	/// The image with a `tinted` part of an interior piece composited within the full-strength content's share of each pixel, mixed with the image's own colors by the blend mode in gamma,
	/// matching the CPU's `composite_in_share`. The `whole` piece also paints plainly where the fill lets nothing of the content show, which a part through the fill otherwise takes.
	fn composite_in_share(&mut self, image: &str, tinted: &str, full_content: &str, blend_mode: BlendMode, whole: bool) -> String {
		if whole && blend_mode == BlendMode::Normal {
			return self.atop_within(image, tinted, full_content);
		}

		// The paint blended with the image's opaque colors, at its own strength, weighted by how much of the share the image covers
		let blended = match self.opaque_blend(image, tinted, blend_mode, "sRGB") {
			Some((_, blended)) => self.push("feComposite", &format!(r#"in="{blended}" in2="{tinted}" operator="in""#), ""),
			None => tinted.to_string(),
		};
		let within = self.push("feComposite", &format!(r#"in="{blended}" in2="{image}" operator="in""#), "");
		let uncovered = self.push("feComposite", &format!(r#"in="{image}" in2="{tinted}" operator="out""#), "");
		let composited = self.add(&within, &uncovered);
		// The full-strength content leaves none of its share bare
		if !whole || image == full_content {
			return composited;
		}

		// Plain paint over the part of the share the image leaves bare
		let in_share = self.push("feComposite", &format!(r#"in="{tinted}" in2="{full_content}" operator="in""#), "");
		let in_image = self.push("feComposite", &format!(r#"in="{tinted}" in2="{image}" operator="in""#), "");
		let plain = self.push("feComposite", &format!(r#"in="{in_share}" in2="{in_image}" operator="arithmetic" k2="1" k3="-1""#), "");
		self.add(&composited, &plain)
	}

	/// The image with `adjustment` applied where SVG's color primitives can express it, or left as it is where they can't.
	fn adjust(&mut self, image: &str, adjustment: &graphic_types::Adjustment) -> String {
		use graphic_types::Adjustment;
		use graphic_types::adjustment::DesaturateMethod;

		// Enough entries that interpolating between them strays less than a level from the curve
		const TABLE_SIZE: usize = 256;

		if !svg_expresses_adjustment(adjustment) {
			return image.to_string();
		}

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
					.map(|position| evaluator.evaluate_mapped(if *reverse { 1. - position } else { position }).to_gamma_srgb_channels())
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

/// A map an SVG filter takes as an image the GPU renders of an effect's coverage.
#[derive(Clone)]
struct SvgEffectMap {
	id: u64,
	capture: EffectCapture,
	result: String,
}

/// The region of the user space an item's effects cover, being the content's `bounds` grown by how far the effects reach past it, with `length_scale` the user space's units along each axis per unit of the effects' lengths.
fn svg_effects_region(effects: &Effects, bounds: [DVec2; 2], length_scale: DVec2) -> [DVec2; 2] {
	let reach = effects.working_reach() * length_scale;
	[bounds[0] - reach, bounds[1] + reach]
}

/// An SVG filter written into the defs, with the maps of effects' coverages it takes.
struct SvgEffectsFilter {
	id: String,
	/// The filter's region in its user space.
	region: [DVec2; 2],
	maps: Vec<SvgEffectMap>,
}

/// The drawn paint of an effect that SVG draws as markup, which is any but a bevel's texture, since that only shapes the maps.
fn svg_drawn_paint(operation: &EffectOperation) -> Option<&Graphic> {
	operation.drawn_paint().filter(|_| !matches!(operation, EffectOperation::BevelAndEmboss { .. }))
}

/// Writes an SVG filter into `svg_defs` that carries out a run of `effects` like the GPU passes do, in a user space that is their effect space, giving the layer at `layer` among their `layers` drawn at `opacity`.
/// `bounds` is the content's extent in that space, and `capture_offset` is the run's place in its item's list, which numbers the maps' captures. `None` if there's no such layer.
/// `paints` holds the markup of the effects' drawn paints in order, other than bevels' textures (see [`svg_drawn_paint`]), and the filter carries out only the effects in the `filtered` range.
/// Shadows and glows shaped for PSD interop measure distances no filter can, so the filter takes their coverages as maps the GPU renders.
#[expect(clippy::too_many_arguments)]
fn write_svg_effects_filter(
	svg_defs: &mut String,
	effects: &Effects,
	layers: &EffectLayers,
	layer: usize,
	fill: f32,
	opacity: f32,
	bounds: [DVec2; 2],
	capture_offset: usize,
	paints: &[String],
	filtered: std::ops::Range<usize>,
) -> Option<SvgEffectsFilter> {
	use graphic_types::EffectOperation;
	use graphic_types::effects::{OutlineFill, outline_extents};
	use styles::OutlineBandColors;

	// SVG takes a length along each axis as a pair of numbers
	let pair = |length: DVec2| format!("{} {}", length.x, length.y);

	// The filter covers how far the effects reach past the content, measured one for one in its effect space
	let region = svg_effects_region(effects, bounds, DVec2::ONE);

	let mut primitives = SvgFilterPrimitives::default();
	let mut paints = paints.iter();

	// Only the content's own layer shows the flattened effects' colors, and another layer needs at most the content's alpha to cut its pieces within it to
	let this_layer = layers.layers.get(layer)?;
	let (parts, shows_content) = (this_layer.parts.as_slice(), this_layer.holds_content());
	let cuts_within = !shows_content && parts.iter().any(|part| matches!(part, LayerPart::Piece(piece) if piece.place == PiecePlace::Within));

	// The content at full strength, which casts every silhouette, and the content faded by the fill, which shows in the result
	let mut full = "SourceGraphic".to_string();
	let mut filled = (fill < 1. && shows_content).then(|| primitives.push("feComponentTransfer", r#"in="SourceGraphic""#, &format!(r#"<feFuncA type="linear" slope="{fill}" />"#)));
	// The shadows and glows stacked beneath the content
	let mut beneath: Option<String> = None;
	// This layer's pieces, each tinted, with those beneath the content knocked out by it
	let mut pieces: HashMap<EffectCapture, String> = HashMap::new();

	// The strengths of the trailing masks this filter carries out, fading its result, and of them all, fading the opacity a part over the content works from
	let (mut layer_fade, mut opacity_fade): (Option<String>, Option<String>) = (None, None);
	let fades_opacity = parts
		.iter()
		.any(|part| matches!(part, LayerPart::Piece(piece) if piece.place == PiecePlace::IntoContent { over: true }));

	// A glow's solid color tints its maps, a gradient's maps come tinted (so it has no color), and a fill that paints nothing has neither
	let glow_color = |paint: &Graphic| match GlowFill::of(paint)? {
		GlowFill::Solid(color) => Some(Some(color)),
		fill => (!fill.is_invisible()).then_some(None),
	};

	// The coverage planes that the GPU works out for an effect, each shading within the content or beneath it, in a color (or none, for a map already tinted) mixed by a blend mode
	let shade = |planes: &[(usize, bool)], color: Option<Color>, blend_mode: BlendMode| planes.iter().map(|&(plane, within)| (plane, within, color, blend_mode)).collect::<Vec<_>>();
	const BENEATH: &[(usize, bool)] = &[(0, false)];
	const WITHIN: &[(usize, bool)] = &[(0, true)];

	let operations: Vec<EffectOperation> = effects.operations().collect();
	let last_pixel_filter = (operations[..layers.flattened].iter()).rposition(|operation| !operation.changes_nothing() && is_pixel_filter(operation));
	// Shadows beneath the content reach its alpha only through a later pixel filter (see `is_pixel_filter`)
	let flattened_shown = |index: usize, within: bool| shows_content || (cuts_within && !within && last_pixel_filter.is_some_and(|last| index < last));

	for (index, operation) in operations.into_iter().enumerate().filter(|(_, operation)| !operation.changes_nothing()) {
		let paint_markup = svg_drawn_paint(&operation).and_then(|_| paints.next());
		// A trailing mask that SVG's own mask carries out still fades the opacity here
		let (carried_out, fades_layers) = (filtered.contains(&index), layers.trailing_masks.contains(&index));
		if !carried_out && !(fades_layers && fades_opacity) {
			continue;
		}

		// A layered effect's coverages become pieces, kept for the layer drawing each, while the flattened ones are carried out within the content's image
		let layered = index >= layers.flattened;
		let in_this_layer = |plane: usize| parts.iter().any(|part| matches!(part, LayerPart::Piece(piece) if piece.capture == EffectCapture { index, plane }));
		// The maps of the run's effects are numbered by their places in the item's whole list
		let map_capture = |plane: usize| EffectCapture { index: capture_offset + index, plane };
		// An outline painted with anything but a solid color takes each band's colors from the paint
		let mut band_colors: Option<OutlineBandColors> = None;

		let planes = match operation {
			EffectOperation::Blur { radius, box_blur, gamma } => {
				let radius = DVec2::splat(radius);
				if radius.max_element() < 0.01 || !(shows_content || cuts_within) {
					continue;
				}

				primitives.merge_beneath(&mut full, &mut filled, beneath.take());

				// SVG has no box blur, so a box becomes the Gaussian of the same variance
				let sigma = pair(if box_blur { (radius * (radius + 1.) / 3.).map(f64::sqrt) } else { radius });
				let space = if gamma { "sRGB" } else { "linearRGB" };
				let attributes = |input: &str| format!(r#"in="{input}" stdDeviation="{sigma}" color-interpolation-filters="{space}""#);

				filled = filled.map(|filled| primitives.push("feGaussianBlur", &attributes(&filled), ""));
				full = primitives.push("feGaussianBlur", &attributes(&full), "");
				continue;
			}
			EffectOperation::DropShadow { color, .. } => shade(BENEATH, Some(color), BlendMode::Normal),
			EffectOperation::InnerShadow { color, blend_mode, .. } | EffectOperation::Satin { color, blend_mode, .. } => shade(WITHIN, Some(color), blend_mode),
			EffectOperation::OuterGlow { color, .. } => match glow_color(&color) {
				Some(color) => shade(BENEATH, color, BlendMode::Normal),
				None => continue,
			},
			EffectOperation::InnerGlow { color, blend_mode, .. } => match glow_color(&color) {
				Some(color) => shade(WITHIN, color, blend_mode),
				None => continue,
			},
			// The band outside lies beneath the content, and the band inside within it
			EffectOperation::Outline {
				paint,
				blend_mode,
				weight,
				align,
				contour_gradient,
			} => {
				let (color, colors) = match OutlineFill::of(&paint, contour_gradient) {
					Some(OutlineFill::Solid(color)) => (Some(color), None),
					Some(OutlineFill::Drawn(_)) => match paint_markup {
						Some(markup) => (None, Some(OutlineBandColors::Drawn(markup.clone()))),
						None => continue,
					},
					Some(OutlineFill::ContourGradient(gradient)) => (None, Some(OutlineBandColors::ContourGradient(gradient))),
					None => continue,
				};
				band_colors = colors;

				let (outside, inside) = outline_extents(weight, align);
				let bands = [(0, false, outside), (1, true, inside)].into_iter().filter(|band| band.2 > 0.);
				bands.map(|(plane, within, _)| (plane, within, color, if within { blend_mode } else { BlendMode::Normal })).collect()
			}
			// Planes 0 and 1 light the slopes inside the content with the highlight and shadow, and 2 and 3 those outside it, beneath it
			EffectOperation::BevelAndEmboss {
				style,
				size,
				highlight_color,
				highlight_blend_mode,
				shadow_color,
				shadow_blend_mode,
				ref gloss_contour,
				..
			} => {
				if size <= 0. && gloss_contour.is_identity() {
					continue;
				}
				let inside = (style != BevelStyle::OuterBevel).then_some([(0, true, Some(highlight_color), highlight_blend_mode), (1, true, Some(shadow_color), shadow_blend_mode)]);
				let outside = (style != BevelStyle::InnerBevel).then_some([(2, false, Some(highlight_color), BlendMode::Normal), (3, false, Some(shadow_color), BlendMode::Normal)]);
				inside.into_iter().chain(outside).flatten().collect()
			}
			// A paint overlay paints within the content alone, as a piece of its own when layered
			EffectOperation::PaintOverlay { .. } | EffectOperation::Adjustment(Adjustment::ColorOverlay { .. }) => {
				let markup = matches!(operation, EffectOperation::PaintOverlay { .. }).then_some(paint_markup).flatten();
				if if layered { !in_this_layer(0) } else { !shows_content } {
					continue;
				}

				let (paint, blend_mode) = match (operation, markup) {
					(EffectOperation::PaintOverlay { blend_mode, opacity, .. }, Some(markup)) => {
						let paint = primitives.paint_image(markup, region);
						let slope = opacity / 100.;
						match opacity < 100. {
							true => (
								primitives.push("feComponentTransfer", &format!(r#"in="{paint}""#), &format!(r#"<feFuncA type="linear" slope="{slope}" />"#)),
								blend_mode,
							),
							false => (paint, blend_mode),
						}
					}
					(EffectOperation::Adjustment(Adjustment::ColorOverlay { color, blend_mode, opacity }), _) => {
						let paint = color.with_alpha(color.a() * (opacity / 100.) as f32);
						let flood = format!(r##"flood-color="#{}" flood-opacity="{}""##, SRGBA8::from(paint.with_alpha(1.)).to_rgb_hex(), paint.a());
						(primitives.push("feFlood", &flood, ""), piece_blend_mode(Some(color), blend_mode))
					}
					_ => continue,
				};

				match layered {
					true => {
						pieces.insert(EffectCapture { index, plane: 0 }, paint);
					}
					false => primitives.shade_within(&mut full, &mut filled, &paint, None, blend_mode),
				}
				continue;
			}
			EffectOperation::Mask {
				ref mask,
				mode,
				density,
				feather,
				invert,
			} => {
				let Some(markup) = paint_markup else { continue };
				if !(fades_layers || shows_content || cuts_within) {
					continue;
				}
				if !fades_layers {
					primitives.merge_beneath(&mut full, &mut filled, beneath.take());
				}

				// The GPU's map gives the strength where embedding the markup as an image can't: with effects of the graphic's own, or markup that's live in the document
				let strength = match masks::holds_effects(mask) || !carried_out {
					true => primitives.map_unresolved_as(map_capture(0), 1.),
					false => {
						let image = primitives.paint_image(markup, region);
						primitives.mask_strength(&image, mode, density, masks::svg_feather_sigma(feather, DVec2::ONE), invert)
					}
				};
				let mut masked = |image: &str| primitives.push("feComposite", &format!(r#"in="{image}" in2="{strength}" operator="in""#), "");

				if fades_layers {
					let mut faded = |fade: Option<String>| fade.map_or_else(|| strength.clone(), |fade| masked(&fade));
					if carried_out {
						layer_fade = Some(faded(layer_fade.take()));
					}
					if fades_opacity {
						opacity_fade = Some(faded(opacity_fade.take()));
					}
					continue;
				}

				filled = filled.map(|filled| masked(&filled));
				full = masked(&full);
				continue;
			}
			// The strength the content is faded by measures distances no filter can, so the GPU renders it as a map
			EffectOperation::Feather { .. } => {
				if !(shows_content || cuts_within) {
					continue;
				}
				primitives.merge_beneath(&mut full, &mut filled, beneath.take());

				// Until the GPU resolves it, the map leaves the content unfeathered
				let strength = primitives.map_unresolved_as(map_capture(0), 1.);
				let mut faded = |image: &str| primitives.push("feComposite", &format!(r#"in="{image}" in2="{strength}" operator="in""#), "");
				filled = filled.map(|filled| faded(&filled));
				full = faded(&full);
				continue;
			}
			EffectOperation::Adjustment(ref adjustment) => {
				if !(shows_content || cuts_within) {
					continue;
				}
				primitives.merge_beneath(&mut full, &mut filled, beneath.take());
				filled = filled.map(|filled| primitives.adjust(&filled, adjustment));
				full = primitives.adjust(&full, adjustment);
				continue;
			}
		};

		for (plane, within, color, blend_mode) in planes {
			if color.is_some_and(|color| color.a() <= 0.) || (layered && !in_this_layer(plane)) || (!layered && !flattened_shown(index, within)) {
				continue;
			}
			let colors = band_colors.as_mut().map(|colors| primitives.outline_band_colors(colors, capture_offset + index, region));
			let coverage = primitives.map(map_capture(plane));
			let coverage = match colors {
				Some(colors) => primitives.push("feComposite", &format!(r#"in="{colors}" in2="{coverage}" operator="in""#), ""),
				None => coverage,
			};
			match (layered, within) {
				(true, _) => {
					pieces.insert(EffectCapture { index, plane }, primitives.tinted(&coverage, color));
				}
				(false, true) => primitives.shade_within(&mut full, &mut filled, &coverage, color, blend_mode),
				(false, false) => beneath = Some(primitives.shade_beneath(beneath.take(), &coverage, color)),
			}
		}
	}

	// The share each pixel's fill lets show through within the content, which the GPU works out from the content's finished layer
	let mut through_share = None;
	let mut through_share = |primitives: &mut SvgFilterPrimitives| through_share.get_or_insert_with(|| primitives.map(EffectCapture::THROUGH_MASK)).clone();

	// This layer stacks its parts in order: what lies beneath the content, then the content, then what lies within it, in the content's own layer
	let shown = filled.unwrap_or_else(|| full.clone());
	let mut stack = beneath.filter(|_| shows_content);
	let mut content_with_within: Option<String> = None;
	for part in parts {
		let LayerPart::Piece(piece) = part else {
			content_with_within = Some(shown.clone());
			continue;
		};
		let Some(tinted) = pieces.get(&piece.capture) else { continue };

		let upper = match (piece.place, &mut content_with_within) {
			(PiecePlace::Within, Some(image)) => {
				*image = primitives.atop_within(image, tinted, &full);
				continue;
			}
			(PiecePlace::IntoContent { over }, Some(image)) => {
				// A part over the content takes the share showing through the fill, faded by the opacity, leaving this the rest
				let tinted = match over {
					true => {
						let mut share = through_share(&mut primitives);
						if opacity < 1. {
							share = primitives.push("feComponentTransfer", &format!(r#"in="{share}""#), &format!(r#"<feFuncA type="linear" slope="{opacity}" />"#));
						}
						if let Some(fade) = &opacity_fade {
							share = primitives.push("feComposite", &format!(r#"in="{share}" in2="{fade}" operator="in""#), "");
						}
						primitives.push("feComposite", &format!(r#"in="{tinted}" in2="{share}" operator="out""#), "")
					}
					false => tinted.clone(),
				};
				*image = primitives.composite_in_share(image, &tinted, &full, piece.blend_mode, false);
				continue;
			}
			(PiecePlace::Within, None) => primitives.push("feComposite", &format!(r#"in="{tinted}" in2="{full}" operator="in""#), ""),
			(PiecePlace::Through { .. }, _) => {
				let share = through_share(&mut primitives);
				primitives.push("feComposite", &format!(r#"in="{tinted}" in2="{share}" operator="in""#), "")
			}
			(PiecePlace::Beneath, _) | (PiecePlace::IntoContent { .. }, None) => tinted.clone(),
		};
		stack = Some(match stack {
			Some(stack) => primitives.composite_beneath(&upper, &stack),
			None => upper,
		});
	}

	// The filter's result is its last primitive's, so finish with the layer's image
	let image = match (content_with_within, stack) {
		(Some(image), Some(stack)) => primitives.composite_beneath(&image, &stack),
		(Some(image), None) | (None, Some(image)) => primitives.push("feOffset", &format!(r#"in="{image}""#), ""),
		(None, None) => primitives.push("feFlood", r#"flood-opacity="0""#, ""),
	};
	if let Some(fade) = layer_fade {
		primitives.push("feComposite", &format!(r#"in="{image}" in2="{fade}" operator="in""#), "");
	}

	let [min, max] = region;
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

	Some(SvgEffectsFilter {
		id,
		region: [min, max],
		maps: primitives.maps,
	})
}

/// Grows a layer's bounds in the scene's space by a length along each of its axes.
fn inflate_bounds(bounds: [DVec2; 2], amount: DVec2) -> kurbo::Rect {
	kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y).inflate(amount.x, amount.y)
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

	let adjusted = item
		.attribute::<Effects>(ATTR_EFFECTS)
		.and_then(|effects| adjusted_gradient(gradient, settings, &adjustments(effects), 1.));
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

	/// Writes the clip or mask of the masker's `svg_string` into the defs, a mask covering the masker's `region` in the clipped content's user space if it has one.
	fn write_to_defs(self, svg_defs: &mut String, uuid: u64, svg_string: String, region: Option<[DVec2; 2]>) {
		let id = format!("mask-{uuid}");
		match (self, region) {
			(Self::Clip, _) => write!(svg_defs, r##"<clipPath id="{id}">{svg_string}</clipPath>"##).unwrap(),
			// By default a mask covers only a tenth past the clipped content's geometry, which cuts off its effects
			(Self::Mask, Some([min, max])) => write!(
				svg_defs,
				r##"<mask id="{id}" mask-type="alpha" maskUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}">{svg_string}</mask>"##,
				min.x,
				min.y,
				max.x - min.x,
				max.y - min.y,
			)
			.unwrap(),
			(Self::Mask, None) => write!(svg_defs, r##"<mask id="{id}" mask-type="alpha">{svg_string}</mask>"##).unwrap(),
		}
	}
}

/// Mutable state used whilst rendering to an SVG
pub struct SvgRender {
	pub svg: Vec<SvgSegment>,
	pub svg_defs: String,
	/// Maps the user space of the content being rendered into the document's space.
	pub transform: DAffine2,
	/// The images drawn to canvases rather than embedded, by the canvas ID their placeholders name.
	pub image_data: HashMap<u64, CanvasImage>,
	/// The maps of effects' coverages the filters take, for the GPU to render.
	pub effect_maps: Vec<EffectMap>,
	/// The content with effects SVG filters can't carry out, for the GPU to render images of in place of the markup standing in for it.
	pub deferred_images: Vec<DeferredImage>,
	/// Whether markup standing in for deferred content is being drawn, inside which nothing more is deferred since the content's image covers it.
	drawing_deferred_markup: bool,
	/// How far the filters of the groups around the content being drawn read past it, in document pixels, which the maps and images inside cover past the view.
	pub(crate) ancestors_reach: f64,
	indent: usize,
}

impl SvgRender {
	pub fn new() -> Self {
		Self {
			svg: Vec::default(),
			svg_defs: String::new(),
			transform: DAffine2::IDENTITY,
			image_data: HashMap::new(),
			effect_maps: Vec::new(),
			deferred_images: Vec::new(),
			drawing_deferred_markup: false,
			ancestors_reach: 0.,
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
	pub effect_maps: Vec<EffectMap>,
	pub deferred_images: Vec<DeferredImage>,
}

impl From<&SvgRenderOutput> for SvgRender {
	fn from(value: &SvgRenderOutput) -> Self {
		Self {
			svg: vec![value.svg.clone().into()],
			svg_defs: value.svg_defs.clone(),
			transform: DAffine2::IDENTITY,
			image_data: value.image_data.clone(),
			effect_maps: value.effect_maps.clone(),
			deferred_images: value.deferred_images.clone(),
			drawing_deferred_markup: false,
			ancestors_reach: 0.,
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
			effect_maps: val.effect_maps,
			deferred_images: val.deferred_images,
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
	/// The view an SVG is shown through, mapping the document into its pixels, of which content filling the plane (like a generator) draws only the part in view, or nothing without one.
	pub view: Option<Footprint>,
	/// Set by an SVG render without a `view` that leaves out content drawing only the part of it in view, so its markup depends on the view.
	#[cache_hash(skip)]
	pub view_wanted: Arc<std::sync::atomic::AtomicBool>,
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
	/// Whether images are embedded in the markup instead of drawn to canvases, which don't paint within an SVG mask.
	pub embeds_images: bool,
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
		!self.for_export && !self.thumbnail && !self.for_mask && !self.inside_pattern && !self.embeds_images
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
		let (user_to_document, paint_reach) = (attributes.0.transform, attributes.0.ancestors_reach);
		let defs = &mut attributes.0.svg_defs;
		let fill_attribute = fill_paint
			.map(|paint| {
				paint.render(
					defs,
					user_to_document,
					paint_reach,
					item_transform,
					element_transform,
					applied_stroke_transform,
					bounds_matrix,
					render_params,
					PaintTarget::Fill,
				)
			})
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

/// Maps `transform` into the space of `reference`, exactly for the reference itself even when it can't be inverted.
fn transform_relative_to(reference: DAffine2, transform: DAffine2) -> DAffine2 {
	if transform == reference {
		DAffine2::IDENTITY
	} else if transform_is_invertible(reference) {
		reference.inverse() * transform
	} else {
		transform
	}
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

// TODO: Click targets can be removed from the render output, since the vector data is available in the vector modify data the editor observes on 'Path' nodes.
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

/// Leaves only the masks and feathers among a clipping base's effects, which shape what clips to it as they shape the base, unlike the shadows and glows it casts, which don't widen it.
fn keep_only_shaping<T>(masker: &mut Item<T>) {
	match masker.attribute::<Effects>(ATTR_EFFECTS).and_then(Effects::shaping_only) {
		Some(masks) => masker.set_attribute(ATTR_EFFECTS, masks),
		None => {
			masker.remove_attribute::<Effects>(ATTR_EFFECTS);
		}
	}
}

/// The clipping base an item makes of its `element` placed by `item_transform`: the element alone, or nested in a group taking the item's masks and feathers if it has any (see [`keep_only_shaping`]).
fn clipping_base<'a>(element: &'a Graphic, item_transform: DAffine2, effects: Option<&Effects>) -> Cow<'a, Graphic> {
	match effects.and_then(Effects::shaping_only) {
		Some(masks) => Cow::Owned(Graphic::Graphic(Box::new(
			Item::new_from_element(element.clone())
				.with_attribute(ATTR_TRANSFORM, item_transform)
				.with_attribute(ATTR_EFFECTS, masks),
		))),
		None => Cow::Borrowed(element),
	}
}

/// The region a mask of the `masker` placed by `transform` covers in the user space: its drawn bounds grown by the reach of the effects nested inside, or `None` without edges.
fn masker_region(masker: &Graphic, transform: DAffine2) -> Option<[DVec2; 2]> {
	let RenderBoundingBox::Rectangle([min, max]) = drawn_bounding_box(masker, transform) else {
		return None;
	};

	let reach = nested_effects_reach(masker, transform, EffectOperation::painted_reach);
	Some([min - reach, max + reach])
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
	let compositing = item.compositing(render_params, transform).clipped(mask_state.is_some());

	let matrix = format_transform_matrix(transform);
	let mut masked_by = None;

	if next_clips && mask_state.is_none() {
		let uuid = generate_uuid();
		let base = clipping_base(element, transform, item.attribute::<Effects>(ATTR_EFFECTS));
		let mask_type = if base.can_reduce_to_clip_path() { MaskType::Clip } else { MaskType::Mask };

		// A base nested to take the item's masks carries the item's transform itself
		let nested = matches!(base, Cow::Owned(_));
		let mut svg = SvgRender::new();
		svg.transform = if nested { render.transform } else { render.transform * transform };
		svg.ancestors_reach = render.ancestors_reach;
		base.render_svg(&mut svg, &render_params.for_clipper());

		// The def is resolved in this list's space, so the masker's own transform has to be baked into it
		let masker = match matrix.is_empty() || nested {
			true => svg.svg.to_svg_string(),
			false => format!(r##"<g transform="{matrix}">{}</g>"##, svg.svg.to_svg_string()),
		};

		render.svg_defs.push_str(&svg.svg_defs);
		render.effect_maps.extend(svg.effect_maps);
		mask_type.write_to_defs(&mut render.svg_defs, uuid, masker, masker_region(element, transform));

		*mask_state = Some((uuid, mask_type));
	} else if let Some((uuid, mask_type)) = *mask_state {
		if !next_clips {
			*mask_state = None;
		}

		masked_by = Some((mask_type.to_attribute(), format!("url(#mask-{uuid})")));
	}

	let render_item = |render: &mut SvgRender| {
		// The filter also covers how far the effects nested inside reach past the content, measured in this user space
		let effects = compositing.effects.map(|(effects, _)| effects);
		let bounds = || match drawn_bounding_box(element, transform) {
			RenderBoundingBox::Rectangle([min, max]) => {
				let nested_reach = nested_effects_reach(element, transform, EffectOperation::painted_reach);
				RenderBoundingBox::Rectangle([min - nested_reach, max + nested_reach])
			}
			// Masks can give content without edges, like a color layer, the edges its effects need
			RenderBoundingBox::Infinite => match effects.and_then(|effects| masks::masked_extent(effects, transform)) {
				Some(extent) => RenderBoundingBox::Rectangle(extent),
				None => RenderBoundingBox::Infinite,
			},
			bounds => bounds,
		};

		// Masks that leave it without edges wrap it in a group of their own, which takes the blend mode so it blends with what lies beneath
		let has_mask = effects.is_some_and(|effects| effects.operations().any(|operation| matches!(operation, EffectOperation::Mask { .. })));
		let endless_mask = match effects {
			Some(effects) if has_mask && matches!(bounds(), RenderBoundingBox::Infinite) => masks::write_endless_content_mask(render, effects, transform, render_params),
			_ => None,
		};
		let endless_masked = endless_mask.is_some();

		render_svg_within_mask(render, endless_mask, blend_mode, |render| {
			let deferred = start_deferred_markup(render, render_params, &compositing, bounds);
			let filters = compositing.svg_filters(render, render_params, bounds, |keeps| Graphic::Graphic(Box::new(effects_content(item, element.clone(), keeps))));
			let filtered = filters.is_some();

			render_svg_through_filters(render, filters, compositing.opacity, render_params.to_canvas(), |render| {
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

						if blend_mode != BlendMode::default() && !endless_masked {
							attributes.push("style", blend_mode.render());
						}
					},
					|render| {
						let (parent_transform, parent_reach) = (render.transform, render.ancestors_reach);
						render.transform = parent_transform * transform;
						if filtered {
							render.ancestors_reach += compositing.effects.map_or(0., |(effects, _)| effects.read_reach(render.transform));
						}
						element.render_svg(render, render_params);
						(render.transform, render.ancestors_reach) = (parent_transform, parent_reach);
					},
				);
			});

			finish_deferred_markup(render, deferred, render_params, &compositing, blend_mode, || {
				Graphic::Graphic(Box::new(without_opacity_and_blend_mode(item, element.clone())))
			});
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
	mask_element_and_transform: &mut Option<(Cow<'a, Graphic>, DAffine2)>,
	scene: &mut VelloSceneBuilder,
	parent_transform: DAffine2,
	render_params: &RenderParams,
) {
	let Some(element) = item.element() else { return };
	let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let transform = parent_transform * item_transform;
	let blend_mode_attr: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
	// This item's declared appearance (if any) cascades to descendants lacking their own
	let child_render_params = item
		.attribute::<Appearance>(ATTR_APPEARANCE)
		.and_then(Appearance::declared)
		.map(|appearance| render_params.for_child_item(appearance));
	let render_params = child_render_params.as_ref().unwrap_or(render_params);
	let compositing = item.compositing(render_params, transform).clipped(mask_element_and_transform.is_some());

	let blend_mode = match render_params.render_mode {
		RenderMode::Outline => peniko::Mix::Normal,
		_ => blend_mode_attr.to_peniko(),
	};
	let needs_layer = !compositing.effects_draw_compositing() && (compositing.opacity < 1. || (render_params.render_mode != RenderMode::Outline && blend_mode_attr != BlendMode::default()));

	// The bounds are only worth finding for the layers or effects group that need them
	let bounds = if needs_layer || mask_element_and_transform.is_some() || compositing.effects.is_some() {
		drawn_bounding_box(element, transform)
	} else {
		RenderBoundingBox::None
	};

	// The layers and effects group also cover how far the effects nested inside reach past the content
	let nested_reach = match bounds {
		RenderBoundingBox::Rectangle(_) => nested_effects_reach(element, transform, EffectOperation::painted_reach),
		_ => DVec2::ZERO,
	};
	let layer_rect = match bounds {
		RenderBoundingBox::Rectangle(bounds) => Some(inflate_bounds(bounds, compositing.reach() + nested_reach)),
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

	let effects_bounds = || match bounds {
		RenderBoundingBox::Rectangle(bounds) => rect_bounds(inflate_bounds(bounds, nested_reach)),
		bounds => bounds,
	};
	let draw_content = |scene: &mut VelloSceneBuilder| {
		compositing.draw_with_effects(scene, parent_transform, render_params, effects_bounds, |scene| element.render_to_vello(scene, transform, render_params));
	};

	if next_clips && mask_element_and_transform.is_none() {
		// A base nested to take the item's masks carries the item's transform itself
		let base = clipping_base(element, item_transform, item.attribute::<Effects>(ATTR_EFFECTS));
		let base_transform = if matches!(base, Cow::Owned(_)) { parent_transform } else { transform };
		*mask_element_and_transform = Some((base, base_transform));

		draw_content(scene);
	} else if let Some((mask_element, transform_mask)) = mask_element_and_transform.clone() {
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
		let own_reach = item.attribute::<Effects>(ATTR_EFFECTS).map_or(DVec2::ZERO, |effects| effects.reach_along_axes(footprint.transform));
		let reach = (own_reach + nested_effects_reach(element, footprint.transform, EffectOperation::painted_reach)).max_element();
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
			Graphic::Color(item) => render_color_item_to_vello(ItemRef::Item(item), scene, transform, render_params),
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

			// Background, which goes inside a clipping artboard's group since a clip path isolates what blends inside it from anything outside
			let background_rect = |render: &mut SvgRender, offset: DVec2| {
				render.leaf_tag("rect", |attributes| {
					attributes.push("fill", format!("#{}", SRGBA8::from(background).to_rgb_hex()));
					if background.a() < 1. {
						attributes.push("fill-opacity", ((background.a() * 1000.).round() / 1000.).to_string());
					}
					attributes.push("x", (x - offset.x).to_string());
					attributes.push("y", (y - offset.y).to_string());
					attributes.push("width", width.to_string());
					attributes.push("height", height.to_string());
				});
			};
			if !clip {
				background_rect(render, DVec2::ZERO);
			}

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
							r##"<clipPath id="{id}"><rect x="{}" y="{}" width="{width}" height="{height}" /></clipPath>"##,
							x - location.x,
							y - location.y,
						)
						.unwrap();
						attributes.push("clip-path", selector);
					}
				},
				// Artwork content
				|render| {
					if clip {
						background_rect(render, location);
					}

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
	let opacity = if filtered {
		1.
	} else {
		item.compositing(render_params, item_transform).opacity_without_effects()
	};

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

		let (user_to_document, paint_reach) = (attributes.0.transform, attributes.0.ancestors_reach);
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
					stroke.render(
						defs,
						user_to_document,
						paint_reach,
						item_transform,
						element_transform,
						applied_stroke_transform,
						bounds_matrix,
						&render_params,
						PaintTarget::Stroke,
					)
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
					paint.render(
						defs,
						user_to_document,
						paint_reach,
						item_transform,
						element_transform,
						applied_stroke_transform,
						paint_bounds,
						&render_params,
						PaintTarget::Stroke,
					)
				})
				.unwrap_or_else(|| r#" stroke="none""#.to_string())
		} else {
			String::new()
		};

		let fill_attribute = if needs_separate_alignment_fill || use_face_fill {
			r#" fill="none""#.to_string()
		} else {
			fill_paint
				.map(|paint| {
					paint.render(
						defs,
						user_to_document,
						paint_reach,
						item_transform,
						element_transform,
						applied_stroke_transform,
						bounds_matrix,
						&render_params,
						PaintTarget::Fill,
					)
				})
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
		let mut masker = Item::from_parts(vector.clone(), item.clone_item_attributes());
		keep_only_shaping(&mut masker);
		let masker = Graphic::VectorList(List::new_from_item(masker));
		let mask_type = if masker.can_reduce_to_clip_path() { MaskType::Clip } else { MaskType::Mask };
		let uuid = generate_uuid();

		let mut masker_svg = SvgRender::new();
		masker_svg.transform = render.transform;
		masker.render_svg(&mut masker_svg, &render_params.for_clipper());
		render.svg_defs.push_str(&masker_svg.svg_defs);
		render.effect_maps.extend(masker_svg.effect_maps);
		mask_type.write_to_defs(&mut render.svg_defs, uuid, masker_svg.svg.to_svg_string(), masker_region(&masker, DAffine2::IDENTITY));

		*clip_mask_state = Some((uuid, mask_type));
	} else if let Some((uuid, mask_type)) = *clip_mask_state {
		if !next_clips {
			*clip_mask_state = None;
		}

		masked_by = Some((mask_type.to_attribute(), format!("url(#mask-{uuid})")));
	}

	let compositing = item.compositing(render_params, item.attribute_cloned_or_default(ATTR_TRANSFORM)).clipped(masked_by.is_some());
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
		let bounds = || {
			let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
			let stroke = appearance().and_then(|appearance| appearance.fill_and_stroke().stroke);
			let paths = item.attribute::<List<Path>>(ATTR_PATHS).unwrap_or_else(|| no_paths());

			vector
				.stroke_inclusive_bounding_box_with_transform(paths, item_transform, stroke.as_ref())
				.map_or(RenderBoundingBox::None, RenderBoundingBox::Rectangle)
		};
		let deferred = start_deferred_markup(render, render_params, &compositing, bounds);
		// The filter's maps are of the effects that remain after those folded into the paints, which it numbers them by, and the folded ones keep alpha
		let filters = compositing.svg_filters(render, render_params, bounds, |keeps| {
			let mut content = effects_content(item, vector.clone(), keeps);
			content.set_attribute(ATTR_EFFECTS, compositing.effects.map(|(effects, _)| effects.clone()).unwrap_or_default());
			Graphic::Vector(Box::new(content))
		});
		let filtered = filters.is_some();
		let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);

		render_svg_through_filters(render, filters, compositing.opacity, render_params.to_canvas(), |render| {
			// The item's own filters read its paints around each pixel, so a pattern paint's images and maps cover past the view as far as they read
			let own_reach = if filtered {
				compositing.effects.map_or(0., |(effects, _)| effects.read_reach(render.transform * compositing.content_transform))
			} else {
				0.
			};
			render.ancestors_reach += own_reach;
			render_vector_shape_svg(item, vector, filtered, folded.as_ref(), render, render_params);
			render.ancestors_reach -= own_reach;
		});

		finish_deferred_markup(render, deferred, render_params, &compositing, blend_mode, || {
			Graphic::Vector(Box::new(without_opacity_and_blend_mode(item, vector.clone())))
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
	let multiplied_transform = parent_transform * item_transform;
	let compositing = item.compositing(render_params, multiplied_transform).clipped(clip_masker.is_some());

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
	if !overlaps_scene(scene, layer_bounds, multiplied_transform, stroke_inflation, element_transform, compositing.reach().length()) {
		if next_clips && clip_masker.is_none() {
			let mut masker = Item::from_parts(element.clone(), item.clone_item_attributes());
			keep_only_shaping(&mut masker);
			*clip_masker = Some(List::new_from_item(masker));
		} else if clip_masker.is_some() && !next_clips {
			*clip_masker = None;
		}
		return;
	}

	let Some(placed) = item.placed_contours(applied_stroke_transform) else { return };
	let path = placed.path();

	let opacity = compositing.opacity;
	let needs_blend_layer = !compositing.effects_draw_compositing() && (opacity < 1. || blend_mode_attr != BlendMode::default());

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

	// The layers also cover what the effects paint past the content
	let effects_inflation = compositing.reach_in_content_space();
	let layer_rect_with_effects = |layer_rect: kurbo::Rect| layer_rect.inflate(effects_inflation.x, effects_inflation.y);

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
		let mut masker = Item::from_parts(element.clone(), item.clone_item_attributes());
		keep_only_shaping(&mut masker);
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
			// The shape's own bounds as placed, which SVG also fits a paint overlay to
			let effects_bounds = || {
				let bounds = element.stroke_inclusive_bounding_box_with_transform(element_paths, multiplied_transform, stroke_params.as_ref());
				bounds.map_or(RenderBoundingBox::None, RenderBoundingBox::Rectangle)
			};
			compositing.draw_with_effects(scene, parent_transform, render_params, effects_bounds, |scene| {
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

			let source_paths = item.attribute::<List<Path>>(ATTR_PATHS).unwrap_or_else(|| no_paths());
			let item_relative_transform = transform_relative_to(reference_transform, transform);

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
	let compositing = item.compositing(render_params, transform);

	if image.data.is_empty() {
		return;
	}

	// SVG filters can't carry out every adjustment, so those leading the effects are carried out in the image's pixels
	let source_image = image;
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

	let bounds = || RenderBoundingBox::Rectangle((transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box());
	let deferred = start_deferred_markup(render, render_params, &compositing, bounds);
	// The filter's maps are of the adjusted image with the effects that remain, which the filter numbers them by
	let filters = compositing.svg_filters(render, render_params, bounds, |keeps| {
		let mut content = effects_content(item, image.clone(), keeps);
		content.set_attribute(ATTR_EFFECTS, compositing.effects.map(|(effects, _)| effects.clone()).unwrap_or_default());
		Graphic::RasterCPU(Box::new(content))
	});

	// A filter's wrapping group takes the opacity and blend mode instead
	let (opacity, blend_mode) = match filters {
		Some(_) => (1., BlendMode::default()),
		None => (compositing.opacity_without_effects(), blend_mode_attr),
	};

	render_svg_through_filters(render, filters, compositing.opacity, render_params.to_canvas(), |render| {
		render_raster_cpu_image_svg(image, transform, opacity, blend_mode, render, render_params)
	});

	// The GPU carries out the leading adjustments too, on the image as it was
	finish_deferred_markup(render, deferred, render_params, &compositing, blend_mode_attr, || {
		Graphic::RasterCPU(Box::new(without_opacity_and_blend_mode(item, source_image.clone())))
	});
}

/// Emits a CPU raster's image as SVG, either as a placeholder for a canvas or as an embedded PNG.
pub(crate) fn render_raster_cpu_image_svg(image: &Raster<CPU>, transform: DAffine2, opacity: f32, blend_mode: BlendMode, render: &mut SvgRender, render_params: &RenderParams) {
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
	let transform_attribute: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let compositing = item.compositing(render_params, transform * transform_attribute);
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
		// GPU passes, an effects group's included, leave fully transparent pixels empty, so adjustments changing alpha are carried out on the CPU like in SVG at any size
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
	if !compositing.effects_draw_compositing()
		&& (opacity < 1. || (render_params.render_mode != RenderMode::Outline && blend_mode_attr != BlendMode::default()))
		&& let RenderBoundingBox::Rectangle(bounds) = whole_bounds()
	{
		let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
		let rect = inflate_bounds(bounds, compositing.reach());
		scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &rect);
		layer = true;
	}

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
	compositing.draw_with_effects(scene, transform, render_params, item_bounds, |scene| {
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

	// An image of the area in view, grown by how far its own effects and the filters of the groups around it read, so they find no edges in view, stands in for it
	let own_reach = item
		.attribute::<Effects>(ATTR_EFFECTS)
		.map_or(0., |effects| effects.reach_along_axes_by(render.transform * transform, EffectOperation::working_reach).max_element());
	let Some((area, width, height)) = generator_area_in_view(transform, render.ancestors_reach + own_reach, render, render_params) else {
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
	// Its effects keep measuring in the generator's space, as if the generator's geometry had shrunk into the image's unit square
	let area_to_generator = DAffine2::from_translation(area[0]) * DAffine2::from_scale(area[1] - area[0]);
	let mut image = Item::from_parts(Raster::new_cpu(pixels), item.clone_item_attributes());
	image.set_attribute(ATTR_TRANSFORM, transform * area_to_generator);
	graphic_types::graphic::rebase_stamped_transforms(image.attributes_mut(), area_to_generator.inverse());
	render_raster_cpu_item_svg(ItemRef::Item(&image), render, render_params);
}

/// The part of a generator's own space in view, which `transform` places, grown past the view by `reach` in document pixels, with the pixels across and down it covers in the view.
fn generator_area_in_view(transform: DAffine2, reach: f64, render: &SvgRender, render_params: &RenderParams) -> Option<([DVec2; 2], u32, u32)> {
	// A thumbnail's footprint starts at the document area it shows, one pixel per unit, rather than mapping the document into a view
	let (footprint, document_to_view, max_pixels) = match (render_params.thumbnail, render_params.view) {
		(true, _) => (
			render_params.footprint,
			DAffine2::from_translation(-render_params.footprint.transform.translation),
			MAX_GENERATOR_THUMBNAIL_PIXELS,
		),
		(false, Some(view)) => (view, view.transform, MAX_GENERATOR_IMAGE_PIXELS),
		(false, None) => {
			render_params.view_wanted.store(true, std::sync::atomic::Ordering::Relaxed);
			return None;
		}
	};
	let local_to_view = document_to_view * render.transform * transform;
	if local_to_view.matrix2.determinant() == 0. || !local_to_view.is_finite() || footprint.resolution.min_element() <= 1 {
		return None;
	}

	let reach = reach * document_to_view.matrix2.determinant().abs().sqrt();
	let view = Quad::from_box([DVec2::splat(-reach), footprint.resolution.as_dvec2() + reach]);
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
	let local_to_scene = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
	let compositing = item.compositing(render_params, local_to_scene);
	let bounds = generator.bounding_box(local_to_scene, false);
	if bounds == RenderBoundingBox::None {
		return;
	}

	// Opacity and blend modes composite through a layer covering the whole render
	let mut layer = false;
	if !compositing.effects_draw_compositing() && (compositing.opacity < 1. || blend_mode_attr != BlendMode::default()) {
		let blending = peniko::BlendMode::new(blend_mode_attr.to_peniko(), peniko::Compose::SrcOver);
		scene.push_viewport_layer(blending, compositing.opacity);
		layer = true;
	}

	let pattern = GeneratorPattern::decode(generator, &|key| item.attribute_any(key));
	compositing.draw_with_effects(
		scene,
		transform,
		render_params,
		|| bounds,
		|scene| scene.draw_generator(&pattern, kurbo::Affine::new(local_to_scene.to_cols_array())),
	);

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
	let transform_attribute: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let compositing = item.compositing(render_params, transform * transform_attribute);
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
	// The effects group blends by the item's opacity and blend mode itself, if it takes them
	let (blend_mode, opacity) = match compositing.effects_draw_compositing() {
		true => (peniko::Mix::Normal, 1.),
		false => (blend_mode, compositing.opacity),
	};
	let any_nondefault = blend_mode != peniko::Mix::Normal || opacity < 1. || (clip_attr && !compositing.effects_draw_compositing());
	if (render_params.render_mode != RenderMode::Outline && any_nondefault)
		&& let RenderBoundingBox::Rectangle(bounds) = whole_bounds()
	{
		let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
		let rect = inflate_bounds(bounds, compositing.reach());
		scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &rect);
		layer = true;
	}

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
	compositing.draw_with_effects(scene, transform, render_params, item_bounds, |scene| {
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

	fn render_to_vello(&self, scene: &mut VelloSceneBuilder, parent_transform: DAffine2, render_params: &RenderParams) {
		for index in 0..self.len() {
			render_color_item_to_vello(ItemRef::ListItem(self, index), scene, parent_transform, render_params);
		}
	}
}

/// Emits one item of color content as SVG, painting a stand-in for an infinite background.
fn render_color_item_svg(item: ItemRef<'_, Color>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(&color) = item.element() else { return };
	let compositing = item.compositing(render_params, item.attribute_cloned_or_default(ATTR_TRANSFORM));

	// Leading masks that give the color edges let its effects be carried out within them like any other content's
	let extent = compositing.effects.and_then(|(effects, _)| masks::masked_extent(effects, compositing.placement));
	if let Some(extent) = extent {
		let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
		let bounds = || RenderBoundingBox::Rectangle(extent);
		let deferred = start_deferred_markup(render, render_params, &compositing, bounds);
		let filters = compositing.svg_filters_within(render, render_params, extent, None, |keeps| Graphic::Color(effects_content(item, color, keeps)));

		// Without filters, as in an effect space that collapses, the color takes its opacity and blend mode as if its masks left it without edges
		let filtered = filters.is_some();
		render_svg_through_filters(render, filters, compositing.opacity, render_params.to_canvas(), |render| match filtered {
			true => emit_endless_color(render, color, 1., BlendMode::Normal, None),
			false => render_edgeless_color_svg(render, &compositing, color, render_params),
		});
		finish_deferred_markup(render, deferred, render_params, &compositing, blend_mode, || {
			Graphic::Color(without_opacity_and_blend_mode(item, color))
		});
		return;
	}

	render_edgeless_color_svg(render, &compositing, color, render_params);
}

/// Emits a color filling everything, which its masks may confine, with its compositing carried out on the color and its masks rather than by filters.
fn render_edgeless_color_svg(render: &mut SvgRender, compositing: &ItemCompositing, color: Color, render_params: &RenderParams) {
	// SVG filters change nothing visible about an endless color, so its adjustments are carried out on the color itself
	let paint = compositing.endless_paint();
	let shown = filled_color(color, &paint.adjustments, paint.fill);
	let full_alpha = adjusted_color(color, &paint.adjustments).a();

	// The masks among its effects wrap it in a group, which takes its blend mode so it blends with what lies beneath the group
	let mask = (compositing.effects).and_then(|(effects, _)| masks::write_endless_content_mask(render, effects, compositing.placement, render_params));
	let content_blend_mode = if mask.is_some() { BlendMode::Normal } else { paint.blend_mode };
	render_svg_within_mask(render, mask, paint.blend_mode, |render| {
		emit_endless_color(render, shown, paint.opacity, content_blend_mode, None);
		for &(overlay, blend_mode) in &paint.overlays {
			emit_endless_color(render, overlay.with_alpha(overlay.a() * full_alpha), paint.opacity, blend_mode, None);
		}
	});
}

/// Emits a stand-in for an infinite background, or the thumbnail's `rect` from its corner and size, filled with `color` in a layer of `opacity` and `blend_mode`.
fn emit_endless_color(render: &mut SvgRender, color: Color, opacity: f32, blend_mode: BlendMode, rect: Option<(DVec2, DVec2)>) {
	render.leaf_tag(if rect.is_some() { "rect" } else { "polyline" }, |attributes| {
		emit_endless_shape(attributes, rect);

		attributes.push("fill", format!("#{}", SRGBA8::from(color).to_rgb_hex()));
		if color.a() < 1. {
			attributes.push("fill-opacity", ((color.a() * 1000.).round() / 1000.).to_string());
		}
		if opacity < 1. {
			attributes.push("opacity", opacity.to_string());
		}
		if blend_mode != BlendMode::default() {
			attributes.push("style", blend_mode.render());
		}
	});
}

/// How far a stand-in for endless content, like a color or gradient filling everything, extends each way.
/// Chrome's SVG renderer keeps internal coordinates in f32 and loses precision past about 2^24, causing tile-boundary artifacts that pop in and out during panning, so this stays under that while far larger than any practical document extent.
pub(crate) const ENDLESS_EXTENT: f64 = 1e7;

/// Pushes the geometry of a stand-in for an infinite background, or of the thumbnail's `rect` from its corner and size.
fn emit_endless_shape(attributes: &mut SvgRenderAttrs, rect: Option<(DVec2, DVec2)>) {
	if let Some((min, size)) = rect {
		attributes.push("x", min.x.to_string());
		attributes.push("y", min.y.to_string());
		attributes.push("width", size.x.to_string());
		attributes.push("height", size.y.to_string());
		return;
	}

	let extent = ENDLESS_EXTENT;
	attributes.push("points", format!("{extent},{extent} -{extent},{extent} -{extent},-{extent} {extent},-{extent}"));
}

/// Draws an endless item's layer, whose content `draw` fills the viewport with, in `opacity` and `blend_mode`.
fn draw_endless_layer(scene: &mut VelloSceneBuilder, opacity: f32, blend_mode: BlendMode, draw: impl FnOnce(&mut VelloSceneBuilder)) {
	let layer = opacity < 1. || blend_mode != BlendMode::default();
	if layer {
		scene.push_viewport_layer(peniko::BlendMode::new(blend_mode.to_peniko(), peniko::Compose::SrcOver), opacity);
	}
	draw(scene);
	if layer {
		scene.pop_layer();
	}
}

/// The masks among an endless item's effects, if it has any, which wrap its paint as SVG's mask group does, with its other effects left undone.
fn endless_masks(compositing: &ItemCompositing) -> Option<Effects> {
	let (effects, _) = compositing.effects?;
	let mask_rows = (effects.operations().enumerate()).filter(|(_, operation)| matches!(operation, EffectOperation::Mask { .. }));
	let masks = Effects(mask_rows.filter_map(|(index, _)| effects.0.clone_item(index)).collect());
	(!masks.is_empty()).then_some(masks)
}

/// Draws the layers of an endless item that `draw` draws through its `masks` (see [`endless_masks`]), if any, in a layer taking the `blend_mode` like SVG's mask group.
fn draw_through_endless_masks(
	scene: &mut VelloSceneBuilder,
	compositing: &ItemCompositing,
	masks: Option<&Effects>,
	blend_mode: BlendMode,
	parent_transform: DAffine2,
	render_params: &RenderParams,
	mut draw: impl FnMut(&mut VelloSceneBuilder),
) {
	let Some(masks) = masks else { return draw(scene) };

	let masking = ItemCompositing {
		opacity: 1.,
		blend_mode,
		effects: Some((masks, 1.)),
		layered: true,
		placement: compositing.placement,
		content_transform: compositing.content_transform,
	};
	masking.draw_with_effects(scene, parent_transform, render_params, || RenderBoundingBox::Infinite, draw);
}

/// Draws an endless item through its effects group, for leading masks that give it edges (see [`masks::masked_extent`]), pushing a layer of its own for its opacity and blend mode unless the group draws them.
fn draw_masked_endless_item(
	scene: &mut VelloSceneBuilder,
	compositing: &ItemCompositing,
	blend_mode: BlendMode,
	parent_transform: DAffine2,
	render_params: &RenderParams,
	draw: impl FnMut(&mut VelloSceneBuilder),
) {
	let layer = !compositing.effects_draw_compositing() && (compositing.opacity < 1. || blend_mode != BlendMode::default());
	if layer {
		scene.push_viewport_layer(peniko::BlendMode::new(blend_mode.to_peniko(), peniko::Compose::SrcOver), compositing.opacity);
	}
	compositing.draw_with_effects(scene, parent_transform, render_params, || RenderBoundingBox::Infinite, draw);
	if layer {
		scene.pop_layer();
	}
}

/// Draws one item of color content into the Vello scene, covering the whole render area, as SVG does.
fn render_color_item_to_vello(item: ItemRef<'_, Color>, scene: &mut VelloSceneBuilder, parent_transform: DAffine2, render_params: &RenderParams) {
	let Some(&color) = item.element() else { return };
	let compositing = item.compositing(render_params, parent_transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM));

	// Leading masks that give the color edges let its effects be carried out within them like any other content's, as in SVG
	if compositing.effects.is_some_and(|(effects, _)| masks::masked_extent(effects, compositing.placement).is_some()) {
		let vello_color = SRGBA8::from(color).to_peniko_color();
		let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
		return draw_masked_endless_item(scene, &compositing, blend_mode, parent_transform, render_params, |scene| scene.fill_viewport(vello_color, None));
	}

	let paint = compositing.endless_paint();
	let shown = SRGBA8::from(filled_color(color, &paint.adjustments, paint.fill)).to_peniko_color();
	let full_alpha = adjusted_color(color, &paint.adjustments).a();

	let masks = endless_masks(&compositing);
	let content_blend_mode = if masks.is_some() { BlendMode::Normal } else { paint.blend_mode };
	draw_through_endless_masks(scene, &compositing, masks.as_ref(), paint.blend_mode, parent_transform, render_params, |scene| {
		draw_endless_layer(scene, paint.opacity, content_blend_mode, |scene| scene.fill_viewport(shown, None));
		for &(overlay, blend_mode) in &paint.overlays {
			let overlay = SRGBA8::from(overlay.with_alpha(overlay.a() * full_alpha)).to_peniko_color();
			draw_endless_layer(scene, paint.opacity, blend_mode, |scene| scene.fill_viewport(overlay, None));
		}
	});
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
	let Some(source_gradient) = item.element() else { return };
	let compositing = item.compositing(render_params, item.attribute_cloned_or_default(ATTR_TRANSFORM));
	let settings = gradient_settings_from_item(item);

	// Leading masks that give the gradient edges let its effects be carried out within them like any other content's
	let extent = compositing.effects.and_then(|(effects, _)| masks::masked_extent(effects, compositing.placement));
	if let Some(extent) = extent.filter(|_| thumbnail_rect.is_none()) {
		let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
		let bounds = || RenderBoundingBox::Rectangle(extent);
		let deferred = start_deferred_markup(render, render_params, &compositing, bounds);
		let filters = compositing.svg_filters_within(render, render_params, extent, None, |keeps| Graphic::Gradient(effects_content(item, source_gradient.clone(), keeps)));

		// Without filters, as in an effect space that collapses, the gradient takes its opacity and blend mode as if its masks left it without edges
		let filtered = filters.is_some();
		render_svg_through_filters(render, filters, compositing.opacity, render_params.to_canvas(), |render| match filtered {
			true => render_gradient_shape_svg(item, source_gradient, settings, None, 1., BlendMode::Normal, render, render_params),
			false => render_edgeless_gradient_svg(item, source_gradient, settings, None, &compositing, render, render_params),
		});
		finish_deferred_markup(render, deferred, render_params, &compositing, blend_mode, || {
			Graphic::Gradient(without_opacity_and_blend_mode(item, source_gradient.clone()))
		});
		return;
	}

	render_edgeless_gradient_svg(item, source_gradient, settings, thumbnail_rect, &compositing, render, render_params);
}

/// Emits a gradient filling everything, or its thumbnail's rectangle, which its masks may confine, with its compositing carried out along its ramp and by its masks rather than by filters.
fn render_edgeless_gradient_svg(
	item: ItemRef<'_, Gradient>,
	source_gradient: &Gradient,
	settings: GradientSettings,
	thumbnail_rect: Option<(DVec2, DVec2)>,
	compositing: &ItemCompositing,
	render: &mut SvgRender,
	render_params: &RenderParams,
) {
	// SVG filters change nothing visible about an endless gradient, so its adjustments are carried out along its ramp
	let paint = compositing.endless_paint();
	let (fill, opacity) = paint.ramp_fill_and_opacity();
	let (gradient, settings) = match adjusted_gradient(source_gradient, settings, &paint.adjustments, fill) {
		Some((adjusted, settings)) => (Cow::Owned(adjusted), settings),
		None => (Cow::Borrowed(source_gradient), settings),
	};

	// The masks among its effects wrap it in a group, which takes its blend mode so it blends with what lies beneath the group
	let mask = (compositing.effects).and_then(|(effects, _)| masks::write_endless_content_mask(render, effects, compositing.placement, render_params));
	let content_blend_mode = if mask.is_some() { BlendMode::Normal } else { paint.blend_mode };
	render_svg_within_mask(render, mask, paint.blend_mode, |render| {
		render_gradient_shape_svg(item, &gradient, settings, thumbnail_rect, opacity, content_blend_mode, render, render_params);
		for &(overlay, blend_mode) in &paint.overlays {
			emit_endless_color(render, overlay, paint.opacity, blend_mode, thumbnail_rect);
		}
	});
}

/// Emits the shape a gradient item fills, its thumbnail's rectangle or a stand-in for an endless area, filled by the `gradient` read with its `settings`, faded by `opacity` and mixed by `blend_mode`.
#[allow(clippy::too_many_arguments)]
fn render_gradient_shape_svg(
	item: ItemRef<'_, Gradient>,
	gradient: &Gradient,
	settings: GradientSettings,
	thumbnail_rect: Option<(DVec2, DVec2)>,
	opacity: f32,
	blend_mode: BlendMode,
	render: &mut SvgRender,
	render_params: &RenderParams,
) {
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
	let tag = if thumbnail_rect.is_some() { "rect" } else { "polyline" };

	render.leaf_tag(tag, |attributes| {
		emit_endless_shape(attributes, thumbnail_rect);

		let samples = spread_adjusted_samples(gradient, settings, gradient_form);

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

	let Some(source_gradient) = item.element() else { return };
	let compositing = item.compositing(render_params, parent_transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM));
	let settings = gradient_settings_from_item(item);

	// Leading masks that give the gradient edges let its effects be carried out within them like any other content's, as in SVG
	if compositing.effects.is_some_and(|(effects, _)| masks::masked_extent(effects, compositing.placement).is_some()) {
		let (brush, brush_transform) = gradient_brush(item, source_gradient, settings, parent_transform);
		let blend_mode: BlendMode = item.attribute_cloned_or_default(ATTR_BLEND_MODE);
		return draw_masked_endless_item(scene, &compositing, blend_mode, parent_transform, render_params, |scene| {
			scene.fill_viewport(&brush, Some(brush_transform))
		});
	}

	// Its adjustments are carried out along the ramp, as in SVG
	let paint = compositing.endless_paint();
	let (fill, opacity) = paint.ramp_fill_and_opacity();
	let (gradient, settings) = match adjusted_gradient(source_gradient, settings, &paint.adjustments, fill) {
		Some((adjusted, settings)) => (Cow::Owned(adjusted), settings),
		None => (Cow::Borrowed(source_gradient), settings),
	};
	let (brush, brush_transform) = gradient_brush(item, &gradient, settings, parent_transform);

	let masks = endless_masks(&compositing);
	let content_blend_mode = if masks.is_some() { BlendMode::Normal } else { paint.blend_mode };
	draw_through_endless_masks(scene, &compositing, masks.as_ref(), paint.blend_mode, parent_transform, render_params, |scene| {
		draw_endless_layer(scene, opacity, content_blend_mode, |scene| scene.fill_viewport(&brush, Some(brush_transform)));
		for &(overlay, blend_mode) in &paint.overlays {
			draw_endless_layer(scene, paint.opacity, blend_mode, |scene| scene.fill_viewport(SRGBA8::from(overlay).to_peniko_color(), None));
		}
	});
}

/// The brush filling the viewport with a gradient item's `gradient` read with its `settings`, and the brush's transform, given the transform placing the item's space.
fn gradient_brush(item: ItemRef<'_, Gradient>, gradient: &Gradient, settings: GradientSettings, parent_transform: DAffine2) -> (peniko::Brush, kurbo::Affine) {
	let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let samples = spread_adjusted_samples(gradient, settings, gradient_form);

	let brush = peniko::Brush::Gradient(peniko::Gradient {
		kind: unit_gradient_kind(gradient_form),
		stops: peniko_color_stops(&samples),
		extend: peniko_extend(settings.spread),
		interpolation_alpha_space: peniko::InterpolationAlphaSpace::Unpremultiplied,
		..Default::default()
	});
	let brush_transform = kurbo::Affine::new(gradient_placement(parent_transform * transform, gradient_form).to_cols_array());
	(brush, brush_transform)
}

/// The metadata pass over a run of gradient items: each contributes its control geometry as targets under the
/// run's `element_id`, baked relative to the first item's transform (recorded as its `local_transforms` entry).
fn collect_gradient_items_metadata<'a>(items: impl Iterator<Item = ItemRef<'a, Gradient>>, metadata: &mut RenderMetadata, element_id: Option<NodeId>) {
	let Some(element_id) = element_id else { return };

	let mut item_zero_transform = None;
	let mut outline_targets = Vec::new();
	let mut click_targets = Vec::new();
	for item in items {
		let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
		let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);

		// The first item's transform is the reference all targets bake against
		let item_zero_transform = *item_zero_transform.get_or_insert(item_transform);

		let mut target = ClickTarget::new_with_path(gradient_control_outline(gradient_form), 0.);
		target.apply_transform(transform_relative_to(item_zero_transform, item_transform));
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
	let normalized_coords: Vec<NormalizedCoord> = run.normalized_coords().iter().map(|c| NormalizedCoord::from_bits(c.to_bits())).collect();

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
		accumulate_text_item_bounds(ItemRef::ListItem(list, index), outer_transform, false, &mut bounds);
	}
	match bounds {
		Some(bounds) => RenderBoundingBox::Rectangle(bounds),
		None => RenderBoundingBox::None,
	}
}

/// Folds one laid-out text item's corner points into the running bounds, padded by its font size for glyphs overhanging their layout box if `padded`.
fn accumulate_text_item_bounds(item: ItemRef<'_, String>, outer_transform: DAffine2, padded: bool, bounds: &mut Option<[DVec2; 2]>) {
	let Some((size, transform)) = text_item_size_and_transform(item) else { return };
	let padding = if padded { item.attribute_cloned_or(ATTR_FONT_SIZE, DEFAULT_FONT_SIZE) } else { 0. };
	let (min, max) = (DVec2::splat(-padding), size + padding);

	let full_transform = outer_transform * transform;
	for corner in [min, DVec2::new(max.x, min.y), DVec2::new(min.x, max.y), max] {
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
			accumulate_text_item_bounds(ItemRef::Item(item), item_transform, false, &mut bounds);
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
		Graphic::Generator(item) => generator_thumbnail_bounding_box(ItemRef::Item(item), item_transform),
		Graphic::GeneratorList(list) => {
			(0..list.len())
				.map(|index| generator_thumbnail_bounding_box(ItemRef::ListItem(list, index), item_transform))
				.fold(RenderBoundingBox::None, |combined, bounds| match (combined, bounds) {
					(RenderBoundingBox::Rectangle(a), RenderBoundingBox::Rectangle(b)) => RenderBoundingBox::Rectangle(Quad::combine_bounds(a, b)),
					(RenderBoundingBox::Rectangle(bounds), _) | (_, RenderBoundingBox::Rectangle(bounds)) => RenderBoundingBox::Rectangle(bounds),
					(RenderBoundingBox::Infinite, _) | (_, RenderBoundingBox::Infinite) => RenderBoundingBox::Infinite,
					_ => RenderBoundingBox::None,
				})
		}
		other => other.thumbnail_bounding_box(item_transform, true),
	}
}

/// A generator item's thumbnail bounds, framing the part of its pattern worth showing, like a fractal's body, or the whole plane for a pattern alike across it.
fn generator_thumbnail_bounding_box(item: ItemRef<'_, Generator>, transform: DAffine2) -> RenderBoundingBox {
	let Some(&generator) = item.element() else { return RenderBoundingBox::None };
	let item_transform = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);

	let pattern = GeneratorPattern::decode(generator, &|key| item.attribute_any(key));
	match raster_kernels::generators::thumbnail_area(&pattern) {
		Some(area) if item_transform.matrix2.determinant() != 0. => RenderBoundingBox::Rectangle((item_transform * Quad::from_box(area)).bounding_box()),
		_ => generator.thumbnail_bounding_box(item_transform, true),
	}
}

/// The bounds a graphic draws within, like the `BoundingBox` trait's with strokes included, but laying out its text, which that reports as `None`,
/// and confining content without edges, like a color layer, to the extent its leading masks give it. O(items), with a layout for each text item.
fn drawn_bounding_box(graphic: &Graphic, transform: DAffine2) -> RenderBoundingBox {
	fn confined(bounds: RenderBoundingBox, effects: Option<&Effects>, placement: DAffine2) -> RenderBoundingBox {
		match (bounds, effects.and_then(|effects| masks::masked_extent(effects, placement))) {
			(RenderBoundingBox::Infinite, Some(extent)) => RenderBoundingBox::Rectangle(extent),
			(bounds, _) => bounds,
		}
	}
	fn combined(rows: impl Iterator<Item = RenderBoundingBox>) -> RenderBoundingBox {
		let mut combined = RenderBoundingBox::None;
		for bounds in rows {
			combined = match (combined, bounds) {
				(RenderBoundingBox::Infinite, _) | (_, RenderBoundingBox::Infinite) => return RenderBoundingBox::Infinite,
				(RenderBoundingBox::Rectangle(existing), RenderBoundingBox::Rectangle(bounds)) => RenderBoundingBox::Rectangle(Quad::combine_bounds(existing, bounds)),
				(RenderBoundingBox::None, bounds) | (bounds, RenderBoundingBox::None) => bounds,
			};
		}
		combined
	}
	// Each row of a list of edgeless content, confined by its own masks
	fn edgeless_rows<T>(list: &List<T>, transform: DAffine2) -> RenderBoundingBox {
		combined((0..list.len()).map(|index| {
			let placement = transform * list.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, index);
			confined(RenderBoundingBox::Infinite, list.attribute::<Effects>(ATTR_EFFECTS, index), placement)
		}))
	}

	let mut text_bounds = None;
	match graphic {
		Graphic::Text(item) => accumulate_text_item_bounds(ItemRef::Item(item), transform, true, &mut text_bounds),
		Graphic::TextList(list) => (0..list.len()).for_each(|index| accumulate_text_item_bounds(ItemRef::ListItem(list, index), transform, true, &mut text_bounds)),
		Graphic::Graphic(item) => {
			let placement = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
			return confined(drawn_bounding_box(item.element(), placement), item.attribute::<Effects>(ATTR_EFFECTS), placement);
		}
		Graphic::GraphicList(list) => {
			return combined((0..list.len()).filter_map(|index| {
				let element = list.element(index)?;
				let placement = transform * list.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, index);
				Some(confined(drawn_bounding_box(element, placement), list.attribute::<Effects>(ATTR_EFFECTS, index), placement))
			}));
		}
		Graphic::Color(item) => {
			let placement = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
			return confined(graphic.bounding_box(transform, true), item.attribute::<Effects>(ATTR_EFFECTS), placement);
		}
		Graphic::Gradient(item) => {
			let placement = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
			return confined(graphic.bounding_box(transform, true), item.attribute::<Effects>(ATTR_EFFECTS), placement);
		}
		Graphic::ColorList(list) => return edgeless_rows(list, transform),
		Graphic::GradientList(list) => return edgeless_rows(list, transform),
		other => return other.bounding_box(transform, true),
	}

	match text_bounds {
		Some(bounds) => RenderBoundingBox::Rectangle(bounds),
		None => RenderBoundingBox::None,
	}
}

/// Emits one item of text content as SVG, laying out its glyphs and wrapping them in a styled group.
fn render_text_item_svg(item: ItemRef<'_, String>, render: &mut SvgRender, render_params: &RenderParams) {
	let Some(text) = item.element() else { return };
	if text.is_empty() {
		return;
	}

	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let compositing = item.compositing(render_params, transform);
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

	let deferred = start_deferred_markup(render, render_params, &compositing, || layout_bounds);
	let filters = compositing.svg_filters(render, render_params, || layout_bounds, |keeps| Graphic::Text(effects_content(item, text.clone(), keeps)));
	let filtered = filters.is_some();

	// Wrap all glyph <path> elements in a <g> with the item's transform/opacity/blend-mode, which a filter's wrapping group takes instead
	render_svg_through_filters(render, filters, compositing.opacity, render_params.to_canvas(), |render| {
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
				for path_d in &glyph_paths {
					render.leaf_tag("path", |attributes| {
						attributes.push("d", path_d.clone());
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

	finish_deferred_markup(render, deferred, render_params, &compositing, blend_mode_attr, || {
		Graphic::Text(without_opacity_and_blend_mode(item, text.clone()))
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
	let compositing = item.compositing(render_params, transform * item_transform);
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

		let needs_layer = !compositing.effects_draw_compositing() && (opacity < 1. || blend_mode_attr != BlendMode::default());
		if needs_layer {
			let blending = peniko::BlendMode::new(blend_mode_attr.to_peniko(), peniko::Compose::SrcOver);
			let reach = compositing.reach();
			scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &transformed_bounds().inflate(reach.x, reach.y));
		}

		let tilt_tan = letter_tilt.to_radians().tan();

		compositing.draw_with_effects(
			scene,
			transform,
			render_params,
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

	let mut accumulated_click_targets: HashMap<NodeId, Vec<Arc<ClickTarget>>> = HashMap::new();

	for item in items {
		// The first item's transform is the reference all targets bake against
		let item_zero_transform = *item_zero_transform.get_or_insert_with(|| item.attribute_cloned_or_default(ATTR_TRANSFORM));

		let Some(element_id) = caller_element_id.or(item.layer()) else { continue };

		// When recovering element_id from the item's tag (caller passed None), also store the transform metadata.
		if caller_element_id.is_none() {
			metadata.upstream_footprints.entry(element_id).or_insert(footprint);
			metadata.local_transforms.entry(element_id).or_insert(item_zero_transform);
		}

		let Some((size, item_transform)) = text_item_size_and_transform(item) else { continue };
		let mut target = ClickTarget::new_with_path(rectangle_path(DVec2::ZERO, size), 0.);
		target.apply_transform(transform_relative_to(item_zero_transform, item_transform));
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

	/// Writes the filters of each layer `effects` draw on content blending normally, in order, as a run starting at `capture_offset` in its item's list.
	fn write_filters(defs: &mut String, effects: &Effects, fill: f32, bounds: [DVec2; 2], capture_offset: usize, paints: &[String]) -> Vec<SvgEffectsFilter> {
		let layers = EffectLayers::plan(effects, BlendMode::Normal, fill, true);
		(0..layers.layers.len())
			.filter_map(|layer| write_svg_effects_filter(defs, effects, &layers, layer, fill, 1., bounds, capture_offset, paints, 0..effects.len()))
			.collect()
	}

	/// A hard-edged drop shadow of `size` falling at `distance` from the content in the direction `angle`.
	fn drop_shadow(angle: f64, distance: f64, size: f64) -> graphic_types::EffectOperation {
		graphic_types::EffectOperation::DropShadow {
			color: Color::BLACK,
			blend_mode: BlendMode::Multiply,
			angle,
			distance,
			spread: 0.,
			size,
			contour: Default::default(),
			knockout: true,
		}
	}

	#[test]
	fn svg_effects_filters_take_shadows_as_maps_the_gpu_renders() {
		let mut defs = String::new();
		let [filter] = write_filters(&mut defs, &Effects::from_operations([drop_shadow(180., 4., 2.)]), 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[])
			.try_into()
			.unwrap_or_else(|_| panic!("a black shadow multiplying draws as one layer"));

		// The shadow's coverage waits in a placeholder that leaves nothing covered, which its map replaces
		assert_eq!(filter.maps.len(), 1);
		assert_eq!(filter.maps[0].capture, EffectCapture { index: 0, plane: 0 });
		assert!(
			defs.contains(&format!(r#"<!--effect-map-{}--><feFlood flood-opacity="0" result="{}" />"#, filter.maps[0].id, filter.maps[0].result)),
			"{defs}"
		);
	}

	#[test]
	fn svg_effects_filters_measure_lengths_as_given_in_their_effect_space() {
		let mut defs = String::new();

		// The filter's user space is the effects' own, so the shadow's 7 unit reach is 7 units, and its map is numbered by the run's place in the list
		let effects = Effects::from_operations([drop_shadow(180., 4., 2.)]);
		let filters = write_filters(&mut defs, &effects, 1., [DVec2::ZERO, DVec2::splat(10.)], 2, &[]);

		assert!(defs.contains(r#"x="-7" y="-7" width="24" height="24""#), "{defs}");
		assert_eq!(filters[0].region, [DVec2::splat(-7.), DVec2::splat(17.)]);
		assert_eq!(filters[0].maps[0].capture, EffectCapture { index: 2, plane: 0 });
	}

	/// The SVG of a 10 unit orange square blurred by 3 units, placed by a scale of 2 applied before the blur, or after it when `scaled_after`.
	fn blurred_square_scaled_svg(scaled_after: bool) -> String {
		use graphic_types::{Effect, EffectOperation, stamp_effect};

		let blur = EffectOperation::Blur {
			radius: 3.,
			box_blur: false,
			gamma: false,
		};
		let mut item = orange_square_item(&[], 1., 1.);
		if !scaled_after {
			item.set_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(2.)));
		}
		stamp_effect(&mut item, Effect::new(&blur));
		if scaled_after {
			item.set_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(2.)));
		}

		let mut render = SvgRender::new();
		List::new_from_item(item).render_svg(&mut render, &RenderParams::default());
		render.svg.to_svg_string()
	}

	#[test]
	fn svg_effects_before_a_transform_scale_with_it() {
		// The filter measures in the square's own units, which the group around it scales up by 2, before the square returns to the user space inside
		let svg = blurred_square_scaled_svg(true);
		assert!(svg.contains(r#"<g transform="matrix(2,0,0,2,0,0)" filter="url(#effects-"#), "{svg}");
		assert!(svg.contains(r#"<g transform="matrix(0.5,0,0,0.5,0,0)">"#), "{svg}");
	}

	#[test]
	fn svg_effects_after_a_transform_measure_as_viewed() {
		// The blur was stamped on the scaled square, so its filter measures in the user space as it's viewed, with no group transforming into it
		let svg = blurred_square_scaled_svg(false);
		assert!(svg.contains(r#"<g filter="url(#effects-"#), "{svg}");
		assert!(!svg.contains("matrix(0.5"), "{svg}");
	}

	#[test]
	fn svg_maps_draw_their_content_where_it_lies_whatever_space_their_filter_measures_in() {
		use graphic_types::{Effect, stamp_effect};

		// The shadow's filter measures in the square's own units, which a scale applied after stamping doubles, while its map draws the square where it lies
		let mut item = orange_square_item(&[], 1., 1.);
		stamp_effect(&mut item, Effect::new(&drop_shadow(135., 10., 2.)));
		item.set_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(2.)));

		let mut render = SvgRender::new();
		List::new_from_item(item).render_svg(&mut render, &RenderParams::default());
		assert_eq!(render.effect_maps.len(), 1);
		assert_eq!(render.effect_maps[0].user_to_document, DAffine2::from_scale(DVec2::splat(2.)));
		assert_eq!(render.effect_maps[0].content_to_document, DAffine2::IDENTITY);
	}

	#[test]
	fn svg_effects_on_content_left_as_stamped_add_no_groups() {
		use graphic_types::raster_types::Image;
		use graphic_types::{Effect, stamp_effect};

		// An image's pixel scale times its inverse rounds away from the identity, which mustn't set up groups or move its map's content
		let placement = DAffine2::from_scale_angle_translation(DVec2::new(4000., 3000.), 10_f64.to_radians(), DVec2::new(250., 100.));
		let mut image = Item::new_from_element(Raster::new_cpu(Image::new(2, 2, Color::WHITE))).with_attribute(ATTR_TRANSFORM, placement);
		stamp_effect(&mut image, Effect::new(&drop_shadow(135., 10., 2.)));

		let mut render = SvgRender::new();
		List::new_from_element(Graphic::RasterCPU(Box::new(image))).render_svg(&mut render, &RenderParams::default());
		let svg = render.svg.to_svg_string();
		assert!(svg.contains(r#"<g filter="url(#effects-"#), "{svg}");
		assert_eq!(render.effect_maps.len(), 1);
		assert_eq!(render.effect_maps[0].user_to_document, DAffine2::IDENTITY);
		assert_eq!(render.effect_maps[0].content_to_document, DAffine2::IDENTITY);
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
		write_filters(&mut defs, &Effects::from_operations([blur]), 0.5, [DVec2::ZERO, DVec2::ONE], 0, &[]);

		// The fill fades a copy of the content, which is blurred in linear light and passed through as the filter's last result
		assert!(defs.contains(r#"<feFuncA type="linear" slope="0.5" />"#), "{defs}");
		assert!(defs.contains(r#"in="effect-1" stdDeviation="3 3" color-interpolation-filters="linearRGB""#), "{defs}");
		assert!(defs.ends_with(r#"<feOffset in="effect-2" result="effect-4" /></filter>"#), "{defs}");
	}

	#[test]
	fn svg_effects_filters_map_glows_satins_outlines_and_bevels_and_embed_paint_overlays() {
		use graphic_types::effects::GRADIENT_TABLE_SIZE;
		use graphic_types::vector_types::vector::style::StrokeAlign;
		use graphic_types::{BevelStyle, EffectKind, EffectOperation};
		use vector_types::GradientStop;

		// The planes of the maps the GPU renders for an effect, across the filters of all its layers
		let planes = |operation| {
			let filters = write_filters(&mut String::new(), &Effects::from_operations([operation]), 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[]);
			filters.iter().flat_map(|filter| filter.maps.iter().map(|map| map.capture.plane)).collect::<Vec<_>>()
		};

		// Inner glows and satins shade within the content from one map, and an outline takes one map beneath the content and another within it
		assert_eq!(planes(EffectOperation::default_for(EffectKind::InnerGlow)), [0]);
		assert_eq!(planes(EffectOperation::default_for(EffectKind::Satin)), [0]);
		let outline = |paint: Graphic, align, contour_gradient| EffectOperation::Outline {
			paint,
			blend_mode: BlendMode::Normal,
			weight: 4.,
			align,
			contour_gradient,
		};
		let black = || Graphic::from(Color::BLACK);
		assert_eq!(planes(outline(black(), StrokeAlign::Center, false)), [0, 1]);
		assert_eq!(planes(outline(black(), StrokeAlign::Outside, false)), [0]);
		assert_eq!(planes(outline(black(), StrokeAlign::Inside, true)), [1]);

		// A contour gradient colors the bands from a third map, of each pixel's place across the outline, looked up in tables of the gradient
		let stops = [(0., Color::RED), (1., Color::from_rgbaf32_unchecked(0., 0., 1., 0.5))].map(|(position, color)| GradientStop { position, midpoint: 0.5, color });
		let contour_gradient = outline(Graphic::from(Gradient::new(stops)), StrokeAlign::Center, true);
		assert_eq!(planes(contour_gradient.clone()), [2, 0, 1]);
		let mut contour_gradient_defs = String::new();
		write_filters(&mut contour_gradient_defs, &Effects::from_operations([contour_gradient]), 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[]);
		let red_table = contour_gradient_defs
			.split(r#"<feFuncR type="table" tableValues=""#)
			.nth(1)
			.and_then(|rest| rest.split('"').next())
			.unwrap_or_default();
		let red_table: Vec<&str> = red_table.split(' ').collect();
		let (samples, _) = GlowFill::of(&Graphic::from(Gradient::new(stops))).and_then(|fill| fill.gradient_samples(GRADIENT_TABLE_SIZE)).unwrap();
		assert_eq!(red_table.len(), GRADIENT_TABLE_SIZE, "{contour_gradient_defs}");
		assert_eq!(red_table[100], format!("{:.4}", samples[100][0]), "the table holds the gradient's samples in order");
		// The gradient's alpha is looked up on its own, since it isn't opaque
		assert!(contour_gradient_defs.contains(r#"<feFuncA type="table" tableValues="1.0000 "#), "{contour_gradient_defs}");

		// Without a contour gradient, a gradient is drawn like a paint overlay's paint, embedded as an image the bands take their colors from
		let mut drawn_defs = String::new();
		let paints = [r#"<rect width="10" height="10" fill="red" />"#.to_string()];
		let drawn = outline(Graphic::from(Gradient::new(stops)), StrokeAlign::Outside, false);
		let filters = write_filters(&mut drawn_defs, &Effects::from_operations([drawn]), 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &paints);
		assert_eq!(filters.iter().flat_map(|filter| filter.maps.iter().map(|map| map.capture.plane)).collect::<Vec<_>>(), [0]);
		assert!(drawn_defs.contains(r#"<feImage href="data:image/svg+xml;base64,"#), "{drawn_defs}");

		// A bevel lights the slopes inside the content from the first two maps and those outside, beneath it, from the last two
		let bevel = |style| {
			let mut bevel = EffectOperation::default_for(EffectKind::BevelAndEmboss);
			if let EffectOperation::BevelAndEmboss { style: bevel_style, .. } = &mut bevel {
				*bevel_style = style;
			}
			bevel
		};
		assert_eq!(planes(bevel(BevelStyle::InnerBevel)), [0, 1]);
		assert_eq!(planes(bevel(BevelStyle::OuterBevel)), [2, 3]);
		assert_eq!(planes(bevel(BevelStyle::PillowEmboss)), [0, 1, 2, 3]);

		// A paint overlay embeds its paint's markup as an image of the filter region, which blends with the content's colors within its share at a full fill
		let overlay = EffectOperation::PaintOverlay {
			paint: Graphic::from(Gradient::default()),
			blend_mode: BlendMode::Multiply,
			opacity: 50.,
		};
		let mut overlay_defs = String::new();
		let paints = [r#"<rect width="10" height="10" fill="red" />"#.to_string()];
		let filters = write_filters(&mut overlay_defs, &Effects::from_operations([overlay]), 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &paints);
		assert_eq!(filters.len(), 1, "the content's layer alone");
		assert!(
			overlay_defs.contains(r#"<feImage href="data:image/svg+xml;base64,"#) && overlay_defs.contains(r#"x="0" y="0" width="10" height="10""#),
			"{overlay_defs}"
		);
		assert!(overlay_defs.contains(r#"<feFuncA type="linear" slope="0.5" />"#), "{overlay_defs}");
		assert!(overlay_defs.contains(r#"mode="multiply" color-interpolation-filters="sRGB""#), "{overlay_defs}");

		// So can a blend mode CSS lacks, built from arithmetic on opaque copies, here the backdrop plus the source less one
		let linear_burn = EffectOperation::PaintOverlay {
			paint: Graphic::from(Gradient::default()),
			blend_mode: BlendMode::LinearBurn,
			opacity: 100.,
		};
		let mut linear_burn_defs = String::new();
		write_filters(&mut linear_burn_defs, &Effects::from_operations([linear_burn]), 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &paints);
		assert!(
			linear_burn_defs.contains(r#"operator="arithmetic" k1="0" k2="1" k3="1" k4="-1""#) && !linear_burn_defs.contains("feBlend"),
			"{linear_burn_defs}"
		);
	}

	#[test]
	fn svg_effects_filters_express_adjustments_where_svg_can() {
		use graphic_types::adjustment::DesaturateMethod;
		use graphic_types::{Adjustment, EffectOperation};

		let filter = |adjustment| {
			let mut defs = String::new();
			write_filters(&mut defs, &Effects::from_operations([EffectOperation::Adjustment(adjustment)]), 1., [DVec2::ZERO, DVec2::ONE], 0, &[]);
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

			fn render_effects(
				&mut self,
				_: glam::UVec2,
				_: kurbo::Affine,
				_: &Effects,
				_: &EffectLayers,
				_: f32,
				_: f32,
				_: DAffine2,
				_: Option<graphic_types::effects::EffectCapture>,
				_: &mut dyn FnMut(usize, &mut VelloSceneBuilder),
				_: &mut dyn FnMut(&mut VelloSceneBuilder),
			) -> Vec<(wgpu::Texture, BlendMode)> {
				panic!("the vector is drawn through an effects group");
			}
		}
		let mut effects_renderer = NoEffectsGroups;
		let mut scene = VelloSceneBuilder::new(16, 16, kurbo::Affine::IDENTITY, 4096).with_effects_renderer(&mut effects_renderer);
		vector.render_to_vello(&mut scene, DAffine2::IDENTITY, &RenderParams::default());
	}

	/// A 10 unit orange square with `effects` stamped on it, its `fill` opacity, and its `opacity`.
	fn orange_square(effects: &[graphic_types::EffectOperation], fill: f64, opacity: f64) -> List<Vector> {
		List::new_from_item(orange_square_item(effects, fill, opacity))
	}

	fn orange_square_item(effects: &[graphic_types::EffectOperation], fill: f64, opacity: f64) -> Item<Vector> {
		use graphic_types::{Effect, stamp_effect};
		use kurbo::Shape;

		let orange = Color::from_gamma_srgb_channels(1., 0.5, 0., 1.);
		let mut item = Vector::item_from_bezpath(&kurbo::Rect::new(0., 0., 10., 10.).to_path(0.1));
		item.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), Graphic::ColorList(List::new_from_element(orange))));
		item.set_attribute(ATTR_OPACITY_FILL, fill);
		item.set_attribute(ATTR_OPACITY, opacity);
		effects.iter().for_each(|operation| stamp_effect(&mut item, Effect::new(operation)));
		item
	}

	#[test]
	fn svg_draws_each_layer_of_effects_as_a_sibling_group() {
		use graphic_types::EffectOperation;

		let mut red_shadow = drop_shadow(180., 3., 0.);
		if let EffectOperation::DropShadow { color, .. } = &mut red_shadow {
			*color = Color::RED;
		}
		let svg = |effects: &[EffectOperation], render_params: &RenderParams| {
			let mut render = SvgRender::new();
			orange_square(effects, 1., 0.5).render_svg(&mut render, render_params);
			render.svg.to_svg_string()
		};
		let export = RenderParams {
			for_export: true,
			..Default::default()
		};

		// A red shadow multiplying blends in a layer of its own, beneath the content's, each faded by the opacity, with the content's markup drawn once
		let layered = svg(std::slice::from_ref(&red_shadow), &export);
		assert_eq!(layered.matches("<g filter=").count(), 2, "{layered}");
		assert_eq!(layered.matches(r#"opacity="0.5""#).count(), 2, "{layered}");
		assert_eq!(layered.matches("mix-blend-mode: multiply").count(), 1, "{layered}");
		let content_id = layered.split(r#"id=""#).nth(1).and_then(|rest| rest.split('"').next()).expect("the content's group has an id");
		assert!(content_id.starts_with("effects-content-") && layered.contains(&format!(r##"<use href="#{content_id}""##)), "{layered}");
		assert_eq!(layered.matches("<path").count(), 1, "{layered}");

		// In the viewport, where images are canvases a copy leaves out, each layer holds the markup again
		let in_viewport = svg(std::slice::from_ref(&red_shadow), &RenderParams::default());
		assert!(!in_viewport.contains("<use") && in_viewport.matches("<path").count() == 2, "{in_viewport}");

		// A black shadow multiplying blends normally, so everything draws as one layer
		let single = svg(&[drop_shadow(180., 3., 0.)], &export);
		assert!(single.matches("<g filter=").count() == 1 && !single.contains("<use") && !single.contains("mix-blend-mode"), "{single}");
	}

	#[test]
	fn svg_filtered_groups_hold_an_unpainted_rectangle_spanning_their_filter_region() {
		let mut red_shadow = drop_shadow(180., 30., 10.);
		if let graphic_types::EffectOperation::DropShadow { color, .. } = &mut red_shadow {
			*color = Color::RED;
		}

		// Within a group blurring it, the shadow blends in a layer of its own beside the content's
		let mut group = Item::new_from_element(Graphic::VectorList(orange_square(&[red_shadow], 1., 1.)));
		let blur = graphic_types::EffectOperation::Blur {
			radius: 2.,
			box_blur: false,
			gamma: false,
		};
		group.set_attribute(ATTR_EFFECTS, Effects::from_operations([blur]));
		let mut render = SvgRender::new();
		List::new_from_item(group).render_svg(
			&mut render,
			&RenderParams {
				for_export: true,
				..Default::default()
			},
		);
		let svg = render.svg.to_svg_string();

		// Each of the three layers' groups, the group's own and the item's two, spans its filter's region whatever lies in view
		let regions: Vec<&str> = (render.svg_defs.split(r#"<filter id="effects-"#).skip(1))
			.filter_map(|filter| filter.split(r#"filterUnits="userSpaceOnUse" "#).nth(1)?.split(" color-interpolation-filters").next())
			.collect();
		assert_eq!(regions.len(), 3, "{}", render.svg_defs);
		for region in regions {
			assert!(svg.contains(&format!(r#"<rect {region} fill="none"/>"#)), "{region} in {svg}");
		}
	}

	#[test]
	fn solid_paint_overlays_fold_into_solid_vector_paints_at_a_full_fill() {
		use graphic_types::{Adjustment, EffectOperation};

		let white_overlay = EffectOperation::Adjustment(Adjustment::ColorOverlay {
			color: Color::WHITE,
			blend_mode: BlendMode::Normal,
			opacity: 50.,
		});
		let render = |fill: f64| {
			let mut render = SvgRender::new();
			orange_square(std::slice::from_ref(&white_overlay), fill, 1.).render_svg(&mut render, &RenderParams::default());
			render
		};

		// Half of white over orange, mixed in gamma, fills the shape with no filter
		let folded = render(1.);
		assert!(!folded.svg_defs.contains("<filter"), "{}", folded.svg_defs);
		let svg = folded.svg.to_svg_string();
		let hex = svg.split(r##"fill="#"##).nth(1).map(|rest| &rest[..6]).expect("a solid fill");
		let channels = [0, 1, 2].map(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap());
		assert!(channels.iter().zip([255_u8, 191, 128]).all(|(actual, expected)| actual.abs_diff(expected) <= 1), "#{hex}");

		// A fill fades the content but not the overlay painted over it, which a filter draws
		let faded = render(0.5);
		assert!(faded.svg_defs.contains("<filter"), "{}", faded.svg_defs);
	}

	#[test]
	fn a_clipping_artboard_draws_its_background_inside_its_clip() {
		let artboard = Artboard::default();
		let mut list = List::new_from_element(artboard);
		list.set_attribute(ATTR_LOCATION, 0, DVec2::new(10., 20.));
		list.set_attribute(ATTR_DIMENSIONS, 0, DVec2::new(-30., 40.));
		list.set_attribute(ATTR_BACKGROUND, 0, Color::RED);
		list.set_attribute(ATTR_CLIP, 0, true);

		let mut render = SvgRender::new();
		list.render_svg(&mut render, &RenderParams::default());
		let svg = render.svg.to_svg_string();

		// Content blends with the background within the clip, which spans the artboard however its dimensions run
		let clip = svg.find("clip-path=").expect("the artboard clips");
		let background = svg.find(r##"fill="#ff0000""##).expect("the background is drawn");
		assert!(background > clip, "{svg}");
		assert!(svg.contains(r#"x="-30" y="0" width="30" height="40""#), "{svg}");
		assert!(render.svg_defs.contains(r#"<rect x="-30" y="0" width="30" height="40" />"#), "{}", render.svg_defs);
	}

	#[test]
	fn svg_masks_of_clipping_content_cover_it_whatever_the_clipped_content_draws() {
		use graphic_types::raster_types::Image;

		// A raster can't reduce to a clip path, so it masks the square above it, which casts a shadow far past its own geometry
		let raster = Item::new_from_element(Raster::new_cpu(Image::new(2, 2, Color::WHITE))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(200.)));
		let square = orange_square_item(&[drop_shadow(135., 60., 10.)], 1., 1.).with_attribute(ATTR_CLIPPING_MASK, true);
		let mut list = List::new_from_element(Graphic::RasterCPU(Box::new(raster)));
		list.push(Item::new_from_element(Graphic::Vector(Box::new(square))));

		let mut render = SvgRender::new();
		list.render_svg(
			&mut render,
			&RenderParams {
				for_export: true,
				..Default::default()
			},
		);

		// The mask covers the raster rather than a tenth past the square's geometry
		assert!(
			render.svg_defs.contains(r#"mask-type="alpha" maskUnits="userSpaceOnUse" x="0" y="0" width="200" height="200">"#),
			"{}",
			render.svg_defs
		);
	}

	#[test]
	fn lists_paint_as_far_as_the_effects_nested_anywhere_inside_them_paint() {
		use graphic_types::EffectOperation;

		// A shadow on a square inside a group paints past the group
		let shadow = drop_shadow(135., 20., 5.);
		let in_group = |effects: &[EffectOperation]| List::new_from_element(Graphic::VectorList(orange_square(effects, 1., 1.)));
		let square_grown_by = |reach: f64| RenderBoundingBox::Rectangle([DVec2::splat(-reach), DVec2::splat(10. + reach)]);
		assert_eq!(graphic_list_painted_bounds(&in_group(std::slice::from_ref(&shadow)), true), square_grown_by(shadow.reach()));

		// An SVG with no GPU leaves the shadow out, so it frames the square alone
		assert_eq!(graphic_list_painted_bounds(&in_group(std::slice::from_ref(&shadow)), false), square_grown_by(0.));

		// Each item grows by its own reach, so a plain square far from the shadowed one isn't padded by its shadow
		let mut list = in_group(std::slice::from_ref(&shadow));
		list.push(Item::new_from_element(Graphic::VectorList(orange_square(&[], 1., 1.))).with_attribute(ATTR_TRANSFORM, DAffine2::from_translation(DVec2::splat(100.))));
		assert_eq!(
			graphic_list_painted_bounds(&list, true),
			RenderBoundingBox::Rectangle([DVec2::splat(-shadow.reach()), DVec2::splat(110.)])
		);
	}

	#[test]
	fn svg_maps_cover_past_the_view_as_far_as_the_filters_reading_them_reach() {
		let blur = graphic_types::EffectOperation::Blur {
			radius: 4.,
			box_blur: false,
			gamma: false,
		};
		let blur_reach = Effects::from_operations([blur.clone()]).reach();
		let feathered_mask = graphic_types::EffectOperation::Mask {
			mask: Graphic::from(Color::WHITE),
			mode: graphic_types::MaskMode::Luminance,
			density: 100.,
			feather: 20.,
			invert: false,
		};

		// A shadow's map is read by the blur after it and by the blur of the group around the item, but not by the mask's feather, which reads another image
		let mut group = Item::new_from_element(Graphic::VectorList(orange_square(&[drop_shadow(180., 30., 10.), blur.clone(), feathered_mask], 1., 1.)));
		group.set_attribute(ATTR_EFFECTS, Effects::from_operations([blur]));
		let mut render = SvgRender::new();
		List::new_from_item(group).render_svg(
			&mut render,
			&RenderParams {
				for_export: true,
				..Default::default()
			},
		);
		let [map] = &render.effect_maps[..] else {
			panic!("expected one map, got {}", render.effect_maps.len())
		};
		assert!((map.margin - 2. * blur_reach).abs() < 1e-9, "{} for {blur_reach}", map.margin);

		// So in a view cutting across the shadow, the map covers that far past each of its edges, with no more pixels than four views hold
		let layout = map.layout(DAffine2::IDENTITY, DVec2::new(5., 5.), 8192).expect("the map lies in view");
		let expected = 5. + 2. * map.margin;
		let covered = layout.placement.transform.matrix2.x_axis.x;
		assert!((covered - expected).abs() <= 1., "{covered} for {expected}");
		assert!(layout.size.x == layout.size.y && layout.size.x <= 10, "{}", layout.size);
	}

	#[test]
	fn svg_maps_of_a_mask_graphic_cover_past_the_view_as_far_as_its_feather_and_the_groups_around_it_read() {
		let blur = graphic_types::EffectOperation::Blur {
			radius: 4.,
			box_blur: false,
			gamma: false,
		};

		// A mask graphic casting a shadow of its own, feathered, on a square inside a blurred group
		let feathered_mask = graphic_types::EffectOperation::Mask {
			mask: Graphic::VectorList(orange_square(&[drop_shadow(180., 30., 10.)], 1., 1.)),
			mode: graphic_types::MaskMode::Luminance,
			density: 100.,
			feather: 2.,
			invert: false,
		};
		let blur_after = graphic_types::EffectOperation::Blur {
			radius: 2.,
			box_blur: false,
			gamma: false,
		};

		// With a blur after the mask too, which reads the masked content around each pixel
		let cases = [(vec![feathered_mask.clone()], 0.), (vec![feathered_mask.clone(), blur_after.clone()], blur_after.reach())];
		for (effects, reach_after) in cases {
			let mut group = Item::new_from_element(Graphic::VectorList(orange_square(&effects, 1., 1.)));
			group.set_attribute(ATTR_EFFECTS, Effects::from_operations([blur.clone()]));
			let mut render = SvgRender::new();
			List::new_from_item(group).render_svg(
				&mut render,
				&RenderParams {
					for_export: true,
					..Default::default()
				},
			);

			let expected = blur.reach() + feathered_mask.reach() + reach_after;
			let [map] = &render.effect_maps[..] else {
				panic!("expected the mask graphic's one map, got {}", render.effect_maps.len())
			};
			assert!((map.margin - expected).abs() < 1e-9, "{} for {expected}", map.margin);
		}
	}

	#[test]
	fn svg_maps_of_a_clipped_vector_are_of_the_effects_left_after_a_fold() {
		use graphic_types::{Adjustment, EffectKind, EffectOperation};

		// Clipped content flattens its effects, so a red overlay multiplying folds into its solid fill, leaving the inner shadow
		let overlay = EffectOperation::Adjustment(Adjustment::ColorOverlay {
			color: Color::RED,
			blend_mode: BlendMode::Multiply,
			opacity: 100.,
		});
		let mut list = orange_square(&[], 1., 1.);
		list.push(orange_square_item(&[overlay, EffectOperation::default_for(EffectKind::InnerShadow)], 1., 1.));
		list.set_attribute(ATTR_CLIPPING_MASK, 1, true);

		let mut render = SvgRender::new();
		list.render_svg(
			&mut render,
			&RenderParams {
				for_export: true,
				..Default::default()
			},
		);

		// The map the filter numbers as the first effect is of content whose first effect is the inner shadow
		let [map] = &render.effect_maps[..] else {
			panic!("expected one map, got {}", render.effect_maps.len())
		};
		let Graphic::Vector(content) = &map.content else { panic!("the map is of the vector") };
		let effects: Vec<EffectOperation> = content.attribute::<Effects>(ATTR_EFFECTS).map(|effects| effects.operations().collect()).unwrap_or_default();
		assert_eq!(map.capture, EffectCapture { index: 0, plane: 0 });
		assert!(matches!(effects[..], [EffectOperation::InnerShadow { .. }]), "{effects:?}");
	}

	#[test]
	fn svg_layers_without_the_content_skip_the_flattened_rows_they_cant_show() {
		use graphic_types::EffectOperation;

		let red = |kind| {
			let mut operation = EffectOperation::default_for(kind);
			if let EffectOperation::DropShadow { color, blend_mode, .. } | EffectOperation::InnerShadow { color, blend_mode, .. } = &mut operation {
				(*color, *blend_mode) = (Color::RED, BlendMode::Multiply);
			}
			operation
		};
		let blur = EffectOperation::Blur {
			radius: 2.,
			box_blur: false,
			gamma: false,
		};
		let captures = |filter: &SvgEffectsFilter| filter.maps.iter().map(|map| (map.capture.index, map.capture.plane)).collect::<Vec<_>>();
		let write = |effects: Vec<EffectOperation>| write_filters(&mut String::new(), &Effects::from_operations(effects), 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[]);

		// A layer of only a shadow beneath the content needs no map of the inner shadow flattened into the content
		let filters = write(vec![red(graphic_types::EffectKind::InnerShadow), blur.clone(), red(graphic_types::EffectKind::DropShadow)]);
		assert_eq!(captures(&filters[0]), [(2, 0)]);
		assert_eq!(captures(&filters[1]), [(0, 0)]);

		// Over content in a mode of its own, a layer of only an inner shadow keeps the flattened shadow a later blur merges into the content's alpha it cuts to
		let effects = Effects::from_operations([red(graphic_types::EffectKind::DropShadow), blur, red(graphic_types::EffectKind::InnerShadow)]);
		let layers = EffectLayers::plan(&effects, BlendMode::Overlay, 1., true);
		let within_layer = write_svg_effects_filter(&mut String::new(), &effects, &layers, 1, 1., 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[], 0..effects.len());
		assert_eq!(within_layer.as_ref().map(captures), Some(vec![(0, 0), (2, 0)]));
	}

	#[test]
	fn svg_color_and_gradient_layers_fade_by_their_fill_once() {
		use graphic_types::{Adjustment, Effect, EffectOperation, stamp_effect};

		let overlay = |blend_mode| {
			Effect::new(&EffectOperation::Adjustment(Adjustment::ColorOverlay {
				color: Color::WHITE,
				blend_mode,
				opacity: 50.,
			}))
		};
		let svg = |item: Graphic| {
			let mut render = SvgRender::new();
			item.render_svg(&mut render, &RenderParams::default());
			render.svg.to_svg_string()
		};

		// Half of white over black at half fill covers three quarters, faded once in the color rather than again by opacity
		let mut color = Item::new_from_element(Color::BLACK).with_attribute(ATTR_OPACITY_FILL, 0.5_f64);
		stamp_effect(&mut color, overlay(BlendMode::Normal));
		let folded = svg(Graphic::Color(color.clone()));
		assert!(folded.contains(r#"fill-opacity="0.75""#) && !folded.contains(" opacity="), "{folded}");

		// Over a color blending in a mode of its own, an overlay draws as a layer of its own
		let layered = svg(Graphic::Color(color.with_attribute(ATTR_BLEND_MODE, BlendMode::Multiply)));
		assert_eq!(layered.matches("<polyline").count(), 2, "{layered}");
		assert_eq!(layered.matches("mix-blend-mode: multiply").count(), 1, "{layered}");

		// Below a full fill, a blended overlay paints over everything in its mode, rather than folding into the faded color
		let mut multiplied = Item::new_from_element(Color::WHITE).with_attribute(ATTR_OPACITY_FILL, 0.5_f64);
		stamp_effect(&mut multiplied, overlay(BlendMode::Multiply));
		let multiplied = svg(Graphic::Color(multiplied));
		assert_eq!(multiplied.matches("<polyline").count(), 2, "{multiplied}");
		assert_eq!(multiplied.matches("mix-blend-mode: multiply").count(), 1, "{multiplied}");

		// A gradient with nothing to carry out keeps its own ramp, faded by its fill as a whole
		let gradient = Item::new_from_element(Gradient::from(vec![Color::BLACK, Color::WHITE])).with_attribute(ATTR_OPACITY_FILL, 0.5_f64);
		let gradient = svg(Graphic::Gradient(gradient));
		assert!(gradient.contains(r#"opacity="0.5""#), "{gradient}");
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

		// Without a view to fit, a pattern filling the plane has no part to draw, so its markup depends on the view
		let without_view = RenderParams::default();
		assert_eq!(canvases(&without_view), Vec::new());
		assert!(without_view.view_wanted.load(std::sync::atomic::Ordering::Relaxed));

		// Content drawing all of itself doesn't depend on the view
		let drawing_all = RenderParams::default();
		orange_square(&[], 1., 1.).render_svg(&mut SvgRender::new(), &drawing_all);
		assert!(!drawing_all.view_wanted.load(std::sync::atomic::Ordering::Relaxed));

		// A 25 pixel view at half scale, drawn at the view's resolution
		let view = Footprint {
			transform: DAffine2::from_scale(DVec2::splat(0.5)),
			resolution: UVec2::splat(25),
			..Default::default()
		};
		let in_view = RenderParams {
			view: Some(view),
			..Default::default()
		};
		assert_eq!(canvases(&in_view), vec![(25, 25)]);
	}

	#[test]
	fn svg_generators_painting_a_fill_draw_the_part_of_it_in_view() {
		use glam::UVec2;
		use kurbo::Shape;

		// A square 20 units across its own origin, placed 60 across and 45 down, filled by a generator
		let mut item = Vector::item_from_bezpath(&kurbo::Rect::new(-10., -10., 10., 10.).to_path(0.1));
		item.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), Graphic::from(Generator::Mandelbrot)));
		item.set_attribute(ATTR_TRANSFORM, DAffine2::from_translation(DVec2::new(60., 45.)));

		let view = Footprint {
			resolution: UVec2::new(96, 64),
			..Default::default()
		};
		let render_params = RenderParams {
			view: Some(view),
			..Default::default()
		};
		let mut render = SvgRender::new();
		List::new_from_item(item).render_svg(&mut render, &render_params);

		// The pattern's image of the generator, placed in the square's own space, covers all of the square
		let image = render.svg_defs.split("<image").nth(1).expect("the pattern holds an image");
		let matrix = image.split(r#"transform="matrix("#).nth(1).and_then(|rest| rest.split(')').next()).expect("the image is placed");
		let numbers: Vec<f64> = matrix.split(',').filter_map(|number| number.parse().ok()).collect();
		let [width, _, _, height, x, y] = numbers[..] else { panic!("{matrix}") };
		assert!(x <= -10. && y <= -10. && x + width >= 10. && y + height >= 10., "{matrix}");
	}

	#[test]
	fn svg_clipping_vectors_draw_in_the_space_they_are_placed_in() {
		use glam::UVec2;
		use kurbo::Shape;

		// A square 20 units across its own origin filled by a generator, in a group placing it 60 across and 45 down, clipping an orange square after it
		let mut generator_square = Vector::item_from_bezpath(&kurbo::Rect::new(-10., -10., 10., 10.).to_path(0.1));
		generator_square.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), Graphic::from(Generator::Mandelbrot)));
		let mut vectors = List::new_from_item(generator_square);
		vectors.push(orange_square_item(&[], 1., 1.).with_attribute(ATTR_CLIPPING_MASK, true));
		let group = List::new_from_item(Item::new_from_element(Graphic::VectorList(vectors)).with_attribute(ATTR_TRANSFORM, DAffine2::from_translation(DVec2::new(60., 45.))));

		let render_params = RenderParams {
			view: Some(Footprint {
				resolution: UVec2::new(96, 64),
				..Default::default()
			}),
			..Default::default()
		};
		let mut render = SvgRender::new();
		group.render_svg(&mut render, &render_params);

		// The clipper's image of the generator, placed in the square's own space, covers all of the square
		let image = render.svg_defs.split("<image").nth(1).expect("the pattern holds an image");
		let matrix = image.split(r#"transform="matrix("#).nth(1).and_then(|rest| rest.split(')').next()).expect("the image is placed");
		let numbers: Vec<f64> = matrix.split(',').filter_map(|number| number.parse().ok()).collect();
		let [width, _, _, height, x, y] = numbers[..] else { panic!("{matrix}") };
		assert!(x <= -10. && y <= -10. && x + width >= 10. && y + height >= 10., "{matrix}");
	}

	#[test]
	fn svg_generator_images_cover_past_the_view_as_far_as_the_filters_reading_them_reach() {
		use glam::UVec2;
		use kurbo::Shape;

		let blur = graphic_types::EffectOperation::Blur {
			radius: 10.,
			box_blur: false,
			gamma: false,
		};
		let reach = blur.reach();
		let render_params = RenderParams {
			view: Some(Footprint {
				resolution: UVec2::new(96, 64),
				..Default::default()
			}),
			..Default::default()
		};
		let rectangle = |paint: Graphic| {
			let mut item = Vector::item_from_bezpath(&kurbo::Rect::new(-100., -100., 300., 300.).to_path(0.1));
			item.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), paint));
			item
		};

		// A rectangle past the view filled by a generator, blurred, so the blur reads the fill that far past the view
		let filled = rectangle(Graphic::from(Generator::Mandelbrot)).with_attribute(ATTR_EFFECTS, Effects::from_operations([blur.clone()]));
		let mut render = SvgRender::new();
		List::new_from_item(filled).render_svg(&mut render, &render_params);
		let image = render.svg_defs.split("<image").nth(1).expect("the pattern holds an image");
		let matrix = image.split(r#"transform="matrix("#).nth(1).and_then(|rest| rest.split(')').next()).expect("the image is placed");
		let numbers: Vec<f64> = matrix.split(',').filter_map(|number| number.parse().ok()).collect();
		let [width, _, _, height, x, y] = numbers[..] else { panic!("{matrix}") };
		assert!(
			x <= -reach + 1e-6 && y <= -reach + 1e-6 && x + width >= 96. + reach - 1e-6 && y + height >= 64. + reach - 1e-6,
			"{matrix}"
		);

		// A generator layer in a group given edges by a mask and then blurred, so the blur reads it that far past the view too
		let mask = graphic_types::EffectOperation::Mask {
			mask: Graphic::VectorList(List::new_from_item(rectangle(Graphic::ColorList(List::new_from_element(Color::WHITE))))),
			mode: graphic_types::MaskMode::Luminance,
			density: 100.,
			feather: 0.,
			invert: false,
		};
		let group = Item::new_from_element(Graphic::GeneratorList(List::new_from_element(Generator::Mandelbrot))).with_attribute(ATTR_EFFECTS, Effects::from_operations([mask, blur]));
		let mut render = SvgRender::new();
		List::new_from_item(group).render_svg(&mut render, &render_params);
		let sizes: Vec<_> = render.image_data.values().map(|canvas| (canvas.raster.width, canvas.raster.height)).collect();
		let grown = (96. + 2. * reach, 64. + 2. * reach);
		assert!(
			matches!(sizes[..], [(width, height)] if (width as f64 - grown.0).abs() <= 1. && (height as f64 - grown.1).abs() <= 1.),
			"{sizes:?} for {grown:?}"
		);
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
	fn content_with_effects_svg_cannot_express_defers_to_an_image() {
		use graphic_types::raster_types::Image;
		use graphic_types::{Effect, EffectOperation, stamp_effect};

		// A shadow, which a filter expresses, followed by Vibrance, which none does
		let shadow = EffectOperation::DropShadow {
			color: Color::BLACK,
			blend_mode: BlendMode::Multiply,
			angle: 180.,
			distance: 3.,
			spread: 0.,
			size: 0.,
			contour: Default::default(),
			knockout: true,
		};
		let mut item = Item::new_from_element(Raster::new_cpu(Image::new(2, 2, Color::WHITE)))
			.with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(10.)))
			.with_attribute(ATTR_OPACITY, 0.5_f64);
		stamp_effect(&mut item, Effect::new(&shadow));
		stamp_effect(&mut item, Effect::new(&EffectOperation::Adjustment(Adjustment::Vibrance { vibrance: 0., saturation: -100. })));

		let mut render = SvgRender::new();
		List::new_from_item(item).render_svg(
			&mut render,
			&RenderParams {
				for_export: true,
				..Default::default()
			},
		);
		let mut svg = render.svg.to_svg_string();

		// The image's own markup stands between the markers, and the image of it covers the shadow, taking the item's opacity
		let [deferred] = &render.deferred_images[..] else {
			panic!("expected one deferred image, got {}", render.deferred_images.len())
		};
		let (opening, closing) = deferred_image::image_markers(deferred.id);
		let (start, end) = (svg.find(&opening).unwrap(), svg.find(&closing).unwrap());
		assert!(svg[start..end].contains("<image"), "{svg}");
		assert_eq!(deferred.bounds, [DVec2::splat(-4.), DVec2::splat(14.)]);
		assert_eq!(deferred.opacity, 0.5);
		assert!(matches!(&deferred.content, Graphic::RasterCPU(content) if content.attribute::<f64>(ATTR_OPACITY).is_none()));

		// In a view of the document's first 100 units, the image spans the part of the reach in it, replacing the markup
		let layout = deferred.layout(DAffine2::IDENTITY, DVec2::splat(100.), 8192).unwrap();
		assert_eq!(layout.size, glam::UVec2::splat(14));
		deferred_image::substitute_deferred_image(&mut svg, &mut Vec::new(), &layout.placement, &Raster::new_cpu(Image::new(14, 14, Color::BLACK)));
		assert!(!svg.contains(&opening) && !svg.contains(&closing), "{svg}");
		assert!(svg.contains(r#"opacity="0.5""#) && svg.contains("matrix(14,0,0,14,0,0)"), "{svg}");
	}

	#[test]
	fn svg_paint_overlays_blend_with_the_content_in_gamma() {
		let mut primitives = SvgFilterPrimitives::default();
		let (mut full, mut filled) = ("SourceGraphic".to_string(), Some("filled".to_string()));
		primitives.shade_within(&mut full, &mut filled, "paint", None, BlendMode::Hue);

		// The hue blends gamma-encoded channels, cut to where the paint covers, as on the GPU
		let markup = &primitives.markup;
		assert!(markup.contains(r#"mode="hue" color-interpolation-filters="sRGB""#), "{markup}");
		assert!(!markup.contains("linearRGB"), "{markup}");
		assert!(filled.is_some_and(|filled| filled != "filled"), "the faded fill is painted over too");
	}

	#[test]
	fn svg_masks_around_the_other_effects_are_native_and_those_between_them_are_filter_primitives() {
		use graphic_types::{EffectOperation, MaskMode};

		let mask = || EffectOperation::Mask {
			mask: Graphic::from(Color::WHITE),
			mode: MaskMode::Luminance,
			density: 50.,
			feather: 2.,
			invert: true,
		};
		let blur = || EffectOperation::Blur {
			radius: 3.,
			box_blur: false,
			gamma: false,
		};
		let effects = Effects::from_operations([mask(), blur(), mask(), blur(), mask(), mask()]);
		let (leading, trailing) = masks::native_masks(&effects);
		assert_eq!((leading.clone(), trailing.clone()), (0..1, 4..6));

		// With nothing layered before them, the trailing masks leave the one layer for its group to take
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., true);
		assert_eq!(layers.layers.len(), 1);

		// The mask between the blurs embeds its graphic's markup as an image, taking its luminance over black, feathering it, then mapping it through its inverted density
		let paints = vec![r#"<rect width="10" height="10" fill="white" />"#.to_string(); 4];
		let bounds = [DVec2::ZERO, DVec2::splat(10.)];
		let mut defs = String::new();
		write_svg_effects_filter(&mut defs, &effects, &layers, 0, 1., 1., bounds, 0, &paints, leading.end..trailing.start).unwrap();
		assert_eq!(defs.matches("<feImage").count(), 1, "{defs}");
		assert!(defs.contains(r#"type="luminanceToAlpha""#) && defs.contains(r#"stdDeviation="2 2""#), "{defs}");
		assert!(defs.contains(r#"<feFuncA type="linear" slope="-0.5" intercept="1" />"#), "{defs}");

		// The two trailing masks become one, which shows the content the second masks, while the leading one stands alone
		let mut defs = String::new();
		let [leading, trailing] = masks::write_native_masks(&mut defs, &effects, [leading, trailing], &paints, bounds);
		assert!(leading.is_some() && trailing.is_some());
		assert_eq!(defs.matches("<mask ").count(), 3, "{defs}");
		assert_eq!(defs.matches(r#"<g mask="url(#mask-"#).count(), 1, "{defs}");
		assert!(defs.contains(r#"mask-type="alpha" maskUnits="userSpaceOnUse""#), "{defs}");

		// A mask between the effects with a graphic carrying effects of its own takes its strength as the GPU's map, which leaves the content shown until resolved
		let blurred = Item::new_from_element(Color::WHITE).with_attribute(ATTR_EFFECTS, Effects::from_operations([blur()]));
		let mask_with_effects = EffectOperation::Mask {
			mask: Graphic::Color(blurred),
			mode: MaskMode::Luminance,
			density: 100.,
			feather: 0.,
			invert: false,
		};
		let effects = Effects::from_operations([blur(), mask_with_effects, blur()]);
		let mut defs = String::new();
		let [filter] = write_filters(&mut defs, &effects, 1., bounds, 0, &paints[..1])
			.try_into()
			.unwrap_or_else(|_| panic!("blurs and masks draw one layer"));
		assert_eq!(filter.maps.len(), 1);
		assert!(!defs.contains("<feImage") && defs.contains(r#"<feFlood flood-opacity="1""#), "{defs}");
	}

	#[test]
	fn svg_leading_masks_wrap_the_content_every_layer_draws() {
		use graphic_types::{EffectOperation, MaskMode};

		let mut circle = Vector::item_from_bezpath(&kurbo::Circle::new((5., 5.), 4.).to_path(0.1));
		circle.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), Graphic::ColorList(List::new_from_element(Color::WHITE))));
		let mask = EffectOperation::Mask {
			mask: Graphic::Vector(Box::new(circle)),
			mode: MaskMode::Luminance,
			density: 100.,
			feather: 0.,
			invert: false,
		};
		let mut red_shadow = drop_shadow(180., 3., 0.);
		if let EffectOperation::DropShadow { color, .. } = &mut red_shadow {
			*color = Color::RED;
		}
		let export = RenderParams {
			for_export: true,
			..Default::default()
		};

		// The red shadow multiplying blends in a layer of its own, which copies the content's group, so the mask goes inside that group
		let mut render = SvgRender::new();
		orange_square(&[mask.clone(), red_shadow.clone()], 1., 1.).render_svg(&mut render, &export);
		let svg = render.svg.to_svg_string();
		assert_eq!(svg.matches("<g filter=").count(), 2, "{svg}");
		let content = svg.split(r#"id="effects-content-"#).nth(1).expect("the content's group has an id");
		assert!(content.split_once('>').is_some_and(|(_, inside)| inside.trim_start().starts_with(r#"<g mask="url(#mask-"#)), "{svg}");

		// A trailing mask leaves the shadow before it in its own layer, and each layer takes the mask after its filter
		let mut render = SvgRender::new();
		orange_square(&[red_shadow, mask], 1., 1.).render_svg(&mut render, &export);
		let svg = render.svg.to_svg_string();
		assert_eq!(svg.matches("<g filter=").count(), 2, "{svg}");
		assert_eq!(svg.matches(r#"mask="url(#mask-"#).count(), 2, "{svg}");
		assert!(svg.contains("mix-blend-mode: multiply") && svg.contains("<use"), "{svg}");
	}

	#[test]
	fn svg_filters_carry_out_the_trailing_masks_their_groups_do_not() {
		use graphic_types::{EffectOperation, MaskMode};

		let mask = EffectOperation::Mask {
			mask: Graphic::from(Color::WHITE),
			mode: MaskMode::Luminance,
			density: 50.,
			feather: 0.,
			invert: false,
		};
		let mut red_shadow = drop_shadow(180., 3., 0.);
		if let EffectOperation::DropShadow { color, .. } = &mut red_shadow {
			*color = Color::RED;
		}
		let paints = vec![r#"<rect width="10" height="10" fill="white" />"#.to_string()];
		let bounds = [DVec2::ZERO, DVec2::splat(10.)];

		// Without a mask of SVG's own, as for effects in a space of their own, each layer's filter ends by cutting its image to the mask's strength
		let effects = Effects::from_operations([red_shadow, mask.clone()]);
		let mut defs = String::new();
		assert_eq!(write_filters(&mut defs, &effects, 1., bounds, 0, &paints).len(), 2);
		for filter in defs.split("</filter>").filter(|filter| !filter.is_empty()) {
			let last = filter.rsplit("<fe").next().unwrap_or_default();
			assert!(filter.contains("<feImage") && last.starts_with("Composite") && last.contains(r#"operator="in""#), "{filter}");
		}

		// A part over the content works from the opacity that SVG's own mask fades too, so its filter takes that mask's strength as the GPU's map
		let overlaying = EffectOperation::InnerShadow {
			color: Color::RED,
			blend_mode: BlendMode::Overlay,
			angle: 180.,
			distance: 3.,
			choke: 0.,
			size: 0.,
			contour: Default::default(),
		};
		let effects = Effects::from_operations([overlaying, mask]);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 0.5, true);
		let mut defs = String::new();
		let filter = write_svg_effects_filter(&mut defs, &effects, &layers, 0, 0.5, 1., bounds, 0, &paints, 0..1).expect("the content's layer comes first");
		assert!(filter.maps.iter().any(|map| map.capture == EffectCapture { index: 1, plane: 0 }), "{defs}");
		assert!(!defs.contains("<feImage"), "{defs}");
	}

	#[test]
	fn svg_masks_give_content_without_edges_the_edges_its_effects_follow() {
		use graphic_types::{EffectOperation, MaskMode};

		let mut circle = Vector::item_from_bezpath(&kurbo::Circle::new((20., 20.), 10.).to_path(0.1));
		circle.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), Graphic::ColorList(List::new_from_element(Color::WHITE))));
		let mask = |density, invert| EffectOperation::Mask {
			mask: Graphic::Vector(Box::new(circle.clone())),
			mode: MaskMode::Luminance,
			density,
			feather: 0.,
			invert,
		};
		let blur = EffectOperation::Blur {
			radius: 2.,
			box_blur: false,
			gamma: false,
		};
		let render = |effects: Vec<EffectOperation>| {
			let color = Graphic::Color(Item::new_from_element(Color::BLUE).with_attribute(ATTR_EFFECTS, Effects::from_operations(effects)));
			let group = Graphic::Graphic(Box::new(Item::new_from_element(Graphic::GraphicList(List::new_from_element(color.clone())))));
			[color, group].map(|graphic| {
				let mut render = SvgRender::new();
				graphic.render_svg(&mut render, &RenderParams::default());
				format!("{}{}", render.svg_defs, render.svg.to_svg_string())
			})
		};

		// A mask confining a color to its graphic's extent lets the blur after it soften the edges it gives
		let [color, _] = render(vec![mask(100., false), blur]);
		assert!(color.contains("<feGaussianBlur") && color.contains(r#"mask="url(#mask-"#), "{color}");

		// At partial density the color shows by one less the density everywhere beneath the mask's coverage, so its region's edge leaves no seam
		let [color, group] = render(vec![mask(50., false)]);
		for markup in [color, group] {
			assert!(markup.contains(r#"fill-opacity="0.5""#) && !markup.contains("evenodd"), "{markup}");
			assert!(markup.contains(r#"<feFuncA type="linear" slope="1" intercept="0" />"#), "{markup}");
		}

		// Inverted, it shows the color fully past its region
		let [color, _] = render(vec![mask(50., true)]);
		assert!(color.contains("evenodd") && color.contains(r#"slope="-0.5" intercept="1""#), "{color}");
	}

	#[test]
	fn svg_masks_in_the_viewport_embed_their_images_since_canvases_dont_paint_within_a_mask() {
		use graphic_types::raster_types::Image;
		use graphic_types::{EffectOperation, MaskMode};

		let image = Image {
			width: 2,
			height: 2,
			data: vec![Color::WHITE; 4],
		};
		let mask = EffectOperation::Mask {
			mask: Graphic::RasterCPU(Box::new(
				Item::new_from_element(Raster::new_cpu(image)).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(10.))),
			)),
			mode: MaskMode::Luminance,
			density: 100.,
			feather: 0.,
			invert: false,
		};

		// The viewport otherwise draws images to canvases
		let viewport = RenderParams::default();
		assert!(viewport.to_canvas());
		let mut render = SvgRender::new();
		orange_square(&[mask], 1., 1.).render_svg(&mut render, &viewport);
		assert!(render.svg_defs.contains("<mask") && render.svg_defs.contains("data:image/"), "{}", render.svg_defs);
		assert!(!render.svg_defs.contains("data-canvas-placeholder") && render.image_data.is_empty(), "{}", render.svg_defs);
	}

	#[test]
	fn svg_masks_on_a_clipping_base_shape_what_clips_to_it_unlike_its_shadows() {
		use graphic_types::{EffectOperation, MaskMode};

		let mut half = Vector::item_from_bezpath(&kurbo::Rect::new(0., 0., 5., 10.).to_path(0.1));
		half.set_attribute(ATTR_APPEARANCE, Appearance::new_single(Coverage::new_fill(), Graphic::ColorList(List::new_from_element(Color::WHITE))));
		let mask = EffectOperation::Mask {
			mask: Graphic::Vector(Box::new(half)),
			mode: MaskMode::Alpha,
			density: 100.,
			feather: 0.,
			invert: false,
		};
		let export = RenderParams {
			for_export: true,
			..Default::default()
		};
		let clipped_by = |base_effects: &[EffectOperation]| {
			let mut list = orange_square(base_effects, 1., 1.);
			list.push(orange_square_item(&[], 1., 1.).with_attribute(ATTR_CLIPPING_MASK, true));
			let mut render = SvgRender::new();
			list.render_svg(&mut render, &export);
			render.svg_defs
		};

		// A base casting only a shadow clips by its outline, and one with a mask clips by a mask that takes the base's mask too
		let shadowed = clipped_by(&[drop_shadow(135., 3., 0.)]);
		assert!(shadowed.contains("<clipPath") && !shadowed.contains("<mask"), "{shadowed}");
		let masked = clipped_by(&[mask, drop_shadow(135., 3., 0.)]);
		assert!(!masked.contains("<clipPath"), "{masked}");
		let clip = masked
			.split("<mask id=")
			.find(|mask| mask.contains(r#"<g mask="url(#mask-"#))
			.expect("the clip's mask nests the base's");
		assert!(!clip.contains("filter="), "the base's shadow doesn't reach the clip: {clip}");
	}

	#[test]
	fn svg_feathers_fade_the_content_by_a_map_of_their_strength() {
		use graphic_types::EffectOperation;

		let mut defs = String::new();
		let effects = Effects::from_operations([EffectOperation::Feather { distance: 4. }]);
		let [filter] = write_filters(&mut defs, &effects, 0.5, [DVec2::ZERO, DVec2::splat(10.)], 0, &[])
			.try_into()
			.unwrap_or_else(|_| panic!("a feather draws one layer"));

		// One map the GPU renders fades both the full-strength content and the faded fill
		assert_eq!(filter.maps.len(), 1);
		assert_eq!(filter.maps[0].capture, EffectCapture { index: 0, plane: 0 });
		assert_eq!(defs.matches(&format!(r#"in2="{}" operator="in""#, filter.maps[0].result)).count(), 2, "{defs}");

		// In a later run, the map names the feather by its place among all the effects
		let [filter] = write_filters(&mut String::new(), &effects, 0.5, [DVec2::ZERO, DVec2::splat(10.)], 2, &[])
			.try_into()
			.unwrap_or_else(|_| panic!("a feather draws one layer"));
		assert_eq!(filter.maps[0].capture, EffectCapture { index: 2, plane: 0 });
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
