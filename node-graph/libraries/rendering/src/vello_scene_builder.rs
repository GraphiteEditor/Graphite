use glam::{DMat2, DVec2, UVec2};
use graphene_hash::CacheHash;
use graphic_types::generator::GeneratorPattern;
use graphic_types::raster_types::{CPU, ImageError, Raster, Texture};
use graphic_types::{Adjustment, Effects};
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Stroke};
use std::borrow::Cow;
use std::collections::HashMap;
use vello_gpu::peniko::{self, BlendMode, BrushRef, Fill};
use vello_gpu::{Image, ImageSource, PaintType, TextureId, TextureRegion};

/// Tolerance for turning shapes other than paths into paths, matching Vello's own.
const SHAPE_TOLERANCE: f64 = 0.1;

/// The texture size an effects group's area rounds up to a multiple of, so a group moving across renders keeps reusing the same pooled textures.
const EFFECTS_SIZE_CLASS: u32 = 64;

/// The pixels across and down a generator's tile covers, besides the samples bordering it that let neighboring tiles overlap seamlessly.
const GENERATOR_TILE_PIXELS: u32 = 256;

/// The samples bordering each side of a generator's tile, and how many of them its neighbor's drawing overlaps.
const GENERATOR_TILE_BORDER: u32 = 2;
const GENERATOR_TILE_OVERLAP: u32 = 1;

/// The most tiles one generator draws in a render, past which larger tiles render at a lower resolution, as for a view skewed far along one axis.
const MAX_GENERATOR_TILES_PER_DRAW: f64 = 256.;

/// Builds a Vello GPU scene of a fixed size, recording the images it draws so the renderer can supply their textures at render time.
pub struct VelloSceneBuilder<'a> {
	scene: vello_gpu::Scene,
	/// Places the drawn content in the scene, applied on top of every transform it's drawn with.
	view: Affine,
	/// The images drawn, each sampled as the external texture whose ID is its index.
	images: Vec<SceneImage>,
	image_ids: HashMap<ImageKey, TextureId>,
	/// The largest width or height an image's texture can have, beyond which images are drawn from a downscaled texture.
	max_texture_size: u32,
	pushed: Vec<Pushed>,
	/// Renders effects groups as they're drawn. A scene drawn ahead of its render has none, so it draws effects groups without their effects.
	effects_renderer: Option<&'a mut dyn EffectsRenderer>,
}

/// Renders the content of a scene's effects groups offscreen and carries out their effects, while the scene is drawn.
pub trait EffectsRenderer: Send {
	/// Whether the GPU can carry out effects, without which effects groups draw their content faded by their fill.
	fn renders_effects(&self) -> bool;

	/// Renders the content `draw` draws, and each paint overlay's paint `draw_paint` draws by index, placed by `view` in premultiplied textures of `size`, returning one with `effects` carried out.
	/// `lengths_to_pixels` maps the effects' lengths onto the texture's pixels, and `fill` fades the content but not the shadows and glows cast from it.
	#[expect(clippy::too_many_arguments)]
	fn render_effects(
		&mut self,
		size: UVec2,
		view: Affine,
		effects: &Effects,
		fill: f32,
		lengths_to_pixels: DMat2,
		draw_paint: &mut dyn FnMut(usize, &mut VelloSceneBuilder),
		draw: &mut dyn FnMut(&mut VelloSceneBuilder),
	) -> wgpu::Texture;
}

/// The pixels behind an image the scene draws.
#[derive(Clone, Debug)]
pub enum SceneImage {
	/// Straight-alpha RGBA8 pixels in CPU memory, identified by their blob's ID so the renderer uploads them once.
	Pixels(peniko::ImageData),
	/// A CPU raster's straight-alpha linear pixels, identified by their buffer's ID so the renderer uploads them once.
	/// The adjustments, if any, are carried out on its pixels before they are sampled.
	Raster { raster: Raster<CPU>, adjustments: Vec<Adjustment> },
	/// Straight-alpha pixels in a GPU texture.
	Texture(Texture),
	/// Premultiplied pixels of an effects group's result, rendered for this scene.
	Rendered(wgpu::Texture),
	/// One tile of a generator's pattern, rendered on the CPU the first time it's drawn.
	GeneratorTile { pattern: GeneratorPattern, tile: GeneratorTile },
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum ImageKey {
	Blob(u64),
	Raster(u64),
	Texture(Texture),
	Rendered(wgpu::Texture),
	GeneratorTile(GeneratorTile),
}

/// One tile of a generator's pattern, from a grid in the pattern's space whose tiles span a power of two of units.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GeneratorTile {
	/// Identifies the pattern by the hash of its settings.
	pub pattern: u64,
	/// The tiles span `2^-level` units, so a tile is about the same number of pixels in view at any zoom.
	pub level: i32,
	pub column: i64,
	pub row: i64,
}

impl GeneratorTile {
	/// The samples across and down the tile and its border.
	pub const SAMPLES: u32 = GENERATOR_TILE_PIXELS + 2 * GENERATOR_TILE_BORDER;

