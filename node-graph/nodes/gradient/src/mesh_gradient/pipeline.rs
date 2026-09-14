use std::sync::Arc;

use brush_types::BrushCache;
use core_types::{
	bounds::{BoundingBox, RenderBoundingBox},
	math::bbox::AxisAlignedBbox,
	transform::Footprint,
};
use glam::{DAffine2, DVec2, UVec2};
use raster_types::Texture;
use vector_types::{
	GradientInterpolation, GradientSpace, MeshGradient,
	mesh_gradient::{MeshGradientCache, MeshGradientEvaluator},
};
use wgpu_executor::{AsyncWgpuPipeline, WgpuExecutor};

use crate::mesh_gradient::{
	MeshGradientTessellator,
	tessellate::{MeshVertex, Metadata},
};

const MAX_RESOLUTION: u32 = 8192;

pub struct MeshGradientPipeline {
	renderer: Renderer,
}

pub struct MeshGradientPipelineArgs<'a> {
	pub footprint: Footprint,
	pub mesh_to_target: DAffine2,
	pub mesh_gradient: &'a MeshGradient,
	pub interpolation_space: GradientSpace,
	pub interpolation_method: GradientInterpolation,
	pub cache: &'a BrushCache,
	pub mesh_gradient_cache: &'a MeshGradientCache,
	pub debug: bool,
}

impl AsyncWgpuPipeline for MeshGradientPipeline {
	type Args<'a> = MeshGradientPipelineArgs<'a>;
	type Out = Option<(Texture, DAffine2)>;

	fn create(executor: &wgpu_executor::WgpuExecutor) -> Self {
		let device = &executor.context().device;
		Self { renderer: Renderer::new(device) }
	}

	async fn run<'a>(&'a self, executor: &'a wgpu_executor::WgpuExecutor, args: &'a Self::Args<'_>) -> Self::Out {
		// let state = args.cache.take(&args.footprint).unwrap_or_default();

		// FIXME: cache, evaluator does not depend on the footprint, but any other changes on the mesh gradient itself.
		// the one in the MeshGradient struct is still necessary for tools (afaik)
		let evaluator = args.mesh_gradient.evaluator(args.interpolation_space, args.interpolation_method).ok()?;

		let (mesh_to_output, mesh_to_texture, texture_transform, texture_size) = transforms(&args.mesh_gradient, args.footprint, args.mesh_to_target)?;

		let tessellator = MeshGradientTessellator::new(&evaluator, mesh_to_texture, mesh_to_output);

		// FIXME: cache, color related changes should not affect to the result of tessellation
		// possibly the quadtree can be cached too and continue the tessellation from there, but not sure about zoom-out. possibly we can simply use the cached one for it
		let (vertices, indices) = match tessellator.tessellate() {
			Ok(result) => result,
			Err(error) => {
				log::error!("Failed to tessellate mesh gradient: {error:?}");
				return None;
			}
		};

		if vertices.is_empty() {
			return None;
		};

		let color_data = pack_color_data(&evaluator, args.interpolation_method);

		let metadata = Metadata {
			patch_count: evaluator.patches().count() as u32,
			interpolation_space: try_interpolation_space_to_u32(args.interpolation_space)?,
			interpolation_method: interpolation_method_to_u32(args.interpolation_method),
		};

		// FIXME: cache
		let rendered_texture = self
			.renderer
			.render(executor, texture_size, vertices.as_slice(), indices.as_slice(), &color_data, metadata, args.debug)?;

		// args.cache.store(&args.footprint, rendered.state);

		Some((rendered_texture, texture_transform))
	}
}

fn transforms(mesh_gradient: &MeshGradient, footprint: Footprint, mesh_to_target: DAffine2) -> Option<(DAffine2, DAffine2, DAffine2, UVec2)> {
	let mesh_to_output = footprint.transform * mesh_to_target;
	let mesh_bbox = mesh_gradient.bounding_box(mesh_to_output, false);
	let (texture_to_output, texture_size) = calc_texture_to_output(mesh_bbox, footprint)?;
	if !texture_to_output.matrix2.determinant().recip().is_finite() {
		return None;
	}
	let mesh_to_texture = texture_to_output.inverse() * mesh_to_output;
	// Need to offset the paint target's transform to prevent duplicated application
	let texture_transform = footprint.transform.inverse() * texture_to_output;
	Some((mesh_to_output, mesh_to_texture, texture_transform, texture_size))
}

struct Renderer {
	render_pipeline: wgpu::RenderPipeline,
	debug_outline_pipeline: wgpu::RenderPipeline, // FIXME: only for debug
	color_data_layout: wgpu::BindGroupLayout,
}

