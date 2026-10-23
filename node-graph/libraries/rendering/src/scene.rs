use graphic_types::generator::GeneratorPattern;
use graphic_types::raster_types::{CPU, Raster, Texture};
use graphic_types::{Adjustment, Effects};
use kurbo::{Affine, BezPath, Rect, Shape, Stroke};
use vello_gpu::PencilGradient;
use vello_gpu::peniko::{self, BlendMode, Brush, Fill};

/// Tolerance for turning shapes other than paths into paths, matching Vello's own.
const SHAPE_TOLERANCE: f64 = 0.1;

/// A recording of drawing operations, kept independent of any render's size or viewport so it can be cached and replayed into Vello at every render.
#[derive(Clone, Debug, Default)]
pub struct Scene {
	commands: Vec<SceneCommand>,
}

/// One recorded drawing operation.
#[derive(Clone, Debug)]
pub enum SceneCommand {
	/// Fills a path given in the space of `transform`, with the brush placed by `transform * brush_transform`.
	Fill {
		fill_rule: Fill,
		transform: Affine,
		brush: SceneBrush,
		brush_transform: Option<Affine>,
		path: BezPath,
	},
	/// Strokes a path given in the space of `transform`, with the brush placed by `transform * brush_transform`.
	Stroke {
		stroke: Stroke,
		transform: Affine,
		brush: SceneBrush,
		brush_transform: Option<Affine>,
		path: BezPath,
	},
	/// Fills the whole render area, for content without bounds, with the brush placed by `brush_transform` in the scene's space.
	FillViewport { brush: SceneBrush, brush_transform: Option<Affine> },
	/// Begins an isolated layer that composites its content with a blend mode and opacity, clipped to a path or covering the whole render area.
	PushLayer { blend_mode: BlendMode, alpha: f32, clip: Option<ClipPath> },
	/// Begins clipping the content that follows to a path, drawing it directly onto what lies beneath rather than into an isolated layer.
	PushClip { clip: ClipPath },
	/// Begins content that renders through effects, which apply to its pixels before the result composites onto what lies beneath.
	PushEffects(EffectsGroup),
	/// Ends the most recently begun layer, clip, or effects group.
	Pop,
	/// Draws an image's rectangle of pixels, from the origin to its width and height, placed by `transform`.
	DrawImage { image: SceneImage, transform: Affine },
	/// Draws a generator's pattern, whose own space `transform` places, at the resolution and across the area the render needs.
	DrawGenerator { pattern: GeneratorPattern, transform: Affine },
}

/// What a fill or stroke paints with.
#[derive(Clone, Debug)]
pub enum SceneBrush {
	/// A solid color, gradient, or image.
	Peniko(Brush),
	/// A gradient whose contours are a pencil of circles, which only Vello GPU's fork can draw.
	PencilGradient(PencilGradient),
}

impl From<Brush> for SceneBrush {
	fn from(brush: Brush) -> Self {
		Self::Peniko(brush)
	}
}

impl From<&Brush> for SceneBrush {
	fn from(brush: &Brush) -> Self {
		Self::Peniko(brush.clone())
	}
}

impl From<peniko::Color> for SceneBrush {
	fn from(color: peniko::Color) -> Self {
		Self::Peniko(Brush::Solid(color))
	}
}

impl From<&peniko::Gradient> for SceneBrush {
	fn from(gradient: &peniko::Gradient) -> Self {
		Self::Peniko(Brush::Gradient(gradient.clone()))
	}
}

impl From<PencilGradient> for SceneBrush {
	fn from(gradient: PencilGradient) -> Self {
		Self::PencilGradient(gradient)
	}
}

/// A clipping path given in the space of `transform`.
#[derive(Clone, Debug)]
pub struct ClipPath {
	pub fill_rule: Fill,
	pub transform: Affine,
	pub path: BezPath,
}

/// The effects that content renders through, as recorded by [`Scene::push_effects`].
#[derive(Clone, Debug)]
pub struct EffectsGroup {
	pub effects: Effects,
	/// How much of the content itself shows in the result, while the shadows and glows cast from it stay at full strength.
	pub fill: f32,
	/// Maps the space the effects' lengths are measured in into the scene's space.
	pub transform: Affine,
	/// The extent of the content in the scene's space, or `None` for content that covers the whole render area.
	pub bounds: Option<Rect>,
	/// The paints of the effects' paint overlays that aren't solid colors, in order, each drawn in the space `transform` maps into the scene's.
	pub paints: Vec<Scene>,
}

