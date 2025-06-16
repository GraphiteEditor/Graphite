mod context;
mod executor;

use anyhow::Result;
pub use context::Context;
use dyn_any::{DynAny, StaticType};
pub use executor::GpuExecutor;
use glam::UVec2;
use graphene_core::application_io::{ApplicationIo, EditorApi, SurfaceHandle};
use graphene_core::{Color, Ctx, Node};
use std::sync::Arc;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use wgpu::{Buffer, Origin3d, ShaderModule, SurfaceConfiguration, Texture, TextureAspect, TextureView};

#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;

#[derive(dyn_any::DynAny)]
pub struct WgpuExecutor {
	pub context: Context,
	vello_renderer: futures::lock::Mutex<vello::Renderer>,
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

impl graphene_core::application_io::Size for Surface {
	fn size(&self) -> UVec2 {
		self.resolution
	}
}

#[derive(Debug, DynAny)]
#[repr(transparent)]
pub struct CommandBuffer(wgpu::CommandBuffer);

#[derive(Debug, DynAny)]
#[repr(transparent)]
pub struct ShaderModuleWrapper(ShaderModule);
pub type ShaderHandle = ShaderModuleWrapper;
pub type BufferHandle = Buffer;
pub type TextureHandle = Texture;
pub struct Surface {
	pub inner: wgpu::Surface<'static>,
	resolution: UVec2,
}
#[cfg(target_arch = "wasm32")]
pub type Window = HtmlCanvasElement;
#[cfg(not(target_arch = "wasm32"))]
pub type Window = Arc<winit::window::Window>;

unsafe impl StaticType for Surface {
	type Static = Surface;
}

pub use graphene_core::renderer::RenderContext;

// pub trait SpirVCompiler {
// 	fn compile(&self, network: &[ProtoNetwork], io: &ShaderIO) -> Result<Shader>;
// }

impl WgpuExecutor {
	pub async fn render_vello_scene(&self, scene: &Scene, surface: &WgpuSurface, width: u32, height: u32, context: &RenderContext, background: Color) -> Result<()> {
		let surface = &surface.surface.inner;
		let surface_caps = surface.get_capabilities(&self.context.adapter);
		surface.configure(
			&self.context.device,
			&SurfaceConfiguration {
				usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::STORAGE_BINDING,
				format: wgpu::TextureFormat::Rgba8Unorm,
				width,
				height,
				present_mode: surface_caps.present_modes[0],
				alpha_mode: wgpu::CompositeAlphaMode::Opaque,
				view_formats: vec![],
				desired_maximum_frame_latency: 2,
			},
		);
		let surface_texture = surface.get_current_texture()?;

		let [r, g, b, _] = background.to_rgba8_srgb();
		let render_params = RenderParams {
			// We are using an explicit opaque color here to eliminate the alpha premultiplication step
			// which would be required to support a transparent webgpu canvas
			base_color: vello::peniko::Color::from_rgba8(r, g, b, 0xff),
			width,
			height,
			antialiasing_method: AaConfig::Msaa16,
		};

		{
			let mut renderer = self.vello_renderer.lock().await;
			for (id, texture) in context.resource_overrides.iter() {
				let texture_view = wgpu::ImageCopyTextureBase {
					texture: texture.clone(),
					mip_level: 0,
					origin: Origin3d::ZERO,
					aspect: TextureAspect::All,
				};
				renderer.override_image(
					&vello::peniko::Image::new(vello::peniko::Blob::from_raw_parts(Arc::new(vec![]), *id), vello::peniko::Format::Rgba8, 0, 0),
					Some(texture_view),
				);
			}
			renderer.render_to_surface(&self.context.device, &self.context.queue, scene, &surface_texture, &render_params).unwrap();
		}

		surface_texture.present();

		Ok(())
	}

	#[cfg(target_arch = "wasm32")]
	pub fn create_surface(&self, canvas: graphene_core::WasmSurfaceHandle) -> Result<SurfaceHandle<Surface>> {
		let surface = self.context.instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.surface))?;

