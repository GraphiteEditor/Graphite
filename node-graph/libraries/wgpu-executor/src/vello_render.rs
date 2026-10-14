use glam::UVec2;
use rendering::scene::{Scene, SceneCommand, SceneImage};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
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

/// How many times a render with too much content for Vello GPU is halved, reaching areas 1/4096 the size of the original.
const MAX_SPLIT_DEPTH: u32 = 12;

/// The widest or tallest area a single render can draw an isolated layer across, since that is the limit of the intermediate textures Vello GPU draws layers into.
/// These grow to fit the largest layer and stay that size, so a larger limit keeps more memory after one large render. Renders larger than this are split.
pub const MAX_LAYER_RENDER_SIZE: u32 = 4096;

/// Vello GPU's renderer and GPU resources, shared by every [`VelloCanvas`] so their memory and pipelines exist once.
pub(crate) struct VelloRenderer {
	/// Locked only while a recorded scene is drawn and submitted, so one canvas recording its scene on the CPU doesn't hold up another's render.
	gpu: Mutex<(Renderer, Resources)>,
	alpha_conversion: AlphaConversion,
}

/// A stream of frames rendered through the shared Vello renderer, such as the document or the overlays, keeping what its frames reuse.
pub struct VelloCanvas {
	/// Reused between renders to keep its allocations.
	scene: vello_gpu::Scene,
	/// The premultiplied texture Vello GPU renders into before conversion to straight alpha, kept while the render size stays the same.
	premultiplied_target: Option<wgpu::Texture>,
	images: ImageTextures,
}

