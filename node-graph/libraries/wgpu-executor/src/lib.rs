mod context;
mod pipeline;
pub mod shader_runtime;
mod texture_cache;
pub mod texture_conversion;

use std::sync::Arc;

use crate::shader_runtime::ShaderRuntime;
use crate::texture_cache::TextureCache;
use anyhow::Result;
use core_types::Color;
use core_types::color::SRGBA8;
use futures::lock::Mutex;
use glam::UVec2;
use graphene_application_io::{ApplicationIo, EditorApi};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use wgpu::{Origin3d, TextureAspect};

pub use context::Context as WgpuContext;
pub use context::ContextBuilder as WgpuContextBuilder;
pub use pipeline::AsyncPipeline as AsyncWgpuPipeline;
pub use pipeline::Pipeline as WgpuPipeline;
pub use pipeline::PipelineCache as WgpuPipelineCache;
pub use rendering::RenderContext;
pub use wgpu::Backends as WgpuBackends;
pub use wgpu::Features as WgpuFeatures;

const TEXTURE_CACHE_SIZE: u64 = 256 * 1024 * 1024; // 256 MiB

#[derive(dyn_any::DynAny, Clone)]
pub struct WgpuExecutor {
	inner: Arc<WgpuExecutorInner>,
}

impl WgpuExecutor {
	pub fn context(&self) -> &WgpuContext {
		&self.inner.context
	}

	pub fn shader_runtime(&self) -> &ShaderRuntime {
		&self.inner.shader_runtime
	}
}

#[derive(dyn_any::DynAny)]
pub struct WgpuExecutorInner {
	context: WgpuContext,
	texture_cache: Mutex<TextureCache>,
	vello_renderer: Mutex<Renderer>,
	shader_runtime: ShaderRuntime,
}

impl std::fmt::Debug for WgpuExecutor {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("WgpuExecutor").field("context", &self.context()).finish()
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

		let SRGBA8 { red, green, blue, alpha } = background.unwrap_or(Color::TRANSPARENT).into();
		let render_params = RenderParams {
			base_color: vello::peniko::Color::from_rgba8(red, green, blue, alpha),
			width: size.x,
			height: size.y,
			antialiasing_method: AaConfig::Msaa16,
		};

		{
			let mut renderer = self.inner.vello_renderer.lock().await;
			for (image_brush, texture) in context.resource_overrides.iter() {
				let texture_view = wgpu::TexelCopyTextureInfoBase {
					texture: texture.clone(),
					mip_level: 0,
					origin: Origin3d::ZERO,
					aspect: TextureAspect::All,
				};
				renderer.override_image(&image_brush.image, Some(texture_view));
			}
			renderer.render_to_texture(&self.context().device, &self.context().queue, scene, &texture_view, &render_params)?;
			for (image_brush, _) in context.resource_overrides.iter() {
				renderer.override_image(&image_brush.image, None);
			}
		}

		Ok(texture)
	}

	pub fn pipeline_init<P: WgpuPipeline>(&self, pipeline: &WgpuPipelineCache) {
		pipeline.init::<P>(self);
	}

	pub async fn request_texture(&self, size: UVec2) -> Arc<wgpu::Texture> {
		self.inner.texture_cache.lock().await.request_texture(&self.context().device, size)
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

		let shader_runtime = ShaderRuntime::new(&context);

		Some(Self {
			inner: Arc::new(WgpuExecutorInner {
				context,
				texture_cache: texture_cache.into(),
				vello_renderer: vello_renderer.into(),
				shader_runtime,
			}),
		})
	}
}
