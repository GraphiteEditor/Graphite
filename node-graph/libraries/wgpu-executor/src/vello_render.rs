use crate::vello_effects::{EffectPasses, EffectTextures};
use glam::{DMat2, UVec2};
use graphic_types::Effects;
use raster_types::WeakCPU;
use rendering::vello_scene_builder::{EffectsRenderer, SceneImage, VelloSceneBuilder, image_downscale_factor, image_texture_size};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use vello_gpu::kurbo::Affine;
use vello_gpu::peniko;
use vello_gpu::{
	ClearSettings, IntermediateTextureError, LayersConfig, MemorySettings, RenderError, RenderSettings, RenderSize, RenderTargetConfig, Renderer, Resources, SizeU16, TargetInit, TextureBindings,
	TextureId,
};
use wgpu_sync::Queue;

/// The format of the textures Vello GPU renders into, of the image textures it samples, and of the textures handed back.
const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How many renders a GPU texture's premultiplied copy survives without being drawn before it is freed.
const UNUSED_IMAGE_RENDER_LIMIT: u64 = 60;

/// How many times a render with too much content for Vello GPU is halved, reaching areas 1/4096 the size of the original.
const MAX_SPLIT_DEPTH: u32 = 12;

/// The widest or tallest area a single render can draw an isolated layer across, since that is the limit of the intermediate textures Vello GPU draws layers into.
/// These grow to fit the largest layer and stay that size, so a larger limit keeps more memory after one large render. Renders larger than this are split.
pub const MAX_LAYER_RENDER_SIZE: u32 = 4096;

/// Vello GPU's renderer and GPU resources, shared by every [`VelloCanvas`] so their memory and pipelines exist once.
pub(crate) struct VelloRenderer {
	/// Locked only while a scene is rendered and submitted, so one canvas drawing its scene on the CPU doesn't hold up another's render.
	gpu: Mutex<(Renderer, Resources)>,
	alpha_conversion: AlphaConversion,
	effect_passes: EffectPasses,
}

/// A stream of frames rendered through the shared Vello renderer, such as the document or the overlays, keeping what its frames reuse.
#[derive(Default)]
pub struct VelloCanvas {
	/// Scenes to draw into, one for each depth of effects groups nested in a render, reused between renders to keep their allocations.
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
	queue: &'a Queue,
}

impl VelloRenderer {
	pub(crate) fn new(device: &wgpu::Device, adapter: &wgpu::Adapter) -> Self {
		let render_target_config = RenderTargetConfig {
			format: TEXTURE_FORMAT,
			width: 1,
			height: 1,
		};

		let settings = RenderSettings::default().with_memory_settings(MemorySettings {
			layers_config: LayersConfig {
				max_texture_size: SizeU16::new(MAX_LAYER_RENDER_SIZE as u16),
				..Default::default()
			},
			..Default::default()
		});

		Self {
			gpu: Mutex::new(Renderer::new_with(device, &render_target_config, settings)),
			alpha_conversion: AlphaConversion::new(device),
			effect_passes: EffectPasses::new(device, adapter),
		}
	}

	/// Renders the scene `draw` draws on `canvas`, with its content placed by `view`, into `target` (an `Rgba8Unorm` texture of `size` with straight alpha).
	/// A render with more content than Vello GPU can hold calls `draw` again for each part it splits into.
	#[expect(clippy::too_many_arguments)]
	pub(crate) fn render(
		&self,
		canvas: &mut VelloCanvas,
		device: &wgpu::Device,
		queue: &Queue,
		draw: &mut dyn FnMut(&mut VelloSceneBuilder),
		view: Affine,
		size: UVec2,
		target: &wgpu::Texture,
	) -> anyhow::Result<()> {
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
		context.render_area(draw, view, size, target, TargetAlpha::Straight);
		canvas.images.free_unused();
		canvas.effect_textures.finish_render();

		Ok(())
	}

