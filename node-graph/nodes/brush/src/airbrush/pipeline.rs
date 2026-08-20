use super::consts::{SIGMA_CUTOFF, LUT_SIZE, LUT_T_MAX, LUT_V_MAX, RIDGE_GAIN};
use super::convert::Convert;
use super::kernel::{Kernel, KernelCache};
use super::region::{Crop, Region};
use super::stroke::{Edge, StyledStroke};
use brush_types::BrushCache;
use bytemuck::{Pod, Zeroable};
use core_types::Color;
use core_types::transform::Footprint;
use glam::{DAffine2, UVec2};
use raster_types::Texture;
use wgpu_executor::{AsyncWgpuPipeline, Buffer, WgpuExecutor};

pub(super) const DENSITY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
pub(super) const COMPOSITE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ScatterUniforms {
	frame_size: [f32; 2],
	kernel_scale: f32,
	kernel_exponent: f32,
	kernel_section_scale: f32,
	_pad: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ResolveUniforms {
	color: [f32; 4],
	density_offset: [f32; 2],
	_pad: [f32; 2],
}

pub struct AirbrushPipeline {
	scatter: Scatter,
	resolve: Resolve,
	convert: Convert,
	kernels: KernelCache,
}

pub(super) struct Field {
	pub(super) density: Texture,
	pub(super) stamp: Texture,
}

impl Field {
	pub(super) fn request(executor: &WgpuExecutor, size: UVec2) -> Self {
		Self {
			density: executor.request_texture_with_format(size, DENSITY_FORMAT),
			stamp: executor.request_texture_with_format(size, DENSITY_FORMAT),
		}
	}

	pub(super) fn views(&self) -> FieldViews {
		FieldViews {
			density: self.density.create_view(&wgpu::TextureViewDescriptor::default()),
			stamp: self.stamp.create_view(&wgpu::TextureViewDescriptor::default()),
		}
	}
}

pub(super) struct FieldViews {
	pub(super) density: wgpu::TextureView,
	pub(super) stamp: wgpu::TextureView,
}

pub struct AirbrushPipelineArgs<'a> {
	pub(super) footprint: Footprint,
	pub(super) strokes: &'a [StyledStroke],
	pub(super) cache: &'a BrushCache,
}

impl AsyncWgpuPipeline for AirbrushPipeline {
	type Args<'a> = AirbrushPipelineArgs<'a>;
	type Out = Option<(Texture, DAffine2)>;

	fn create(executor: &WgpuExecutor) -> Self {
		let device = &executor.context().device;
		Self {
			scatter: Scatter::new(device),
			resolve: Resolve::new(device),
			convert: Convert::new(device),
			kernels: KernelCache::default(),
		}
	}

	async fn run<'a>(&'a self, executor: &'a WgpuExecutor, args: &'a Self::Args<'_>) -> Self::Out {
		let frame = super::render::Frame::new(args.strokes)?;
		let region = Region::new(&args.footprint)?;
		let state = args.cache.take(&args.footprint).unwrap_or_default();
		let rendered = super::render::render(self, executor, frame, region, state)?;
		args.cache.store(&args.footprint, rendered.state);
		Some((rendered.texture, rendered.transform))
	}
}

struct Scatter {
	pipeline: wgpu::RenderPipeline,
	layout: wgpu::BindGroupLayout,
	sampler: wgpu::Sampler,
}

