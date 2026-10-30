use crate::{WgpuExecutor, WgpuPipeline};
use core_types::Color;
use glam::{DAffine2, UVec2};
use raster_types::{GPU, Raster};
use std::any::TypeId;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use wgpu::util::BufferInitDescriptor;

/// An input raster of a kernel, with the affine mapping output pixel coordinates to the input's texel coordinates.
pub struct RasterKernelInput<'a> {
	pub raster: &'a Raster<GPU>,
	pub output_to_texel: DAffine2,
}

/// A lookup table of a kernel, bound as an `Rgba32Float` texture after its inputs.
pub enum RasterKernelLookup<'a> {
	/// A `width x 1` 2D texture of at most 4096 entries.
	Lut1d(&'a [Color]),
	/// A `size x size x size` 3D texture, its entries ordered with x fastest, then y, then z.
	Lut3d { size: u32, data: &'a [Color] },
}

impl RasterKernelLookup<'_> {
	fn data(&self) -> &[Color] {
		match self {
			Self::Lut1d(data) | Self::Lut3d { data, .. } => data,
		}
	}

	fn extent(&self) -> wgpu::Extent3d {
		match self {
			Self::Lut1d(data) => wgpu::Extent3d {
				width: data.len() as u32,
				height: 1,
				depth_or_array_layers: 1,
			},
			Self::Lut3d { size, .. } => wgpu::Extent3d {
				width: *size,
				height: *size,
				depth_or_array_layers: *size,
			},
		}
	}

	fn dimension(&self) -> (wgpu::TextureDimension, wgpu::TextureViewDimension) {
		match self {
			Self::Lut1d(_) => (wgpu::TextureDimension::D2, wgpu::TextureViewDimension::D2),
			Self::Lut3d { .. } => (wgpu::TextureDimension::D3, wgpu::TextureViewDimension::D3),
		}
	}

	fn content_hash(&self) -> u64 {
		let mut hasher = DefaultHasher::new();
		let extent = self.extent();
		(extent.width, extent.height, extent.depth_or_array_layers, matches!(self, Self::Lut3d { .. })).hash(&mut hasher);
		bytemuck::cast_slice::<Color, u8>(self.data()).hash(&mut hasher);
		hasher.finish()
	}
}

/// One run of a raster kernel, writing a texture of `output_size` in `output_format`.
///
/// `wgsl` must define `fn kernel(coord: vec2<i32>) -> vec4<f32>`, and reads its parameters from `uniform_params`, the 32-bit `params` packed into `vec4<u32>` slots.
/// It may call `output_size()` and, for each input `i`, `input_i_size()`, `input_i_load(coord)` reading the input's own texel clamped to its edge,
/// and `input_i_at(coord)` sampling the input bilinearly where the output pixel's center lies, transparent beyond the input.
/// With `filtered`, it may also call `input_i_sample(texel)`, sampling the input bilinearly at texel coordinates, whose texel centers lie at +0.5, clamped to its edge,
/// which requires every input to be in a filterable format such as `Rgba16Float` rather than [`crate::RASTER_FORMAT`].
/// It declares the texture of each of the `lookups` itself, the `k`th at binding `2 + inputs + k`.
/// An empty input reads as transparent with a size of zero, and an empty `output_size` gives the empty raster without running the kernel.
/// Every run of the same `kernel` must have the same number of inputs, the same kinds of lookups, and the same `filtered`.
pub struct RasterKernelArgs<'a> {
	pub kernel: TypeId,
	pub label: &'static str,
	pub wgsl: fn() -> String,
	pub params: Vec<u32>,
	pub inputs: Vec<RasterKernelInput<'a>>,
	pub lookups: Vec<RasterKernelLookup<'a>>,
	pub filtered: bool,
	pub output_size: UVec2,
	pub output_format: wgpu::TextureFormat,
}

struct KernelPipeline {
	bind_group_layout: wgpu::BindGroupLayout,
	pipeline: wgpu::RenderPipeline,
}

/// The number of lookup textures kept uploaded, the most recently used first, so a kernel run again with the same table skips the upload.
const LOOKUP_CACHE_SIZE: usize = 16;

pub struct RasterKernelRuntime {
	pipelines: Mutex<HashMap<(TypeId, wgpu::TextureFormat), KernelPipeline>>,
	lookups: Mutex<VecDeque<(u64, wgpu::Texture)>>,
	sampler: wgpu::Sampler,
	empty_input: wgpu::Texture,
}

