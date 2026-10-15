use crate::vello_effects::{AdjustmentPassTextures, EffectPasses, EffectTextures, adjustment_run_hash};
use core_types::graphene_hash::CacheHash;
use glam::{DMat2, DVec2, UVec2};
use graphic_types::Adjustment;
use graphic_types::generator::GeneratorPattern;
use graphic_types::raster_types::{CPU, Raster};
use rendering::scene::{EffectsGroup, Scene, SceneBrush, SceneCommand, SceneImage};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use vello_gpu::kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape};
use vello_gpu::peniko::{self, Brush};
use vello_gpu::{
	ClearSettings, Image, ImageSource, IntermediateTextureError, LayersConfig, MemorySettings, PaintType, RectU16, RenderError, RenderSettings, RenderSize, RenderTargetConfig, Renderer, Resources,
	SizeU16, TargetInit, TextureBindings, TextureId,
};

/// The format of the textures Vello GPU renders into, of the image textures it samples, and of the textures handed back.
const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How many renders an image's texture survives without being drawn before it is freed.
const UNUSED_IMAGE_RENDER_LIMIT: u64 = 60;

/// The pixels across and down a generator's tile covers, besides the samples bordering it that let neighboring tiles overlap seamlessly.
const GENERATOR_TILE_PIXELS: u32 = 256;

/// The samples bordering each side of a generator's tile, and how many of them its neighbor's drawing overlaps.
const GENERATOR_TILE_BORDER: u32 = 2;
const GENERATOR_TILE_OVERLAP: u32 = 1;

/// The most tiles of generators kept between renders, past which the least recently drawn are freed.
const MAX_CACHED_GENERATOR_TILES: usize = 256;

/// The most tiles one generator draws in a render, past which larger tiles render at a lower resolution, as for a view skewed far along one axis.
const MAX_GENERATOR_TILES_PER_DRAW: f64 = 256.;

/// How many times a render with too much content for Vello GPU is halved, reaching areas 1/4096 the size of the original.
const MAX_SPLIT_DEPTH: u32 = 12;

/// The widest or tallest area a single render can draw an isolated layer across, since that is the limit of the intermediate textures Vello GPU draws layers into.
/// These grow to fit the largest layer and stay that size, so a larger limit keeps more memory after one large render. Renders larger than this are split.
pub const MAX_LAYER_RENDER_SIZE: u32 = 4096;

/// The texture size an effects group's area rounds up to a multiple of, so a group moving across renders keeps reusing the same pooled textures.
const EFFECTS_SIZE_CLASS: u32 = 64;

/// The first ID of the textures holding a render's effects results, far from the IDs of image textures, which count up from zero.
const FIRST_EFFECTS_TEXTURE_ID: u64 = 1 << 63;

/// Vello GPU's renderer and GPU resources, shared by every [`VelloCanvas`] so their memory and pipelines exist once.
pub(crate) struct VelloRenderer {
	/// Locked only while a recorded scene is drawn and submitted, so one canvas recording its scene on the CPU doesn't hold up another's render.
	gpu: Mutex<(Renderer, Resources)>,
	alpha_conversion: AlphaConversion,
	effect_passes: EffectPasses,
}

/// A stream of frames rendered through the shared Vello renderer, such as the document or the overlays, keeping what its frames reuse.
#[derive(Default)]
pub struct VelloCanvas {
	/// Scenes to record into, one for each depth of effects groups nested in a render, reused between renders to keep their allocations.
	spare_scenes: Vec<vello_gpu::Scene>,
	/// The premultiplied texture Vello GPU renders into before conversion to straight alpha, kept while the render size stays the same.
	premultiplied_target: Option<wgpu::Texture>,
	images: ImageTextures,
	effect_textures: EffectTextures,
}

/// Whether a render leaves its target with the straight alpha the rest of Graphite expects, or the premultiplied alpha the effect passes take.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetAlpha {
	Straight,
	Premultiplied,
}

/// A render in progress: the shared renderer and GPU, and the canvas whose resources it draws with.
struct RenderContext<'a> {
	renderer: &'a VelloRenderer,
	canvas: &'a mut VelloCanvas,
	device: &'a wgpu::Device,
	queue: &'a wgpu::Queue,
}

/// What one Vello GPU render records: its scene, and the textures its paints sample.
struct Recording {
	scene: vello_gpu::Scene,
	texture_bindings: TextureBindings,
	/// The textures holding the results of the render's effects groups, kept until the render that draws them is submitted.
	effect_results: Vec<wgpu::Texture>,
}

impl VelloRenderer {
	pub(crate) fn new(device: &wgpu::Device) -> Self {
		let render_target_config = RenderTargetConfig {
			format: TEXTURE_FORMAT,
			width: 1,
			height: 1,
		};

		let settings = RenderSettings {
			memory_settings: MemorySettings {
				layers_config: LayersConfig {
					max_texture_size: SizeU16::new(MAX_LAYER_RENDER_SIZE as u16),
					..Default::default()
				},
				..Default::default()
			},
			..Default::default()
		};

		Self {
			gpu: Mutex::new(Renderer::new_with(device, &render_target_config, settings)),
			alpha_conversion: AlphaConversion::new(device),
			effect_passes: EffectPasses::new(device),
		}
	}

	/// Renders `scene` on `canvas`, placed by `transform`, into `target` (an `Rgba8Unorm` texture of `size` with straight alpha).
	#[expect(clippy::too_many_arguments)]
	pub(crate) fn render(&self, canvas: &mut VelloCanvas, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene, transform: Affine, size: UVec2, target: &wgpu::Texture) -> anyhow::Result<()> {
		if u16::try_from(size.x).is_err() || u16::try_from(size.y).is_err() {
			anyhow::bail!("Render size {size} exceeds Vello GPU's limit");
		}

		canvas.images.render_count += 1;
		let mut context = RenderContext {
			renderer: self,
			canvas,
			device,
			queue,
		};
		context.render_area(scene.commands(), transform, size, target, TargetAlpha::Straight, 0);
		canvas.images.free_unused();
		canvas.effect_textures.finish_render();

		Ok(())
	}
}

