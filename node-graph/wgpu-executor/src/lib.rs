mod context;
mod executor;

pub use context::Context;
use dyn_any::{DynAny, StaticType};
pub use executor::GpuExecutor;
use gpu_executor::{ComputePassDimensions, Shader, ShaderInput, StorageBufferOptions, TextureBufferOptions, TextureBufferType, ToStorageBuffer, ToUniformBuffer};
use graph_craft::Type;

use anyhow::{bail, Result};
use futures::Future;
use graphene_core::application_io::{ApplicationIo, EditorApi, SurfaceHandle};

use std::cell::Cell;
use std::pin::Pin;
use std::sync::Arc;

use wgpu::util::DeviceExt;
use wgpu::{Buffer, BufferDescriptor, CommandBuffer, ShaderModule, SurfaceConfiguration, SurfaceError, Texture, TextureView};

#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;

#[derive(dyn_any::DynAny)]
pub struct WgpuExecutor {
	pub context: Context,
	render_configuration: RenderConfiguration,
	surface_config: Cell<Option<SurfaceConfiguration>>,
}

impl std::fmt::Debug for WgpuExecutor {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("WgpuExecutor")
			.field("context", &self.context)
			.field("render_configuration", &self.render_configuration)
			.finish()
	}
}

impl<'a, T: ApplicationIo<Executor = WgpuExecutor>> From<EditorApi<'a, T>> for &'a WgpuExecutor {
	fn from(editor_api: EditorApi<'a, T>) -> Self {
		editor_api.application_io.gpu_executor().unwrap()
	}
}

pub type WgpuSurface<'window> = Arc<SurfaceHandle<wgpu::Surface<'window>>>;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
	position: [f32; 3],
	tex_coords: [f32; 2],
}

impl Vertex {
	fn desc() -> wgpu::VertexBufferLayout<'static> {
		use std::mem;
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &[
				wgpu::VertexAttribute {
					offset: 0,
					shader_location: 0,
					format: wgpu::VertexFormat::Float32x3,
				},
				wgpu::VertexAttribute {
					offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
					shader_location: 1,
					format: wgpu::VertexFormat::Float32x2,
				},
			],
		}
	}
}

const VERTICES: &[Vertex] = &[
	Vertex {
		position: [-1., 1., 0.0],
		tex_coords: [0., 0.],
	}, // A
	Vertex {
		position: [-1., -1., 0.0],
		tex_coords: [0., 1.],
	}, // B
	Vertex {
		position: [1., 1., 0.0],
		tex_coords: [1., 0.],
	}, // C
	Vertex {
		position: [1., -1., 0.0],
		tex_coords: [1., 1.],
	}, // D
];

const INDICES: &[u16] = &[0, 1, 2, 2, 1, 3];

type WgpuShaderInput = ShaderInput<WgpuExecutor>;

#[derive(Debug, DynAny)]
#[repr(transparent)]
pub struct CommandBufferWrapper(CommandBuffer);

#[derive(Debug, DynAny)]
#[repr(transparent)]
pub struct ShaderModuleWrapper(ShaderModule);

impl gpu_executor::GpuExecutor for WgpuExecutor {
	type ShaderHandle = ShaderModuleWrapper;
	type BufferHandle = Buffer;
	type TextureHandle = Texture;
	type TextureView = TextureView;
	type CommandBuffer = CommandBufferWrapper;
	type Surface<'window> = wgpu::Surface<'window>;
	#[cfg(target_arch = "wasm32")]
	type Window = HtmlCanvasElement;
	#[cfg(not(target_arch = "wasm32"))]
	type Window = Arc<winit::window::Window>;