impl RasterKernelRuntime {
	fn lookup_texture(&self, executor: &WgpuExecutor, lookup: &RasterKernelLookup<'_>) -> wgpu::Texture {
		let hash = lookup.content_hash();
		let mut lookups = self.lookups.lock().unwrap();
		if let Some(index) = lookups.iter().position(|(cached, _)| *cached == hash) {
			let entry = lookups.remove(index).unwrap();
			let texture = entry.1.clone();
			lookups.push_front(entry);
			return texture;
		}

		let size = lookup.extent();
		let texture = executor.context().device.create_texture(&wgpu::TextureDescriptor {
			label: Some("raster_kernel_lookup"),
			size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: lookup.dimension().0,
			format: wgpu::TextureFormat::Rgba32Float,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});
		executor.context().queue.write_texture(
			texture.as_image_copy(),
			bytemuck::cast_slice(lookup.data()),
			wgpu::TexelCopyBufferLayout {
				offset: 0,
				bytes_per_row: Some(16 * size.width),
				rows_per_image: Some(size.height),
			},
			size,
		);
		lookups.push_front((hash, texture.clone()));
		lookups.truncate(LOOKUP_CACHE_SIZE);
		texture
	}
}

impl WgpuPipeline for RasterKernelRuntime {
	type Args<'a> = RasterKernelArgs<'a>;
	type Out = Raster<GPU>;

	fn create(executor: &WgpuExecutor) -> Self {
		let sampler = executor.context().device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("raster_kernel_sampler"),
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			..Default::default()
		});
		let empty_input = executor.context().device.create_texture(&wgpu::TextureDescriptor {
			label: Some("raster_kernel_empty_input"),
			size: wgpu::Extent3d {
				width: 1,
				height: 1,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba16Float,
			usage: wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});
		Self {
			pipelines: Mutex::default(),
			lookups: Mutex::default(),
			sampler,
			empty_input,
		}
	}

	fn run(&self, executor: &WgpuExecutor, args: &RasterKernelArgs<'_>) -> Raster<GPU> {
		if args.output_size.x == 0 || args.output_size.y == 0 {
			return Raster::default();
		}

		let device = &executor.context().device;
		let param_slots = slots(args.params.len());

		let mut pipelines = self.pipelines.lock().unwrap();
		let kernel_pipeline = pipelines.entry((args.kernel, args.output_format)).or_insert_with(|| create_pipeline(device, args, param_slots));

		let mut params = args.params.clone();
		params.resize(param_slots * 4, 0);
		let params = executor.create_buffer_init(&BufferInitDescriptor {
			label: Some(args.label),
			contents: bytemuck::cast_slice(&params),
			usage: wgpu::BufferUsages::UNIFORM,
		});
		let frame = executor.create_buffer_init(&BufferInitDescriptor {
			label: Some(args.label),
			contents: bytemuck::cast_slice(&frame_data(args)),
			usage: wgpu::BufferUsages::UNIFORM,
		});

		let input_views = args
			.inputs
			.iter()
			.map(|input| input.raster.texture().map_or(&self.empty_input, |texture| texture).create_view(&wgpu::TextureViewDescriptor::default()))
			.collect::<Vec<_>>();
		let lookup_views = args
			.lookups
			.iter()
			.map(|lookup| self.lookup_texture(executor, lookup).create_view(&wgpu::TextureViewDescriptor::default()))
			.collect::<Vec<_>>();
		let mut entries = vec![
			wgpu::BindGroupEntry {
				binding: 0,
				resource: params.as_entire_binding(),
			},
			wgpu::BindGroupEntry {
				binding: 1,
				resource: frame.as_entire_binding(),
			},
		];
		entries.extend(input_views.iter().enumerate().map(|(index, view)| wgpu::BindGroupEntry {
			binding: 2 + index as u32,
			resource: wgpu::BindingResource::TextureView(view),
		}));
		entries.extend(lookup_views.iter().enumerate().map(|(index, view)| wgpu::BindGroupEntry {
			binding: (2 + args.inputs.len() + index) as u32,
			resource: wgpu::BindingResource::TextureView(view),
		}));
		if args.filtered {
			entries.push(wgpu::BindGroupEntry {
				binding: sampler_binding(args),
				resource: wgpu::BindingResource::Sampler(&self.sampler),
			});
		}
		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some(args.label),
			layout: &kernel_pipeline.bind_group_layout,
			entries: &entries,
		});

		let output = executor.request_texture_with_format(args.output_size, args.output_format);
		let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(args.label) });
		{
			let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some(args.label),
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
			pass.set_pipeline(&kernel_pipeline.pipeline);
			pass.set_bind_group(0, &bind_group, &[]);
			pass.draw(0..3, 0..1);
		}
		executor.context().queue.submit([encoder.finish()]);

		Raster::new_gpu(output)
	}
}