impl Renderer {
	fn new(device: &wgpu::Device) -> Self {
		let shader = device.create_shader_module(wgpu::include_wgsl!("render.wgsl"));

		let data_buffer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("mesh_gradient_data_buffer_layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Storage { read_only: true },
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
			bind_group_layouts: &[Some(&data_buffer_layout)],
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
			color_data_layout: data_buffer_layout,
		}
	}

	fn render(&self, executor: &WgpuExecutor, texture_size: UVec2, vertices: &[MeshVertex], indices: &[u32], color_data: &[[f32; 4]], metadata: Metadata, debug: bool) -> Option<Texture> {
		let mut encoder = executor.context().device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("mesh_gradient_renderer_encoder"),
		});

		if texture_size.x == 0 || texture_size.y == 0 {
			return None;
		}

		let texture = executor.request_texture(texture_size);
		let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		let vertex_buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("mesh_gradient_vertex_buffer"),
			contents: bytemuck::cast_slice(vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});

		let index_buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("mesh_gradient_index_buffer"),
			contents: bytemuck::cast_slice(indices),
			usage: wgpu::BufferUsages::INDEX,
		});

		let color_data_buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("mesh_gradient_color_data_buffer"),
			contents: bytemuck::cast_slice(color_data),
			usage: wgpu::BufferUsages::STORAGE,
		});

		let metadata_buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("mesh_gradient_metadata_buffer"),
			contents: bytemuck::cast_slice(&[metadata]),
			usage: wgpu::BufferUsages::UNIFORM,
		});

		let depth_texture = executor.context().device.create_texture(&wgpu::TextureDescriptor {
			label: Some("mesh_gradient_depth_texture"),
			size: wgpu::Extent3d {
				width: texture_size.x,
				height: texture_size.y,
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

		let color_data_bind_group = executor.context().device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("mesh_gradient_color_data_bind_group"),
			layout: &self.color_data_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: color_data_buffer.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: metadata_buffer.as_entire_binding(),
				},
			],
		});

		// FIXME: debug
		let line_indices = indices
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
						load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
			render_pass.set_bind_group(0, &color_data_bind_group, &[]);
			render_pass.set_vertex_buffer(0, (*vertex_buffer).slice(..));
			render_pass.set_index_buffer((*index_buffer).slice(..), wgpu::IndexFormat::Uint32);
			render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);

			// FIXME: only for debug
			if debug {
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

fn calc_texture_to_output(mesh_bbox: RenderBoundingBox, footprint: Footprint) -> Option<(DAffine2, UVec2)> {
	let mesh_aabb = match mesh_bbox {
		RenderBoundingBox::Rectangle([start, end]) => AxisAlignedBbox::from((start, end)),
		RenderBoundingBox::None | RenderBoundingBox::Infinite => return None,
	};
	let output_aabb = AxisAlignedBbox::from((DVec2::ZERO, footprint.resolution.as_dvec2()));
	let mut crop_aabb = mesh_aabb.intersect(&output_aabb);

	// Cast the texture size to integer by expanding the size
	crop_aabb.start = crop_aabb.start.floor();
	crop_aabb.end = crop_aabb.end.ceil();

	let crop_size = crop_aabb.size();
	if crop_size.x <= 0. || crop_size.y <= 0. {
		return None;
	}
	let texture_to_output = DAffine2::from_scale_angle_translation(crop_size, 0., crop_aabb.start);
	let texture_scale = ((MAX_RESOLUTION as f64) / crop_size.x.max(crop_size.y)).min(1.);
	let texture_size = (crop_size * texture_scale).ceil().max(DVec2::ONE).min(DVec2::splat(MAX_RESOLUTION as f64)).as_uvec2();

	Some((texture_to_output, texture_size))
}

pub(crate) fn try_interpolation_space_to_u32(space: GradientSpace) -> Option<u32> {
	match space {
		GradientSpace::RgbGamma => Some(0),
		GradientSpace::RgbLinear => Some(1),
		GradientSpace::OkLab => Some(2),
		GradientSpace::Lab => Some(3),
		_ => None,
	}
}

pub(crate) fn interpolation_method_to_u32(method: GradientInterpolation) -> u32 {
	match method {
		GradientInterpolation::Stepped => 0,
		GradientInterpolation::Linear => 1,
		GradientInterpolation::Smooth => 2,
	}
}

pub(crate) fn pack_color_data(evaluator: &MeshGradientEvaluator, interpolation_method: GradientInterpolation) -> Vec<[f32; 4]> {
	match interpolation_method {
		GradientInterpolation::Stepped => evaluator.patches().map(|patch| patch.colors()[0].to_array()).collect::<Vec<_>>(),
		GradientInterpolation::Linear => evaluator.patches().flat_map(|patch| patch.colors().map(|color| color.to_array())).collect::<Vec<_>>(),
		GradientInterpolation::Smooth => evaluator.patches().flat_map(|patch| patch.color_bezier_net().to_array()).collect::<Vec<_>>(),
	}
}
