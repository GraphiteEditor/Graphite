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
