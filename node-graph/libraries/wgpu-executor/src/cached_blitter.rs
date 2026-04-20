use crate::VELLO_SURFACE_FORMAT;

/// A texture blitter that caches its bind group to avoid recreating it every frame.
///
/// The standard wgpu `TextureBlitter` creates a new bind group on every `copy()` call,
/// which causes excessive GPU resource allocation during viewport panning. This blitter
/// maintains a persistent intermediate texture (recreated only on size change) and a cached
/// bind group bound to it. Each frame, the source is copied into the persistent texture
/// via `copy_texture_to_texture` (same format, no bind groups), then the cached bind group
/// is used for the format-converting render pass.
pub struct CachedBlitter {
	pipeline: wgpu::RenderPipeline,
	bind_group_layout: wgpu::BindGroupLayout,
	sampler: wgpu::Sampler,
	cache: std::sync::Mutex<Option<BlitCache>>,
}

struct BlitCache {
	source_texture: wgpu::Texture,
	bind_group: wgpu::BindGroup,
	size: wgpu::Extent3d,
}

const BLIT_SHADER: &str = include_str!("blit.wgsl");

impl CachedBlitter {
	pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("CachedBlitter::sampler"),
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("CachedBlitter::bind_group_layout"),
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
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
					count: None,
				},
			],
		});

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("CachedBlitter::pipeline_layout"),
			bind_group_layouts: &[&bind_group_layout],
			push_constant_ranges: &[],
		});

		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("CachedBlitter::shader"),
			source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
		});

		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("CachedBlitter::pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				compilation_options: wgpu::PipelineCompilationOptions::default(),
				buffers: &[],
			},
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				..Default::default()
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs_main"),
				compilation_options: wgpu::PipelineCompilationOptions::default(),
				targets: &[Some(wgpu::ColorTargetState {
					format,
					blend: None,
					write_mask: wgpu::ColorWrites::ALL,
				})],
			}),
			multiview: None,
			cache: None,
		});

		Self {
			pipeline,
			bind_group_layout,
			sampler,
			cache: std::sync::Mutex::new(None),
		}
	}

	/// Copies the source texture to the target with format conversion, using a cached bind group.
	///
	/// Internally maintains a persistent intermediate texture. Each frame:
	/// 1. Copies `source` → intermediate via `copy_texture_to_texture` (same format, no bind groups)
	/// 2. Blits intermediate → `target` via a render pass with the cached bind group
	///
	/// The bind group and intermediate texture are only recreated when the source size changes.
	pub fn copy(
		&self,
		device: &wgpu::Device,
		encoder: &mut wgpu::CommandEncoder,
		source: &wgpu::Texture,
		target: &wgpu::TextureView,
	) {
		let size = source.size();

		// Take cache out of mutex to avoid holding the lock during GPU operations
		let mut cache = self.cache.lock().unwrap().take();

		// Recreate the persistent texture and bind group if size changed
		if !matches!(&cache, Some(c) if c.size == size) {
			let texture = device.create_texture(&wgpu::TextureDescriptor {
				label: Some("CachedBlitter::intermediate"),
				size,
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: VELLO_SURFACE_FORMAT,
				usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
				view_formats: &[],
			});
			let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
			let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
				label: Some("CachedBlitter::bind_group"),
				layout: &self.bind_group_layout,
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(&view),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Sampler(&self.sampler),
					},
				],
			});
			cache = Some(BlitCache { source_texture: texture, bind_group, size });
		}

		let c = cache.as_ref().unwrap();

		// Copy source → persistent intermediate texture (same format, no bind group creation)
		encoder.copy_texture_to_texture(
			wgpu::TexelCopyTextureInfoBase {
				texture: source,
				mip_level: 0,
				origin: Default::default(),
				aspect: Default::default(),
			},
			wgpu::TexelCopyTextureInfoBase {
				texture: &c.source_texture,
				mip_level: 0,
				origin: Default::default(),
				aspect: Default::default(),
			},
			size,
		);

		// Blit intermediate → target with format conversion using the cached bind group
		{
			let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("CachedBlitter::pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: target,
					depth_slice: None,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Load,
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: None,
				timestamp_writes: None,
				occlusion_query_set: None,
			});
			pass.set_pipeline(&self.pipeline);
			pass.set_bind_group(0, &c.bind_group, &[]);
			pass.draw(0..3, 0..1);
		}

		// Put cache back for next frame
		*self.cache.lock().unwrap() = cache;
	}
}
