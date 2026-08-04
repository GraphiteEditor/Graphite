use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub(super) struct Resampler {
	device: wgpu::Device,
	pipeline: Arc<OnceLock<Pipeline>>,
}

struct Pipeline {
	format: wgpu::TextureFormat,
	sampler: wgpu::Sampler,
	layout: wgpu::BindGroupLayout,
	pipeline: wgpu::RenderPipeline,
}

impl Resampler {
	pub(super) fn new(device: wgpu::Device) -> Self {
		Self {
			device,
			pipeline: Arc::new(OnceLock::new()),
		}
	}

	pub(super) fn encode(&self, encoder: &mut wgpu::CommandEncoder, source: &wgpu::Texture, content_origin: wgpu::Origin3d, content_size: wgpu::Extent3d, target: &wgpu::Texture) {
		let pipeline = self.pipeline.get_or_init(|| Pipeline::new(&self.device, target.format()));
		debug_assert_eq!(pipeline.format, target.format());

		let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("CEF Resample Bind Group"),
			layout: &pipeline.layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&source.create_view(&Default::default())),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
				},
			],
		});
		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("CEF Resample Pass"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &target.create_view(&Default::default()),
				depth_slice: None,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
					store: wgpu::StoreOp::Store,
				},
			})],
			depth_stencil_attachment: None,
			timestamp_writes: None,
			occlusion_query_set: None,
			multiview_mask: None,
		});
		pass.set_pipeline(&pipeline.pipeline);
		pass.set_immediates(
			0,
			bytemuck::bytes_of(&Immediates {
				content_origin: [content_origin.x as f32, content_origin.y as f32],
				content_size: [content_size.width as f32, content_size.height as f32],
			}),
		);
		pass.set_bind_group(0, &bind_group, &[]);
		pass.draw(0..3, 0..1);
	}
}

impl Pipeline {
	fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("CEF Resample Sampler"),
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			..Default::default()
		});
		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("CEF Resample Bind Group Layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
			],
		});
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("CEF Resample Pipeline Layout"),
			bind_group_layouts: &[Some(&layout)],
			immediate_size: std::mem::size_of::<Immediates>() as u32,
		});
		let shader = device.create_shader_module(wgpu::include_wgsl!("resample.wgsl"));
		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("CEF Resample Pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				compilation_options: wgpu::PipelineCompilationOptions::default(),
				buffers: &[],
			},
			primitive: wgpu::PrimitiveState::default(),
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
			multiview_mask: None,
			cache: None,
		});
		Self { format, sampler, layout, pipeline }
	}
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Immediates {
	content_origin: [f32; 2],
	content_size: [f32; 2],
}
