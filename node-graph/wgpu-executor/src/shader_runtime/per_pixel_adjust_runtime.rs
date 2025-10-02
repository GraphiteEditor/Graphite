use crate::WgpuContext;
use crate::shader_runtime::{FULLSCREEN_VERTEX_SHADER_NAME, ShaderRuntime};
use futures::lock::Mutex;
use graphene_core::raster_types::{GPU, Raster};
use graphene_core::shaders::buffer_struct::BufferStruct;
use graphene_core::table::{Table, TableRow};
use std::borrow::Cow;
use std::collections::HashMap;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
	BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding, BufferBindingType, BufferUsages, ColorTargetState, Face,
	FragmentState, FrontFace, LoadOp, Operations, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
	ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureViewDescriptor, TextureViewDimension, VertexState,
};

pub struct PerPixelAdjustShaderRuntime {
	// TODO: PerPixelAdjustGraphicsPipeline already contains the key as `name`
	pipeline_cache: Mutex<HashMap<String, PerPixelAdjustGraphicsPipeline>>,
}

impl PerPixelAdjustShaderRuntime {
	pub fn new() -> Self {
		Self {
			pipeline_cache: Mutex::new(HashMap::new()),
		}
	}
}

impl ShaderRuntime {
	pub async fn run_per_pixel_adjust<T: BufferStruct>(&self, shaders: &Shaders<'_>, textures: Table<Raster<GPU>>, args: Option<&T>) -> Table<Raster<GPU>> {
		let mut cache = self.per_pixel_adjust.pipeline_cache.lock().await;
		let pipeline = cache
			.entry(shaders.fragment_shader_name.to_owned())
			.or_insert_with(|| PerPixelAdjustGraphicsPipeline::new(&self.context, shaders));

		let arg_buffer = args.map(|args| {
			let device = &self.context.device;
			device.create_buffer_init(&BufferInitDescriptor {
				label: Some(&format!("{} arg buffer", pipeline.name.as_str())),
				usage: BufferUsages::STORAGE,
				contents: bytemuck::bytes_of(&T::write(*args)),
			})
		});
		pipeline.dispatch(&self.context, textures, arg_buffer)
	}
}

pub struct Shaders<'a> {
	pub wgsl_shader: &'a str,
	pub fragment_shader_name: &'a str,
	pub has_uniform: bool,
}

pub struct PerPixelAdjustGraphicsPipeline {
	name: String,
	has_uniform: bool,
	pipeline: wgpu::RenderPipeline,
}