impl Default for VelloCanvas {
	fn default() -> Self {
		Self {
			scene: vello_gpu::Scene::new(1, 1),
			premultiplied_target: None,
			images: ImageTextures::default(),
		}
	}
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
		}
	}

	/// Renders `scene` on `canvas`, placed by `transform`, into `target` (an `Rgba8Unorm` texture of `size` with straight alpha).
	#[expect(clippy::too_many_arguments)]
	pub(crate) fn render(&self, canvas: &mut VelloCanvas, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene, transform: Affine, size: UVec2, target: &wgpu::Texture) -> anyhow::Result<()> {
		if u16::try_from(size.x).is_err() || u16::try_from(size.y).is_err() {
			anyhow::bail!("Render size {size} exceeds Vello GPU's limit");
		}

		canvas.images.render_count += 1;
		self.render_area(canvas, device, queue, scene, transform, size, target, 0);
		canvas.images.free_unused();

		Ok(())
	}

	/// Renders `scene` into `target`, a texture of `size`. When the scene holds more content than one render can, renders each half of the area separately and copies it into place.
	#[expect(clippy::too_many_arguments)]
	fn render_area(&self, canvas: &mut VelloCanvas, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene, transform: Affine, size: UVec2, target: &wgpu::Texture, split_depth: u32) {
		let error = match self.render_once(canvas, device, queue, scene, transform, size, target) {
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
				self.render_area(canvas, device, queue, scene, half_transform, half_size, &half_target, split_depth + 1);

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

	/// Renders `scene` into `target`, a texture of `size`, in a single Vello GPU render.
	#[expect(clippy::too_many_arguments)]
	fn render_once(&self, canvas: &mut VelloCanvas, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene, transform: Affine, size: UVec2, target: &wgpu::Texture) -> Result<(), RenderError> {
		let (width, height) = (size.x as u16, size.y as u16);
		let VelloCanvas {
			scene: gpu_scene,
			premultiplied_target,
			images,
		} = canvas;

		// Each render submits its own encoder, since Vello GPU stages its uploads for the next submission
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_gpu_render") });

		// Record the scene for this render's size and placement, preparing the textures of the images it draws
		gpu_scene.reset_and_resize(width, height);
		let mut texture_bindings = TextureBindings::new();
		let viewport = Rect::new(0., 0., width as f64, height as f64);
		replay(gpu_scene, scene, transform, viewport, &mut |image| {
			let image_texture = images.prepare(device, queue, &mut encoder, &self.alpha_conversion, image)?;
			texture_bindings.insert(image_texture.texture_id, image_texture.view.clone());
			Some(image_texture.paint_source())
		});

		// Render premultiplied, then convert to the straight alpha the rest of Graphite expects
		let premultiplied_target = match premultiplied_target {
			Some(texture) if texture.width() == size.x && texture.height() == size.y => texture.clone(),
			_ => {
				let texture = create_texture(
					device,
					"vello_gpu_premultiplied_target",
					size,
					wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
				);
				*premultiplied_target = Some(texture.clone());
				texture
			}
		};
		let premultiplied_view = premultiplied_target.create_view(&wgpu::TextureViewDescriptor::default());
		let target_init = TargetInit::Clear(ClearSettings::Viewport { color: peniko::Color::TRANSPARENT });

		// Vello GPU stages this render's uploads into resources every render shares, so another can't stage its own until this one is submitted
		let mut gpu = self.gpu.lock().unwrap_or_else(PoisonError::into_inner);
		let (renderer, resources) = &mut *gpu;
		let rendered = renderer.render(
			gpu_scene,
			resources,
			device,
			queue,
			&mut encoder,
			&RenderSize { width, height },
			&premultiplied_view,
			None,
			&texture_bindings,
			target_init,
		);

		if rendered.is_ok() {
			let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
			self.alpha_conversion.unpremultiply(device, &mut encoder, &premultiplied_view, &target_view);
		}

		// Submitted even after a failure, since it also prepares the textures of the scene's images
		queue.submit([encoder.finish()]);
		drop(gpu);

		rendered
	}
}

/// The source and placement of an image ready to be drawn, with the rectangle of texture pixels it covers.
struct ResolvedImage {
	source: ImageSource,
	/// Maps the drawn texture's pixels onto the image's own pixels, which differ when the texture had to be downscaled.
	texture_to_image: Affine,
	texture_size: (u16, u16),
}

/// Records `scene` into Vello GPU's scene, placed by `transform`, with `viewport` as the area fills of unbounded content cover.
fn replay(target: &mut vello_gpu::Scene, scene: &Scene, transform: Affine, viewport: Rect, resolve_image: &mut dyn FnMut(&SceneImage) -> Option<ResolvedImage>) {
	enum Pushed {
		Layer,
		Clip,
	}
	let mut pushed = Vec::new();

	// How deep the replay is inside a layer or clip that covers nothing in the viewport, whose content is skipped
	let mut culled_depth = 0_usize;

	for command in scene.commands() {
		if culled_depth > 0 {
			match command {
				SceneCommand::PushLayer { .. } | SceneCommand::PushClip { .. } => culled_depth += 1,
				SceneCommand::Pop => culled_depth -= 1,
				_ => {}
			}
			continue;
		}

		match command {
			SceneCommand::Fill {
				fill_rule,
				transform: path_transform,
				brush,
				brush_transform,
				path,
			} => {
				let Some((paint, paint_to_brush)) = resolve_paint(brush, resolve_image) else { continue };
				target.set_transform(transform * *path_transform);
				target.set_fill_rule(*fill_rule);
				target.set_paint(paint);
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
				let Some((paint, paint_to_brush)) = resolve_paint(brush, resolve_image) else { continue };
				target.set_transform(transform * *path_transform);
				target.set_stroke(stroke.clone());
				target.set_paint(paint);
				target.set_paint_transform(brush_transform.unwrap_or(Affine::IDENTITY) * paint_to_brush);
				target.stroke_path(&finite_path(path));
			}
			SceneCommand::FillViewport { brush, brush_transform } => {
				let Some((paint, paint_to_brush)) = resolve_paint(brush, resolve_image) else { continue };
				target.set_transform(Affine::IDENTITY);
				target.set_paint(paint);
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
			SceneCommand::Pop => match pushed.pop() {
				Some(Pushed::Layer) => target.pop_layer(),
				Some(Pushed::Clip) => target.pop_clip(),
				None => log::error!("A recorded scene ends a layer or clip it never began"),
			},
			SceneCommand::DrawImage { image, transform: image_transform } => {
				let Some(resolved) = resolve_image(image) else { continue };
				let sampler = match image {
					SceneImage::Pixels(brush) => brush.sampler,
					SceneImage::Texture { sampler, .. } => *sampler,
				};

				target.set_transform(transform * *image_transform * resolved.texture_to_image);
				target.set_paint(PaintType::Image(Image { image: resolved.source, sampler }));
				target.set_paint_transform(Affine::IDENTITY);
				let (width, height) = resolved.texture_size;
				target.fill_rect(&Rect::new(0., 0., width as f64, height as f64));
			}
		}
	}

	// Close whatever a malformed scene left open
	if !pushed.is_empty() {
		log::error!("A recorded scene leaves {} layers or clips unended", pushed.len());
	}
	for kind in pushed.into_iter().rev() {
		match kind {
			Pushed::Layer => target.pop_layer(),
			Pushed::Clip => target.pop_clip(),
		}
	}
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

/// The Vello GPU paint for a recorded brush, plus the transform from the paint's space into the brush's (which differ for an image whose texture was downscaled).
fn resolve_paint(brush: &Brush, resolve_image: &mut dyn FnMut(&SceneImage) -> Option<ResolvedImage>) -> Option<(PaintType, Affine)> {
	match brush {
		Brush::Solid(color) => Some((PaintType::Solid(*color), Affine::IDENTITY)),
		Brush::Gradient(gradient) => Some((PaintType::Gradient(gradient.clone()), Affine::IDENTITY)),
		Brush::Image(image_brush) => {
			let resolved = resolve_image(&SceneImage::Pixels(image_brush.clone()))?;
			let paint = PaintType::Image(Image {
				image: resolved.source,
				sampler: image_brush.sampler,
			});
			Some((paint, resolved.texture_to_image))
		}
	}
}

/// Identifies the pixels behind an image texture: a blob of CPU pixels, or a GPU texture.
#[derive(Clone, PartialEq, Eq, Hash)]
enum ImageKey {
	Blob(u64),
	Texture(wgpu::Texture),
}

/// A premultiplied copy of an image's pixels that Vello GPU samples as an external texture.
struct ImageTexture {
	texture_id: TextureId,
	view: wgpu::TextureView,
	/// The copy's size, smaller than the image's own when the image exceeds the device's texture size limit.
	size: (u16, u16),
	/// Maps the copy's pixels onto the image's own pixels.
	texture_to_image: Affine,
	last_used: u64,
}

impl ImageTexture {
	fn paint_source(&self) -> ResolvedImage {
		let (width, height) = self.size;

		ResolvedImage {
			source: ImageSource::external_texture(self.texture_id, RectU16::new(0, 0, width, height), true),
			texture_to_image: self.texture_to_image,
			texture_size: self.size,
		}
	}
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
	fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, alpha_conversion: &AlphaConversion, image: &SceneImage) -> Option<&ImageTexture> {
		let key = match image {
			SceneImage::Pixels(brush) => ImageKey::Blob(brush.image.data.id()),
			SceneImage::Texture { texture, .. } => ImageKey::Texture((**texture).clone()),
		};

		if !self.textures.contains_key(&key) {
			let (texture, texture_to_image) = match image {
				SceneImage::Pixels(brush) => upload_pixels(device, queue, &brush.image)?,
				SceneImage::Texture { texture, .. } => (premultiply_texture(device, encoder, alpha_conversion, texture), Affine::IDENTITY),
			};
			let size = (u16::try_from(texture.width()).ok()?, u16::try_from(texture.height()).ok()?);

			self.textures.insert(
				key.clone(),
				ImageTexture {
					texture_id: TextureId(self.next_texture_id),
					view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
					size,
					texture_to_image,
					last_used: self.render_count,
				},
			);
			self.next_texture_id += 1;
		}

		let image_texture = self.textures.get_mut(&key)?;
		image_texture.last_used = self.render_count;
		Some(image_texture)
	}

	fn free_unused(&mut self) {
		let render_count = self.render_count;
		self.textures.retain(|_, image_texture| render_count - image_texture.last_used < UNUSED_IMAGE_RENDER_LIMIT);
	}
}

/// Uploads CPU pixels as a premultiplied texture, downscaling them by a whole factor if they exceed the device's texture size limit.
fn upload_pixels(device: &wgpu::Device, queue: &wgpu::Queue, image: &peniko::ImageData) -> Option<(wgpu::Texture, Affine)> {
	let (width, height) = (image.width, image.height);
	if width == 0 || height == 0 {
		return None;
	}
	if image.format != peniko::ImageFormat::Rgba8 {
		log::error!("Unsupported image format {:?}", image.format);
		return None;
	}

	let premultiplied: Vec<u8> = match image.alpha_type {
		peniko::ImageAlphaType::Alpha => image
			.data
			.data()
			.as_chunks::<4>()
			.0
			.iter()
			.flat_map(|pixel| {
				let alpha = pixel[3] as u32;
				let premultiply = |channel: u8| ((channel as u32 * alpha + 127) / 255) as u8;
				[premultiply(pixel[0]), premultiply(pixel[1]), premultiply(pixel[2]), pixel[3]]
			})
			.collect(),
		peniko::ImageAlphaType::AlphaPremultiplied => image.data.data().to_vec(),
	};

	// Box-average whole blocks of pixels down until both sides fit, which is exact to average since the pixels are premultiplied
	let limit = device.limits().max_texture_dimension_2d.min(u16::MAX as u32);
	let factor = width.max(height).div_ceil(limit).max(1);
	let (texture_width, texture_height) = (width.div_ceil(factor), height.div_ceil(factor));
	let pixels = if factor == 1 { premultiplied } else { box_downscale(&premultiplied, width, height, factor) };

	let texture = create_texture(
		device,
		"vello_gpu_image",
		UVec2::new(texture_width, texture_height),
		wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
	);
	queue.write_texture(
		wgpu::TexelCopyTextureInfo {
			texture: &texture,
			mip_level: 0,
			origin: wgpu::Origin3d::ZERO,
			aspect: wgpu::TextureAspect::All,
		},
		&pixels,
		wgpu::TexelCopyBufferLayout {
			offset: 0,
			bytes_per_row: Some(4 * texture_width),
			rows_per_image: Some(texture_height),
		},
		wgpu::Extent3d {
			width: texture_width,
			height: texture_height,
			depth_or_array_layers: 1,
		},
	);

	let texture_to_image = Affine::scale_non_uniform(width as f64 / texture_width as f64, height as f64 / texture_height as f64);
	Some((texture, texture_to_image))
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