	fn load_shader(&self, shader: Shader) -> Result<Self::ShaderHandle> {
		#[cfg(not(feature = "passthrough"))]
		let shader_module = self.context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some(shader.name),
			source: wgpu::ShaderSource::SpirV(shader.source),
		});
		#[cfg(feature = "passthrough")]
		let shader_module = unsafe {
			self.context.device.create_shader_module_spirv(&wgpu::ShaderModuleDescriptorSpirV {
				label: Some(shader.name),
				source: shader.source,
			})
		};
		Ok(ShaderModuleWrapper(shader_module))
	}

	fn create_uniform_buffer<T: ToUniformBuffer>(&self, data: T) -> Result<WgpuShaderInput> {
		let bytes = data.to_bytes();
		let buffer = self.context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytes.as_ref(),
			usage: wgpu::BufferUsages::UNIFORM,
		});
		Ok(ShaderInput::UniformBuffer(buffer, Type::new::<T>()))
	}

	fn create_storage_buffer<T: ToStorageBuffer>(&self, data: T, options: StorageBufferOptions) -> Result<WgpuShaderInput> {
		let bytes = data.to_bytes();
		let mut usage = wgpu::BufferUsages::empty();

		if options.storage {
			usage |= wgpu::BufferUsages::STORAGE;
		}
		if options.gpu_writable {
			usage |= wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
		}
		if options.cpu_readable {
			usage |= wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;
		}
		if options.cpu_writable {
			usage |= wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC;
		}

		log::debug!("Creating storage buffer with usage {:?} and len: {}", usage, bytes.len());
		let buffer = self.context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytes.as_ref(),
			usage,
		});
		Ok(ShaderInput::StorageBuffer(buffer, data.ty()))
	}
	fn create_texture_buffer<T: gpu_executor::ToTextureBuffer>(&self, data: T, options: TextureBufferOptions) -> Result<WgpuShaderInput> {
		let bytes = data.to_bytes();
		let usage = match options {
			TextureBufferOptions::Storage => wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
			TextureBufferOptions::Texture => wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			TextureBufferOptions::Surface => wgpu::TextureUsages::RENDER_ATTACHMENT,
		};
		let format = match T::format() {
			TextureBufferType::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
			TextureBufferType::Rgba8Srgb => wgpu::TextureFormat::Bgra8UnormSrgb,
		};

		let buffer = self.context.device.create_texture_with_data(
			self.context.queue.as_ref(),
			&wgpu::TextureDescriptor {
				label: None,
				size: wgpu::Extent3d {
					width: data.size().0,
					height: data.size().1,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format,
				usage,
				view_formats: &[format],
			},
			wgpu::util::TextureDataOrder::LayerMajor,
			bytes.as_ref(),
		);
		match options {
			TextureBufferOptions::Storage => Ok(ShaderInput::StorageTextureBuffer(buffer, T::ty())),
			TextureBufferOptions::Texture => Ok(ShaderInput::TextureBuffer(buffer, T::ty())),
			TextureBufferOptions::Surface => Ok(ShaderInput::TextureBuffer(buffer, T::ty())),
		}
	}

	fn create_output_buffer(&self, len: usize, ty: Type, cpu_readable: bool) -> Result<WgpuShaderInput> {
		log::debug!("Creating output buffer with len: {len}");
		let create_buffer = |usage| {
			Ok::<_, anyhow::Error>(self.context.device.create_buffer(&BufferDescriptor {
				label: None,
				size: len as u64 * ty.size().ok_or_else(|| anyhow::anyhow!("Cannot create buffer of type {ty:?}"))? as u64,
				usage,
				mapped_at_creation: false,
			}))
		};
		let buffer = match cpu_readable {
			true => ShaderInput::ReadBackBuffer(create_buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ)?, ty),
			false => ShaderInput::OutputBuffer(create_buffer(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC)?, ty),
		};
		Ok(buffer)
	}
	fn create_compute_pass(&self, layout: &gpu_executor::PipelineLayout<Self>, read_back: Option<Arc<WgpuShaderInput>>, instances: ComputePassDimensions) -> Result<Self::CommandBuffer> {
		let compute_pipeline = self.context.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
			label: None,
			layout: None,
			module: &layout.shader.0,
			entry_point: layout.entry_point.as_str(),
		});
		let bind_group_layout = compute_pipeline.get_bind_group_layout(0);

		let entries = layout
			.bind_group
			.buffers
			.iter()
			.chain(std::iter::once(&layout.output_buffer))
			.flat_map(|input| input.binding())
			.enumerate()
			.map(|(i, buffer)| wgpu::BindGroupEntry {
				binding: i as u32,
				resource: match buffer {
					gpu_executor::BindingType::UniformBuffer(buf) => buf.as_entire_binding(),
					gpu_executor::BindingType::StorageBuffer(buf) => buf.as_entire_binding(),
					gpu_executor::BindingType::TextureView(buf) => wgpu::BindingResource::TextureView(buf),
				},
			})
			.collect::<Vec<_>>();

		let bind_group = self.context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout: &bind_group_layout,
			entries: entries.as_slice(),
		});

		let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("compute encoder") });
		{
			let dimensions = instances.get();
			let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
			cpass.set_pipeline(&compute_pipeline);
			cpass.set_bind_group(0, &bind_group, &[]);
			cpass.insert_debug_marker("compute node network evaluation");
			cpass.push_debug_group("compute shader");
			cpass.dispatch_workgroups(dimensions.0, dimensions.1, dimensions.2); // Number of cells to run, the (x,y,z) size of item being processed
			cpass.pop_debug_group();
		}
		// Sets adds copy operation to command encoder.
		// Will copy data from storage buffer on GPU to staging buffer on CPU.
		if let Some(buffer) = read_back {
			let ShaderInput::ReadBackBuffer(output, _ty) = buffer.as_ref() else {
				bail!("Tried to read back from a non read back buffer");
			};
			let size = output.size();
			let ShaderInput::OutputBuffer(output_buffer, ty) = layout.output_buffer.as_ref() else {
				bail!("Tried to read back from a non output buffer");
			};
			assert_eq!(size, output_buffer.size());
			assert_eq!(ty, &layout.output_buffer.ty());
			encoder.copy_buffer_to_buffer(output_buffer, 0, output, 0, size);
		}

		// Submits command encoder for processing
		Ok(CommandBufferWrapper(encoder.finish()))
	}

	fn create_render_pass(&self, texture: Arc<ShaderInput<Self>>, canvas: Arc<SurfaceHandle<wgpu::Surface>>) -> Result<()> {
		let texture = texture.texture().expect("Expected texture input");
		let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
		let result = canvas.as_ref().surface.get_current_texture();

		let surface = &canvas.as_ref().surface;
		let surface_caps = surface.get_capabilities(&self.context.adapter);
		println!("{surface_caps:?}");
		if surface_caps.formats.is_empty() {
			log::warn!("No surface formats available");
			//return Ok(());
		}
		let Some(config) = self.surface_config.take() else { return Ok(()) };
		let new_config = config.clone();
		self.surface_config.replace(Some(config));
		let output = match result {
			Err(SurfaceError::Timeout) => {
				log::warn!("Timeout when getting current texture");
				return Ok(());
			}
			Err(SurfaceError::Lost) => {
				log::warn!("Surface lost");

				surface.configure(&self.context.device, &new_config);
				return Ok(());
			}
			Err(SurfaceError::OutOfMemory) => {
				log::warn!("Out of memory");
				return Ok(());
			}
			Err(SurfaceError::Outdated) => {
				log::warn!("Surface outdated");
				surface.configure(&self.context.device, &new_config);
				return Ok(());
			}
			Ok(surface) => surface,
		};
		let view = output.texture.create_view(&wgpu::TextureViewDescriptor {
			format: Some(wgpu::TextureFormat::Bgra8Unorm),
			..Default::default()
		});
		let output_texture_bind_group = self.context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &self.render_configuration.texture_bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&texture_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&self.render_configuration.sampler),
				},
			],
			label: Some("output_texture_bind_group"),
		});

		let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") });

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &view,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Load,
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: None,
				timestamp_writes: None,
				occlusion_query_set: None,
			});

			render_pass.set_pipeline(&self.render_configuration.render_pipeline);
			render_pass.set_bind_group(0, &output_texture_bind_group, &[]);
			render_pass.set_vertex_buffer(0, self.render_configuration.vertex_buffer.slice(..));
			render_pass.set_index_buffer(self.render_configuration.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
			render_pass.draw_indexed(0..self.render_configuration.num_indices, 0, 0..1);
			render_pass.insert_debug_marker("render node network");
		}

		let encoder = encoder.finish();
		#[cfg(feature = "profiling")]
		nvtx::range_push!("render");
		self.context.queue.submit(Some(encoder));
		#[cfg(feature = "profiling")]
		nvtx::range_pop!();
		log::trace!("Submitted render pass");
		output.present();

		Ok(())
	}

	fn execute_compute_pipeline(&self, encoder: Self::CommandBuffer) -> Result<()> {
		self.context.queue.submit(Some(encoder.0));

		Ok(())
	}

	fn read_output_buffer(&self, buffer: Arc<ShaderInput<Self>>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>>>> {
		Box::pin(async move {
			if let ShaderInput::ReadBackBuffer(buffer, _) = buffer.as_ref() {
				let buffer_slice = buffer.slice(..);

				// Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
				let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
				buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

				// Wait for the mapping to finish.
				#[cfg(feature = "profiling")]
				nvtx::range_push!("compute");
				let result = receiver.receive().await;
				#[cfg(feature = "profiling")]
				nvtx::range_pop!();

				if result == Some(Ok(())) {
					// Gets contents of buffer
					let data = buffer_slice.get_mapped_range();
					// Since contents are got in bytes, this converts these bytes back to u32
					let result = bytemuck::cast_slice(&data).to_vec();

					// With the current interface, we have to make sure all mapped views are
					// dropped before we unmap the buffer.
					drop(data);
					buffer.unmap(); // Unmaps buffer from memory

					// Returns data from buffer
					Ok(result)
				} else {
					bail!("failed to run compute on gpu!")
				}
			} else {
				bail!("Tried to read a non readback buffer")
			}
		})
	}

	fn create_texture_view(&self, texture: ShaderInput<Self>) -> Result<ShaderInput<Self>> {
		//Ok(ShaderInput::TextureView(texture.create_view(&wgpu::TextureViewDescriptor::default()), ) )
		let ShaderInput::TextureBuffer(texture, ty) = &texture else {
			bail!("Tried to create a texture view from a non texture");
		};
		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
		Ok(ShaderInput::TextureView(view, ty.clone()))
	}

	#[cfg(target_arch = "wasm32")]
	fn create_surface(&self, canvas: graphene_core::WasmSurfaceHandle) -> Result<SurfaceHandle<wgpu::Surface>> {
		let surface = self.context.instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.surface))?;

		let surface_caps = surface.get_capabilities(&self.context.adapter);
		let surface_format = wgpu::TextureFormat::Bgra8Unorm;
		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: surface_format,
			width: 1920,
			height: 1080,
			present_mode: surface_caps.present_modes[0],
			alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
			view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
			desired_maximum_frame_latency: 2,
		};
		surface.configure(&self.context.device, &config);
		Ok(SurfaceHandle {
			surface_id: canvas.surface_id,
			surface,
		})
	}
	#[cfg(not(target_arch = "wasm32"))]
	fn create_surface(&self, window: SurfaceHandle<Self::Window>) -> Result<SurfaceHandle<wgpu::Surface>> {
		let size = window.surface.inner_size();
		let surface = self.context.instance.create_surface(wgpu::SurfaceTarget::Window(Box::new(window.surface)))?;

		let surface_caps = surface.get_capabilities(&self.context.adapter);
		println!("{surface_caps:?}");
		let surface_format = wgpu::TextureFormat::Bgra8Unorm;
		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: surface_format,
			width: size.width,
			height: size.height,
			present_mode: surface_caps.present_modes[0],
			alpha_mode: surface_caps.alpha_modes[0],
			view_formats: vec![],
			desired_maximum_frame_latency: 2,
		};
		surface.configure(&self.context.device, &config);
		self.surface_config.set(Some(config));

		let surface_id = window.surface_id;
		Ok(SurfaceHandle { surface_id, surface })
	}
}

