use glam::{Affine2, Vec2};
use wgpu::util::DeviceExt;

pub struct BackgroundCompositor {
	checker_rect_pipeline: wgpu::RenderPipeline,
	checker_viewport_pipeline: wgpu::RenderPipeline,
	fullscreen_pipeline: wgpu::RenderPipeline,
	checker_bind_group_layout: wgpu::BindGroupLayout,
	fullscreen_bind_group_layout: wgpu::BindGroupLayout,
	sampler: wgpu::Sampler,
}

impl BackgroundCompositor {
	pub fn new(device: &wgpu::Device) -> Self {
		let format = wgpu::TextureFormat::Rgba8Unorm;
		let checker_rect_shader = device.create_shader_module(wgpu::include_wgsl!("checker_rect.wgsl"));
		let checker_viewport_shader = device.create_shader_module(wgpu::include_wgsl!("checker_viewport.wgsl"));
		let fullscreen_shader = device.create_shader_module(wgpu::include_wgsl!("fullscreen.wgsl"));

		let checker_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("background_checker_bind_group_layout"),
			entries: &[wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			}],
		});

		let checker_rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("background_checker_rect_pipeline_layout"),
			bind_group_layouts: &[&checker_bind_group_layout],
			immediate_size: 0,
		});

		let checker_viewport_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("background_checker_viewport_pipeline_layout"),
			bind_group_layouts: &[&checker_bind_group_layout],
			immediate_size: 0,
		});

		let fullscreen_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("background_fullscreen_bind_group_layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
			],
		});

		let fullscreen_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("background_fullscreen_pipeline_layout"),
			bind_group_layouts: &[&fullscreen_bind_group_layout],
			immediate_size: 0,
		});

		let checker_rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("background_checker_rect_pipeline"),
			layout: Some(&checker_rect_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &checker_rect_shader,
				entry_point: Some("vs_main"),
				compilation_options: Default::default(),
				buffers: &[],
			},
			fragment: Some(wgpu::FragmentState {
				module: &checker_rect_shader,
				entry_point: Some("fs_main"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::ColorTargetState {
					format,
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
		});

		let checker_viewport_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("background_checker_viewport_pipeline"),
			layout: Some(&checker_viewport_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &checker_viewport_shader,
				entry_point: Some("vs_main"),
				compilation_options: Default::default(),
				buffers: &[],
			},
			fragment: Some(wgpu::FragmentState {
				module: &checker_viewport_shader,
				entry_point: Some("fs_main"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::ColorTargetState {
					format,
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
		});

		let fullscreen_blend = wgpu::BlendState {
			color: wgpu::BlendComponent {
				src_factor: wgpu::BlendFactor::SrcAlpha,
				dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
				operation: wgpu::BlendOperation::Add,
			},
			alpha: wgpu::BlendComponent {
				src_factor: wgpu::BlendFactor::One,
				dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
				operation: wgpu::BlendOperation::Add,
			},
		};

		let fullscreen_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("background_fullscreen_pipeline"),
			layout: Some(&fullscreen_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &fullscreen_shader,
				entry_point: Some("vs_main"),
				compilation_options: Default::default(),
				buffers: &[],
			},
			fragment: Some(wgpu::FragmentState {
				module: &fullscreen_shader,
				entry_point: Some("fs_main"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::ColorTargetState {
					format,
					blend: Some(fullscreen_blend),
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
		});

		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("background_fullscreen_sampler"),
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			mipmap_filter: wgpu::MipmapFilterMode::Nearest,
			..Default::default()
		});

		Self {
			checker_rect_pipeline,
			checker_viewport_pipeline,
			fullscreen_pipeline,
			checker_bind_group_layout,
			fullscreen_bind_group_layout,
			sampler,
		}
	}

	pub fn composite(&self, context: &crate::WgpuContext, foreground: &wgpu::Texture, output: &wgpu::Texture, backgrounds: &[rendering::Background], document_to_screen: Affine2, zoom: f32) {
		if zoom <= 0.0 {
			return;
		}

		let device = &context.device;
		let queue = &context.queue;

		let checker_size_doc = 8.0 / zoom;
		let screen_to_document = document_to_screen.inverse();
		let viewport_size = output.size();
		let viewport_size = Vec2::new(viewport_size.width as f32, viewport_size.height as f32);

		let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
		let foreground_view = foreground.create_view(&wgpu::TextureViewDescriptor::default());

		let checker_draws = if backgrounds.is_empty() {
			vec![(
				3,
				self.create_checker_bind_group(device, CompositeUniforms::fullscreen(viewport_size, screen_to_document, checker_size_doc)),
			)]
		} else {
			backgrounds
				.iter()
				.filter_map(|background| {
					let a = background.location.as_vec2();
					let b = (background.location + background.dimensions).as_vec2();

					let min = a.min(b);
					let max = a.max(b);

					if max.x <= min.x || max.y <= min.y {
						return None;
					}

					let uniforms = CompositeUniforms::rect(min, max, document_to_screen, viewport_size, checker_size_doc);
					Some((6, self.create_checker_bind_group(device, uniforms)))
				})
				.collect()
		};

		let fullscreen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("background_fullscreen_bind_group"),
			layout: &self.fullscreen_bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(&foreground_view),
				},
			],
		});

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("background_encoder") });

		{
			let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("background_pass"),
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
				multiview_mask: None,
			});

			if backgrounds.is_empty() {
				pass.set_pipeline(&self.checker_viewport_pipeline);
				for (vertex_count, bind_group) in &checker_draws {
					pass.set_bind_group(0, bind_group, &[]);
					pass.draw(0..*vertex_count, 0..1);
				}
			} else {
				pass.set_pipeline(&self.checker_rect_pipeline);
				for (vertex_count, bind_group) in &checker_draws {
					pass.set_bind_group(0, bind_group, &[]);
					pass.draw(0..*vertex_count, 0..1);
				}
			}

			pass.set_pipeline(&self.fullscreen_pipeline);
			pass.set_bind_group(0, &fullscreen_bind_group, &[]);
			pass.draw(0..3, 0..1);
		}

		queue.submit(std::iter::once(encoder.finish()));
	}

	fn create_checker_bind_group(&self, device: &wgpu::Device, uniforms: CompositeUniforms) -> wgpu::BindGroup {
		let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("background_checker_uniforms"),
			contents: bytemuck::bytes_of(&uniforms),
			usage: wgpu::BufferUsages::UNIFORM,
		});

		device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("background_checker_bind_group"),
			layout: &self.checker_bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: buffer.as_entire_binding(),
			}],
		})
	}
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CompositeUniforms {
	transform_x: [f32; 2],
	transform_y: [f32; 2],
	transform_translation: [f32; 2],
	rect_min: [f32; 2],
	rect_max: [f32; 2],
	viewport_size: [f32; 2],
	pattern_origin: [f32; 2],
	checker_size: f32,
	_pad: f32,
}

impl CompositeUniforms {
	fn fullscreen(viewport_size: Vec2, screen_to_document: Affine2, checker_size_doc: f32) -> Self {
		Self::new(screen_to_document, Vec2::ZERO, Vec2::ZERO, viewport_size, Vec2::ZERO, checker_size_doc)
	}

	fn rect(rect_min: Vec2, rect_max: Vec2, document_to_screen: Affine2, viewport_size: Vec2, checker_size_doc: f32) -> Self {
		Self::new(document_to_screen, rect_min, rect_max, viewport_size, rect_min, checker_size_doc)
	}

	fn new(transform: Affine2, rect_min: Vec2, rect_max: Vec2, viewport_size: Vec2, pattern_origin: Vec2, checker_size: f32) -> Self {
		Self {
			transform_x: transform.matrix2.x_axis.to_array(),
			transform_y: transform.matrix2.y_axis.to_array(),
			transform_translation: transform.translation.to_array(),
			rect_min: rect_min.to_array(),
			rect_max: rect_max.to_array(),
			viewport_size: viewport_size.to_array(),
			pattern_origin: pattern_origin.to_array(),
			checker_size,
			_pad: 0.,
		}
	}
}