impl Scatter {
	fn new(device: &wgpu::Device) -> Self {
		let shader = device.create_shader_module(wgpu::include_wgsl!("scatter.wgsl"));
		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("airbrush_density_bind_group_layout"),
			entries: &[
				uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 2,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
			],
		});
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("airbrush_kernel_sampler"),
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			..Default::default()
		});
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("airbrush_density_pipeline_layout"),
			bind_group_layouts: &[Some(&layout)],
			immediate_size: 0,
		});
		let instance_layout = wgpu::VertexBufferLayout {
			array_stride: std::mem::size_of::<Edge>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &[
				wgpu::VertexAttribute {
					offset: 0,
					shader_location: 0,
					format: wgpu::VertexFormat::Float32x2,
				},
				wgpu::VertexAttribute {
					offset: 8,
					shader_location: 1,
					format: wgpu::VertexFormat::Float32x2,
				},
				wgpu::VertexAttribute {
					offset: 16,
					shader_location: 2,
					format: wgpu::VertexFormat::Float32,
				},
				wgpu::VertexAttribute {
					offset: 20,
					shader_location: 3,
					format: wgpu::VertexFormat::Float32,
				},
			],
		};
		let additive = wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::One,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Add,
		};
		let union = wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::One,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Max,
		};
		let options = wgpu::PipelineCompilationOptions {
			constants: &[("CUTOFF_SIGMA", SIGMA_CUTOFF as f64), ("LUT_SIZE", LUT_SIZE as f64), ("LUT_V_MAX", LUT_V_MAX), ("LUT_T_MAX", LUT_T_MAX)],
			..Default::default()
		};
		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("airbrush_density_pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				compilation_options: options.clone(),
				buffers: &[instance_layout],
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs_main"),
				compilation_options: options,
				targets: &[
					Some(wgpu::ColorTargetState {
						format: DENSITY_FORMAT,
						blend: Some(wgpu::BlendState { color: additive, alpha: additive }),
						write_mask: wgpu::ColorWrites::ALL,
					}),
					Some(wgpu::ColorTargetState {
						format: DENSITY_FORMAT,
						blend: Some(wgpu::BlendState { color: union, alpha: union }),
						write_mask: wgpu::ColorWrites::ALL,
					}),
				],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleStrip,
				..Default::default()
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview_mask: None,
			cache: None,
		});
		Self { pipeline, layout, sampler }
	}

	fn bind(&self, device: &wgpu::Device, globals: &wgpu::Buffer, kernel: &wgpu::TextureView) -> wgpu::BindGroup {
		device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("airbrush_density_bind_group"),
			layout: &self.layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: globals.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(kernel),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
			],
		})
	}

	fn encode(&self, encoder: &mut wgpu::CommandEncoder, target: &FieldViews, bind: &wgpu::BindGroup, buffer: &wgpu::Buffer, instances: u32) {
		let attachment = |view| {
			Some(wgpu::RenderPassColorAttachment {
				view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: wgpu::StoreOp::Store,
				},
				depth_slice: None,
			})
		};
		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("airbrush_density_pass"),
			color_attachments: &[attachment(&target.density), attachment(&target.stamp)],
			..Default::default()
		});
		pass.set_pipeline(&self.pipeline);
		pass.set_bind_group(0, bind, &[]);
		pass.set_vertex_buffer(0, buffer.slice(..));
		pass.draw(0..4, 0..instances);
	}
}

struct Resolve {
	pipeline: wgpu::RenderPipeline,
	layout: wgpu::BindGroupLayout,
}

impl Resolve {
	fn new(device: &wgpu::Device) -> Self {
		let shader = device.create_shader_module(wgpu::include_wgsl!("resolve.wgsl"));
		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("airbrush_resolve_bind_group_layout"),
			entries: &[uniform_entry(0, wgpu::ShaderStages::FRAGMENT), texture_entry(1), texture_entry(2)],
		});
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("airbrush_resolve_pipeline_layout"),
			bind_group_layouts: &[Some(&layout)],
			immediate_size: 0,
		});
		let options = wgpu::PipelineCompilationOptions {
			constants: &[("RIDGE_GAIN", RIDGE_GAIN as f64), ("RIDGE_NORM", 1. / (1. - (-RIDGE_GAIN as f64).exp()))],
			..Default::default()
		};
		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("airbrush_resolve_pipeline"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs_main"),
				compilation_options: options.clone(),
				buffers: &[],
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs_main"),
				compilation_options: options,
				targets: &[Some(wgpu::ColorTargetState {
					format: COMPOSITE_FORMAT,
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
		Self { pipeline, layout }
	}

	fn bind(&self, device: &wgpu::Device, globals: &wgpu::Buffer, source: &FieldViews) -> wgpu::BindGroup {
		device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("airbrush_resolve_bind_group"),
			layout: &self.layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: globals.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(&source.density),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&source.stamp),
				},
			],
		})
	}

	fn encode(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, bind: &wgpu::BindGroup, scissor: (UVec2, UVec2)) {
		let (origin, size) = scissor;
		if !size.cmpgt(UVec2::ZERO).all() {
			return;
		}
		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("airbrush_resolve_pass"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: target,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: wgpu::StoreOp::Store,
				},
				depth_slice: None,
			})],
			..Default::default()
		});
		pass.set_pipeline(&self.pipeline);
		pass.set_bind_group(0, bind, &[]);
		pass.set_scissor_rect(origin.x, origin.y, size.x, size.y);
		pass.draw(0..3, 0..1);
	}
}

pub(super) struct Recorder<'a> {
	pipeline: &'a AirbrushPipeline,
	executor: &'a WgpuExecutor,
	encoder: wgpu::CommandEncoder,
	region: Region,
	buffers: Vec<Buffer>,
	textures: Vec<Texture>,
}

