use graphic_types::raster_types::Texture;
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Stroke};
use std::borrow::Cow;
use std::collections::HashMap;
use vello_gpu::peniko::{self, BlendMode, BrushRef, Fill};
use vello_gpu::{Image, ImageSource, PaintType, TextureId, TextureRegion};

/// Tolerance for turning shapes other than paths into paths, matching Vello's own.
const SHAPE_TOLERANCE: f64 = 0.1;

/// Builds a Vello GPU scene of a fixed size, recording the images it draws so the renderer can supply their textures at render time.
#[derive(Debug)]
pub struct VelloSceneBuilder {
	scene: vello_gpu::Scene,
	/// Places the drawn content in the scene, applied on top of every transform it's drawn with.
	view: Affine,
	/// The images drawn, each sampled as the external texture whose ID is its index.
	images: Vec<SceneImage>,
	image_ids: HashMap<ImageKey, TextureId>,
	/// The largest width or height an image's texture can have, beyond which images are drawn from a downscaled texture.
	max_texture_size: u32,
	pushed: Vec<Pushed>,
}

/// The pixels behind an image the scene draws.
#[derive(Clone, Debug)]
pub enum SceneImage {
	/// Straight-alpha RGBA8 pixels in CPU memory, identified by their blob's ID so the renderer uploads them once.
	Pixels(peniko::ImageData),
	/// Straight-alpha pixels in a GPU texture.
	Texture(Texture),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum ImageKey {
	Blob(u64),
	Texture(Texture),
}

#[derive(Debug)]
enum Pushed {
	Layer,
	Clip,
}

impl VelloSceneBuilder {
	pub fn new(width: u16, height: u16, view: Affine, max_texture_size: u32) -> Self {
		Self::from_vello(vello_gpu::Scene::new(width, height), view, max_texture_size)
	}

	/// Wraps a Vello GPU scene, resetting it so its allocations can be reused.
	pub fn from_vello(mut scene: vello_gpu::Scene, view: Affine, max_texture_size: u32) -> Self {
		scene.reset();
		Self {
			scene,
			view,
			images: Vec::new(),
			image_ids: HashMap::new(),
			max_texture_size,
			pushed: Vec::new(),
		}
	}

	pub fn width(&self) -> u16 {
		self.scene.width()
	}

	pub fn height(&self) -> u16 {
		self.scene.height()
	}

	pub fn max_texture_size(&self) -> u32 {
		self.max_texture_size
	}

	/// The finished Vello GPU scene and the images its external textures stand for, closing whatever layers and clips were left open.
	pub fn finish(mut self) -> (vello_gpu::Scene, Vec<SceneImage>) {
		if !self.pushed.is_empty() {
			log::error!("A scene leaves {} layers or clips unended", self.pushed.len());
		}
		while !self.pushed.is_empty() {
			self.pop_layer();
		}

		(self.scene, self.images)
	}

	/// The content's placement in the scene, applied on top of every transform it's drawn with.
	pub fn view(&self) -> Affine {
		self.view
	}

	/// The tolerance Vello GPU expands a stroke drawn with `transform` at, so a stroke outline expanded at it fills identically.
	pub fn stroke_tolerance(&self, transform: Affine) -> f64 {
		// Matches `vello_common::flatten::stroke`, including its use of the diagonal coefficients as the scale
		const TOLERANCE: f64 = 0.25;
		let [a, _, _, d, _, _] = (self.view * transform).as_coeffs();
		TOLERANCE / a.abs().max(d.abs()).max(1.)
	}

	/// Fills a shape given in the space of `transform`, with the brush placed by `transform * brush_transform`.
	pub fn fill<'b>(&mut self, fill_rule: Fill, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, shape: &impl Shape) {
		self.fill_path(fill_rule, transform, brush, brush_transform, &shape.to_path(SHAPE_TOLERANCE));
	}

