mod context;

use anyhow::Result;
pub use context::Context;
use dyn_any::StaticType;
use futures::lock::Mutex;
use glam::UVec2;
use graphene_application_io::{ApplicationIo, EditorApi, SurfaceHandle, SurfaceId};
use graphene_core::{Color, Ctx};
pub use graphene_svg_renderer::RenderContext;
use std::sync::Arc;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use wgpu::util::TextureBlitter;
use wgpu::{Origin3d, SurfaceConfiguration, TextureAspect};

#[derive(dyn_any::DynAny)]
pub struct WgpuExecutor {
	pub context: Context,
	vello_renderer: Mutex<Renderer>,
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

pub struct TargetTexture {
	view: wgpu::TextureView,
	size: UVec2,
}

#[cfg(target_arch = "wasm32")]
pub type Window = web_sys::HtmlCanvasElement;
#[cfg(not(target_arch = "wasm32"))]
pub type Window = Arc<winit::window::Window>;

unsafe impl StaticType for Surface {
	type Static = Surface;
}

const VELLO_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl WgpuExecutor {
	pub async fn render_vello_scene(&self, scene: &Scene, surface: &WgpuSurface, size: UVec2, context: &RenderContext, background: Color) -> Result<()> {
		let mut guard = surface.surface.target_texture.lock().await;
		let target_texture = if let Some(target_texture) = &*guard
			&& target_texture.size == size
		{
			target_texture
		} else {
			let texture = self.context.device.create_texture(&wgpu::TextureDescriptor {
				label: None,
				size: wgpu::Extent3d {
					width: size.x,
					height: size.y,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
				format: VELLO_SURFACE_FORMAT,
				view_formats: &[],
			});
			let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
			*guard = Some(TargetTexture { size, view });
			guard.as_ref().unwrap()
		};

		let surface_inner = &surface.surface.inner;
		let surface_caps = surface_inner.get_capabilities(&self.context.adapter);
		surface_inner.configure(
			&self.context.device,
			&SurfaceConfiguration {
				usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::STORAGE_BINDING,
				format: VELLO_SURFACE_FORMAT,
				width: size.x,
				height: size.y,
				present_mode: surface_caps.present_modes[0],
				alpha_mode: wgpu::CompositeAlphaMode::Opaque,
				view_formats: vec![],
				desired_maximum_frame_latency: 2,
			},
		);

		let [r, g, b, _] = background.to_rgba8_srgb();
		let render_params = RenderParams {
			// We are using an explicit opaque color here to eliminate the alpha premultiplication step
			// which would be required to support a transparent webgpu canvas
			base_color: vello::peniko::Color::from_rgba8(r, g, b, 0xff),
			width: size.x,
			height: size.y,
			antialiasing_method: AaConfig::Msaa16,
		};

		{
			let mut renderer = self.vello_renderer.lock().await;
			for (id, texture) in context.resource_overrides.iter() {
				let texture = texture.clone();
				let texture_view = wgpu::TexelCopyTextureInfoBase {
					texture,
					mip_level: 0,
					origin: Origin3d::ZERO,
					aspect: TextureAspect::All,
				};
				renderer.override_image(
					&vello::peniko::Image::new(vello::peniko::Blob::from_raw_parts(Arc::new(vec![]), *id), vello::peniko::ImageFormat::Rgba8, 0, 0),
					Some(texture_view),
				);
			}
			renderer.render_to_texture(&self.context.device, &self.context.queue, scene, &target_texture.view, &render_params)?;
		}

		let surface_texture = surface_inner.get_current_texture()?;
		let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Surface Blit") });
		surface.surface.blitter.copy(
			&self.context.device,
			&mut encoder,
			&target_texture.view,
			&surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default()),
		);
		self.context.queue.submit([encoder.finish()]);
		surface_texture.present();

		Ok(())
	}

	#[cfg(target_arch = "wasm32")]
	pub fn create_surface(&self, canvas: graphene_application_io::WasmSurfaceHandle) -> Result<SurfaceHandle<Surface>> {
		let surface = self.context.instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.surface))?;
		self.create_surface_inner(surface, canvas.window_id)
	}
	#[cfg(not(target_arch = "wasm32"))]
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
		let context = Context::new().await?;

		let vello_renderer = Renderer::new(
			&context.device,
			RendererOptions {
				// surface_format: Some(wgpu::TextureFormat::Rgba8Unorm),
				pipeline_cache: None,
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

pub type WindowHandle = Arc<SurfaceHandle<Window>>;

#[node_macro::node(skip_impl)]
fn create_gpu_surface<'a: 'n, Io: ApplicationIo<Executor = WgpuExecutor, Surface = Window> + 'a + Send + Sync>(_: impl Ctx + 'a, editor_api: &'a EditorApi<Io>) -> Option<WgpuSurface> {
	let canvas = editor_api.application_io.as_ref()?.window()?;
	let executor = editor_api.application_io.as_ref()?.gpu_executor()?;
	Some(Arc::new(executor.create_surface(canvas).ok()?))
}
