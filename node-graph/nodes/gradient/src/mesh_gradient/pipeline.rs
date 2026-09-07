use glam::UVec2;
use raster_types::Texture;
use wgpu_executor::{AsyncWgpuPipeline, WgpuExecutor};

use crate::mesh_gradient::tessellate::{InterpolationSetting, MeshVertex, PatchData};

pub struct MeshGradientPipeline {
	renderer: Renderer,
}

pub struct MeshGradientPipelineArgs<'a> {
	pub vertices: &'a [MeshVertex],
	pub indices: &'a [u32],
	pub output_size: UVec2,
	pub patches: &'a [PatchData],
	pub interpolation_setting: &'a InterpolationSetting,
	pub debug: bool,
}

impl AsyncWgpuPipeline for MeshGradientPipeline {
	type Args<'a> = MeshGradientPipelineArgs<'a>;
	type Out = Option<Texture>;

	fn create(executor: &wgpu_executor::WgpuExecutor) -> Self {
		let device = &executor.context().device;
		Self { renderer: Renderer::new(device) }
	}

	async fn run<'a>(&'a self, executor: &'a wgpu_executor::WgpuExecutor, args: &'a Self::Args<'_>) -> Self::Out {
		self.renderer.render(executor, args)
	}
}

struct Renderer {
	render_pipeline: wgpu::RenderPipeline,
	debug_outline_pipeline: wgpu::RenderPipeline, // FIXME: only for debug
	patch_data_layout: wgpu::BindGroupLayout,
}

impl Renderer {
	fn new(device: &wgpu::Device) -> Self {
		let shader = device.create_shader_module(wgpu::include_wgsl!("render.wgsl"));

		let patch_data_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("mesh_gradient_patch_data_layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Storage { read_only: true },
						has_dynamic_offset: false,
						min_binding_size: None,
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
			label: Some("mesh_gradient_renderer_pipeline_layout"),
			bind_group_layouts: &[Some(&patch_data_layout)],
			immediate_size: 0,
		});

		let vertex_buffer_layout = wgpu::VertexBufferLayout {
			array_stride: std::mem::size_of::<MeshVertex>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &wgpu::vertex_attr_array![
				0 => Uint32,
				1 => Float32x2,
				2 => Float32x2
			],
		};

		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("mesh_gradient_renderer_pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				compilation_options: Default::default(),
				// FIXME: remove clone before commit
				buffers: &[vertex_buffer_layout.clone()],
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs_main"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::ColorTargetState {
					format: wgpu::TextureFormat::Rgba8Unorm,
					blend: None,
					write_mask: wgpu::ColorWrites::ALL,
				})],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				cull_mode: None,
				..Default::default()
			},
			depth_stencil: Some(wgpu::DepthStencilState {
				format: wgpu::TextureFormat::Depth32Float,
				depth_write_enabled: Some(true),
				depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
				stencil: Default::default(),
				bias: Default::default(),
			}),
			multisample: wgpu::MultisampleState::default(),
			multiview_mask: None,
			cache: None,
		});

		// FIXME: only for debug
		let debug_outline_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("mesh_gradient_renderer_pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				compilation_options: Default::default(),
				buffers: &[vertex_buffer_layout],
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs_debug_outline"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::ColorTargetState {
					format: wgpu::TextureFormat::Rgba8Unorm,
					blend: None,
					write_mask: wgpu::ColorWrites::ALL,
				})],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::LineList,
				cull_mode: None,
				..Default::default()
			},
			depth_stencil: Some(wgpu::DepthStencilState {
				format: wgpu::TextureFormat::Depth32Float,
				depth_write_enabled: Some(false),
				depth_compare: Some(wgpu::CompareFunction::Always),
				stencil: Default::default(),
				bias: Default::default(),
			}),
			multisample: wgpu::MultisampleState::default(),
			multiview_mask: None,
			cache: None,
		});

		Self {
			render_pipeline,
			debug_outline_pipeline,
			patch_data_layout,
		}
	}

	fn render(&self, executor: &WgpuExecutor, args: &MeshGradientPipelineArgs) -> Option<Texture> {
		let mut encoder = executor.context().device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("mesh_gradient_renderer_encoder"),
		});

		if args.output_size == UVec2::ZERO {
			return None;
		}

		let texture = executor.request_texture(args.output_size);
		let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		let vertex_buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("mesh_gradient_vertex_buffer"),
			contents: bytemuck::cast_slice(args.vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});

		let index_buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("mesh_gradient_index_buffer"),
			contents: bytemuck::cast_slice(args.indices),
			usage: wgpu::BufferUsages::INDEX,
		});

		let patch_data_buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("mesh_gradient_patch_data_buffer"),
			contents: bytemuck::cast_slice(args.patches),
			usage: wgpu::BufferUsages::STORAGE,
		});

		let interpolation_setting_buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("mesh_gradient_interpolation_setting_buffer"),
			contents: bytemuck::cast_slice(&[*args.interpolation_setting]),
			usage: wgpu::BufferUsages::UNIFORM,
		});

		let depth_texture = executor.context().device.create_texture(&wgpu::TextureDescriptor {
			label: Some("mesh_gradient_depth_texture"),
			size: wgpu::Extent3d {
				width: args.output_size.x,
				height: args.output_size.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Depth32Float,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[],
		});
		let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

		let patch_data_bind_group = executor.context().device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("mesh_gradient_patch_data_bind_group"),
			layout: &self.patch_data_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: patch_data_buffer.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: interpolation_setting_buffer.as_entire_binding(),
				},
			],
		});

		// FIXME: debug
		let line_indices = args
			.indices
			.chunks_exact(3)
			.flat_map(|triangle| {
				let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
				[a, b, b, c, c, a]
			})
			.collect::<Vec<u32>>();

		let line_index_buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("mesh_gradient_debug_line_index_buffer"),
			contents: bytemuck::cast_slice(&line_indices),
			usage: wgpu::BufferUsages::INDEX,
		});

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("mesh_gradient_render_pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &texture_view,
					depth_slice: None,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.01, g: 0.01, b: 0.01, a: 1. }),
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: &depth_view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(0.0),
						store: wgpu::StoreOp::Discard,
					}),
					stencil_ops: None,
				}),
				timestamp_writes: None,
				occlusion_query_set: None,
				multiview_mask: None,
			});

			render_pass.set_pipeline(&self.render_pipeline);
			render_pass.set_bind_group(0, &patch_data_bind_group, &[]);
			render_pass.set_vertex_buffer(0, (*vertex_buffer).slice(..));
			render_pass.set_index_buffer((*index_buffer).slice(..), wgpu::IndexFormat::Uint32);
			render_pass.draw_indexed(0..args.indices.len() as u32, 0, 0..1);

			// FIXME: only for debug
			if args.debug {
				render_pass.set_pipeline(&self.debug_outline_pipeline);
				render_pass.set_index_buffer((*line_index_buffer).slice(..), wgpu::IndexFormat::Uint32);
				render_pass.draw_indexed(0..line_indices.len() as u32, 0, 0..1);
			}
		}

		let command_buffer = encoder.finish();
		executor.context().queue.submit([command_buffer]);

		Some(texture)
	}
}
