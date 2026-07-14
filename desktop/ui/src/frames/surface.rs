use std::sync::{Arc, Mutex};

#[cfg(feature = "accelerated_paint")]
use super::import::ContentMapping;
#[cfg(feature = "accelerated_paint")]
use super::resample::Resampler;

#[derive(Clone)]
pub(crate) struct FrameSurface {
	device: wgpu::Device,
	queue: wgpu_sync::Queue,
	slot: Arc<Mutex<Option<wgpu::Texture>>>,
	#[cfg(feature = "accelerated_paint")]
	resampler: Resampler,
}

impl FrameSurface {
	pub(crate) fn new(device: wgpu::Device, queue: wgpu_sync::Queue) -> Self {
		Self {
			#[cfg(feature = "accelerated_paint")]
			resampler: Resampler::new(device.clone()),
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

		if buffer.chunks_exact(4).take(width as usize).all(|pixel| pixel[3] == 0) {
			tracing::debug!("Skipping fully transparent frame");
			return None;
		}

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
	pub(crate) fn import_texture(&self, importer: impl crate::frames::import::TextureImporter, content_rect: crate::frames::import::ContentRect) -> Option<wgpu::Texture> {
		let imported = match importer.import_to_wgpu(&self.device) {
			Ok(texture) => texture,
			Err(e) => {
				tracing::error!("Failed to import remote accelerated frame: {e}");
				return None;
			}
		};

		let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("CEF Frame Copy Encoder"),
		});
		let output = match content_rect.mapping(imported.width(), imported.height()) {
			ContentMapping::Identity => {
				let output = self.device.create_texture(&wgpu::TextureDescriptor {
					label: Some("CEF Imported Frame Copy"),
					size: imported.size(),
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: imported.format(),
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
					view_formats: &[],
				});
				encoder.copy_texture_to_texture(
					wgpu::TexelCopyTextureInfo {
						texture: &imported,
						mip_level: 0,
						origin: wgpu::Origin3d::ZERO,
						aspect: wgpu::TextureAspect::All,
					},
					wgpu::TexelCopyTextureInfo {
						texture: &output,
						mip_level: 0,
						origin: wgpu::Origin3d::ZERO,
						aspect: wgpu::TextureAspect::All,
					},
					imported.size(),
				);
				output
			}
			ContentMapping::Scaled(content_rect) => {
				let output = self.device.create_texture(&wgpu::TextureDescriptor {
					label: Some("CEF Imported Scaled Frame Copy"),
					size: wgpu::Extent3d {
						width: content_rect.source_width,
						height: content_rect.source_height,
						depth_or_array_layers: 1,
					},
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: imported.format(),
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
					view_formats: &[],
				});
				let size = wgpu::Extent3d {
					width: content_rect.width,
					height: content_rect.height,
					depth_or_array_layers: 1,
				};
				self.resampler.encode(
					&mut encoder,
					&imported,
					wgpu::Origin3d {
						x: content_rect.x,
						y: content_rect.y,
						z: 0,
					},
					size,
					&output,
				);
				output
			}
		};

		let blank_check = blank_check::encode_readback(&self.device, &mut encoder, &output);

		let submission = self.queue.submit([encoder.finish()]);

		let blank_check = blank_check.map();

		let _ = self.device.poll(wgpu::PollType::Wait {
			submission_index: Some(submission),
			timeout: None,
		});

		if blank_check.check_is_blank() {
			tracing::debug!("Skipping fully transparent accelerated frame");
			return None;
		}

		let Ok(mut slot) = self.slot.lock() else {
			tracing::error!("Failed to lock the frame surface");
			return None;
		};
		*slot = Some(output.clone());
		Some(output)
	}
}

#[cfg(feature = "accelerated_paint")]
mod blank_check {
	use std::sync::mpsc;

	const STRIP_BYTES: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
	const STRIP_TEXELS: u32 = STRIP_BYTES / 4;

	pub(super) fn encode_readback(device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture) -> PendingBlankCheck {
		let buffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("CEF Blank Check"),
			size: STRIP_BYTES as u64,
			usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
			mapped_at_creation: false,
		});
		let width = texture.width().min(STRIP_TEXELS);
		encoder.copy_texture_to_buffer(
			wgpu::TexelCopyTextureInfo {
				texture,
				mip_level: 0,
				origin: wgpu::Origin3d {
					x: (texture.width() - width) / 2,
					y: 0,
					z: 0,
				},
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::TexelCopyBufferInfo {
				buffer: &buffer,
				layout: wgpu::TexelCopyBufferLayout {
					offset: 0,
					bytes_per_row: Some(STRIP_BYTES),
					rows_per_image: None,
				},
			},
			wgpu::Extent3d {
				width,
				height: 1,
				depth_or_array_layers: 1,
			},
		);
		PendingBlankCheck { buffer, width }
	}

	pub(super) struct PendingBlankCheck {
		buffer: wgpu::Buffer,
		width: u32,
	}

	impl PendingBlankCheck {
		pub(super) fn map(self) -> MappedBlankCheck {
			let (sender, receiver) = mpsc::channel();
			self.buffer.slice(..u64::from(self.width) * 4).map_async(wgpu::MapMode::Read, move |result| {
				let _ = sender.send(result);
			});
			MappedBlankCheck {
				buffer: self.buffer,
				width: self.width,
				receiver,
			}
		}
	}

	pub(super) struct MappedBlankCheck {
		buffer: wgpu::Buffer,
		width: u32,
		receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
	}

	impl MappedBlankCheck {
		pub(super) fn check_is_blank(self) -> bool {
			match self.receiver.try_recv() {
				Ok(Ok(())) => {
					let slice = self.buffer.slice(..u64::from(self.width) * 4);
					slice.get_mapped_range().chunks_exact(4).all(|texel| texel[3] == 0)
				}
				_ => false,
			}
		}
	}
}