impl RenderContext<'_> {
	/// Renders `commands` into `target`, a texture of `size`. When they hold more content than one render can, renders each half of the area separately and copies it into place.
	fn render_area(&mut self, commands: &[SceneCommand], transform: Affine, size: UVec2, target: &wgpu::Texture, target_alpha: TargetAlpha, split_depth: u32) {
		let (device, queue) = (self.device, self.queue);
		let error = match self.render_once(commands, transform, size, target, target_alpha) {
			Ok(()) => return,
			Err(error) => error,
		};

		// Each half covers less of the scene, so it has less content and smaller layers to hold
		let too_much_to_hold = matches!(
			error,
			RenderError::ResourceTextureOverflow { .. } | RenderError::IntermediateTexture(IntermediateTextureError::TooLarge { .. })
		);
		if too_much_to_hold && split_depth < MAX_SPLIT_DEPTH && size.max_element() > 1 {
			let halves = if size.x >= size.y {
				let left = size.x / 2;
				[(UVec2::ZERO, UVec2::new(left, size.y)), (UVec2::new(left, 0), UVec2::new(size.x - left, size.y))]
			} else {
				let top = size.y / 2;
				[(UVec2::ZERO, UVec2::new(size.x, top)), (UVec2::new(0, top), UVec2::new(size.x, size.y - top))]
			};

			for (offset, half_size) in halves {
				let half_target = create_texture(
					device,
					"vello_gpu_split_target",
					half_size,
					wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
				);
				let half_transform = Affine::translate((-(offset.x as f64), -(offset.y as f64))) * transform;
				self.render_area(commands, half_transform, half_size, &half_target, target_alpha, split_depth + 1);

				let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_gpu_split_copy") });
				encoder.copy_texture_to_texture(
					half_target.as_image_copy(),
					wgpu::TexelCopyTextureInfo {
						origin: wgpu::Origin3d { x: offset.x, y: offset.y, z: 0 },
						..target.as_image_copy()
					},
					half_target.size(),
				);
				queue.submit([encoder.finish()]);
			}
			return;
		}

		// A failed render leaves its area transparent rather than taking down the editor
		log::error!("Vello GPU failed to render: {error}");
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_gpu_clear") });
		clear_texture(&mut encoder, &target.create_view(&wgpu::TextureViewDescriptor::default()));
		queue.submit([encoder.finish()]);
	}

	/// Renders `commands` into `target`, a texture of `size`, in a single Vello GPU render.
	fn render_once(&mut self, commands: &[SceneCommand], transform: Affine, size: UVec2, target: &wgpu::Texture, target_alpha: TargetAlpha) -> Result<(), RenderError> {
		let (device, queue) = (self.device, self.queue);
		let (width, height) = (size.x as u16, size.y as u16);

		// Record the scene for this render's size and placement, preparing the textures of its images and rendering its effects groups
		let mut recording = Recording {
			scene: self.canvas.spare_scenes.pop().unwrap_or_else(|| vello_gpu::Scene::new(1, 1)),
			texture_bindings: TextureBindings::new(),
			effect_results: Vec::new(),
		};
		recording.scene.reset_and_resize(width, height);
		self.replay(&mut recording, commands, transform, Rect::new(0., 0., width as f64, height as f64));

		// Each render submits its own encoder, since Vello GPU stages its uploads for the next submission
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_gpu_render") });

		// Vello GPU renders premultiplied, which is converted to straight alpha unless the target wants it premultiplied
		let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
		let premultiplied_view = match (target_alpha, &self.canvas.premultiplied_target) {
			(TargetAlpha::Premultiplied, _) => target_view.clone(),
			(TargetAlpha::Straight, Some(texture)) if texture.width() == size.x && texture.height() == size.y => texture.create_view(&wgpu::TextureViewDescriptor::default()),
			(TargetAlpha::Straight, _) => {
				let texture = create_texture(
					device,
					"vello_gpu_premultiplied_target",
					size,
					wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
				);
				let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
				self.canvas.premultiplied_target = Some(texture);
				view
			}
		};
		let target_init = TargetInit::Clear(ClearSettings::Viewport { color: peniko::Color::TRANSPARENT });

		// Vello GPU stages this render's uploads into resources every render shares, so another can't stage its own until this one is submitted
		let mut gpu = self.renderer.gpu.lock().unwrap_or_else(PoisonError::into_inner);
		let (renderer, resources) = &mut *gpu;
		let rendered = renderer.render(
			&recording.scene,
			resources,
			device,
			queue,
			&mut encoder,
			&RenderSize { width, height },
			&premultiplied_view,
			None,
			&recording.texture_bindings,
			target_init,
		);

		if rendered.is_ok() && target_alpha == TargetAlpha::Straight {
			self.renderer.alpha_conversion.unpremultiply(device, &mut encoder, &premultiplied_view, &target_view);
		}

		queue.submit([encoder.finish()]);
		drop(gpu);

		// Once the render that draws them is submitted, its scene and effects results are free to reuse
		self.canvas.spare_scenes.push(recording.scene);
		for texture in recording.effect_results {
			self.canvas.effect_textures.release(texture);
		}

		rendered
	}

	/// Records `commands` into the render's Vello GPU scene, placed by `transform`, with `viewport` as the area fills of unbounded content cover.
	fn replay(&mut self, recording: &mut Recording, commands: &[SceneCommand], transform: Affine, viewport: Rect) {
		enum Pushed {
			Layer,
			Clip,
		}
		let mut pushed = Vec::new();

		// How deep the replay is inside a layer or clip that covers nothing in the viewport, whose content is skipped
		let mut culled_depth = 0_usize;

		let mut index = 0;
		while let Some(command) = commands.get(index) {
			index += 1;

			if culled_depth > 0 {
				match command {
					SceneCommand::PushLayer { .. } | SceneCommand::PushClip { .. } | SceneCommand::PushEffects(_) => culled_depth += 1,
					SceneCommand::Pop => culled_depth -= 1,
					_ => {}
				}
				continue;
			}

			let target = &mut recording.scene;
			let texture_bindings = &mut recording.texture_bindings;
			match command {
				SceneCommand::Fill {
					fill_rule,
					transform: path_transform,
					brush,
					brush_transform,
					path,
				} => {
					let Some(paint_to_brush) = set_paint(target, brush, &mut |image| self.resolve_image(texture_bindings, image)) else {
						continue;
					};
					target.set_transform(transform * *path_transform);
					target.set_fill_rule(*fill_rule);
					target.set_paint_transform(brush_transform.unwrap_or(Affine::IDENTITY) * paint_to_brush);
					target.fill_path(&finite_path(path));
				}
				SceneCommand::Stroke {
					stroke,
					transform: path_transform,
					brush,
					brush_transform,
					path,
				} => {
					let Some(paint_to_brush) = set_paint(target, brush, &mut |image| self.resolve_image(texture_bindings, image)) else {
						continue;
					};
					target.set_transform(transform * *path_transform);
					target.set_stroke(stroke.clone());
					target.set_paint_transform(brush_transform.unwrap_or(Affine::IDENTITY) * paint_to_brush);
					target.stroke_path(&finite_path(path));
				}
				SceneCommand::FillViewport { brush, brush_transform } => {
					let Some(paint_to_brush) = set_paint(target, brush, &mut |image| self.resolve_image(texture_bindings, image)) else {
						continue;
					};
					target.set_transform(Affine::IDENTITY);
					target.set_paint_transform(transform * brush_transform.unwrap_or(Affine::IDENTITY) * paint_to_brush);
					target.fill_rect(&viewport);
				}
				SceneCommand::PushLayer { blend_mode, alpha, clip } => {
					let clip_path = clip.as_ref().map(|clip| finite_path(&clip.path));
					if let (Some(clip), Some(clip_path)) = (clip, &clip_path) {
						if !clip_reaches(clip_path, transform * clip.transform, viewport) {
							culled_depth = 1;
							continue;
						}

						target.set_transform(transform * clip.transform);
						target.set_fill_rule(clip.fill_rule);
					}
					target.push_layer(clip_path.as_deref(), Some(*blend_mode), Some(*alpha), None, None);
					pushed.push(Pushed::Layer);
				}
				SceneCommand::PushClip { clip } => {
					let clip_path = finite_path(&clip.path);
					if !clip_reaches(&clip_path, transform * clip.transform, viewport) {
						culled_depth = 1;
						continue;
					}

					target.set_transform(transform * clip.transform);
					target.set_fill_rule(clip.fill_rule);
					target.push_clip_path(&clip_path);
					pushed.push(Pushed::Clip);
				}
				// The group's content is consumed through its own render rather than replayed here
				SceneCommand::PushEffects(group) => {
					let end = matching_pop(commands, index);
					self.draw_effects_group(recording, group, &commands[index..end], transform, viewport);
					index = end + 1;
				}
				SceneCommand::Pop => match pushed.pop() {
					Some(Pushed::Layer) => target.pop_layer(),
					Some(Pushed::Clip) => target.pop_clip(),
					None => log::error!("A recorded scene ends a layer or clip it never began"),
				},
				SceneCommand::DrawImage { image, transform: image_transform } => {
					let Some(resolved) = self.resolve_image(texture_bindings, image) else { continue };
					let sampler = image.sampler();

					target.set_transform(transform * *image_transform * resolved.texture_to_image);
					target.set_paint(PaintType::Image(Image { image: resolved.source, sampler }));
					target.set_paint_transform(Affine::IDENTITY);
					let (width, height) = resolved.texture_size;
					target.fill_rect(&Rect::new(0., 0., width as f64, height as f64));
				}
				SceneCommand::DrawGenerator { pattern, transform: local_transform } => self.draw_generator(recording, pattern, transform * *local_transform, viewport),
			}
		}

		// Close whatever a malformed scene left open
		if !pushed.is_empty() {
			log::error!("A recorded scene leaves {} layers or clips unended", pushed.len());
		}
		for kind in pushed.into_iter().rev() {
			match kind {
				Pushed::Layer => recording.scene.pop_layer(),
				Pushed::Clip => recording.scene.pop_clip(),
			}
		}
	}

	/// The texture to draw `image` with, bound for the render being recorded.
	fn resolve_image(&mut self, texture_bindings: &mut TextureBindings, image: &SceneImage) -> Option<ResolvedImage> {
		let image_texture = self.canvas.images.prepare(self.device, self.queue, &self.renderer.alpha_conversion, image)?;

		// Adjustments draw a copy of the texture they're carried out on
		let (texture_id, view, resolved) = match image {
			SceneImage::Raster { raster, adjustments, .. } if !adjustments.is_empty() => {
				let run: Vec<&Adjustment> = adjustments.iter().collect();
				let effects = (&self.renderer.effect_passes, &mut self.canvas.effect_textures);
				self.canvas.images.prepare_adjusted(self.device, self.queue, effects, raster, &run)?
			}
			_ => (image_texture.texture_id, image_texture.view.clone(), image_texture.paint_source(image_texture.texture_id)),
		};
		texture_bindings.insert(texture_id, view);

		Some(resolved)
	}

	/// Draws a generator's pattern across the part of the `viewport` it covers, as tiles each rendered on the CPU the first time it's seen.
	/// The tiles span a power of two of the pattern's units, so a pan reuses them, as does a zoom until the scale nears the next power.
	fn draw_generator(&mut self, recording: &mut Recording, pattern: &GeneratorPattern, local_to_device: Affine, viewport: Rect) {
		let [a, b, c, d, _, _] = local_to_device.as_coeffs();
		let determinant = a * d - b * c;
		if !determinant.is_normal() {
			return;
		}

		// The part of the pattern's space in view
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
		let texel = span / GENERATOR_TILE_PIXELS as f64;

		let mut hasher = std::hash::DefaultHasher::new();
		pattern.cache_hash(&mut hasher);
		let pattern_hash = std::hash::Hasher::finish(&hasher);

		let (border, overlap) = (GENERATOR_TILE_BORDER as f64, GENERATOR_TILE_OVERLAP as f64);
		let pixels_with_border = GENERATOR_TILE_PIXELS + 2 * GENERATOR_TILE_BORDER;
		for row in tile_range(visible.y0, visible.y1, span) {
			for column in tile_range(visible.x0, visible.x1, span) {
				// The tile's samples, including its border, from this corner of the pattern's space
				let corner = DVec2::new(column as f64, row as f64) * span - border * texel;
				let render = || pattern.render([corner, corner + DVec2::splat(pixels_with_border as f64 * texel)], pixels_with_border, pixels_with_border);

				let tile = GeneratorTile {
					pattern: pattern_hash,
					level,
					column,
					row,
				};
				let Some(image_texture) = self.canvas.images.prepare_generator_tile(self.device, self.queue, tile, || Raster::new_cpu(render())) else {
					continue;
				};
				recording.texture_bindings.insert(image_texture.texture_id, image_texture.view.clone());
				let resolved = image_texture.paint_source(image_texture.texture_id);

				// Drawn across the tile and part of its border, overlapping its neighbors' identical samples so no seam shows between them
				let texels_to_device = local_to_device * Affine::translate((corner.x, corner.y)) * Affine::scale(texel);
				recording.scene.set_transform(texels_to_device * resolved.texture_to_image);
				recording.scene.set_paint(PaintType::Image(Image {
					image: resolved.source,
					sampler: peniko::ImageSampler::new(),
				}));
				recording.scene.set_paint_transform(Affine::IDENTITY);
				let (start, end) = (border - overlap, border + GENERATOR_TILE_PIXELS as f64 + overlap);
				recording.scene.fill_rect(&Rect::new(start, start, end, end));
			}
		}
	}

	/// Renders an effects group's `content` offscreen, carries out its effects there, and draws the result in place of the content.
	fn draw_effects_group(&mut self, recording: &mut Recording, group: &EffectsGroup, content: &[SceneCommand], transform: Affine, viewport: Rect) {
		let [a, b, c, d, _, _] = (transform * group.transform).as_coeffs();
		let lengths_to_device = DMat2::from_cols_array(&[a, b, c, d]);
		let reach = group.effects.reach() * lengths_to_device.determinant().abs().sqrt();

		// The content reaching into the viewport, rounded out to whole pixels
		let content_bounds = content_extent(group, content).map_or(viewport, |bounds| transform.transform_rect_bbox(bounds));
		let region = content_bounds.inflate(reach, reach).intersect(viewport.inflate(reach, reach)).expand();
		if !(region.width() > 0. && region.height() > 0.) {
			return;
		}

		// A region too large for one texture renders at a lower resolution, which the blurs and shadows that make it so large hide.
		// Its size rounds up to a size class so a group that moves between renders keeps reusing the same textures.
		let max_size = self.device.limits().max_texture_dimension_2d.min(u16::MAX as u32);
		let resolution = (max_size as f64 / region.width().max(region.height())).min(1.);
		let size_along = |length: f64| ((length * resolution).ceil() as u32).next_multiple_of(EFFECTS_SIZE_CLASS).min(max_size);
		let size = UVec2::new(size_along(region.width()), size_along(region.height()));
		let origin = region.origin().to_vec2();

		let content_texture = self.canvas.effect_textures.acquire(self.device, size, TEXTURE_FORMAT);
		let content_transform = Affine::scale(resolution) * Affine::translate(-origin) * transform;
		self.render_area(content, content_transform, size, &content_texture, TargetAlpha::Premultiplied, 0);

		// The paint overlays' paints, each rendered over the same pixels as the content
		let paint_transform = content_transform * group.transform;
		let paint_textures: Vec<wgpu::Texture> = (group.paints.iter())
			.map(|paint| {
				let paint_texture = self.canvas.effect_textures.acquire(self.device, size, TEXTURE_FORMAT);
				self.render_area(paint.commands(), paint_transform, size, &paint_texture, TargetAlpha::Premultiplied, 0);
				paint_texture
			})
			.collect();

		let effect_textures = &mut self.canvas.effect_textures;
		let lengths_to_pixels = lengths_to_device * resolution;
		let result = (self.renderer.effect_passes).apply(
			self.device,
			self.queue,
			effect_textures,
			&content_texture,
			&group.effects,
			group.fill,
			lengths_to_pixels,
			&paint_textures,
		);
		effect_textures.release(content_texture);
		paint_textures.into_iter().for_each(|paint_texture| effect_textures.release(paint_texture));

		// Draw the result where the content was, under an ID none of the render's images can share
		let texture_id = TextureId(FIRST_EFFECTS_TEXTURE_ID + recording.effect_results.len() as u64);
		recording.texture_bindings.insert(texture_id, result.create_view(&wgpu::TextureViewDescriptor::default()));
		let target = &mut recording.scene;
		target.set_transform(Affine::translate(origin) * Affine::scale(1. / resolution));
		target.set_paint(PaintType::Image(Image {
			image: ImageSource::external_texture(texture_id, RectU16::new(0, 0, size.x as u16, size.y as u16), true),
			sampler: peniko::ImageSampler::new(),
		}));
		target.set_paint_transform(Affine::IDENTITY);
		target.fill_rect(&Rect::new(0., 0., size.x as f64, size.y as f64));

		recording.effect_results.push(result);
	}
}