	/// Renders a `scene` drawn beforehand on `canvas` into `target` (an `Rgba8Unorm` texture of the scene's size with straight alpha).
	/// Unlike [`Self::render`], it can't split the scene, so one with more content than Vello GPU can hold renders transparent.
	pub(crate) fn render_scene(&self, canvas: &mut VelloCanvas, device: &wgpu::Device, queue: &Queue, scene: VelloSceneBuilder, target: &wgpu::Texture) {
		canvas.images.render_count += 1;
		let max_texture_size = scene.max_texture_size();
		let mut context = RenderContext {
			renderer: self,
			canvas,
			device,
			queue,
		};
		if let Err(error) = context.submit(scene.finish(), max_texture_size, target, TargetAlpha::Straight) {
			log::error!("Vello GPU failed to render: {error}");
			clear(device, queue, target);
		}
		canvas.images.free_unused();
		canvas.effect_textures.finish_render();
	}
}

impl RenderContext<'_> {
	/// Renders the scene `draw` draws into `target`, a texture of `size`, splitting its area into halves while it holds more content than one render can.
	fn render_area(&mut self, draw: &mut dyn FnMut(&mut VelloSceneBuilder), view: Affine, size: UVec2, target: &wgpu::Texture, target_alpha: TargetAlpha) {
		if let Err(error) = self.render_once(draw, view, size, target, target_alpha) {
			self.render_halves(draw, view, size, target, target_alpha, error, 1);
		}
	}

	/// Renders each half of the area of `target`, a texture of `size`, after rendering it whole failed with `error`, splitting again each half that still holds too much.
	#[expect(clippy::too_many_arguments)]
	fn render_halves(&mut self, draw: &mut dyn FnMut(&mut VelloSceneBuilder), view: Affine, size: UVec2, target: &wgpu::Texture, target_alpha: TargetAlpha, error: RenderError, split_depth: u32) {
		let (device, queue) = (self.device, self.queue);

		// Each half covers less of the scene, so it has less content and smaller layers to hold
		let too_much_to_hold = matches!(
			error,
			RenderError::ResourceTextureOverflow { .. } | RenderError::IntermediateTexture(IntermediateTextureError::TooLarge { .. })
		);
		if !too_much_to_hold || split_depth > MAX_SPLIT_DEPTH || size.max_element() <= 1 {
			// A failed render leaves its area transparent rather than taking down the editor
			log::error!("Vello GPU failed to render: {error}");
			clear(device, queue, target);
			return;
		}

		let halves = if size.x >= size.y {
			let left = size.x / 2;
			[(UVec2::ZERO, UVec2::new(left, size.y)), (UVec2::new(left, 0), UVec2::new(size.x - left, size.y))]
		} else {
			let top = size.y / 2;
			[(UVec2::ZERO, UVec2::new(size.x, top)), (UVec2::new(0, top), UVec2::new(size.x, size.y - top))]
		};
		let halves = halves.map(|(offset, half_size)| {
			let half_target = create_texture(
				device,
				"vello_gpu_split_target",
				half_size,
				wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
			);
			let half_view = Affine::translate((-(offset.x as f64), -(offset.y as f64))) * view;
			let rendered = self.render_once(draw, half_view, half_size, &half_target, target_alpha);
			(offset, half_size, half_view, half_target, rendered)
		});

		// Content that each half still holds all of, like paints stacked across the whole area, can't be split apart
		let stalled = |rendered: &Result<(), RenderError>| match (rendered, &error) {
			(
				Err(RenderError::ResourceTextureOverflow { resource, required_height, .. }),
				RenderError::ResourceTextureOverflow {
					resource: whole_resource,
					required_height: whole_required_height,
					..
				},
			) => resource == whole_resource && required_height >= whole_required_height,
			_ => false,
		};
		if halves.iter().all(|(.., rendered)| stalled(rendered)) {
			log::error!("Vello GPU failed to render: {error}");
			clear(device, queue, target);
			return;
		}

		for (offset, half_size, half_view, half_target, rendered) in halves {
			if let Err(half_error) = rendered {
				self.render_halves(draw, half_view, half_size, &half_target, target_alpha, half_error, split_depth + 1);
			}

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
	}

	/// Renders the scene `draw` draws into `target`, a texture of `size`, in a single Vello GPU render.
	fn render_once(&mut self, draw: &mut dyn FnMut(&mut VelloSceneBuilder), view: Affine, size: UVec2, target: &wgpu::Texture, target_alpha: TargetAlpha) -> Result<(), RenderError> {
		let (width, height) = (size.x as u16, size.y as u16);
		let max_texture_size = self.device.limits().max_texture_dimension_2d;

		// Draw the scene for this render's size and placement, which renders its effects groups as they're drawn
		let mut scene = self.canvas.spare_scenes.pop().unwrap_or_else(|| vello_gpu::Scene::new(width, height));
		scene.reset_and_resize(width, height);
		let mut builder = VelloSceneBuilder::from_vello(scene, view, max_texture_size).with_effects_renderer(self);
		draw(&mut builder);
		let drawn = builder.finish();

		self.submit(drawn, max_texture_size, target, target_alpha)
	}

	/// Renders a drawn scene, with the images it draws, into `target`, a texture of the scene's size, in a single Vello GPU render.
	fn submit(&mut self, (scene, images): (vello_gpu::Scene, Vec<SceneImage>), max_texture_size: u32, target: &wgpu::Texture, target_alpha: TargetAlpha) -> Result<(), RenderError> {
		let device = self.device;
		let (width, height) = (scene.width(), scene.height());
		let size = UVec2::new(width as u32, height as u32);

		// Locked from here to the submission, so drawing a scene doesn't hold up other uses of the queue
		let queue = self.queue.lock();

		// Each render submits its own encoder, since Vello GPU stages its uploads for the next submission
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_gpu_render") });

		// Supply the textures of the images the scene draws, under the IDs it drew them with
		let mut texture_bindings = TextureBindings::new();
		for (id, image) in images.iter().enumerate() {
			if let Some(view) = self.canvas.images.prepare(device, &queue, &mut encoder, &self.renderer.alpha_conversion, image, max_texture_size) {
				texture_bindings.insert(TextureId(id as u64), view);
			}
		}

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
			&scene,
			resources,
			device,
			&queue,
			&mut encoder,
			&RenderSize { width, height },
			&premultiplied_view,
			None,
			&texture_bindings,
			target_init,
		);

		if rendered.is_ok() && target_alpha == TargetAlpha::Straight {
			self.renderer.alpha_conversion.unpremultiply(device, &mut encoder, &premultiplied_view, &target_view);
		}

		// Submitted even after a failure, since it also prepares the textures of the scene's images
		queue.submit([encoder.finish()]);
		drop((gpu, queue));

		// Once the render that draws them is submitted, its scene and effects results are free to reuse
		self.canvas.spare_scenes.push(scene);
		for image in images {
			if let SceneImage::Rendered(texture) = image {
				self.canvas.effect_textures.release(texture);
			}
		}

		rendered
	}
}

