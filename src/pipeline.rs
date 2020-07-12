use std::mem;
use crate::resource_cache::ResourceCache;
use crate::shader_stage;

pub struct Pipeline {
	pub bind_group_layout: wgpu::BindGroupLayout,
	pub render_pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
	pub fn new(device: &wgpu::Device, swap_chain_color_format: wgpu::TextureFormat, extra_layouts: Vec<&wgpu::BindGroupLayout>, shader_cache: &mut ResourceCache<wgpu::ShaderModule>, shader_pair_path: (&str, &str)) -> Self {
		// Load the vertex and fragment shaders
		let shader_pair = Pipeline::get_shader_pair(device, shader_cache, shader_pair_path);

		// Prepare a bind group layout for the GUI element's texture and form factor data
		let bind_group_layout = Pipeline::build_bind_group_layout(device, &vec![
			wgpu::BindingType::UniformBuffer { dynamic: false },
			wgpu::BindingType::SampledTexture {
				dimension: wgpu::TextureViewDimension::D2,
				component_type: wgpu::TextureComponentType::Float,
				multisampled: false,
			},
			wgpu::BindingType::Sampler { comparison: false },
		]);
		
		// Combine all bind group layouts
		let mut bind_group_layouts = vec![&bind_group_layout];
		bind_group_layouts.append(&mut extra_layouts.clone());
		
		// Construct the pipeline
		let render_pipeline = Pipeline::build_pipeline(device, swap_chain_color_format, bind_group_layouts, shader_pair);
		Self {
			bind_group_layout,
			render_pipeline,
		}
	}

	pub fn get_shader_pair<'a>(device: &wgpu::Device, shader_cache: &'a mut ResourceCache<wgpu::ShaderModule>, shader_pair_path: (&str, &str)) -> (&'a wgpu::ShaderModule, &'a wgpu::ShaderModule) {
		// If uncached, construct a vertex shader loaded from its source code file
		if shader_cache.get(shader_pair_path.0).is_none() {
			let vertex_shader_module = shader_stage::compile_from_glsl(device, shader_pair_path.0, glsl_to_spirv::ShaderType::Vertex).unwrap();
			shader_cache.set(shader_pair_path.0, vertex_shader_module);
		}

		// If uncached, construct a fragment shader loaded from its source code file
		if shader_cache.get(shader_pair_path.1).is_none() {
			let fragment_shader_module = shader_stage::compile_from_glsl(&device, shader_pair_path.1, glsl_to_spirv::ShaderType::Fragment).unwrap();
			shader_cache.set(shader_pair_path.1, fragment_shader_module);
		}

		// Get the shader pair
		let vertex_shader = shader_cache.get(shader_pair_path.0).unwrap();
		let fragment_shader = shader_cache.get(shader_pair_path.1).unwrap();

		(vertex_shader, fragment_shader)
	}

	pub fn build_bind_group_layouts(device: &wgpu::Device, bind_group_layouts: &Vec<Vec<wgpu::BindingType>>) -> Vec<wgpu::BindGroupLayout> {
		bind_group_layouts.into_iter().map(|layout_entry| Self::build_bind_group_layout(device, layout_entry)).collect::<Vec<_>>()
	}

	pub fn build_bind_group_layout(device: &wgpu::Device, bind_group_layout: &Vec<wgpu::BindingType>) -> wgpu::BindGroupLayout {
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: None,
			bindings: bind_group_layout.into_iter().enumerate().map(|(index, binding_type)|
				wgpu::BindGroupLayoutEntry {
					binding: index as u32,
					visibility: wgpu::ShaderStage::all(),
					ty: binding_type.clone(),
				}
			).collect::<Vec<_>>().as_slice(),
		})
	}

	pub fn build_binding_staging_buffer<T: bytemuck::Pod>(device: &wgpu::Device, resource: &T) -> wgpu::Buffer {
		// Construct a staging buffer with the binary uniform struct data
		device.create_buffer_with_data(
			bytemuck::cast_slice(&[*resource]),
			wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
		)
	}

	pub fn build_binding_resource(resource_buffer: &wgpu::Buffer) -> wgpu::BindingResource {
		// Return the buffer as a binding resource
		wgpu::BindingResource::Buffer {
			buffer: resource_buffer,
			range: 0..std::mem::size_of_val(resource_buffer) as wgpu::BufferAddress,
		}
	}

	pub fn build_bind_group(device: &wgpu::Device, bind_group_layout: &wgpu::BindGroupLayout, binding_resources: Vec<wgpu::BindingResource>) -> wgpu::BindGroup {
		let bindings = binding_resources.into_iter().enumerate().map(|(index, binding_resource)|
			wgpu::Binding {
				binding: index as u32,
				resource: binding_resource,
			}
		).collect::<Vec<_>>();

		device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: bind_group_layout,
			bindings: bindings.as_slice(),
			label: None,
		})
	}

	pub fn build_pipeline(device: &wgpu::Device, swap_chain_color_format: wgpu::TextureFormat, bind_group_layouts: Vec<&wgpu::BindGroupLayout>, shader_pair: (&wgpu::ShaderModule, &wgpu::ShaderModule)) -> wgpu::RenderPipeline {
		let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			bind_group_layouts: bind_group_layouts.as_slice(),
		});
		
		let (vertex_shader, fragment_shader) = shader_pair;
		device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
				format: swap_chain_color_format,
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
		})
	}
}
