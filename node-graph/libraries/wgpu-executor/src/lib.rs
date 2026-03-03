mod context;
pub mod shader_runtime;
pub mod texture_conversion;

use crate::shader_runtime::ShaderRuntime;
use anyhow::Result;
use core_types::Color;
use dyn_any::StaticType;
use futures::lock::Mutex;
use glam::UVec2;
use graphene_application_io::{ApplicationIo, EditorApi, SurfaceHandle, SurfaceId};
pub use rendering::RenderContext;
use std::sync::Arc;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use wgpu::util::TextureBlitter;
use wgpu::{Origin3d, TextureAspect};

pub use context::Context as WgpuContext;
pub use context::ContextBuilder as WgpuContextBuilder;
pub use wgpu::Backends as WgpuBackends;
pub use wgpu::Features as WgpuFeatures;

#[derive(dyn_any::DynAny)]
pub struct WgpuExecutor {
	pub context: WgpuContext,
	vello_renderer: Mutex<Renderer>,
	pub shader_runtime: ShaderRuntime,
}

impl std::fmt::Debug for WgpuExecutor {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("WgpuExecutor").field("context", &self.context).finish()
	}
}

impl<'a, T: ApplicationIo<Executor = WgpuExecutor>> From<&'a EditorApi<T>> for &'a WgpuExecutor {
	fn from(editor_api: &'a EditorApi<T>) -> Self {
		editor_api.application_io.as_ref().unwrap().gpu_executor().unwrap()
	}
}

pub type WgpuSurface = Arc<SurfaceHandle<Surface>>;
pub type WgpuWindow = Arc<SurfaceHandle<WindowHandle>>;

pub struct Surface {
	pub inner: wgpu::Surface<'static>,
	pub target_texture: Mutex<Option<TargetTexture>>,
	pub blitter: TextureBlitter,
}

#[derive(Clone, Debug)]
pub struct TargetTexture {
	texture: wgpu::Texture,
	view: wgpu::TextureView,
	size: UVec2,
}

impl TargetTexture {
	/// Creates a new TargetTexture with the specified size.
	pub fn new(device: &wgpu::Device, size: UVec2) -> Self {
		let size = size.max(UVec2::ONE);
		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label: None,
			size: wgpu::Extent3d {
				width: size.x,
				height: size.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
			format: VELLO_SURFACE_FORMAT,
			view_formats: &[],
		});
		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		Self { texture, view, size }
	}

	/// Ensures the texture has the specified size, creating a new one if needed.
	/// This allows reusing the same texture across frames when the size hasn't changed.
	pub fn ensure_size(&mut self, device: &wgpu::Device, size: UVec2) {
		let size = size.max(UVec2::ONE);
		if self.size == size {
			return;
		}

		*self = Self::new(device, size);
	}

	/// Returns a reference to the texture view for rendering.
	pub fn view(&self) -> &wgpu::TextureView {
		&self.view
	}

	/// Returns a reference to the underlying texture.
	pub fn texture(&self) -> &wgpu::Texture {
		&self.texture
	}
}

#[cfg(target_family = "wasm")]
pub type Window = web_sys::HtmlCanvasElement;
#[cfg(not(target_family = "wasm"))]
pub type Window = Arc<dyn winit::window::Window>;

unsafe impl StaticType for Surface {
	type Static = Surface;
}