		Ok(SurfaceHandle {
			window_id: canvas.window_id,
			surface: Surface {
				inner: surface,
				resolution: UVec2::ZERO,
			},
		})
	}
	#[cfg(not(target_arch = "wasm32"))]
	pub fn create_surface(&self, window: SurfaceHandle<Window>) -> Result<SurfaceHandle<Surface>> {
		let size = window.surface.inner_size();
		let resolution = UVec2::new(size.width, size.height);
		let surface = self.context.instance.create_surface(wgpu::SurfaceTarget::Window(Box::new(window.surface)))?;

		Ok(SurfaceHandle {
			window_id: window.window_id,
			surface: Surface { inner: surface, resolution },
		})
	}
}

impl WgpuExecutor {
	pub async fn new() -> Option<Self> {
		let context = Context::new().await?;

		let vello_renderer = Renderer::new(
			&context.device,
			RendererOptions {
				surface_format: Some(wgpu::TextureFormat::Rgba8Unorm),
				use_cpu: false,
				antialiasing_support: AaSupport::all(),
				num_init_threads: std::num::NonZeroUsize::new(1),
			},
		)
		.map_err(|e| anyhow::anyhow!("Failed to create Vello renderer: {:?}", e))
		.ok()?;

		Some(Self {
			context,
			vello_renderer: vello_renderer.into(),
		})
	}
}

pub enum BindingType<'a> {
	UniformBuffer(&'a BufferHandle),
	StorageBuffer(&'a BufferHandle),
	TextureView(&'a TextureView),
}

/// Extracts arguments from the function arguments and wraps them in a node.
pub struct ShaderInputNode<T> {
	data: T,
}

impl<'i, T: 'i> Node<'i, ()> for ShaderInputNode<T> {
	type Output = &'i T;

	fn eval(&'i self, _: ()) -> Self::Output {
		&self.data
	}
}

impl<T> ShaderInputNode<T> {
	pub fn new(data: T) -> Self {
		Self { data }
	}
}

pub type WindowHandle = Arc<SurfaceHandle<Window>>;

#[node_macro::node(skip_impl)]
fn create_gpu_surface<'a: 'n, Io: ApplicationIo<Executor = WgpuExecutor, Surface = Window> + 'a + Send + Sync>(_: impl Ctx + 'a, editor_api: &'a EditorApi<Io>) -> Option<WgpuSurface> {
	let canvas = editor_api.application_io.as_ref()?.window()?;
	let executor = editor_api.application_io.as_ref()?.gpu_executor()?;
	Some(Arc::new(executor.create_surface(canvas).ok()?))
}

// #[node_macro::node(category(""))]
// async fn upload_texture<'a: 'n>(_: impl ExtractFootprint + Ctx, input: RasterDataTable<Color>, executor: &'a WgpuExecutor) -> TextureFrameTable {
// 	let mut result_table = TextureFrameTable::default();

// 	for instance in input.instance_ref_iter() {
// 		let image = instance.instance;
// 		let new_data: Vec<SRGBA8> = image.data.iter().map(|x| (*x).into()).collect();
// 		let new_image = Image {
// 			width: image.width,
// 			height: image.height,
// 			data: new_data,
// 			base64_string: None,
// 		};

// 		let shader_input = executor.create_texture_buffer(new_image, TextureBufferOptions::Texture).unwrap();
// 		let texture = match shader_input {
// 			ShaderInput::TextureBuffer(buffer, _) => buffer,
// 			ShaderInput::StorageTextureBuffer(buffer, _) => buffer,
// 			_ => unreachable!("Unsupported ShaderInput type"),
// 		};

// 		result_table.push(Instance {
// 			instance: ImageTexture { texture: texture.into() },
// 			transform: *instance.transform,
// 			alpha_blending: *instance.alpha_blending,
// 			source_node_id: *instance.source_node_id,
// 		});
// 	}

// 	result_table
// }