impl EffectsRenderer for RenderContext<'_> {
	fn renders_effects(&self) -> bool {
		self.renderer.effect_passes.renders_working_formats
	}

	fn render_effects(&mut self, size: UVec2, view: Affine, effects: &Effects, fill: f32, lengths_to_pixels: DMat2, draw: &mut dyn FnMut(&mut VelloSceneBuilder)) -> wgpu::Texture {
		let content_texture = self.canvas.effect_textures.acquire(self.device, size, TEXTURE_FORMAT);
		self.render_area(draw, view, size, &content_texture, TargetAlpha::Premultiplied);

		let effect_textures = &mut self.canvas.effect_textures;
		let result = (self.renderer.effect_passes).apply(self.device, &self.queue.lock(), effect_textures, &content_texture, effects, fill, lengths_to_pixels);
		effect_textures.release(content_texture);

		result
	}
}

/// Leaves `target` transparent.
fn clear(device: &wgpu::Device, queue: &Queue, target: &wgpu::Texture) {
	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_gpu_clear") });
	clear_texture(&mut encoder, &target.create_view(&wgpu::TextureViewDescriptor::default()));
	queue.submit([encoder.finish()]);
}

/// Identifies the pixels behind an image texture: a blob of CPU pixels, or a GPU texture.
#[derive(Clone, PartialEq, Eq, Hash)]
enum ImageKey {
	Blob(u64),
	Raster(u64),
	Texture(wgpu::Texture),
}

