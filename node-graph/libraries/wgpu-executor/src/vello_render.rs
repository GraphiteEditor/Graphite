use glam::UVec2;
use rendering::vello_scene_builder::{SceneImage, VelloSceneBuilder, image_downscale_factor, image_texture_size};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use vello_gpu::kurbo::Affine;
use vello_gpu::peniko;
use vello_gpu::{ClearSettings, RenderSize, RenderTargetConfig, Renderer, Resources, TargetInit, TextureBindings, TextureId};

/// The format of the textures Vello GPU renders into, of the image textures it samples, and of the textures handed back.
const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How many renders a GPU texture's premultiplied copy survives without being drawn before it is freed.
const UNUSED_IMAGE_RENDER_LIMIT: u64 = 60;

/// Vello GPU's renderer and GPU resources, shared by every [`VelloCanvas`] so their memory and pipelines exist once.
pub(crate) struct VelloRenderer {
	/// Locked only while a scene is rendered and submitted, so one canvas drawing its scene on the CPU doesn't hold up another's render.
	gpu: Mutex<(Renderer, Resources)>,
	alpha_conversion: AlphaConversion,
}

/// A stream of frames rendered through the shared Vello renderer, such as the document or the overlays, keeping what its frames reuse.
#[derive(Default)]
pub struct VelloCanvas {
	/// The Vello GPU scene of the last render, handed out again to keep its allocations.
	spare_scene: Option<vello_gpu::Scene>,
	/// The premultiplied texture Vello GPU renders into before conversion to straight alpha, kept while the render size stays the same.
	premultiplied_target: Option<wgpu::Texture>,
	images: ImageTextures,
}

impl VelloCanvas {
	/// An empty scene of `size` to draw into, with its content placed by `view`.
	pub(crate) fn scene(&mut self, device: &wgpu::Device, size: UVec2, view: Affine) -> anyhow::Result<VelloSceneBuilder> {
		let (Ok(width), Ok(height)) = (u16::try_from(size.x), u16::try_from(size.y)) else {
			anyhow::bail!("Render size {size} exceeds Vello GPU's limit");
		};

		let mut scene = self.spare_scene.take().unwrap_or_else(|| vello_gpu::Scene::new(width, height));
		scene.reset_and_resize(width, height);
		Ok(VelloSceneBuilder::from_vello(scene, view, device.limits().max_texture_dimension_2d))
	}
}

impl VelloRenderer {
	pub(crate) fn new(device: &wgpu::Device) -> Self {
		let render_target_config = RenderTargetConfig {
			format: TEXTURE_FORMAT,
			width: 1,
			height: 1,
		};

		Self {
			gpu: Mutex::new(Renderer::new(device, &render_target_config)),
			alpha_conversion: AlphaConversion::new(device),
		}
	}

	/// Renders `scene` on `canvas` into `target` (an `Rgba8Unorm` texture of the scene's size with straight alpha).
	pub(crate) fn render(&self, canvas: &mut VelloCanvas, device: &wgpu::Device, queue: &wgpu::Queue, scene: VelloSceneBuilder, target: &wgpu::Texture) {
		let (width, height) = (scene.width(), scene.height());
		let max_texture_size = scene.max_texture_size();
		let (scene, images) = scene.finish();

		canvas.images.render_count += 1;
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_gpu_render") });

		// Supply the textures of the images the scene draws, under the IDs it drew them with
		let mut texture_bindings = TextureBindings::new();
		for (id, image) in images.iter().enumerate() {
			if let Some(view) = canvas.images.prepare(device, queue, &mut encoder, &self.alpha_conversion, image, max_texture_size) {
				texture_bindings.insert(TextureId(id as u64), view);
			}
		}

		// Render premultiplied, then convert to the straight alpha the rest of Graphite expects
		let size = UVec2::new(width as u32, height as u32);
		let premultiplied_target = match &canvas.premultiplied_target {
			Some(texture) if texture.width() == size.x && texture.height() == size.y => texture.clone(),
			_ => {
				let texture = create_texture(
					device,
					"vello_gpu_premultiplied_target",
					size,
					wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
				);
				canvas.premultiplied_target = Some(texture.clone());
				texture
			}
		};
		let premultiplied_view = premultiplied_target.create_view(&wgpu::TextureViewDescriptor::default());
		let target_init = TargetInit::Clear(ClearSettings::Viewport { color: peniko::Color::TRANSPARENT });

		// Vello GPU stages this render's uploads into resources every render shares, so another can't stage its own until this one is submitted
		let mut gpu = self.gpu.lock().unwrap_or_else(PoisonError::into_inner);
		let (renderer, resources) = &mut *gpu;
		let rendered = renderer.render(
			&scene,
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

		// A failed render leaves its area transparent rather than taking down the editor
		let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
		match rendered {
			Ok(()) => self.alpha_conversion.unpremultiply(device, &mut encoder, &premultiplied_view, &target_view),
			Err(error) => {
				log::error!("Vello GPU failed to render: {error}");
				clear_texture(&mut encoder, &target_view);
			}
		}

		queue.submit([encoder.finish()]);
		drop(gpu);
		canvas.images.free_unused();
		canvas.spare_scene = Some(scene);
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
	texture: wgpu::Texture,
	last_used: u64,
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
			SceneImage::Texture(texture) => ImageKey::Texture((**texture).clone()),
		};

		if !self.textures.contains_key(&key) {
			let texture = match image {
				SceneImage::Pixels(data) => upload_pixels(device, queue, data, max_texture_size)?,
				SceneImage::Texture(texture) => create_texture(
					device,
					"vello_gpu_premultiplied_image",
					UVec2::new(texture.width(), texture.height()),
					wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
				),
			};
			self.textures.insert(key.clone(), ImageTexture { texture, last_used: 0 });
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
				ImageKey::Texture(_) => unused_renders < UNUSED_IMAGE_RENDER_LIMIT,
			}
		});
	}
}

/// Uploads CPU pixels as a premultiplied texture, downscaling them by a whole factor if they exceed `max_texture_size`.
fn upload_pixels(device: &wgpu::Device, queue: &wgpu::Queue, image: &peniko::ImageData, max_texture_size: u32) -> Option<wgpu::Texture> {
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
	let factor = image_downscale_factor(width, height, max_texture_size);
	let (texture_width, texture_height) = image_texture_size(width, height, max_texture_size);
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

	Some(texture)
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