/// The pixels an image command draws.
#[derive(Clone, Debug)]
pub enum SceneImage {
	/// Straight-alpha pixels in CPU memory, identified by their blob's ID so the renderer uploads them once.
	Pixels(peniko::ImageBrush),
	/// A CPU raster's straight-alpha linear pixels, identified by their buffer's ID so the renderer uploads them once.
	/// The adjustments, if any, are carried out on its pixels before they are sampled.
	Raster {
		raster: Raster<CPU>,
		adjustments: Vec<Adjustment>,
		sampler: peniko::ImageSampler,
	},
	/// Straight-alpha pixels in a GPU texture.
	Texture { texture: Texture, sampler: peniko::ImageSampler },
}

impl SceneImage {
	/// The width and height in pixels.
	pub fn size(&self) -> (u32, u32) {
		match self {
			Self::Pixels(brush) => (brush.image.width, brush.image.height),
			Self::Raster { raster, .. } => (raster.width, raster.height),
			Self::Texture { texture, .. } => (texture.width(), texture.height()),
		}
	}

	/// How the image's pixels are sampled where they are drawn.
	pub fn sampler(&self) -> peniko::ImageSampler {
		match self {
			Self::Pixels(brush) => brush.sampler,
			Self::Raster { sampler, .. } | Self::Texture { sampler, .. } => *sampler,
		}
	}
}

impl Scene {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn commands(&self) -> &[SceneCommand] {
		&self.commands
	}

	pub fn is_empty(&self) -> bool {
		self.commands.is_empty()
	}

	pub fn fill(&mut self, fill_rule: Fill, transform: Affine, brush: impl Into<SceneBrush>, brush_transform: Option<Affine>, shape: &impl Shape) {
		self.commands.push(SceneCommand::Fill {
			fill_rule,
			transform,
			brush: brush.into(),
			brush_transform,
			path: shape.to_path(SHAPE_TOLERANCE),
		});
	}

	pub fn stroke(&mut self, stroke: &Stroke, transform: Affine, brush: impl Into<SceneBrush>, brush_transform: Option<Affine>, shape: &impl Shape) {
		self.commands.push(SceneCommand::Stroke {
			stroke: stroke.clone(),
			transform,
			brush: brush.into(),
			brush_transform,
			path: shape.to_path(SHAPE_TOLERANCE),
		});
	}

	/// Fills the whole render area, which is how content without bounds (like a color or gradient layer) paints.
	pub fn fill_viewport(&mut self, brush: impl Into<SceneBrush>, brush_transform: Option<Affine>) {
		self.commands.push(SceneCommand::FillViewport { brush: brush.into(), brush_transform });
	}

	/// Begins an isolated layer clipped to `clip`, given in the space of `transform`. Must be balanced by [`Self::pop_layer`].
	pub fn push_layer(&mut self, fill_rule: Fill, blend_mode: impl Into<BlendMode>, alpha: f32, transform: Affine, clip: &impl Shape) {
		self.commands.push(SceneCommand::PushLayer {
			blend_mode: blend_mode.into(),
			alpha,
			clip: Some(ClipPath {
				fill_rule,
				transform,
				path: clip.to_path(SHAPE_TOLERANCE),
			}),
		});
	}

	/// Begins an isolated layer covering the whole render area, for content without bounds. Must be balanced by [`Self::pop_layer`].
	pub fn push_viewport_layer(&mut self, blend_mode: impl Into<BlendMode>, alpha: f32) {
		self.commands.push(SceneCommand::PushLayer {
			blend_mode: blend_mode.into(),
			alpha,
			clip: None,
		});
	}

	/// Clips the content that follows to `clip`, given in the space of `transform`, without isolating it. Must be balanced by [`Self::pop_layer`].
	pub fn push_clip_layer(&mut self, fill_rule: Fill, transform: Affine, clip: &impl Shape) {
		self.commands.push(SceneCommand::PushClip {
			clip: ClipPath {
				fill_rule,
				transform,
				path: clip.to_path(SHAPE_TOLERANCE),
			},
		});
	}

