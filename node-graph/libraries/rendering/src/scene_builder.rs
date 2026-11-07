use crate::vello_scene_builder::VelloSceneBuilder;
use graphic_types::raster_types::{CPU, Raster, Texture};
use graphic_types::vector_types::vector::PlacedContours;
use kurbo::{Affine, BezPath, PathEl, Point, Shape, Size, Stroke};
use std::borrow::Cow;
use vello_gpu::peniko::{self, BlendMode, BrushRef, Fill};

/// Tolerance for turning shapes other than paths into paths, matching Vello's own.
pub const SHAPE_TOLERANCE: f64 = 0.1;

/// The drawing interface the renderer draws content through, implemented by each backend that content is drawn into, such as a Vello GPU scene or a PDF page.
pub trait SceneBuilder {
	/// The content's placement in the scene, applied on top of every transform it's drawn with.
	fn view(&self) -> Affine;

	/// The width and height of the scene, in the space [`Self::view`] places content into.
	fn size(&self) -> Size;

	/// Fills a shape given in the space of `transform`, with the brush placed by `transform * brush_transform`.
	fn fill<'b>(&mut self, fill_rule: Fill, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, shape: &impl Shape) {
		self.fill_path(fill_rule, transform, brush, brush_transform, &shape.to_path(SHAPE_TOLERANCE));
	}

	/// Like [`Self::fill`] for a path, which it draws without copying.
	fn fill_path<'b>(&mut self, fill_rule: Fill, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, path: &BezPath);

	/// Strokes a shape given in the space of `transform`, with the brush placed by `transform * brush_transform`.
	fn stroke<'b>(&mut self, stroke: &Stroke, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, shape: &impl Shape);

	/// Strokes the path of placed contours given in the space of `transform`, with the brush placed by `transform * brush_transform`.
	/// A backend that fills strokes as outlines reuses the ones the contours cache.
	fn stroke_contours<'b>(&mut self, stroke: &Stroke, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, contours: &PlacedContours);

	/// Fills the whole scene, which is how content without bounds (like a color or gradient layer) paints, with the brush placed by `brush_transform` in the space of the drawn content.
	fn fill_viewport<'b>(&mut self, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>);

	/// Begins an isolated layer clipped to `clip`, given in the space of `transform`. Must be balanced by [`Self::pop_layer`].
	fn push_layer(&mut self, fill_rule: Fill, blend_mode: impl Into<BlendMode>, alpha: f32, transform: Affine, clip: &impl Shape);

	/// Begins an isolated layer covering the whole scene, for content without bounds. Must be balanced by [`Self::pop_layer`].
	fn push_viewport_layer(&mut self, blend_mode: impl Into<BlendMode>, alpha: f32);

	/// Clips the content that follows to `clip`, given in the space of `transform`, without isolating it. Must be balanced by [`Self::pop_layer`].
	fn push_clip_layer(&mut self, fill_rule: Fill, transform: Affine, clip: &impl Shape);

	/// Ends the most recent layer or clip.
	fn pop_layer(&mut self);

	/// Draws an image's rectangle of pixels, from the origin to its width and height, placed by `transform`.
	fn draw_image(&mut self, image: &peniko::ImageBrush, transform: Affine);

	/// Draws a CPU raster's rectangle of pixels, from the origin to its width and height, placed by `transform`.
	fn draw_raster(&mut self, raster: Raster<CPU>, sampler: peniko::ImageSampler, transform: Affine);

	/// Draws a GPU texture's rectangle of pixels, from the origin to its width and height, placed by `transform`.
	fn draw_texture(&mut self, texture: Texture, sampler: peniko::ImageSampler, transform: Affine);

	/// Calls `draw` with this builder if it builds a Vello GPU scene, for what only Vello GPU draws, such as effects and generators, returning whether it did.
	fn draw_with_vello(&mut self, _draw: impl FnOnce(&mut VelloSceneBuilder)) -> bool {
		false
	}
}

/// Stands in for an infinite coordinate: far past any viewport, yet small enough for the transforms and clipping it goes through to stay finite.
const UNBOUNDED_COORDINATE: f64 = 1e200;

/// The path with each infinite coordinate replaced by [`UNBOUNDED_COORDINATE`] of the same sign.
/// Points infinite along both axes, or NaN, have no sensible position and are skipped.
pub(crate) fn finite_path(path: &BezPath) -> Cow<'_, BezPath> {
	let element_is_finite = |element: &PathEl| match *element {
		PathEl::MoveTo(p) | PathEl::LineTo(p) => p.is_finite(),
		PathEl::QuadTo(p1, p2) => p1.is_finite() && p2.is_finite(),
		PathEl::CurveTo(p1, p2, p3) => p1.is_finite() && p2.is_finite() && p3.is_finite(),
		PathEl::ClosePath => true,
	};
	if path.elements().iter().all(element_is_finite) {
		return Cow::Borrowed(path);
	}

	let bounded = |value: f64| if value.is_infinite() { value.signum() * UNBOUNDED_COORDINATE } else { value };
	let finite_point = |p: Point| (!p.x.is_nan() && !p.y.is_nan() && !(p.x.is_infinite() && p.y.is_infinite())).then(|| Point::new(bounded(p.x), bounded(p.y)));

	let mut result = BezPath::new();
	// Whether the subpath has yet to reach a point it can start from
	let mut awaiting_start = true;

	for element in path.elements() {
		let end = match *element {
			PathEl::MoveTo(p) => {
				awaiting_start = true;
				p
			}
			PathEl::ClosePath => {
				if !awaiting_start {
					result.close_path();
				}
				continue;
			}
			PathEl::LineTo(p) | PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => p,
		};
		let Some(end) = finite_point(end) else { continue };

		if awaiting_start {
			result.move_to(end);
			awaiting_start = false;
			continue;
		}

		// A curve with an unplaceable control point becomes a line to its end
		match *element {
			PathEl::QuadTo(p1, _) => match finite_point(p1) {
				Some(p1) => result.quad_to(p1, end),
				None => result.line_to(end),
			},
			PathEl::CurveTo(p1, p2, _) => match (finite_point(p1), finite_point(p2)) {
				(Some(p1), Some(p2)) => result.curve_to(p1, p2, end),
				_ => result.line_to(end),
			},
			_ => result.line_to(end),
		}
	}

	Cow::Owned(result)
}
