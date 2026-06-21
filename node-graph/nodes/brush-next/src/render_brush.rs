use bytemuck::{Pod, Zeroable};
use core_types::list::{Item, List};
use core_types::{ATTR_TRANSFORM, Ctx, ProtoNodeIdentifier};
use glam::{DAffine2, DVec2, UVec2, Vec2};
use raster_types::{GPU, Raster};
use std::marker::PhantomData;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use wgpu_executor::{AsyncWgpuPipeline, WgpuExecutor, WgpuPipelineCache};

const WGPU_EXECUTOR_IDENTIFIER: ProtoNodeIdentifier = ProtoNodeIdentifier::new("graphene_std::platform_application_io::WgpuExecutorNode");

#[node_macro::node(category("Raster: Brush"))]
pub async fn render_brush_next<'a: 'n>(
	_ctx: impl Ctx,
	#[scope(brush_pipeline::IDENTIFIER)] pipeline: WgpuPipelineCache,
	start: DVec2,
	end: DVec2,
	#[default(8)] stamps: u32,
	#[default(16.)] radius: f64,
) -> List<Raster<GPU>> {
	let pad = DVec2::splat(radius);
	let bbox_min = start.min(end) - pad;
	let bbox_max = start.max(end) + pad;
	let bbox_size = (bbox_max - bbox_min).max(DVec2::ONE);
	let size = bbox_size.ceil().as_uvec2().max(UVec2::ONE);

	let args = BrushPipelineArgs {
		start: start - bbox_min,
		end: end - bbox_min,
		stamps,
		radius: radius as f32,
		size,
		_marker: PhantomData,
	};
	let texture = pipeline.run::<BrushPipeline>(&args).await;
	let raster = Raster::<GPU>::new_gpu(texture.as_ref().clone());
	let transform = DAffine2::from_translation(bbox_min) * DAffine2::from_scale(DVec2::new(size.x as f64, size.y as f64));
	List::new_from_item(Item::new_from_element(raster).with_attribute(ATTR_TRANSFORM, transform))
}

#[node_macro::node(category(""), inject_scope)]
async fn brush_pipeline<'a: 'n>(_ctx: impl Ctx, #[scope(WGPU_EXECUTOR_IDENTIFIER)] executor: &'a WgpuExecutor, #[data] pipeline: WgpuPipelineCache) -> WgpuPipelineCache {
	executor.pipeline_init::<BrushPipeline>(pipeline);
	pipeline.clone()
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Stamp {
	center_px: [f32; 2],
	radius_px: f32,
	_pad: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Globals {
	viewport_size: [f32; 2],
	_pad: [f32; 2],
}

pub struct BrushPipeline {
	pipeline: wgpu::RenderPipeline,
	bind_group_layout: wgpu::BindGroupLayout,
}

pub struct BrushPipelineArgs<'a> {
	pub start: DVec2,
	pub end: DVec2,
	pub stamps: u32,
	pub radius: f32,
	pub size: UVec2,
	pub _marker: PhantomData<&'a ()>,
}

impl AsyncWgpuPipeline for BrushPipeline {
	type Args<'a> = BrushPipelineArgs<'a>;
	type Out = Arc<wgpu::Texture>;

	fn create(executor: &WgpuExecutor) -> Self {
		let device = &executor.context().device;
		let shader = device.create_shader_module(wgpu::include_wgsl!("render_brush.wgsl"));

		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("brush_bind_group_layout"),
			entries: &[wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::VERTEX,
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			}],
		});

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("brush_pipeline_layout"),
			bind_group_layouts: &[Some(&bind_group_layout)],
			immediate_size: 0,
		});

		let instance_layout = wgpu::VertexBufferLayout {
			array_stride: std::mem::size_of::<Stamp>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &[
				wgpu::VertexAttribute {
					offset: 0,
					shader_location: 0,
					format: wgpu::VertexFormat::Float32x2,
				},
				wgpu::VertexAttribute {
					offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
					shader_location: 1,
					format: wgpu::VertexFormat::Float32,
				},
			],
		};

		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("brush_pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				compilation_options: Default::default(),
				buffers: &[instance_layout],
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs_main"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::ColorTargetState {
					format: wgpu::TextureFormat::Rgba8Unorm,
					blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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

		Self { pipeline, bind_group_layout }
	}

	async fn run<'a>(&'a self, executor: &'a WgpuExecutor, args: &'a Self::Args<'_>) -> Self::Out {
		let &BrushPipelineArgs { start, end, stamps, radius, size, .. } = args;

		let size = size.max(UVec2::ONE);
		let output = executor.request_texture(size).await;

		let device = &executor.context().device;
		let queue = &executor.context().queue;

		let count = stamps.max(1);
		let mut instances = Vec::with_capacity(count as usize);
		for i in 0..count {
			let t = if count == 1 { 0.0 } else { i as f64 / (count - 1) as f64 };
			let center = start.lerp(end, t);
			instances.push(Stamp {
				center_px: [center.x as f32, center.y as f32],
				radius_px: radius,
				_pad: 0.0,
			});
		}

		let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("brush_instance_buffer"),
			contents: bytemuck::cast_slice(&instances),
			usage: wgpu::BufferUsages::VERTEX,
		});

		let globals = Globals {
			viewport_size: Vec2::new(size.x as f32, size.y as f32).to_array(),
			_pad: [0.0; 2],
		};

		let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("brush_globals_buffer"),
			contents: bytemuck::bytes_of(&globals),
			usage: wgpu::BufferUsages::UNIFORM,
		});

		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("brush_bind_group"),
			layout: &self.bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: globals_buffer.as_entire_binding(),
			}],
		});

		let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("brush_encoder") });

		{
			let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("brush_pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &output_view,
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
			pass.set_bind_group(0, &bind_group, &[]);
			pass.set_vertex_buffer(0, instance_buffer.slice(..));
			pass.draw(0..3, 0..count);
		}

		queue.submit([encoder.finish()]);

		output
	}
}
