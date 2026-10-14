mod buffer;
mod context;
mod pipeline;
pub mod shader_runtime;
mod texture_cache;
pub mod texture_conversion;
mod vello_render;

use crate::shader_runtime::ShaderRuntime;
use crate::texture_cache::TextureCache;
use crate::vello_render::VelloRenderer;
use anyhow::Result;
use glam::{DAffine2, UVec2};
use graphene_application_io::{ApplicationIo, EditorApi};
use std::sync::Arc;
use std::sync::Mutex;
use wgpu::util::DeviceExt;

pub use buffer::Buffer;
pub use context::Context as WgpuContext;
pub use context::ContextBuilder as WgpuContextBuilder;
pub use pipeline::Pipeline as WgpuPipeline;
pub use pipeline::PipelineCache as WgpuPipelineCache;
pub use raster_types::Texture;
pub use rendering::scene::Scene;
pub use wgpu::Backends as WgpuBackends;
pub use wgpu::Features as WgpuFeatures;
pub use wgpu_sync::CurrentSurfaceTexture as WgpuCurrentSurfaceTexture;
pub use wgpu_sync::Instance as WgpuInstance;
pub use wgpu_sync::Queue as WgpuQueue;
pub use wgpu_sync::Surface as WgpuSurface;

#[cfg(not(target_family = "wasm"))]
const TEXTURE_CACHE_SIZE: u64 = 1024 * 1024 * 1024; // 1GB
#[cfg(target_family = "wasm")]
const TEXTURE_CACHE_SIZE: u64 = 512 * 1024 * 1024; // 512MB

#[derive(dyn_any::DynAny, Clone)]
pub struct WgpuExecutor {
	inner: Arc<WgpuExecutorInner>,
}

impl WgpuExecutor {
	pub fn context(&self) -> &WgpuContext {
		&self.inner.context
	}
}

#[derive(dyn_any::DynAny)]
pub struct WgpuExecutorInner {
	context: WgpuContext,
	texture_cache: std::sync::Mutex<TextureCache>,
	vello_renderer: Mutex<VelloRenderer>,
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
	/// Renders a recorded scene, placed by `transform`, into a new texture of `size` with straight alpha.
	pub fn render_vello_scene(&self, scene: &Scene, transform: DAffine2, size: UVec2) -> Result<Texture> {
		let texture = self.request_texture(size);
		let transform = vello_gpu::kurbo::Affine::new(transform.to_cols_array());

		let mut renderer = self.inner.vello_renderer.lock().unwrap();
		let queue = self.context().queue.lock();
		renderer.render(&self.context().device, &queue, scene, transform, size, &texture)?;

		Ok(texture)
	}

	pub fn pipeline_init<P: WgpuPipeline>(&self, pipeline: &WgpuPipelineCache) {
		pipeline.init::<P>(self);
	}

	pub fn request_texture(&self, size: UVec2) -> Texture {
		self.request_texture_with_format(size, wgpu::TextureFormat::Rgba8Unorm)
	}

	pub fn request_texture_with_format(&self, size: UVec2, format: wgpu::TextureFormat) -> Texture {
		self.inner.texture_cache.lock().unwrap().request_texture(&self.context().device, size, format)
	}

	pub fn create_buffer(&self, desc: &wgpu::BufferDescriptor) -> Buffer {
		self.context().device.create_buffer(desc).into()
	}

	pub fn create_buffer_init(&self, desc: &wgpu::util::BufferInitDescriptor) -> Buffer {
		self.context().device.create_buffer_init(desc).into()
	}
}

impl WgpuExecutor {
	pub async fn new() -> Option<Self> {
		Self::with_context(WgpuContext::new().await?)
	}

	pub fn with_context(context: WgpuContext) -> Option<Self> {
		let vello_renderer = VelloRenderer::new(&context.device);

		let texture_cache = TextureCache::new(TEXTURE_CACHE_SIZE);

		let shader_runtime = ShaderRuntime::default();

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
