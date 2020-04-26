pub struct PipelineDetails<'a> {
	pub vertex_shader: &'a wgpu::ShaderModule,
	pub fragment_shader: &'a wgpu::ShaderModule,
	pub texture_view: Option<&'a wgpu::TextureView>,
}

pub struct Pipeline {
	pub render_pipeline: wgpu::RenderPipeline,
	pub vertex_buffer: wgpu::Buffer,
	pub index_buffer: wgpu::Buffer,
	pub index_count: u32,
	pub texture_bind_group: wgpu::BindGroup,
}

impl Pipeline {
	pub fn new(device: &wgpu::Device, pipeline_details: PipelineDetails) -> Self {
		let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			bindings: &[
				wgpu::BindGroupLayoutBinding {
					binding: 0,
					visibility: wgpu::ShaderStage::FRAGMENT,
					ty: wgpu::BindingType::SampledTexture {
						multisampled: false,
						dimension: wgpu::TextureViewDimension::D2,
					},
				},
				// wgpu::BindGroupLayoutBinding {
				// 	binding: 1,
				// 	visibility: wgpu::ShaderStage::FRAGMENT,
				// 	ty: wgpu::BindingType::Sampler,
				// },
			],
		});

		let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &texture_bind_group_layout,
			bindings: &[
				wgpu::Binding {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(pipeline_details.texture_view.unwrap()),
				},
				// wgpu::Binding {
				// 	binding: 1,
				// 	resource: wgpu::BindingResource::Sampler(&texture.sampler),
				// }
			],
		});

		let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			bind_group_layouts: &[&texture_bind_group_layout],
		});

		let vertex_buffer_descriptors = wgpu::VertexBufferDescriptor {
			stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
			step_mode: wgpu::InputStepMode::Vertex,
			attributes: &[
				wgpu::VertexAttributeDescriptor {
					offset: 0,
					shader_location: 0,
					format: wgpu::VertexFormat::Float2,
				},
			],
		};

		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			layout: &render_pipeline_layout,
			vertex_stage: wgpu::ProgrammableStageDescriptor {
				module: pipeline_details.vertex_shader,
				entry_point: "main",
			},
			fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
				module: pipeline_details.fragment_shader,
				entry_point: "main",
			}),
			rasterization_state: Some(wgpu::RasterizationStateDescriptor {
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: wgpu::CullMode::Back,
				depth_bias: 0,
				depth_bias_slope_scale: 0.0,
				depth_bias_clamp: 0.0,
			}),
			color_states: &[
				wgpu::ColorStateDescriptor {
					format: wgpu::TextureFormat::Bgra8UnormSrgb,
					color_blend: wgpu::BlendDescriptor::REPLACE,
					alpha_blend: wgpu::BlendDescriptor::REPLACE,
					write_mask: wgpu::ColorWrite::ALL,
				},
			],
			primitive_topology: wgpu::PrimitiveTopology::TriangleList,
			depth_stencil_state: None,
			index_format: wgpu::IndexFormat::Uint16,
			vertex_buffers: &[vertex_buffer_descriptors],
			sample_count: 1,
			sample_mask: !0,
			alpha_to_coverage_enabled: false,
		});

		let vertex_buffer = device.create_buffer_mapped(VERTICES.len(), wgpu::BufferUsage::VERTEX).fill_from_slice(VERTICES);
		let index_buffer = device.create_buffer_mapped(INDICES.len(), wgpu::BufferUsage::INDEX).fill_from_slice(INDICES);
		let index_count = INDICES.len() as u32;

		Self {
			render_pipeline,
			vertex_buffer,
			index_buffer,
			index_count,
			texture_bind_group,
		}
	}
}

const VERTICES: &[[f32; 2]] = &[
	[-0.0868241, -0.49240386],
	[-0.49513406, -0.06958647],
	[-0.21918549, 0.44939706],
	[0.35966998, 0.3473291],
	[0.44147372, -0.2347359],
];

const INDICES: &[u16] = &[
	0, 1, 4,
	1, 2, 4,
	2, 3, 4,
];