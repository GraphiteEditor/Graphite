mod background; // TODO: Think about where to place this. Likely inlined in the node. Requires refactor of wgpu pipline usage.
mod context;
mod resample;
pub mod shader_runtime;
mod texture_cache;
pub mod texture_conversion;

use std::sync::Arc;

use crate::background::BackgroundCompositor;
use crate::resample::Resampler;
use crate::shader_runtime::ShaderRuntime;
use crate::texture_cache::TextureCache;
use anyhow::Result;
use core_types::Color;
use futures::lock::Mutex;
use glam::{Affine2, UVec2};
use graphene_application_io::{ApplicationIo, EditorApi};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use wgpu::{Origin3d, TextureAspect};

pub use context::Context as WgpuContext;
pub use context::ContextBuilder as WgpuContextBuilder;
pub use rendering::RenderContext;
pub use wgpu::Backends as WgpuBackends;
pub use wgpu::Features as WgpuFeatures;

const TEXTURE_CACHE_SIZE: u64 = 256 * 1024 * 1024; // 256 MiB

#[derive(dyn_any::DynAny)]
pub struct WgpuExecutor {
	pub context: WgpuContext,
	texture_cache: Mutex<TextureCache>,
	vello_renderer: Mutex<Renderer>,
	resampler: Resampler,
	background_compositor: BackgroundCompositor,
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

impl WgpuExecutor {
	pub async fn render_vello_scene(&self, scene: &Scene, size: UVec2, context: &RenderContext, background: Option<Color>) -> Result<Arc<wgpu::Texture>> {
		let texture = self.request_texture(size).await;

		let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		let [r, g, b, a] = background.unwrap_or(Color::TRANSPARENT).to_rgba8();
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
			renderer.render_to_texture(&self.context.device, &self.context.queue, scene, &texture_view, &render_params)?;
			for (image_brush, _) in context.resource_overrides.iter() {
				renderer.override_image(&image_brush.image, None);
			}
		}

		Ok(texture)
	}

	pub async fn resample_texture(&self, source: &wgpu::Texture, size: UVec2, transform: &glam::DAffine2) -> Arc<wgpu::Texture> {
		let out = self.request_texture(size).await;
		self.resampler.resample(&self.context, source, transform, &out);
		out
	}

	pub async fn composite_background(&self, foreground: &wgpu::Texture, backgrounds: &[rendering::Background], document_to_screen: Affine2, zoom: f32) -> Arc<wgpu::Texture> {
		let size = foreground.size();
		let output = self.request_texture(UVec2::new(size.width, size.height)).await;
		self.background_compositor.composite(&self.context, foreground, &output, backgrounds, document_to_screen, zoom);
		output
	}

	pub async fn request_texture(&self, size: UVec2) -> Arc<wgpu::Texture> {
		self.texture_cache.lock().await.request_texture(&self.context.device, size)
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

		let texture_cache = TextureCache::new(TEXTURE_CACHE_SIZE);

		let resampler = Resampler::new(&context.device);
		let background_compositor = BackgroundCompositor::new(&context.device);
		let shader_runtime = ShaderRuntime::new(&context);

		Some(Self {
			context,
			texture_cache: texture_cache.into(),
			vello_renderer: vello_renderer.into(),
			resampler,
			background_compositor,
			shader_runtime,
		})
	}
}
