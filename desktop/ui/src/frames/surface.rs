use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct FrameSurface {
	device: wgpu::Device,
	queue: wgpu_sync::Queue,
	slot: Arc<Mutex<Option<wgpu::Texture>>>,
}

impl FrameSurface {
	pub(crate) fn new(device: wgpu::Device, queue: wgpu_sync::Queue) -> Self {
		Self {
			device,
			queue,
			slot: Arc::new(Mutex::new(None)),
		}
	}

	pub(crate) fn upload_buffer(&self, buffer: &[u8], width: u32, height: u32) -> Option<wgpu::Texture> {
		debug_assert_eq!(buffer.len(), width as usize * height as usize * 4);

		let Ok(mut slot) = self.slot.lock() else {
			tracing::error!("Failed to lock the frame surface");
			return None;
		};

		if slot.as_ref().is_none_or(|texture| texture.width() != width || texture.height() != height) {
			*slot = Some(self.device.create_texture(&wgpu::TextureDescriptor {
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
		let texture = slot.as_ref()?;

		self.queue.write_texture(
			wgpu::TexelCopyTextureInfo {
				texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			buffer,
			wgpu::TexelCopyBufferLayout {
				offset: 0,
				bytes_per_row: Some(4 * width),
				rows_per_image: None,
			},
			wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
		);

		Some(texture.clone())
	}

	#[cfg(feature = "accelerated_paint")]
	pub(crate) fn import_texture(&self, importer: impl crate::frames::import::TextureImporter) -> Option<wgpu::Texture> {
		let imported = match importer.import_to_wgpu(&self.device) {
			Ok(texture) => texture,
			Err(e) => {
				tracing::error!("Failed to import remote accelerated frame: {e}");
				return None;
			}
		};

		let size = wgpu::Extent3d {
			width: imported.width(),
			height: imported.height(),
			depth_or_array_layers: 1,
		};
		let owned = self.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("CEF Imported Frame Copy"),
			size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: imported.format(),
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});

		let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("CEF Frame Copy Encoder"),
		});
		encoder.copy_texture_to_texture(
			wgpu::TexelCopyTextureInfo {
				texture: &imported,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::TexelCopyTextureInfo {
				texture: &owned,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			size,
		);

		let submission = self.queue.submit(std::iter::once(encoder.finish()));

		// Block until the copy is done, so the caller's ack cannot free CEF to
		// overwrite the shared texture while the copy is still reading it.
		let _ = self.device.poll(wgpu::PollType::Wait {
			submission_index: Some(submission),
			timeout: None,
		});

		let Ok(mut slot) = self.slot.lock() else {
			tracing::error!("Failed to lock the frame surface");
			return None;
		};
		*slot = Some(owned.clone());
		Some(owned)
	}
}