/// The area an effects group's content covers in the scene's space, or `None` if unbounded.
/// Effects groups nested in the content paint past their own bounds, so the area grows by how far all of them reach.
fn content_extent(group: &EffectsGroup, content: &[SceneCommand]) -> Option<Rect> {
	let mut bounds = group.bounds?;
	let mut nested_reach = 0.;

	for command in content {
		if let SceneCommand::PushEffects(nested) = command {
			bounds = bounds.union(nested.bounds?);
			nested_reach += nested.effects.reach() * nested.transform.determinant().abs().sqrt();
		}
	}

	Some(bounds.inflate(nested_reach, nested_reach))
}

/// The index of the `Pop` ending the layer, clip, or effects group whose content begins at `start`, or the end of `commands` if a malformed scene never ends it.
fn matching_pop(commands: &[SceneCommand], start: usize) -> usize {
	let mut depth = 0_usize;

	for (index, command) in commands.iter().enumerate().skip(start) {
		match command {
			SceneCommand::PushLayer { .. } | SceneCommand::PushClip { .. } | SceneCommand::PushEffects(_) => depth += 1,
			SceneCommand::Pop if depth == 0 => return index,
			SceneCommand::Pop => depth -= 1,
			_ => {}
		}
	}

	commands.len()
}

/// The source and placement of an image ready to be drawn, with the rectangle of texture pixels it covers.
struct ResolvedImage {
	source: ImageSource,
	/// Maps the drawn texture's pixels onto the image's own pixels, which differ when the texture had to be downscaled.
	texture_to_image: Affine,
	texture_size: (u16, u16),
}