	/// Like [`Self::fill`] for a path, which it draws without copying.
	pub fn fill_path<'b>(&mut self, fill_rule: Fill, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, path: &BezPath) {
		let Some((paint, paint_to_brush)) = self.paint(brush.into()) else { return };
		self.scene.set_transform(self.view * transform);
		self.scene.set_fill_rule(fill_rule);
		self.scene.set_paint(paint);
		self.scene.set_paint_transform(brush_transform.unwrap_or(Affine::IDENTITY) * paint_to_brush);
		self.scene.fill_path(&finite_path(path));
	}

	/// Strokes a shape given in the space of `transform`, with the brush placed by `transform * brush_transform`.
	pub fn stroke<'b>(&mut self, stroke: &Stroke, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, shape: &impl Shape) {
		let Some((paint, paint_to_brush)) = self.paint(brush.into()) else { return };
		self.scene.set_transform(self.view * transform);
		self.scene.set_stroke(stroke.clone());
		self.scene.set_paint(paint);
		self.scene.set_paint_transform(brush_transform.unwrap_or(Affine::IDENTITY) * paint_to_brush);
		self.scene.stroke_path(&finite_path(&shape.to_path(SHAPE_TOLERANCE)));
	}

	/// Fills the whole scene, which is how content without bounds (like a color or gradient layer) paints, with the brush placed by `brush_transform` in the space of the drawn content.
	pub fn fill_viewport<'b>(&mut self, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>) {
		let Some((paint, paint_to_brush)) = self.paint(brush.into()) else { return };
		self.scene.set_transform(Affine::IDENTITY);
		self.scene.set_paint(paint);
		self.scene.set_paint_transform(self.view * brush_transform.unwrap_or(Affine::IDENTITY) * paint_to_brush);
		self.scene.fill_rect(&Rect::new(0., 0., self.width() as f64, self.height() as f64));
	}

	/// Begins an isolated layer clipped to `clip`, given in the space of `transform`. Must be balanced by [`Self::pop_layer`].
	pub fn push_layer(&mut self, fill_rule: Fill, blend_mode: impl Into<BlendMode>, alpha: f32, transform: Affine, clip: &impl Shape) {
		self.scene.set_transform(self.view * transform);
		self.scene.set_fill_rule(fill_rule);
		self.scene
			.push_layer(Some(&finite_path(&clip.to_path(SHAPE_TOLERANCE))), Some(blend_mode.into()), Some(alpha), None, None);
		self.pushed.push(Pushed::Layer);
	}

	/// Begins an isolated layer covering the whole scene, for content without bounds. Must be balanced by [`Self::pop_layer`].
	pub fn push_viewport_layer(&mut self, blend_mode: impl Into<BlendMode>, alpha: f32) {
		self.scene.push_layer(None, Some(blend_mode.into()), Some(alpha), None, None);
		self.pushed.push(Pushed::Layer);
	}

	/// Clips the content that follows to `clip`, given in the space of `transform`, without isolating it. Must be balanced by [`Self::pop_layer`].
	pub fn push_clip_layer(&mut self, fill_rule: Fill, transform: Affine, clip: &impl Shape) {
		self.scene.set_transform(self.view * transform);
		self.scene.set_fill_rule(fill_rule);
		self.scene.push_clip_path(&finite_path(&clip.to_path(SHAPE_TOLERANCE)));
		self.pushed.push(Pushed::Clip);
	}

	/// Ends the most recent layer or clip.
	pub fn pop_layer(&mut self) {
		match self.pushed.pop() {
			Some(Pushed::Layer) => self.scene.pop_layer(),
			Some(Pushed::Clip) => self.scene.pop_clip(),
			None => log::error!("A scene ends a layer or clip it never began"),
		}
	}

	/// Draws an image's rectangle of pixels, from the origin to its width and height, placed by `transform`.
	pub fn draw_image(&mut self, image: &peniko::ImageBrush, transform: Affine) {
		let Some((source, texture_to_image, texture_size)) = self.image_source(SceneImage::Pixels(image.image.clone())) else {
			return;
		};
		self.draw_image_source(source, image.sampler, transform * texture_to_image, texture_size);
	}

	/// Draws a GPU texture's rectangle of pixels, from the origin to its width and height, placed by `transform`.
	pub fn draw_texture(&mut self, texture: Texture, sampler: peniko::ImageSampler, transform: Affine) {
		let Some((source, texture_to_image, texture_size)) = self.image_source(SceneImage::Texture(texture)) else {
			return;
		};
		self.draw_image_source(source, sampler, transform * texture_to_image, texture_size);
	}

	fn draw_image_source(&mut self, source: ImageSource, sampler: peniko::ImageSampler, transform: Affine, (width, height): (u32, u32)) {
		self.scene.set_transform(self.view * transform);
		self.scene.set_paint(PaintType::Image(Image { image: source, sampler }));
		self.scene.set_paint_transform(Affine::IDENTITY);
		self.scene.fill_rect(&Rect::new(0., 0., width as f64, height as f64));
	}

	/// The Vello GPU paint for a brush, plus the transform from the paint's space into the brush's (which differ for an image drawn from a downscaled texture).
	fn paint(&mut self, brush: BrushRef<'_>) -> Option<(PaintType, Affine)> {
		match brush {
			BrushRef::Solid(color) => Some((PaintType::Solid(color), Affine::IDENTITY)),
			BrushRef::Gradient(gradient) => Some((PaintType::Gradient(gradient.clone()), Affine::IDENTITY)),
			BrushRef::Image(image) => {
				let (source, texture_to_image, _) = self.image_source(SceneImage::Pixels(image.image.clone()))?;
				Some((
					PaintType::Image(Image {
						image: source,
						sampler: image.sampler,
					}),
					texture_to_image,
				))
			}
		}
	}

	/// The external texture standing for an image, with the transform from its texture's pixels onto the image's own pixels and its texture's size.
	fn image_source(&mut self, image: SceneImage) -> Option<(ImageSource, Affine, (u32, u32))> {
		let (width, height) = match &image {
			SceneImage::Pixels(data) => (data.width, data.height),
			SceneImage::Texture(texture) => (texture.width(), texture.height()),
		};
		if width == 0 || height == 0 {
			return None;
		}

		let key = match &image {
			SceneImage::Pixels(data) => ImageKey::Blob(data.data.id()),
			SceneImage::Texture(texture) => ImageKey::Texture(texture.clone()),
		};
		let id = *self.image_ids.entry(key).or_insert_with(|| {
			self.images.push(image);
			TextureId(self.images.len() as u64 - 1)
		});

		let (texture_width, texture_height) = image_texture_size(width, height, self.max_texture_size);
		let texture_to_image = Affine::scale_non_uniform(width as f64 / texture_width as f64, height as f64 / texture_height as f64);
		let source = ImageSource::external_texture(id, TextureRegion::Full, true);

		Some((source, texture_to_image, (texture_width, texture_height)))
	}
}

/// Stands in for an infinite coordinate: far past any viewport, yet small enough for the transforms and clipping it goes through to stay finite.
const UNBOUNDED_COORDINATE: f64 = 1e200;

/// The path with each infinite coordinate replaced by [`UNBOUNDED_COORDINATE`] of the same sign.
/// Points infinite along both axes, or NaN, have no sensible position and are skipped.
fn finite_path(path: &BezPath) -> Cow<'_, BezPath> {
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

/// The size of the texture an image is drawn from, downscaled by a whole factor until both sides fit within `max_texture_size`.
pub fn image_texture_size(width: u32, height: u32, max_texture_size: u32) -> (u32, u32) {
	let factor = image_downscale_factor(width, height, max_texture_size);
	(width.div_ceil(factor), height.div_ceil(factor))
}

/// The whole factor an image is downscaled by so both sides fit within `max_texture_size`.
pub fn image_downscale_factor(width: u32, height: u32, max_texture_size: u32) -> u32 {
	width.max(height).div_ceil(max_texture_size.min(u16::MAX as u32)).max(1)
}
