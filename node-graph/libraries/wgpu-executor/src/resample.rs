use crate::WgpuContext;
use glam::{DAffine2, UVec2, Vec2};

pub struct Resampler {
	pipeline: wgpu::RenderPipeline,
	bind_group_layout: wgpu::BindGroupLayout,
}

impl Resampler {
	pub fn new(device: &wgpu::Device) -> Self {
		let shader = device.create_shader_module(wgpu::include_wgsl!("resample_shader.wgsl"));

		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("resample_bind_group_layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: false },
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
			label: Some("resample_pipeline_layout"),
			bind_group_layouts: &[&bind_group_layout],
			push_constant_ranges: &[],
		});

		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("resample_pipeline"),
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

		Resampler { pipeline, bind_group_layout }
	}

	pub fn resample(&self, context: &WgpuContext, source: &wgpu::Texture, target_size: UVec2, transform: &DAffine2) -> wgpu::Texture {
		let device = &context.device;
		let queue = &context.queue;

		let output_texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("resample_output"),
			size: wgpu::Extent3d {
				width: target_size.x.max(1),
				height: target_size.y.max(1),
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});

		let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
		let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

		let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("resample_params"),
			size: 32,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
			mapped_at_creation: false,
		});

		let params_data = [transform.matrix2.x_axis.as_vec2(), transform.matrix2.y_axis.as_vec2(), transform.translation.as_vec2(), Vec2::ZERO];
		queue.write_buffer(&params_buffer, 0, bytemuck::cast_slice(&params_data));

		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("resample_bind_group"),
			layout: &self.bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&source_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: params_buffer.as_entire_binding(),
				},
			],
		});

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("resample_encoder") });

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("resample_pass"),
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