impl WgpuExecutor {
	pub async fn new() -> Option<Self> {
		let context = Context::new().await?;
		println!("wgpu executor created");

		let texture_bind_group_layout = context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: false },
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
					count: None,
				},
			],
			label: Some("texture_bind_group_layout"),
		});

		let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Nearest,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let shader = context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("Shader"),
			source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
		});

		let render_pipeline_layout = context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Render Pipeline Layout"),
			bind_group_layouts: &[&texture_bind_group_layout],
			push_constant_ranges: &[],
		});

		let render_pipeline = context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("Render Pipeline"),
			layout: Some(&render_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: "vs_main",
				buffers: &[Vertex::desc()],
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: "fs_main",
				targets: &[Some(wgpu::ColorTargetState {
					format: wgpu::TextureFormat::Bgra8Unorm,
					blend: Some(wgpu::BlendState {
						color: wgpu::BlendComponent::REPLACE,
						alpha: wgpu::BlendComponent::REPLACE,
					}),
					write_mask: wgpu::ColorWrites::ALL,
				})],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: None,
				// Setting this to anything other than Fill requires Features::POLYGON_MODE_LINE
				// or Features::POLYGON_MODE_POINT
				polygon_mode: wgpu::PolygonMode::Fill,
				// Requires Features::DEPTH_CLIP_CONTROL
				unclipped_depth: false,
				// Requires Features::CONSERVATIVE_RASTERIZATION
				conservative: false,
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			// If the pipeline will be used with a multiview render pass, this
			// indicates how many array layers the attachments will have.
			multiview: None,
		});

		let vertex_buffer = context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Vertex Buffer"),
			contents: bytemuck::cast_slice(VERTICES),
			usage: wgpu::BufferUsages::VERTEX,
		});
		let index_buffer = context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Index Buffer"),
			contents: bytemuck::cast_slice(INDICES),
			usage: wgpu::BufferUsages::INDEX,
		});
		let num_indices = INDICES.len() as u32;
		let render_configuration = RenderConfiguration {
			vertex_buffer,
			index_buffer,
			num_indices,
			render_pipeline,
			texture_bind_group_layout,
			sampler,
		};

		Some(Self {
			context,
			render_configuration,
			surface_config: Cell::new(None),
		})
	}
}

#[derive(Debug)]
struct RenderConfiguration {
	vertex_buffer: wgpu::Buffer,
	index_buffer: wgpu::Buffer,
	num_indices: u32,
	render_pipeline: wgpu::RenderPipeline,
	texture_bind_group_layout: wgpu::BindGroupLayout,
	sampler: wgpu::Sampler,
}