/// Whether a clip path, placed by `transform`, overlaps the viewport. Vello GPU fails to render a layer whose clip covers no pixels.
fn clip_reaches(clip_path: &BezPath, transform: Affine, viewport: Rect) -> bool {
	let bounds = transform.transform_rect_bbox(clip_path.bounding_box());
	bounds.x0 < viewport.x1 && bounds.x1 > viewport.x0 && bounds.y0 < viewport.y1 && bounds.y1 > viewport.y0
}

/// Stands in for an infinite coordinate: far past any viewport, yet small enough for the transforms and clipping it goes through to stay finite.
const UNBOUNDED_COORDINATE: f64 = 1e200;

/// The path with its infinite coordinates made finite, so a line to infinity along one axis keeps the average of its other axis.
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

/// Paints with a recorded brush, returning the transform from the paint's space into the brush's (which differ for an image whose texture was downscaled).
fn set_paint(target: &mut vello_gpu::Scene, brush: &SceneBrush, resolve_image: &mut dyn FnMut(&SceneImage) -> Option<ResolvedImage>) -> Option<Affine> {
	let brush = match brush {
		SceneBrush::Peniko(brush) => brush,
		SceneBrush::PencilGradient(gradient) => {
			target.set_pencil_gradient_paint(gradient.clone());
			return Some(Affine::IDENTITY);
		}
	};

	match brush {
		Brush::Solid(color) => target.set_paint(PaintType::Solid(*color)),
		Brush::Gradient(gradient) => target.set_paint(PaintType::Gradient(gradient.clone())),
		Brush::Image(image_brush) => {
			let resolved = resolve_image(&SceneImage::Pixels(image_brush.clone()))?;
			target.set_paint(PaintType::Image(Image {
				image: resolved.source,
				sampler: image_brush.sampler,
			}));
			return Some(resolved.texture_to_image);
		}
	}
	Some(Affine::IDENTITY)
}