	/// The pattern's pixels across the tile and its border, rendered on the CPU.
	pub fn render(&self, pattern: &GeneratorPattern) -> Result<Raster<CPU>, ImageError> {
		let (corner, texel) = self.corner_and_texel();
		raster_kernels::generators::render_pattern(pattern, [corner, corner + DVec2::splat(Self::SAMPLES as f64 * texel)], Self::SAMPLES, Self::SAMPLES).map(Raster::new_cpu)
	}

	/// The corner of the tile's samples, including its border, in the pattern's space, and the span of one sample.
	pub fn corner_and_texel(&self) -> (DVec2, f64) {
		let span = 2_f64.powi(-self.level);
		let texel = span / GENERATOR_TILE_PIXELS as f64;
		(DVec2::new(self.column as f64, self.row as f64) * span - GENERATOR_TILE_BORDER as f64 * texel, texel)
	}
}

#[derive(Debug)]
enum Pushed {
	Layer,
	Clip,
}

impl std::fmt::Debug for VelloSceneBuilder<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("VelloSceneBuilder")
			.field("scene", &self.scene)
			.field("view", &self.view)
			.field("images", &self.images)
			.finish_non_exhaustive()
	}
}

impl<'a> VelloSceneBuilder<'a> {
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
			effects_renderer: None,
		}
	}

	/// Renders the effects groups drawn into the scene through `renderer`.
	pub fn with_effects_renderer(mut self, renderer: &'a mut dyn EffectsRenderer) -> Self {
		self.effects_renderer = Some(renderer);
		self
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

	/// Draws a CPU raster's rectangle of pixels with `adjustments` carried out on them, from the origin to its width and height, placed by `transform`.
	pub fn draw_raster(&mut self, raster: Raster<CPU>, adjustments: Vec<Adjustment>, sampler: peniko::ImageSampler, transform: Affine) {
		let Some((source, texture_to_image, texture_size)) = self.image_source(SceneImage::Raster { raster, adjustments }) else {
			return;
		};
		self.draw_image_source(source, sampler, transform * texture_to_image, texture_size);
	}

	/// Draws a GPU texture's rectangle of pixels, from the origin to its width and height, placed by `transform`.
	pub fn draw_texture(&mut self, texture: Texture, sampler: peniko::ImageSampler, transform: Affine) {
		let Some((source, texture_to_image, texture_size)) = self.image_source(SceneImage::Texture(texture)) else {
			return;
		};
		self.draw_image_source(source, sampler, transform * texture_to_image, texture_size);
	}

	/// Draws a generator's pattern, whose own space `transform` places, across the part of the scene it covers as tiles the renderer makes the first time each is seen.
	/// The tiles span a power of two of the pattern's units, so a pan reuses them, as does a zoom until the scale nears the next power.
	pub fn draw_generator(&mut self, pattern: &GeneratorPattern, transform: Affine) {
		let local_to_device = self.view * transform;
		let [a, b, c, d, _, _] = local_to_device.as_coeffs();
		let determinant = a * d - b * c;
		if !determinant.is_normal() {
			return;
		}

		// The part of the pattern's space in view
		let viewport = Rect::new(0., 0., self.width() as f64, self.height() as f64);
		let visible = local_to_device.inverse().transform_rect_bbox(viewport);
		if visible.width() <= 0. || visible.height() <= 0. {
			return;
		}

		// The tile size nearest to a tile's pixels in view, grown if the view would otherwise need too many
		let pixels_per_unit = determinant.abs().sqrt();
		let tile_span = |level: i32| 2_f64.powi(-level);
		let tile_range = |start: f64, end: f64, span: f64| (start / span).floor() as i64..(end / span).ceil() as i64;
		let tile_count = |level: i32| {
			let span = tile_span(level);
			(tile_range(visible.x0, visible.x1, span).count() * tile_range(visible.y0, visible.y1, span).count()) as f64
		};
		let mut level = (pixels_per_unit / GENERATOR_TILE_PIXELS as f64).log2().round() as i32;
		while tile_count(level) > MAX_GENERATOR_TILES_PER_DRAW {
			level -= 1;
		}
		let span = tile_span(level);

		let mut hasher = std::hash::DefaultHasher::new();
		pattern.cache_hash(&mut hasher);
		let pattern_hash = std::hash::Hasher::finish(&hasher);

		let (border, overlap) = (GENERATOR_TILE_BORDER as f64, GENERATOR_TILE_OVERLAP as f64);
		for row in tile_range(visible.y0, visible.y1, span) {
			for column in tile_range(visible.x0, visible.x1, span) {
				let tile = GeneratorTile {
					pattern: pattern_hash,
					level,
					column,
					row,
				};
				let id = *self.image_ids.entry(ImageKey::GeneratorTile(tile)).or_insert_with(|| {
					self.images.push(SceneImage::GeneratorTile { pattern: pattern.clone(), tile });
					TextureId(self.images.len() as u64 - 1)
				});

				// Drawn across the tile and part of its border, overlapping its neighbors' identical samples so no seam shows between them
				let (corner, texel) = tile.corner_and_texel();
				self.scene.set_transform(local_to_device * Affine::translate((corner.x, corner.y)) * Affine::scale(texel));
				self.scene.set_paint(PaintType::Image(Image {
					image: ImageSource::external_texture(id, TextureRegion::Full, true),
					sampler: peniko::ImageSampler::new(),
				}));
				self.scene.set_paint_transform(Affine::IDENTITY);
				let (start, end) = (border - overlap, border + GENERATOR_TILE_PIXELS as f64 + overlap);
				self.scene.fill_rect(&Rect::new(start, start, end, end));
			}
		}
	}

	/// Draws the content `draw` draws with `effects` carried out on it, within `bounds` in the space of the drawn content (or everywhere for `None`), drawing again for each part of a split render.
	/// `draw_paint` draws each paint overlay's paint by its index among them, and `fill` fades the content but not the shadows and glows cast from it.
	pub fn draw_effects(&mut self, effects: &Effects, fill: f32, bounds: Option<Rect>, draw_paint: &mut dyn FnMut(usize, &mut VelloSceneBuilder), draw: &mut dyn FnMut(&mut VelloSceneBuilder)) {
		if !self.effects_renderer.as_ref().is_some_and(|renderer| renderer.renders_effects()) {
			self.push_viewport_layer(peniko::Mix::Normal, fill);
			draw(self);
			self.pop_layer();
			return;
		}

		let [a, b, c, d, _, _] = self.view.as_coeffs();
		let lengths_to_device = DMat2::from_cols_array(&[a, b, c, d]);
		let reach = effects.reach() * lengths_to_device.determinant().abs().sqrt();

		// The content reaching into the viewport, rounded out to whole pixels
		let viewport = Rect::new(0., 0., self.width() as f64, self.height() as f64);
		let content_bounds = bounds.map_or(viewport, |bounds| self.view.transform_rect_bbox(bounds));
		let region = content_bounds.inflate(reach, reach).intersect(viewport.inflate(reach, reach)).expand();
		if !(region.width() > 0. && region.height() > 0.) {
			return;
		}

		// A region too large for one texture renders at a lower resolution, which the blurs and shadows that make it so large hide.
		// Its size rounds up to a size class so a group that moves between renders keeps reusing the same textures.
		let max_size = self.max_texture_size.min(u16::MAX as u32);
		let resolution = (max_size as f64 / region.width().max(region.height())).min(1.);
		let size_along = |length: f64| ((length * resolution).ceil() as u32).next_multiple_of(EFFECTS_SIZE_CLASS).min(max_size);
		let size = UVec2::new(size_along(region.width()), size_along(region.height()));
		let origin = region.origin().to_vec2();

		let content_view = Affine::scale(resolution) * Affine::translate(-origin) * self.view;
		let Some(renderer) = self.effects_renderer.as_deref_mut() else { return };
		let result = renderer.render_effects(size, content_view, effects, fill, lengths_to_device * resolution, draw_paint, draw);

		// Draw the result where the content was
		self.images.push(SceneImage::Rendered(result));
		let id = TextureId(self.images.len() as u64 - 1);
		self.scene.set_transform(Affine::translate(origin) * Affine::scale(1. / resolution));
		self.scene.set_paint(PaintType::Image(Image {
			image: ImageSource::external_texture(id, TextureRegion::Full, true),
			sampler: peniko::ImageSampler::new(),
		}));
		self.scene.set_paint_transform(Affine::IDENTITY);
		self.scene.fill_rect(&Rect::new(0., 0., size.x as f64, size.y as f64));
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
			SceneImage::Raster { raster, .. } => (raster.width, raster.height),
			SceneImage::Texture(texture) => (texture.width(), texture.height()),
			SceneImage::Rendered(texture) => (texture.width(), texture.height()),
			SceneImage::GeneratorTile { .. } => (GeneratorTile::SAMPLES, GeneratorTile::SAMPLES),
		};
		if width == 0 || height == 0 {
			return None;
		}

		// Adjusted rasters aren't shared, since the same pixels can be drawn with different adjustments
		let key = match &image {
			SceneImage::Pixels(data) => Some(ImageKey::Blob(data.data.id())),
			SceneImage::Raster { raster, adjustments } if adjustments.is_empty() => Some(ImageKey::Raster(raster.buffer_id())),
			SceneImage::Raster { .. } => None,
			SceneImage::Texture(texture) => Some(ImageKey::Texture(texture.clone())),
			SceneImage::Rendered(texture) => Some(ImageKey::Rendered(texture.clone())),
			SceneImage::GeneratorTile { tile, .. } => Some(ImageKey::GeneratorTile(*tile)),
		};
		let images = &mut self.images;
		let mut add_image = move |image| {
			images.push(image);
			TextureId(images.len() as u64 - 1)
		};
		let id = match key {
			Some(key) => *self.image_ids.entry(key).or_insert_with(|| add_image(image)),
			None => add_image(image),
		};

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
