use cef::Rect;
use std::sync::{Arc, Mutex};

use crate::wrapper::WgpuContext;

#[derive(Clone)]
pub(crate) struct View {
	context: WgpuContext,
	texture: Arc<Mutex<Option<wgpu::Texture>>>,
}

impl View {
	pub(crate) fn new(context: WgpuContext) -> Self {
		Self {
			context,
			texture: Arc::new(Mutex::new(None)),
		}
	}

	pub(crate) fn texture(&self) -> Option<wgpu::Texture> {
		let Ok(texture) = self.texture.lock() else {
			tracing::error!("Failed to lock view texture");
			return None;
		};
		texture.clone()
	}

	pub(super) fn upload_frame_buffer(&self, buffer: &[u8], width: u32, height: u32, dirty_rects: &[Rect]) {
		debug_assert_eq!(buffer.len(), width as usize * height as usize * 4);

		let Ok(mut slot) = self.texture.lock() else {
			tracing::error!("Failed to lock view texture");
			return;
		};

		let needs_new_texture = slot.as_ref().is_none_or(|texture| texture.width() != width || texture.height() != height);
		if needs_new_texture {
			*slot = Some(self.context.device.create_texture(&wgpu::TextureDescriptor {
				label: Some("CEF Texture"),
				size: wgpu::Extent3d {
					width,
					height,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Bgra8Unorm,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
				view_formats: &[],
			}));
		}
		let texture = slot.as_ref().expect("Texture was just created");

		let full_frame = [Rect {
			x: 0,
			y: 0,
			width: width as i32,
			height: height as i32,
		}];
		let rects = if needs_new_texture || dirty_rects.is_empty() { &full_frame } else { dirty_rects };

		for rect in rects {
			let x = (rect.x.max(0) as u32).min(width);
			let y = (rect.y.max(0) as u32).min(height);
			let rect_width = (rect.width.max(0) as u32).min(width - x);
			let rect_height = (rect.height.max(0) as u32).min(height - y);
			if rect_width == 0 || rect_height == 0 {
				continue;
			}
			self.context.queue.write_texture(
				wgpu::TexelCopyTextureInfo {
					texture,
					mip_level: 0,
					origin: wgpu::Origin3d { x, y, z: 0 },
					aspect: wgpu::TextureAspect::All,
				},
				buffer,
				wgpu::TexelCopyBufferLayout {
					offset: 4 * (y as u64 * width as u64 + x as u64),
					bytes_per_row: Some(4 * width),
					rows_per_image: None,
				},
				wgpu::Extent3d {
					width: rect_width,
					height: rect_height,
					depth_or_array_layers: 1,
				},
			);
		}
	}

	#[cfg(feature = "accelerated_paint")]
	pub(super) fn import_shared_texture(&self, info: &cef::AcceleratedPaintInfo) {
		let texture = match cef::osr_texture_import::SharedTextureHandle::new(info).import_texture(&self.context.device) {
			Ok(texture) => texture,
			Err(e) => {
				tracing::error!("Failed to import shared texture: {e}");
				return;
			}
		};
		let Ok(mut slot) = self.texture.lock() else {
			tracing::error!("Failed to lock view texture");
			return;
		};
		*slot = Some(texture);
	}
}