/// Identifies the pixels behind an image texture: a blob of CPU pixels, a GPU texture, or a tile of a generator's pattern.
#[derive(Clone, PartialEq, Eq, Hash)]
enum ImageKey {
	Blob(u64),
	Raster(u64),
	Texture(wgpu::Texture),
	GeneratorTile(GeneratorTile),
}

/// One tile of a generator's pattern, from a grid in the pattern's space whose tiles span a power of two of units.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GeneratorTile {
	/// Identifies the pattern by the hash of its settings.
	pattern: u64,
	/// The tiles span `2^-level` units, so a tile is about the same number of pixels in view at any zoom.
	level: i32,
	column: i64,
	row: i64,
}

/// A premultiplied copy of an image's pixels that Vello GPU samples as an external texture.
struct ImageTexture {
	texture_id: TextureId,
	texture: wgpu::Texture,
	view: wgpu::TextureView,
	/// The copy's size, smaller than the image's own when the image exceeds the device's texture size limit.
	size: (u16, u16),
	mip_level_count: u8,
	/// Maps the copy's pixels onto the image's own pixels.
	texture_to_image: Affine,
	last_used: u64,
	/// Copies with runs of adjustments carried out on them, one for each run the image is drawn with.
	adjusted: Vec<AdjustedTexture>,
}

impl ImageTexture {
	/// Draws this texture, or the copy of it with `texture_id`, which has the same size and placement.
	fn paint_source(&self, texture_id: TextureId) -> ResolvedImage {
		let (width, height) = self.size;

		ResolvedImage {
			source: ImageSource::external_texture(texture_id, RectU16::new(0, 0, width, height), true).with_mip_level_count(self.mip_level_count),
			texture_to_image: self.texture_to_image,
			texture_size: self.size,
		}
	}
}

/// A copy of an image's texture with a run of adjustments carried out on it, made over in place for another run or refined tables.
struct AdjustedTexture {
	texture_id: TextureId,
	texture: wgpu::Texture,
	view: wgpu::TextureView,
	run_hash: u64,
	/// The tables it was made with, which a run's refined tables replace.
	passes: Arc<Vec<AdjustmentPassTextures>>,
	last_used: u64,
}