impl<'a> Recorder<'a> {
	pub(super) fn new(pipeline: &'a AirbrushPipeline, executor: &'a WgpuExecutor, region: &Region) -> Self {
		let device = &executor.context().device;
		let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("airbrush_encoder") });
		Self {
			pipeline,
			executor,
			encoder,
			region: *region,
			buffers: Vec::new(),
			textures: Vec::new(),
		}
	}

	pub(super) fn kernel(&self, stroke: &StyledStroke) -> Kernel {
		self.pipeline.kernels.get(self.executor, stroke, self.region.scale)
	}

	pub(super) fn clear(&mut self, target: &wgpu::TextureView) {
		self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("airbrush_clear_pass"),
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
	}

	pub(super) fn clear_field(&mut self, target: &FieldViews) {
		let attachment = |view| {
			Some(wgpu::RenderPassColorAttachment {
				view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
					store: wgpu::StoreOp::Store,
				},
				depth_slice: None,
			})
		};
		self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("airbrush_clear_pass"),
			color_attachments: &[attachment(&target.density), attachment(&target.stamp)],
			..Default::default()
		});
	}

	pub(super) fn scatter(&mut self, target: &FieldViews, edges: &[Edge], kernel: &Kernel) {
		if edges.is_empty() {
			return;
		}
		let globals = self.executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("airbrush_scatter_uniform"),
			contents: bytemuck::bytes_of(&ScatterUniforms {
				frame_size: [self.region.size.x as f32, self.region.size.y as f32],
				kernel_scale: kernel.scale,
				kernel_exponent: kernel.exponent,
				kernel_section_scale: kernel.section_scale,
				_pad: 0.,
			}),
			usage: wgpu::BufferUsages::UNIFORM,
		});
		let view = kernel.texture.create_view(&wgpu::TextureViewDescriptor::default());
		let bind = self.pipeline.scatter.bind(&self.executor.context().device, &globals, &view);
		let buffer = self.executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("airbrush_segment_buffer"),
			contents: bytemuck::cast_slice(edges),
			usage: wgpu::BufferUsages::VERTEX,
		});
		self.pipeline.scatter.encode(&mut self.encoder, target, &bind, &buffer, edges.len() as u32);
		self.buffers.push(globals);
		self.buffers.push(buffer);
		self.textures.push(kernel.texture.clone());
	}

	pub(super) fn resolve(&mut self, color: Color, crop: &Crop, source: &FieldViews, target: &wgpu::TextureView, scissor: (UVec2, UVec2)) {
		let globals = resolve_uniform(self.executor, color, crop);
		let bind = self.pipeline.resolve.bind(&self.executor.context().device, &globals, source);
		self.pipeline.resolve.encode(&mut self.encoder, target, &bind, scissor);
		self.buffers.push(globals);
	}

	pub(super) fn copy(&mut self, from: &Texture, from_origin: UVec2, to: &Texture, to_origin: UVec2) {
		copy_placed(&mut self.encoder, from, from_origin, to, to_origin);
	}

	pub(super) fn copy_texture(&mut self, from: &Texture, to: &Texture) {
		self.encoder.copy_texture_to_texture(from.as_image_copy(), to.as_image_copy(), from.size());
	}

	pub(super) fn convert(&mut self, source: &wgpu::TextureView, target: &wgpu::TextureView) {
		self.pipeline.convert.encode(&self.executor.context().device, &mut self.encoder, source, target);
	}

	pub(super) fn keep(&mut self, texture: Texture) {
		self.textures.push(texture);
	}

	pub(super) fn submit(self) {
		let command = self.encoder.finish();
		self.executor.context().queue.submit([command]);
	}
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
	wgpu::BindGroupLayoutEntry {
		binding,
		visibility,
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	}
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
	wgpu::BindGroupLayoutEntry {
		binding,
		visibility: wgpu::ShaderStages::FRAGMENT,
		ty: wgpu::BindingType::Texture {
			sample_type: wgpu::TextureSampleType::Float { filterable: false },
			view_dimension: wgpu::TextureViewDimension::D2,
			multisampled: false,
		},
		count: None,
	}
}

fn resolve_uniform(executor: &WgpuExecutor, color: Color, crop: &Crop) -> Buffer {
	let uniforms = ResolveUniforms {
		color: [color.r(), color.g(), color.b(), color.a()],
		density_offset: [crop.origin.x as f32, crop.origin.y as f32],
		_pad: [0.; 2],
	};
	executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("airbrush_resolve_uniform"),
		contents: bytemuck::bytes_of(&uniforms),
		usage: wgpu::BufferUsages::UNIFORM,
	})
}

fn copy_placed(encoder: &mut wgpu::CommandEncoder, from: &wgpu::Texture, from_origin: UVec2, to: &wgpu::Texture, to_origin: UVec2) {
	let start = from_origin.max(to_origin);
	let end = (from_origin + UVec2::new(from.width(), from.height())).min(to_origin + UVec2::new(to.width(), to.height()));
	if !end.cmpgt(start).all() {
		return;
	}
	let info = |texture, origin: UVec2| wgpu::TexelCopyTextureInfo {
		texture,
		mip_level: 0,
		origin: wgpu::Origin3d { x: origin.x, y: origin.y, z: 0 },
		aspect: wgpu::TextureAspect::All,
	};
	let extent = end - start;
	encoder.copy_texture_to_texture(
		info(from, start - from_origin),
		info(to, start - to_origin),
		wgpu::Extent3d {
			width: extent.x,
			height: extent.y,
			depth_or_array_layers: 1,
		},
	);
}