const VELLO_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl WgpuExecutor {
	pub async fn render_vello_scene_to_texture(&self, scene: &Scene, size: UVec2, context: &RenderContext, background: Option<Color>) -> Result<wgpu::Texture> {
		let mut output = None;
		self.render_vello_scene_to_target_texture(scene, size, context, background, &mut output).await?;
		Ok(output.unwrap().texture)
	}
	pub async fn render_vello_scene_to_target_texture(&self, scene: &Scene, size: UVec2, context: &RenderContext, background: Option<Color>, output: &mut Option<TargetTexture>) -> Result<()> {
		// Initialize (lazily) if this is the first call
		if output.is_none() {
			*output = Some(TargetTexture::new(&self.context.device, size));
		}

		if let Some(target_texture) = output.as_mut() {
			target_texture.ensure_size(&self.context.device, size);

			let [r, g, b, a] = background.unwrap_or(Color::TRANSPARENT).to_rgba8_srgb();
			let render_params = RenderParams {
				base_color: vello::peniko::Color::from_rgba8(r, g, b, a),
				width: size.x,
				height: size.y,
				antialiasing_method: AaConfig::Msaa16,
			};

			{
				let mut renderer = self.vello_renderer.lock().await;
				for (image_brush, texture) in context.resource_overrides.iter() {
					let texture_view = wgpu::TexelCopyTextureInfoBase {
						texture: texture.clone(),
						mip_level: 0,
						origin: Origin3d::ZERO,
						aspect: TextureAspect::All,
					};
					renderer.override_image(&image_brush.image, Some(texture_view));
				}
				renderer.render_to_texture(&self.context.device, &self.context.queue, scene, target_texture.view(), &render_params)?;
				for (image_brush, _) in context.resource_overrides.iter() {
					renderer.override_image(&image_brush.image, None);
				}
			}
		}
		Ok(())
	}

	/// Resample `source_texture` into a new texture of `target_size` using an affine transform.
	/// For each output pixel `p`, the source texel coordinate is `source_transform * p + source_offset`.
	/// `filter` selects interpolation: `Nearest` for sharp pixel boundaries (used by the Pixel Preview render mode),
	/// `Linear` for smooth bilinear interpolation (used by tilted viewport compositing).
	pub fn resample_texture(&self, source_texture: &wgpu::Texture, target_size: UVec2, source_transform: glam::Mat2, source_offset: glam::Vec2, filter: wgpu::FilterMode) -> Result<wgpu::Texture> {
		let device = &self.context.device;
		let queue = &self.context.queue;

		let output_texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("resample_output"),
			size: wgpu::Extent3d {
				width: target_size.x,
				height: target_size.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: VELLO_SURFACE_FORMAT,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});

		// Layout: mat2x2<f32> (4 floats = 16 bytes) + vec2<f32> (2 floats = 8 bytes) = 24 bytes
		let mut params_data = [0_u8; 24];
		params_data[0..4].copy_from_slice(&source_transform.x_axis.x.to_le_bytes());
		params_data[4..8].copy_from_slice(&source_transform.x_axis.y.to_le_bytes());
		params_data[8..12].copy_from_slice(&source_transform.y_axis.x.to_le_bytes());
		params_data[12..16].copy_from_slice(&source_transform.y_axis.y.to_le_bytes());
		params_data[16..20].copy_from_slice(&source_offset.x.to_le_bytes());
		params_data[20..24].copy_from_slice(&source_offset.y.to_le_bytes());
		let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("resample_params"),
			size: 24,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
			mapped_at_creation: false,
		});
		queue.write_buffer(&uniform_buf, 0, &params_data);

		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("resample_sampler"),
			mag_filter: filter,
			min_filter: filter,
			..Default::default()
		});

		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("resample_blit"),
			source: wgpu::ShaderSource::Wgsl(
				r#"
				@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
					var pos = array<vec2<f32>, 3>(vec2(-1., 3.), vec2(-1., -1.), vec2(3., -1.));
					return vec4(pos[vi], 0., 1.);
				}
				@group(0) @binding(0) var src: texture_2d<f32>;
				@group(0) @binding(1) var src_sampler: sampler;
				struct Params { transform: mat2x2<f32>, offset: vec2<f32> }
				@group(0) @binding(2) var<uniform> params: Params;
				@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
					let src_coord = params.transform * pos.xy + params.offset;
					let uv = src_coord / vec2<f32>(textureDimensions(src));
					return textureSample(src, src_sampler, uv);
				}
				"#
				.into(),
			),
		});

		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: None,
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 2,
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

		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("resample_pipeline"),
			layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: None,
				bind_group_layouts: &[&bind_group_layout],
				push_constant_ranges: &[],
			})),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: Some("vs"),
				buffers: &[],
				compilation_options: Default::default(),
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: Some("fs"),
				targets: &[Some(wgpu::ColorTargetState {
					format: VELLO_SURFACE_FORMAT,
					blend: None,
					write_mask: wgpu::ColorWrites::ALL,
				})],
				compilation_options: Default::default(),
			}),
			primitive: wgpu::PrimitiveState::default(),
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview: None,
			cache: None,
		});

		let src_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());
		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout: &bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&src_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&sampler),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: uniform_buf.as_entire_binding(),
				},
			],
		});

		let out_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("resample_blit") });
		{
			let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: None,
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &out_view,
					resolve_target: None,
					depth_slice: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: None,
				timestamp_writes: None,
				occlusion_query_set: None,
			});
			pass.set_pipeline(&pipeline);
			pass.set_bind_group(0, &bind_group, &[]);
			pass.draw(0..3, 0..1);
		}
		queue.submit([encoder.finish()]);

		Ok(output_texture)
	}

	#[cfg(target_family = "wasm")]
	pub fn create_surface(&self, canvas: graphene_application_io::WasmSurfaceHandle) -> Result<SurfaceHandle<Surface>> {
		let surface = self.context.instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.surface))?;
		self.create_surface_inner(surface, canvas.window_id)
	}

	#[cfg(not(target_family = "wasm"))]
	pub fn create_surface(&self, window: SurfaceHandle<Window>) -> Result<SurfaceHandle<Surface>> {
		let surface = self.context.instance.create_surface(wgpu::SurfaceTarget::Window(Box::new(window.surface)))?;
		self.create_surface_inner(surface, window.window_id)
	}

	pub fn create_surface_inner(&self, surface: wgpu::Surface<'static>, window_id: SurfaceId) -> Result<SurfaceHandle<Surface>> {
		let blitter = TextureBlitter::new(&self.context.device, VELLO_SURFACE_FORMAT);
		Ok(SurfaceHandle {
			window_id,
			surface: Surface {
				inner: surface,
				target_texture: Mutex::new(None),
				blitter,
			},
		})
	}
}

impl WgpuExecutor {
	pub async fn new() -> Option<Self> {
		Self::with_context(WgpuContext::new().await?)
	}

	pub fn with_context(context: WgpuContext) -> Option<Self> {
		let vello_renderer = Renderer::new(
			&context.device,
			RendererOptions {
				pipeline_cache: None,
				use_cpu: false,
				antialiasing_support: AaSupport::all(),
				num_init_threads: std::num::NonZeroUsize::new(1),
			},
		)
		.map_err(|e| anyhow::anyhow!("Failed to create Vello renderer: {:?}", e))
		.ok()?;

		Some(Self {
			shader_runtime: ShaderRuntime::new(&context),
			context,
			vello_renderer: vello_renderer.into(),
		})
	}
}

pub type WindowHandle = Arc<SurfaceHandle<Window>>;