/// The textures of the images recent renders drew, freed once unused for [`UNUSED_IMAGE_RENDER_LIMIT`] renders.
#[derive(Default)]
struct ImageTextures {
	textures: HashMap<ImageKey, ImageTexture>,
	next_texture_id: u64,
	render_count: u64,
}

impl ImageTextures {
	/// The texture for `image`, uploading or converting its pixels the first time it is drawn.
	/// The conversion is submitted right away, ahead of whichever render first draws the texture, since a render can begin inside another's recording.
	fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, alpha_conversion: &AlphaConversion, image: &SceneImage) -> Option<&ImageTexture> {
		let key = match image {
			SceneImage::Pixels(brush) => ImageKey::Blob(brush.image.data.id()),
			SceneImage::Raster { raster, .. } => ImageKey::Raster(raster.buffer_id()),
			SceneImage::Texture { texture, .. } => ImageKey::Texture((**texture).clone()),
		};

		if !self.textures.contains_key(&key) {
			let (texture, texture_to_image) = match image {
				SceneImage::Pixels(brush) => {
					if brush.image.format != peniko::ImageFormat::Rgba8 {
						log::error!("Unsupported image format {:?}", brush.image.format);
						return None;
					}

					let image = &brush.image;
					upload_pixels(device, queue, image.data.data(), image.width, image.height, image.alpha_type)?
				}
				SceneImage::Raster { raster, .. } => {
					let (pixels, width, height) = raster.to_flat_u8();
					upload_pixels(device, queue, &pixels, width, height, peniko::ImageAlphaType::Alpha)?
				}
				SceneImage::Texture { texture, .. } => {
					let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
						label: Some("vello_gpu_premultiply_image"),
					});
					let premultiplied = premultiply_texture(device, &mut encoder, alpha_conversion, texture);
					queue.submit([encoder.finish()]);
					(premultiplied, Affine::IDENTITY)
				}
			};
			self.insert(key.clone(), texture, texture_to_image)?;
		}

		let image_texture = self.textures.get_mut(&key)?;
		image_texture.last_used = self.render_count;
		Some(image_texture)
	}

	/// The texture of one tile of a generator's pattern, with `render` making its pixels the first time it's drawn.
	fn prepare_generator_tile(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, tile: GeneratorTile, render: impl FnOnce() -> Raster<CPU>) -> Option<&ImageTexture> {
		let key = ImageKey::GeneratorTile(tile);

		if !self.textures.contains_key(&key) {
			let (pixels, width, height) = render().to_flat_u8();
			let (texture, texture_to_image) = upload_pixels(device, queue, &pixels, width, height, peniko::ImageAlphaType::Alpha)?;
			self.insert(key.clone(), texture, texture_to_image)?;
		}

		let image_texture = self.textures.get_mut(&key)?;
		image_texture.last_used = self.render_count;
		Some(image_texture)
	}

	fn insert(&mut self, key: ImageKey, texture: wgpu::Texture, texture_to_image: Affine) -> Option<()> {
		let size = (u16::try_from(texture.width()).ok()?, u16::try_from(texture.height()).ok()?);

		self.textures.insert(
			key,
			ImageTexture {
				texture_id: TextureId(self.next_texture_id),
				view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
				mip_level_count: texture.mip_level_count() as u8,
				texture,
				size,
				texture_to_image,
				last_used: self.render_count,
				adjusted: Vec::new(),
			},
		);
		self.next_texture_id += 1;

		Some(())
	}

	/// The copy of the texture of the `raster` already prepared, with the `run` of adjustments carried out on it, made with the run's latest tables.
	fn prepare_adjusted(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		effects: (&EffectPasses, &mut EffectTextures),
		raster: &Raster<CPU>,
		run: &[&Adjustment],
	) -> Option<(TextureId, wgpu::TextureView, ResolvedImage)> {
		let (effect_passes, effect_textures) = effects;
		let run_hash = adjustment_run_hash(run);
		let passes = effect_textures.adjustment_passes(device, queue, run);
		let render_count = self.render_count;
		let image_texture = self.textures.get_mut(&ImageKey::Raster(raster.buffer_id()))?;

		// A run seen before has its copy, and a new run takes over the copy longest unused, as when dragging a slider makes a new run every render
		let reusable = || {
			image_texture
				.adjusted
				.iter()
				.enumerate()
				.filter(|(_, copy)| copy.last_used < render_count)
				.min_by_key(|(_, copy)| copy.last_used)
		};
		let index = match image_texture.adjusted.iter().position(|copy| copy.run_hash == run_hash) {
			Some(index) => index,
			None => match reusable().map(|(index, _)| index) {
				Some(index) => index,
				None => {
					let texture = device.create_texture(&wgpu::TextureDescriptor {
						label: Some("vello_gpu_adjusted_image"),
						size: image_texture.texture.size(),
						mip_level_count: image_texture.texture.mip_level_count(),
						sample_count: 1,
						dimension: wgpu::TextureDimension::D2,
						format: TEXTURE_FORMAT,
						usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
						view_formats: &[],
					});
					image_texture.adjusted.push(AdjustedTexture {
						texture_id: TextureId(self.next_texture_id),
						view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
						texture,
						run_hash,
						passes: Arc::new(Vec::new()),
						last_used: render_count,
					});
					self.next_texture_id += 1;
					image_texture.adjusted.len() - 1
				}
			},
		};

		let copy = &mut image_texture.adjusted[index];
		if copy.run_hash != run_hash || !Arc::ptr_eq(&copy.passes, &passes) {
			effect_passes.adjust_image(device, queue, effect_textures, &image_texture.texture, &passes, &copy.texture);
			copy.run_hash = run_hash;
			copy.passes = passes;
		}
		copy.last_used = render_count;

		let (texture_id, view) = (copy.texture_id, copy.view.clone());
		Some((texture_id, view, image_texture.paint_source(texture_id)))
	}

	fn free_unused(&mut self) {
		let render_count = self.render_count;
		let in_use = |last_used: u64| render_count - last_used < UNUSED_IMAGE_RENDER_LIMIT;

		self.textures.retain(|_, image_texture| in_use(image_texture.last_used));
		self.textures.values_mut().for_each(|image_texture| image_texture.adjusted.retain(|copy| in_use(copy.last_used)));

		// Panning and zooming across a generator leaves tiles behind faster than they go unused for long, so the least recent beyond a limit are freed too
		let mut tile_ages: Vec<u64> = (self.textures.iter())
			.filter(|(key, _)| matches!(key, ImageKey::GeneratorTile(_)))
			.map(|(_, image_texture)| image_texture.last_used)
			.collect();
		if tile_ages.len() > MAX_CACHED_GENERATOR_TILES {
			let excess = tile_ages.len() - MAX_CACHED_GENERATOR_TILES;
			let (_, &mut oldest_kept, _) = tile_ages.select_nth_unstable(excess);
			self.textures
				.retain(|key, image_texture| !matches!(key, ImageKey::GeneratorTile(_)) || image_texture.last_used >= oldest_kept);
		}
	}
}

