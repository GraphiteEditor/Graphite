pub(super) struct Convert {
	pipeline: wgpu::RenderPipeline,
	layout: wgpu::BindGroupLayout,
}

impl Convert {
	pub(super) fn new(device: &wgpu::Device) -> Self {
		let shader = device.create_shader_module(wgpu::include_wgsl!("convert.wgsl"));
		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("airbrush_convert_bind_group_layout"),
			entries: &[wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Texture {
					sample_type: wgpu::TextureSampleType::Float { filterable: false },
					view_dimension: wgpu::TextureViewDimension::D2,
					multisampled: false,
				},
				count: None,
			}],
		});
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("airbrush_convert_pipeline_layout"),
			bind_group_layouts: &[Some(&layout)],
			immediate_size: 0,
		});
		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("airbrush_convert_pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				compilation_options: Default::default(),
				buffers: &[],
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs_main"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				..Default::default()
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview_mask: None,
			cache: None,
		});
		Self { pipeline, layout }
	}

	pub(super) fn encode(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, source: &wgpu::TextureView, target: &wgpu::TextureView) {
		let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("airbrush_convert_bind_group"),
			layout: &self.layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::TextureView(source),
			}],
		});
		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("airbrush_convert_pass"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: target,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
					store: wgpu::StoreOp::Store,
				},
				depth_slice: None,
			})],
			..Default::default()
		});
		pass.set_pipeline(&self.pipeline);
		pass.set_bind_group(0, &bind, &[]);
		pass.draw(0..3, 0..1);
	}
}
