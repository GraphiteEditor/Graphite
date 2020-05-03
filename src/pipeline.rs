pub struct Pipeline {
	pub bind_group_layout: wgpu::BindGroupLayout,
	pub render_pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
	pub fn new(device: &wgpu::Device, vertex_shader: &wgpu::ShaderModule, fragment_shader: &wgpu::ShaderModule) -> Self {
		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			bindings: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStage::FRAGMENT,
					ty: wgpu::BindingType::SampledTexture {
						dimension: wgpu::TextureViewDimension::D2,
						component_type: wgpu::TextureComponentType::Float,
						multisampled: false,
					},
				},
				// wgpu::BindGroupLayoutEntry {
				// 	binding: 1,
				// 	visibility: wgpu::ShaderStage::FRAGMENT,
				// 	ty: wgpu::BindingType::Sampler,
				// },
			],
			label: None,
		});

		let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			bind_group_layouts: &[&bind_group_layout],
		});
		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			layout: &render_pipeline_layout,
			vertex_stage: wgpu::ProgrammableStageDescriptor {
				module: vertex_shader,
				entry_point: "main",
			},
			fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
				module: fragment_shader,
				entry_point: "main",
			}),
			rasterization_state: Some(wgpu::RasterizationStateDescriptor {
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: wgpu::CullMode::Back,
				depth_bias: 0,
				depth_bias_slope_scale: 0.0,
				depth_bias_clamp: 0.0,
			}),
			primitive_topology: wgpu::PrimitiveTopology::TriangleList,
			color_states: &[
				wgpu::ColorStateDescriptor {
					format: wgpu::TextureFormat::Bgra8UnormSrgb, // TODO: Make this match Application.swap_chain_descriptor
					color_blend: wgpu::BlendDescriptor::REPLACE,
					alpha_blend: wgpu::BlendDescriptor::REPLACE,
					write_mask: wgpu::ColorWrite::ALL,
				},
			],
			depth_stencil_state: None,
			vertex_state: wgpu::VertexStateDescriptor {
				index_format: wgpu::IndexFormat::Uint16,
				vertex_buffers: &[wgpu::VertexBufferDescriptor {
					stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
					step_mode: wgpu::InputStepMode::Vertex,
					attributes: &[wgpu::VertexAttributeDescriptor {
							offset: 0,
							shader_location: 0,
							format: wgpu::VertexFormat::Float2,
						},
					],
				}],
			},
			sample_count: 1,
			sample_mask: !0,
			alpha_to_coverage_enabled: false,
		});

		Self {
			bind_group_layout,
			render_pipeline,
		}
	}
}