/// A premultiplied copy of an image's pixels that Vello GPU samples as an external texture.
struct ImageTexture {
	texture: wgpu::Texture,
	last_used: u64,
	/// The CPU raster pixels copied into the texture, which is freed once they're gone since they can't be drawn again.
	raster: Option<WeakCPU>,
}

/// The textures of the images recent renders drew, each freed once it goes unused for long enough.
#[derive(Default)]
struct ImageTextures {
	textures: HashMap<ImageKey, ImageTexture>,
	render_count: u64,
}

impl ImageTextures {
	/// A view of the premultiplied texture for `image`, uploading CPU pixels the first time they're drawn.
	/// GPU textures are converted again at every render since the texture cache hands their textures out again with new contents.
	fn prepare(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		encoder: &mut wgpu::CommandEncoder,
		alpha_conversion: &AlphaConversion,
		image: &SceneImage,
		max_texture_size: u32,
	) -> Option<wgpu::TextureView> {
		let key = match image {
			SceneImage::Pixels(data) => ImageKey::Blob(data.data.id()),
			SceneImage::Raster(raster) => ImageKey::Raster(raster.buffer_id()),
			SceneImage::Texture(texture) => ImageKey::Texture((**texture).clone()),
			// Effects results are premultiplied already
			SceneImage::Rendered(texture) => return Some(texture.create_view(&wgpu::TextureViewDescriptor::default())),
		};

		if !self.textures.contains_key(&key) {
			let texture = match image {
				SceneImage::Pixels(data) => {
					if data.format != peniko::ImageFormat::Rgba8 {
						log::error!("Unsupported image format {:?}", data.format);
						return None;
					}

					upload_pixels(device, queue, data.data.data(), data.width, data.height, data.alpha_type, max_texture_size)?
				}
				SceneImage::Raster(raster) => {
					let (pixels, width, height) = raster.to_flat_u8();
					upload_pixels(device, queue, &pixels, width, height, peniko::ImageAlphaType::Alpha, max_texture_size)?
				}
				SceneImage::Rendered(_) => return None,
				SceneImage::Texture(texture) => create_texture(
					device,
					"vello_gpu_premultiplied_image",
					UVec2::new(texture.width(), texture.height()),
					wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
				),
			};
			let raster = match image {
				SceneImage::Raster(raster) => Some(raster.downgrade()),
				SceneImage::Pixels(_) | SceneImage::Texture(_) | SceneImage::Rendered(_) => None,
			};
			self.textures.insert(key.clone(), ImageTexture { texture, last_used: 0, raster });
		}

		let image_texture = self.textures.get_mut(&key)?;
		image_texture.last_used = self.render_count;
		let view = image_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

		if let SceneImage::Texture(source) = image {
			let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
			alpha_conversion.premultiply(device, encoder, &source_view, source.format().is_srgb(), &view);
		}

		Some(view)
	}

	fn free_unused(&mut self) {
		let render_count = self.render_count;
		self.textures.retain(|key, image_texture| {
			let unused_renders = render_count - image_texture.last_used;
			match key {
				// CPU pixels get a new blob ID at every render, so a blob this render didn't draw is unlikely to return
				ImageKey::Blob(_) => unused_renders == 0,
				ImageKey::Raster(_) => unused_renders == 0 || (unused_renders < UNUSED_IMAGE_RENDER_LIMIT && image_texture.raster.as_ref().is_some_and(WeakCPU::is_alive)),
				ImageKey::Texture(_) => unused_renders < UNUSED_IMAGE_RENDER_LIMIT,
			}
		});
	}
}

/// Uploads CPU pixels as a premultiplied texture, downscaling them by a whole factor if they exceed `max_texture_size`.
fn upload_pixels(device: &wgpu::Device, queue: &wgpu::Queue, pixels: &[u8], width: u32, height: u32, alpha_type: peniko::ImageAlphaType, max_texture_size: u32) -> Option<wgpu::Texture> {
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
	let factor = image_downscale_factor(width, height, max_texture_size);
	let (texture_width, texture_height) = image_texture_size(width, height, max_texture_size);
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

	Some(texture)
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
				contents: bytemuck::bytes_of(&[source_is_srgb, 0, 0, 0]),
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