impl PerPixelAdjustGraphicsPipeline {
	pub fn new(context: &WgpuContext, info: &Shaders) -> Self {
		let device = &context.device;
		let name = info.fragment_shader_name.to_owned();

		let fragment_name = &name;
		let fragment_name = &fragment_name[(fragment_name.find("::").unwrap() + 2)..];
		// TODO workaround to naga removing `:`
		let fragment_name = fragment_name.replace(":", "");
		let shader_module = device.create_shader_module(ShaderModuleDescriptor {
			label: Some(&format!("PerPixelAdjust {name} wgsl shader")),
			source: ShaderSource::Wgsl(Cow::Borrowed(info.wgsl_shader)),
		});

		let entries: &[_] = if info.has_uniform {
			&[
				BindGroupLayoutEntry {
					binding: 0,
					visibility: ShaderStages::FRAGMENT,
					ty: BindingType::Buffer {
						ty: BufferBindingType::Storage { read_only: true },
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				},
				BindGroupLayoutEntry {
					binding: 1,
					visibility: ShaderStages::FRAGMENT,
					ty: BindingType::Texture {
						sample_type: TextureSampleType::Float { filterable: false },
						view_dimension: TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
			]
		} else {
			&[BindGroupLayoutEntry {
				binding: 0,
				visibility: ShaderStages::FRAGMENT,
				ty: BindingType::Texture {
					sample_type: TextureSampleType::Float { filterable: false },
					view_dimension: TextureViewDimension::D2,
					multisampled: false,
				},
				count: None,
			}]
		};
		let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: Some(&format!("PerPixelAdjust {name} PipelineLayout")),
			bind_group_layouts: &[&device.create_bind_group_layout(&BindGroupLayoutDescriptor {
				label: Some(&format!("PerPixelAdjust {name} BindGroupLayout 0")),
				entries,
			})],
			push_constant_ranges: &[],
		});

		let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: Some(&format!("PerPixelAdjust {name} Pipeline")),
			layout: Some(&pipeline_layout),
			vertex: VertexState {
				module: &shader_module,
				entry_point: Some(FULLSCREEN_VERTEX_SHADER_NAME),
				compilation_options: Default::default(),
				buffers: &[],
			},
			primitive: PrimitiveState {
				topology: PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: FrontFace::Ccw,
				cull_mode: Some(Face::Back),
				unclipped_depth: false,
				polygon_mode: PolygonMode::Fill,
				conservative: false,
			},
			depth_stencil: None,
			multisample: Default::default(),
			fragment: Some(FragmentState {
				module: &shader_module,
				entry_point: Some(&fragment_name),
				compilation_options: Default::default(),
				targets: &[Some(ColorTargetState {
					format: TextureFormat::Rgba8UnormSrgb,
					blend: None,
					write_mask: Default::default(),
				})],
			}),
			multiview: None,
			cache: None,
		});
		Self {
			pipeline,
			name,
			has_uniform: info.has_uniform,
		}
	}

	pub fn dispatch(&self, context: &WgpuContext, textures: Table<Raster<GPU>>, arg_buffer: Option<Buffer>) -> Table<Raster<GPU>> {
		assert_eq!(self.has_uniform, arg_buffer.is_some());
		let device = &context.device;
		let name = self.name.as_str();

		let mut cmd = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some(&format!("{name} cmd encoder")),
		});
		let out = textures
			.iter()
			.map(|instance| {
				let tex_in = &instance.element.texture;
				let view_in = tex_in.create_view(&TextureViewDescriptor::default());
				let format = tex_in.format();

				let entries: &[_] = if let Some(arg_buffer) = arg_buffer.as_ref() {
					&[
						BindGroupEntry {
							binding: 0,
							resource: BindingResource::Buffer(BufferBinding {
								buffer: arg_buffer,
								offset: 0,
								size: None,
							}),
						},
						BindGroupEntry {
							binding: 1,
							resource: BindingResource::TextureView(&view_in),
						},
					]
				} else {
					&[BindGroupEntry {
						binding: 0,
						resource: BindingResource::TextureView(&view_in),
					}]
				};
				let bind_group = device.create_bind_group(&BindGroupDescriptor {
					label: Some(&format!("{name} bind group")),
					// `get_bind_group_layout` allocates unnecessary memory, we could create it manually to not do that
					layout: &self.pipeline.get_bind_group_layout(0),
					entries,
				});

				let tex_out = device.create_texture(&TextureDescriptor {
					label: Some(&format!("{name} texture out")),
					size: tex_in.size(),
					mip_level_count: 1,
					sample_count: 1,
					dimension: TextureDimension::D2,
					format,
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
					view_formats: &[format],
				});

				let view_out = tex_out.create_view(&TextureViewDescriptor::default());
				let mut rp = cmd.begin_render_pass(&RenderPassDescriptor {
					label: Some(&format!("{name} render pipeline")),
					color_attachments: &[Some(RenderPassColorAttachment {
						view: &view_out,
						resolve_target: None,
						ops: Operations {
							// should be dont_care but wgpu doesn't expose that
							load: LoadOp::Clear(wgpu::Color::BLACK),
							store: StoreOp::Store,
						},
					})],
					depth_stencil_attachment: None,
					timestamp_writes: None,
					occlusion_query_set: None,
				});
				rp.set_pipeline(&self.pipeline);
				rp.set_bind_group(0, Some(&bind_group), &[]);
				rp.draw(0..3, 0..1);

				TableRow {
					element: Raster::new(GPU { texture: tex_out }),
					transform: *instance.transform,
					alpha_blending: *instance.alpha_blending,
					source_node_id: *instance.source_node_id,
				}
			})
			.collect::<Table<_>>();
		context.queue.submit([cmd.finish()]);
		out
	}
}