	/// Begins content that renders through `effects`, whose lengths are in the scene's space, within `bounds` (or covering the whole render area for `None`).
	/// `paints` holds the paint overlays' paints that aren't solid colors. Must be balanced by [`Self::pop_layer`].
	pub fn push_effects(&mut self, effects: Effects, fill: f32, bounds: Option<Rect>, paints: Vec<Scene>) {
		self.commands.push(SceneCommand::PushEffects(EffectsGroup {
			effects,
			fill,
			transform: Affine::IDENTITY,
			bounds,
			paints,
		}));
	}

	/// Ends the most recent layer, clip, or effects group.
	pub fn pop_layer(&mut self) {
		self.commands.push(SceneCommand::Pop);
	}

	/// Draws an image's rectangle of pixels, from the origin to its width and height, placed by `transform`.
	pub fn draw_image(&mut self, image: &peniko::ImageBrush, transform: Affine) {
		self.commands.push(SceneCommand::DrawImage {
			image: SceneImage::Pixels(image.clone()),
			transform,
		});
	}

	/// Draws a CPU raster's rectangle of pixels with `adjustments` carried out on them, from the origin to its width and height, placed by `transform`.
	pub fn draw_raster(&mut self, raster: Raster<CPU>, adjustments: Vec<Adjustment>, sampler: peniko::ImageSampler, transform: Affine) {
		self.commands.push(SceneCommand::DrawImage {
			image: SceneImage::Raster { raster, adjustments, sampler },
			transform,
		});
	}

	/// Draws a generator's pattern with its own space placed by `transform`.
	pub fn draw_generator(&mut self, pattern: GeneratorPattern, transform: Affine) {
		self.commands.push(SceneCommand::DrawGenerator { pattern, transform });
	}

	/// Draws a GPU texture's rectangle of pixels, from the origin to its width and height, placed by `transform`.
	pub fn draw_texture(&mut self, texture: Texture, sampler: peniko::ImageSampler, transform: Affine) {
		self.commands.push(SceneCommand::DrawImage {
			image: SceneImage::Texture { texture, sampler },
			transform,
		});
	}

	/// Appends another scene's commands, placed by `transform`.
	pub fn append(&mut self, other: &Scene, transform: Option<Affine>) {
		let Some(transform) = transform else {
			self.commands.extend_from_slice(&other.commands);
			return;
		};

		let place_clip = |clip: &ClipPath| ClipPath {
			transform: transform * clip.transform,
			..clip.clone()
		};

		self.commands.extend(other.commands.iter().map(|command| match command {
			SceneCommand::Fill {
				fill_rule,
				transform: command_transform,
				brush,
				brush_transform,
				path,
			} => SceneCommand::Fill {
				fill_rule: *fill_rule,
				transform: transform * *command_transform,
				brush: brush.clone(),
				brush_transform: *brush_transform,
				path: path.clone(),
			},
			SceneCommand::Stroke {
				stroke,
				transform: command_transform,
				brush,
				brush_transform,
				path,
			} => SceneCommand::Stroke {
				stroke: stroke.clone(),
				transform: transform * *command_transform,
				brush: brush.clone(),
				brush_transform: *brush_transform,
				path: path.clone(),
			},
			// The brush transform is in the scene's own space here, so it is the one placed
			SceneCommand::FillViewport { brush, brush_transform } => SceneCommand::FillViewport {
				brush: brush.clone(),
				brush_transform: Some(transform * brush_transform.unwrap_or(Affine::IDENTITY)),
			},
			SceneCommand::PushLayer { blend_mode, alpha, clip } => SceneCommand::PushLayer {
				blend_mode: *blend_mode,
				alpha: *alpha,
				clip: clip.as_ref().map(place_clip),
			},
			SceneCommand::PushClip { clip } => SceneCommand::PushClip { clip: place_clip(clip) },
			SceneCommand::PushEffects(group) => SceneCommand::PushEffects(EffectsGroup {
				transform: transform * group.transform,
				bounds: group.bounds.map(|bounds| transform.transform_rect_bbox(bounds)),
				..group.clone()
			}),
			SceneCommand::Pop => SceneCommand::Pop,
			SceneCommand::DrawImage { image, transform: command_transform } => SceneCommand::DrawImage {
				image: image.clone(),
				transform: transform * *command_transform,
			},
			SceneCommand::DrawGenerator {
				pattern,
				transform: command_transform,
			} => SceneCommand::DrawGenerator {
				pattern: pattern.clone(),
				transform: transform * *command_transform,
			},
		}));
	}
}
