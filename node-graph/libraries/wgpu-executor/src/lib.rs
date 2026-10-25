mod buffer;
mod context;
mod pipeline;
mod texture_cache;
pub mod texture_conversion;
mod vello_effects;
mod vello_generators;
mod vello_render;

use crate::texture_cache::TextureCache;
use crate::vello_render::VelloRenderer;
pub use crate::vello_render::{MAX_LAYER_RENDER_SIZE, VelloCanvas};
use anyhow::Result;
use glam::{DAffine2, UVec2};
use graphene_application_io::{ApplicationIo, EditorApi};
use std::sync::Arc;
use std::sync::{Mutex, PoisonError};
use wgpu::util::DeviceExt;

pub use buffer::Buffer;
pub use context::Context as WgpuContext;
pub use context::ContextBuilder as WgpuContextBuilder;
pub use pipeline::Pipeline as WgpuPipeline;
pub use pipeline::PipelineCache as WgpuPipelineCache;
pub use pipeline::PipelineError as WgpuPipelineError;
pub use raster_types::Texture;
pub use rendering::vello_scene_builder::VelloSceneBuilder;
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
	vello_renderer: VelloRenderer,
	/// The canvas the node graph renders documents on.
	document_canvas: Mutex<VelloCanvas>,
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
	/// Renders the document scene `draw` draws, with its content placed by `view`, into a new texture of `size` with straight alpha.
	/// A render with more content than Vello GPU can hold calls `draw` again for each part it splits into.
	pub fn render_vello(&self, size: UVec2, view: DAffine2, draw: &mut dyn FnMut(&mut VelloSceneBuilder)) -> Result<Texture> {
		let limit = self.context().device.limits().max_texture_dimension_2d;
		if size.x > limit || size.y > limit {
			anyhow::bail!("Render size {size} exceeds the GPU's texture size limit of {limit}");
		}

		let texture = self.request_texture(size);
		let view = vello_gpu::kurbo::Affine::new(view.to_cols_array());

		let mut canvas = self.inner.document_canvas.lock().unwrap_or_else(PoisonError::into_inner);
		self.inner
			.vello_renderer
			.render(&mut canvas, &self.context().device, &self.context().queue, draw, view, size, &texture)?;

		Ok(texture)
	}

	/// Renders a scene drawn beforehand, on a canvas the caller owns, into a new texture of the scene's size with straight alpha.
	pub fn render_vello_scene_on_canvas(&self, canvas: &mut VelloCanvas, scene: VelloSceneBuilder) -> Result<Texture> {
		let texture = self.request_texture(UVec2::new(scene.width() as u32, scene.height() as u32));

		self.inner.vello_renderer.render_scene(canvas, &self.context().device, &self.context().queue, scene, &texture);

		Ok(texture)
	}

	/// Reads an `Rgba8Unorm` texture's pixels back to the CPU, as tightly packed rows, waiting on the GPU.
	/// Fails in a browser, which hands over the pixels only once control returns to it.
	pub fn read_texture_pixels(&self, texture: &wgpu::Texture) -> Result<Vec<u8>> {
		let device = &self.context().device;
		let (width, height) = (texture.width(), texture.height());
		let row_bytes = width * 4;
		let padded_row_bytes = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

		let buffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("texture_readback_buffer"),
			size: padded_row_bytes as u64 * height as u64,
			usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
			mapped_at_creation: false,
		});
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("texture_readback") });
		encoder.copy_texture_to_buffer(
			texture.as_image_copy(),
			wgpu::TexelCopyBufferInfo {
				buffer: &buffer,
				layout: wgpu::TexelCopyBufferLayout {
					offset: 0,
					bytes_per_row: Some(padded_row_bytes),
					rows_per_image: Some(height),
				},
			},
			texture.size(),
		);
		self.context().queue.lock().submit([encoder.finish()]);

		let (sender, mut receiver) = futures::channel::oneshot::channel();
		buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
			let _ = sender.send(result);
		});
		let _ = device.poll(wgpu::wgt::PollType::wait_indefinitely());
		let Ok(Some(mapping)) = receiver.try_recv() else {
			anyhow::bail!("The GPU didn't map the texture's pixels for reading");
		};
		mapping?;

		let mapped = buffer.slice(..).get_mapped_range()?;
		let pixels = mapped.chunks_exact(padded_row_bytes as usize).flat_map(|row| &row[..row_bytes as usize]).copied().collect();

		Ok(pixels)
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
	#[cfg(not(target_family = "wasm"))]
	pub async fn new() -> Option<Self> {
		Self::with_context(WgpuContext::new().await?)
	}

	pub fn with_context(context: WgpuContext) -> Option<Self> {
		let vello_renderer = VelloRenderer::new(&context.device, &context.adapter);

		let texture_cache = TextureCache::new(TEXTURE_CACHE_SIZE);

		Some(Self {
			inner: Arc::new(WgpuExecutorInner {
				context,
				texture_cache: texture_cache.into(),
				vello_renderer,
				document_canvas: Mutex::default(),
			}),
		})
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("No GPU is available")]
pub struct NoGpuError;
