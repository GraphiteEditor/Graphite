use crate::WgpuContext;
use crate::shader_runtime::{FULLSCREEN_VERTEX_SHADER_NAME, ShaderRuntime};
use core_types::list::{Item, List};
use core_types::shaders::buffer_struct::BufferStruct;
use futures::lock::Mutex;
use raster_types::{GPU, Raster};
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

impl Default for PerPixelAdjustShaderRuntime {
	fn default() -> Self {
		Self::new()
	}
}

impl PerPixelAdjustShaderRuntime {
	pub fn new() -> Self {
		Self {
			pipeline_cache: Mutex::new(HashMap::new()),
		}
	}
}

impl ShaderRuntime {
	pub async fn run_per_pixel_adjust<T: BufferStruct>(&self, shaders: &Shaders<'_>, textures: &[List<Raster<GPU>>], args: Option<&T>) -> List<Raster<GPU>> {
		assert_eq!(shaders.input_images, textures.len());
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
	pub input_images: usize,
	pub has_uniform: bool,
}

pub struct PerPixelAdjustGraphicsPipeline {
	name: String,
	input_images: usize,
	has_uniform: bool,
	pipeline: wgpu::RenderPipeline,
}

impl PerPixelAdjustGraphicsPipeline {
	pub fn new(context: &WgpuContext, info: &Shaders) -> Self {
		let device = &context.device;
		let name = info.fragment_shader_name.to_owned();

		let fragment_name = &name;
		let fragment_name = &fragment_name[(fragment_name.find("::").unwrap() + 2)..];
		let fragment_name = fragment_name.replace("::", "_");
		let shader_module = device.create_shader_module(ShaderModuleDescriptor {
			label: Some(&format!("PerPixelAdjust {name} wgsl shader")),
			source: ShaderSource::Wgsl(Cow::Borrowed(info.wgsl_shader)),
		});

		let mut binding_alloc = Counter::default();
		let mut entries = Vec::new();
		if info.has_uniform {
			entries.push(BindGroupLayoutEntry {
				binding: binding_alloc.alloc(),
				visibility: ShaderStages::FRAGMENT,
				ty: BindingType::Buffer {
					ty: BufferBindingType::Storage { read_only: true },
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			});
		}
		for _ in 0..info.input_images {
			entries.push(BindGroupLayoutEntry {
				binding: binding_alloc.alloc(),
				visibility: ShaderStages::FRAGMENT,
				ty: BindingType::Texture {
					sample_type: TextureSampleType::Float { filterable: false },
					view_dimension: TextureViewDimension::D2,
					multisampled: false,
				},
				count: None,
			});
		}
		let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: Some(&format!("PerPixelAdjust {name} PipelineLayout")),
			bind_group_layouts: &[Some(&device.create_bind_group_layout(&BindGroupLayoutDescriptor {
				label: Some(&format!("PerPixelAdjust {name} BindGroupLayout 0")),
				entries: &entries,
			}))],
			..Default::default()
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
			multiview_mask: None,
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
			cache: None,
		});
		Self {
			pipeline,
			name,
			has_uniform: info.has_uniform,
			input_images: info.input_images,
		}
	}

	pub fn dispatch(&self, context: &WgpuContext, in_textures: &[List<Raster<GPU>>], arg_buffer: Option<Buffer>) -> List<Raster<GPU>> {
		assert_eq!(self.has_uniform, arg_buffer.is_some());
		assert_eq!(self.input_images, in_textures.len());
		let device = &context.device;
		let name = self.name.as_str();

		// Assumption: when we have multiple input images to our node, each input's List of images can have a different
		// length. Only process the minimum between all input images, same as `impl Blend<Color> for List<Raster<CPU>>`.
		let dispatch_cnt = match in_textures.iter().map(|t| t.len()).min() {
			None => {
				return List::new();
			}
			Some(e) => e,
		};

		let mut cmd = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some(&format!("{name} cmd encoder")),
		});
		let out = (0..dispatch_cnt)
			.map(|dispatch_id| {
				let mut binding_alloc = Counter::default();
				let mut entries = Vec::new();
				if let Some(arg_buffer) = arg_buffer.as_ref() {
					entries.push(BindGroupEntry {
						binding: binding_alloc.alloc(),
						resource: BindingResource::Buffer(BufferBinding {
							buffer: arg_buffer,
							offset: 0,
							size: None,
						}),
					});
				}
				let in_texture_views = in_textures
					.iter()
					.map(|texture| {
						let element = texture.element(dispatch_id).unwrap();
						element.texture.create_view(&TextureViewDescriptor::default())
					})
					.collect::<Vec<_>>();
				for view_in in &in_texture_views {
					entries.push(BindGroupEntry {
						binding: binding_alloc.alloc(),
						resource: BindingResource::TextureView(&view_in),
					});
				}

				let bind_group = device.create_bind_group(&BindGroupDescriptor {
					label: Some(&format!("{name} bind group")),
					// `get_bind_group_layout` allocates unnecessary memory, we could create it manually to not do that
					layout: &self.pipeline.get_bind_group_layout(0),
					entries: &entries,
				});

				// Assumption: The output texture has the same size and format as the first input texture. Like the
				// blend node, that writes the output directly back into the first texture.
				let outref_list = &in_textures[0];
				let outref_tex = &outref_list.element(dispatch_id).unwrap().texture;
				let tex_out = device.create_texture(&TextureDescriptor {
					label: Some(&format!("{name} texture out")),
					size: outref_tex.size(),
					mip_level_count: 1,
					sample_count: 1,
					dimension: TextureDimension::D2,
					format: outref_tex.format(),
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
					view_formats: &[outref_tex.format()],
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
						depth_slice: None,
					})],
					..Default::default()
				});
				rp.set_pipeline(&self.pipeline);
				rp.set_bind_group(0, Some(&bind_group), &[]);
				rp.draw(0..3, 0..1);

				let attributes = outref_list.clone_item_attributes(dispatch_id);
				Item::from_parts(Raster::new(GPU { texture: tex_out }), attributes)
			})
			.collect::<List<_>>();
		context.queue.submit([cmd.finish()]);
		out
	}
}

#[derive(Clone, Debug, Default)]
pub struct Counter(pub u32);

impl Counter {
	pub fn alloc(&mut self) -> u32 {
		let out = self.0;
		self.0 += 1;
		out
	}
}