fn create_pipeline(device: &wgpu::Device, args: &RasterKernelArgs<'_>, param_slots: usize) -> KernelPipeline {
	let uniform = |binding| wgpu::BindGroupLayoutEntry {
		binding,
		visibility: wgpu::ShaderStages::FRAGMENT,
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	let texture = |binding, view_dimension, filterable| wgpu::BindGroupLayoutEntry {
		binding,
		visibility: wgpu::ShaderStages::FRAGMENT,
		ty: wgpu::BindingType::Texture {
			sample_type: wgpu::TextureSampleType::Float { filterable },
			view_dimension,
			multisampled: false,
		},
		count: None,
	};
	let mut entries = vec![uniform(0), uniform(1)];
	entries.extend((0..args.inputs.len()).map(|index| texture(2 + index as u32, wgpu::TextureViewDimension::D2, args.filtered)));
	entries.extend(
		args.lookups
			.iter()
			.enumerate()
			.map(|(index, lookup)| texture((2 + args.inputs.len() + index) as u32, lookup.dimension().1, false)),
	);
	if args.filtered {
		entries.push(wgpu::BindGroupLayoutEntry {
			binding: sampler_binding(args),
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
			count: None,
		});
	}
	let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some(args.label),
		entries: &entries,
	});
	let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some(args.label),
		bind_group_layouts: &[Some(&bind_group_layout)],
		immediate_size: 0,
	});

	let source = format!("{}\n{}\n\n{}", bindings_wgsl(param_slots, args), (args.wgsl)(), include_str!("raster_kernel_entry.wgsl"));
	let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some(args.label),
		source: wgpu::ShaderSource::Wgsl(source.into()),
	});

	let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some(args.label),
		layout: Some(&pipeline_layout),
		vertex: wgpu::VertexState {
			module: &module,
			entry_point: Some("vertex"),
			compilation_options: Default::default(),
			buffers: &[],
		},
		fragment: Some(wgpu::FragmentState {
			module: &module,
			entry_point: Some("fragment"),
			compilation_options: Default::default(),
			targets: &[Some(args.output_format.into())],
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

	KernelPipeline { bind_group_layout, pipeline }
}

/// The frame uniform: the output size, then for each input its affine's matrix columns, translation, and size.
fn frame_data(args: &RasterKernelArgs<'_>) -> Vec<f32> {
	let mut data = vec![args.output_size.x as f32, args.output_size.y as f32, 0., 0.];
	for input in &args.inputs {
		let affine = input.output_to_texel;
		let size = input.raster.size();
		data.extend([affine.matrix2.x_axis.x, affine.matrix2.x_axis.y, affine.matrix2.y_axis.x, affine.matrix2.y_axis.y].map(|value| value as f32));
		data.extend([affine.translation.x as f32, affine.translation.y as f32, size.x as f32, size.y as f32]);
	}
	data
}

/// The binding of the sampler of a `filtered` kernel's inputs, after its lookups.
fn sampler_binding(args: &RasterKernelArgs<'_>) -> u32 {
	(2 + args.inputs.len() + args.lookups.len()) as u32
}

fn bindings_wgsl(param_slots: usize, args: &RasterKernelArgs<'_>) -> String {
	let inputs = args.inputs.len();
	let frame_slots = 1 + 2 * inputs;
	let mut source = format!(
		"@group(0) @binding(0) var<uniform> uniform_params: array<vec4<u32>, {param_slots}>;\n@group(0) @binding(1) var<uniform> kernel_frame: array<vec4<f32>, {frame_slots}>;\n\nfn output_size() -> vec2<i32> {{\n\treturn vec2<i32>(kernel_frame[0].xy);\n}}\n"
	);
	if args.filtered {
		source += &format!("\n@group(0) @binding({}) var raster_kernel_sampler: sampler;\n", sampler_binding(args));
	}
	for index in 0..inputs {
		let mut input = include_str!("raster_kernel_input.wgsl").to_string();
		if args.filtered {
			input += include_str!("raster_kernel_input_sample.wgsl");
		}
		source += &input
			.replace("INPUT", &format!("input_{index}"))
			.replace("BINDING", &(2 + index).to_string())
			.replace("AXES", &(1 + 2 * index).to_string())
			.replace("PLACEMENT", &(2 + 2 * index).to_string());
	}
	source
}

/// The `vec4<u32>` slot count holding `params` 32-bit values, at least one since WGSL arrays can't be empty.
fn slots(params: usize) -> usize {
	params.div_ceil(4).max(1)
}
