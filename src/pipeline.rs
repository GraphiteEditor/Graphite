use std::mem;

pub struct Pipeline {
	pub bind_group_layout: wgpu::BindGroupLayout,
	pub render_pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
	pub fn new(device: &wgpu::Device, vertex_shader: &wgpu::ShaderModule, fragment_shader: &wgpu::ShaderModule, bind_group_layout_binding_types: Vec<wgpu::BindingType>) -> Self {
		let bind_group_layout_entries = bind_group_layout_binding_types.into_iter().enumerate().map(|(index, binding_type)|
			wgpu::BindGroupLayoutEntry {
				binding: index as u32,
				visibility: wgpu::ShaderStage::all(),
				ty: binding_type,
			}
		).collect::<Vec<_>>();
		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			bindings: bind_group_layout_entries.as_slice(),
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
				cull_mode: wgpu::CullMode::None,
				depth_bias: 0,
				depth_bias_slope_scale: 0.0,
				depth_bias_clamp: 0.0,
			}),
			primitive_topology: wgpu::PrimitiveTopology::TriangleList,
			color_states: &[wgpu::ColorStateDescriptor {
				format: wgpu::TextureFormat::Bgra8UnormSrgb, // TODO: Make this match Application.swap_chain_descriptor
				color_blend: wgpu::BlendDescriptor::REPLACE,
				alpha_blend: wgpu::BlendDescriptor::REPLACE,
				write_mask: wgpu::ColorWrite::ALL,
			}],
			depth_stencil_state: None,
			vertex_state: wgpu::VertexStateDescriptor {
				index_format: wgpu::IndexFormat::Uint16,
				vertex_buffers: &[wgpu::VertexBufferDescriptor {
					stride: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
					step_mode: wgpu::InputStepMode::Vertex,
					attributes: &[wgpu::VertexAttributeDescriptor {
						offset: 0,
						shader_location: 0,
						format: wgpu::VertexFormat::Float2,
					}],
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

	pub fn build_bind_group(&self, device: &wgpu::Device, binding_resources: Vec<wgpu::BindingResource>) -> wgpu::BindGroup {
		let bindings = binding_resources.into_iter().enumerate().map(|(index, binding_resource)|
			wgpu::Binding {
				binding: index as u32,
				resource: binding_resource,
			}
		).collect::<Vec<_>>();
		device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &self.bind_group_layout,
			bindings: bindings.as_slice(),
			label: None,
		})
	}
}