/// Uploads CPU pixels as a premultiplied texture, downscaling them by a whole factor if they exceed the device's texture size limit.
fn upload_pixels(device: &wgpu::Device, queue: &wgpu::Queue, pixels: &[u8], width: u32, height: u32, alpha_type: peniko::ImageAlphaType) -> Option<(wgpu::Texture, Affine)> {
	if width == 0 || height == 0 {
		return None;
	}

	let premultiplied: Vec<u8> = match alpha_type {
		peniko::ImageAlphaType::Alpha => pixels
			.as_chunks::<4>()
			.0
			.iter()
			.flat_map(|pixel| {
				let alpha = pixel[3] as u32;
				let premultiply = |channel: u8| ((channel as u32 * alpha + 127) / 255) as u8;
				[premultiply(pixel[0]), premultiply(pixel[1]), premultiply(pixel[2]), pixel[3]]
			})
			.collect(),
		peniko::ImageAlphaType::AlphaPremultiplied => pixels.to_vec(),
	};

	// Box-average whole blocks of pixels down until both sides fit, which is exact to average since the pixels are premultiplied
	let limit = device.limits().max_texture_dimension_2d.min(u16::MAX as u32);
	let factor = width.max(height).div_ceil(limit).max(1);
	let (texture_width, texture_height) = (width.div_ceil(factor), height.div_ceil(factor));
	let pixels = if factor == 1 { premultiplied } else { box_downscale(&premultiplied, width, height, factor) };

	// Each mipmap level halves the one before it, so a minified image averages its pixels rather than skipping over them
	let level_count = texture_width.max(texture_height).ilog2() + 1;
	let texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("vello_gpu_image"),
		size: wgpu::Extent3d {
			width: texture_width,
			height: texture_height,
			depth_or_array_layers: 1,
		},
		mip_level_count: level_count,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: TEXTURE_FORMAT,
		usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
		view_formats: &[],
	});

	let (mut level_pixels, mut level_width, mut level_height) = (pixels, texture_width, texture_height);
	for level in 0..level_count {
		if level > 0 {
			(level_pixels, level_width, level_height) = halve_for_mipmap(&level_pixels, level_width, level_height);
		}

		queue.write_texture(
			wgpu::TexelCopyTextureInfo {
				texture: &texture,
				mip_level: level,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			&level_pixels,
			wgpu::TexelCopyBufferLayout {
				offset: 0,
				bytes_per_row: Some(4 * level_width),
				rows_per_image: Some(level_height),
			},
			wgpu::Extent3d {
				width: level_width,
				height: level_height,
				depth_or_array_layers: 1,
			},
		);
	}

	let texture_to_image = Affine::scale_non_uniform(width as f64 / texture_width as f64, height as f64 / texture_height as f64);
	Some((texture, texture_to_image))
}

/// Halves premultiplied RGBA8 pixels to the next mipmap level's size, which rounds down, so the last pixel of each row and column of an odd side averages three.
/// O(width × height).
fn halve_for_mipmap(pixels: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
	let (half_width, half_height) = ((width / 2).max(1), (height / 2).max(1));
	// The source pixels along one side that an output pixel averages
	let span = |index: u32, half: u32, full: u32| {
		let start = index * 2;
		let end = if index + 1 == half { full } else { start + 2 };
		start..end.clamp(start + 1, full)
	};

	let mut output = Vec::with_capacity((half_width * half_height * 4) as usize);
	for output_y in 0..half_height {
		for output_x in 0..half_width {
			let mut sums = [0_u32; 4];
			let mut count = 0;
			for y in span(output_y, half_height, height) {
				for x in span(output_x, half_width, width) {
					let index = ((y * width + x) * 4) as usize;
					sums.iter_mut().zip(&pixels[index..index + 4]).for_each(|(sum, channel)| *sum += *channel as u32);
					count += 1;
				}
			}
			output.extend(sums.map(|sum| ((sum + count / 2) / count) as u8));
		}
	}

	(output, half_width, half_height)
}

