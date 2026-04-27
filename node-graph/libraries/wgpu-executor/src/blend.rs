use crate::WgpuContext;
use glam::UVec2;

pub struct Blender {
	pipeline: wgpu::RenderPipeline,
	bind_group_layout: wgpu::BindGroupLayout,
	sampler: wgpu::Sampler,
}

impl Blender {
	pub fn new(device: &wgpu::Device) -> Self {
		let shader = device.create_shader_module(wgpu::include_wgsl!("blend_shader.wgsl"));

		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("blend_bind_group_layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 2,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
			],
		});

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("blend_pipeline_layout"),
			bind_group_layouts: &[&bind_group_layout],
			push_constant_ranges: &[],
		});

		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("blend_pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				buffers: &[],
				compilation_options: wgpu::PipelineCompilationOptions::default(),
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs_main"),
				targets: &[Some(wgpu::ColorTargetState {
					format: wgpu::TextureFormat::Rgba8Unorm,
					blend: None,
					write_mask: wgpu::ColorWrites::ALL,
				})],
				compilation_options: wgpu::PipelineCompilationOptions::default(),
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				..Default::default()
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview: None,
			cache: None,
		});

		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Nearest,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		Self { pipeline, bind_group_layout, sampler }
	}

	pub fn blend(&self, context: &WgpuContext, foreground: &wgpu::Texture, background: &wgpu::Texture) -> wgpu::Texture {
		let device = &context.device;
		let queue = &context.queue;
		let size = UVec2::new(foreground.width(), foreground.height()).max(UVec2::ONE);

		let output_texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("blend_output"),
			size: wgpu::Extent3d {
				width: size.x,
				height: size.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});

		let foreground_view = foreground.create_view(&wgpu::TextureViewDescriptor::default());
		let background_view = background.create_view(&wgpu::TextureViewDescriptor::default());
		let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("blend_bind_group"),
			layout: &self.bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&foreground_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(&background_view),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
			],
		});

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("blend_encoder") });

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("blend_pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &output_view,
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
			});

			render_pass.set_pipeline(&self.pipeline);
			render_pass.set_bind_group(0, &bind_group, &[]);
			render_pass.draw(0..3, 0..1);
		}

		queue.submit([encoder.finish()]);

		output_texture
	}
}