/// Averages each `factor` by `factor` block of RGBA8 pixels, letting blocks at the right and bottom edges average only the pixels they have.
fn box_downscale(pixels: &[u8], width: u32, height: u32, factor: u32) -> Vec<u8> {
	let (output_width, output_height) = (width.div_ceil(factor), height.div_ceil(factor));
	let mut output = Vec::with_capacity((output_width * output_height * 4) as usize);

	for output_y in 0..output_height {
		for output_x in 0..output_width {
			let mut sums = [0_u32; 4];
			let mut count = 0;

			for y in (output_y * factor)..((output_y + 1) * factor).min(height) {
				for x in (output_x * factor)..((output_x + 1) * factor).min(width) {
					let index = ((y * width + x) * 4) as usize;
					for (sum, channel) in sums.iter_mut().zip(&pixels[index..index + 4]) {
						*sum += *channel as u32;
					}
					count += 1;
				}
			}

			output.extend(sums.map(|sum| ((sum + count / 2) / count) as u8));
		}
	}

	output
}

/// Copies a straight-alpha GPU texture into a premultiplied `Rgba8Unorm` texture of the same size.
fn premultiply_texture(device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, alpha_conversion: &AlphaConversion, source: &wgpu::Texture) -> wgpu::Texture {
	let size = UVec2::new(source.width(), source.height());
	let texture = create_texture(
		device,
		"vello_gpu_premultiplied_image",
		size,
		wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
	);

	let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
	let target_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
	alpha_conversion.premultiply(device, encoder, &source_view, source.format().is_srgb(), &target_view);

	texture
}

fn clear_texture(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
	encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
		label: Some("vello_gpu_clear_pass"),
		color_attachments: &[Some(wgpu::RenderPassColorAttachment {
			view,
			resolve_target: None,
			ops: wgpu::Operations {
				load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
				store: wgpu::StoreOp::Store,
			},
			depth_slice: None,
		})],
		depth_stencil_attachment: None,
		timestamp_writes: None,
		occlusion_query_set: None,
		multiview_mask: None,
	});
}

fn create_texture(device: &wgpu::Device, label: &str, size: UVec2, usage: wgpu::TextureUsages) -> wgpu::Texture {
	device.create_texture(&wgpu::TextureDescriptor {
		label: Some(label),
		size: wgpu::Extent3d {
			width: size.x.max(1),
			height: size.y.max(1),
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: TEXTURE_FORMAT,
		usage,
		view_formats: &[],
	})
}

/// Full-target passes converting between straight and premultiplied alpha.
struct AlphaConversion {
	bind_group_layout: wgpu::BindGroupLayout,
	unpremultiply_pipeline: wgpu::RenderPipeline,
	premultiply_pipeline: wgpu::RenderPipeline,
	/// The parameters for a source whose format stores linear values.
	linear_source_parameters: wgpu::Buffer,
	/// The parameters for a source whose format decodes sRGB on load.
	srgb_source_parameters: wgpu::Buffer,
}

impl AlphaConversion {
	fn new(device: &wgpu::Device) -> Self {
		use wgpu::util::DeviceExt;

		let shader = device.create_shader_module(wgpu::include_wgsl!("vello_alpha.wgsl"));

		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("vello_alpha_bind_group_layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: false },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				},
			],
		});
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("vello_alpha_pipeline_layout"),
			bind_group_layouts: &[Some(&bind_group_layout)],
			immediate_size: 0,
		});

		let create_pipeline = |label: &str, entry_point: &str| {
			device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
				label: Some(label),
				layout: Some(&pipeline_layout),
				vertex: wgpu::VertexState {
					module: &shader,
					entry_point: Some("vertex"),
					compilation_options: Default::default(),
					buffers: &[],
				},
				fragment: Some(wgpu::FragmentState {
					module: &shader,
					entry_point: Some(entry_point),
					compilation_options: Default::default(),
					targets: &[Some(wgpu::ColorTargetState {
						format: TEXTURE_FORMAT,
						blend: None,
						write_mask: wgpu::ColorWrites::ALL,
					})],
				}),
				primitive: wgpu::PrimitiveState {
					topology: wgpu::PrimitiveTopology::TriangleList,
					..Default::default()
				},
				depth_stencil: None,
				multisample: wgpu::MultisampleState::default(),
				multiview_mask: None,
				cache: None,
			})
		};

		let create_parameters = |label: &str, source_is_srgb: u32| {
			device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(label),
				contents: &source_is_srgb.to_ne_bytes(),
				usage: wgpu::BufferUsages::UNIFORM,
			})
		};

		Self {
			unpremultiply_pipeline: create_pipeline("vello_unpremultiply_pipeline", "unpremultiply"),
			premultiply_pipeline: create_pipeline("vello_premultiply_pipeline", "premultiply"),
			linear_source_parameters: create_parameters("vello_alpha_linear_source", 0),
			srgb_source_parameters: create_parameters("vello_alpha_srgb_source", 1),
			bind_group_layout,
		}
	}

	fn unpremultiply(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, source: &wgpu::TextureView, target: &wgpu::TextureView) {
		self.run(device, encoder, &self.unpremultiply_pipeline, &self.linear_source_parameters, source, target);
	}

	fn premultiply(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, source: &wgpu::TextureView, source_is_srgb: bool, target: &wgpu::TextureView) {
		let parameters = if source_is_srgb { &self.srgb_source_parameters } else { &self.linear_source_parameters };
		self.run(device, encoder, &self.premultiply_pipeline, parameters, source, target);
	}

	fn run(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, pipeline: &wgpu::RenderPipeline, parameters: &wgpu::Buffer, source: &wgpu::TextureView, target: &wgpu::TextureView) {
		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("vello_alpha_bind_group"),
			layout: &self.bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(source),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: parameters.as_entire_binding(),
				},
			],
		});

		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("vello_alpha_pass"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: target,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
					store: wgpu::StoreOp::Store,
				},
				depth_slice: None,
			})],
			depth_stencil_attachment: None,
			timestamp_writes: None,
			occlusion_query_set: None,
			multiview_mask: None,
		});
		pass.set_pipeline(pipeline);
		pass.set_bind_group(0, &bind_group, &[]);
		pass.draw(0..3, 0..1);
	}
}

#[cfg(test)]
mod tests;
