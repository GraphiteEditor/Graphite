use super::{ImportedTexture, OwnershipTransfer, TextureImportError, TextureImportResult, TextureImporter, texture_descriptor, wgpu_format};
use ash::vk;
use cef::sys::cef_color_type_t;
use wgpu::TextureUses;
use wgpu::hal::api::Vulkan;

pub struct DmaBufImporter {
	fds: Vec<std::os::fd::OwnedFd>,
	format: cef_color_type_t,
	modifier: u64,
	width: u32,
	height: u32,
	strides: Vec<u32>,
	offsets: Vec<u32>,
}

impl TextureImporter for DmaBufImporter {
	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult {
		if self.fds.len() != 1 {
			return Err(TextureImportError::InvalidHandle(format!("Expected exactly one DMA-BUF plane fd, got {}", self.fds.len())));
		}

		if self.strides.len() != self.fds.len() || self.offsets.len() != self.fds.len() {
			return Err(TextureImportError::InvalidHandle(format!(
				"DMA-BUF plane count mismatch: {} fds, {} strides, {} offsets",
				self.fds.len(),
				self.strides.len(),
				self.offsets.len()
			)));
		}

		let max_dimension = device.limits().max_texture_dimension_2d;
		if self.width == 0 || self.height == 0 || self.width > max_dimension || self.height > max_dimension {
			return Err(TextureImportError::InvalidHandle(format!(
				"Invalid DMA-BUF dimensions {}x{} for the device limit of {max_dimension}",
				self.width, self.height
			)));
		}

		if !device.features().contains(wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF) {
			return Err(TextureImportError::HardwareUnavailable {
				reason: "Device does not support VULKAN_EXTERNAL_MEMORY_DMA_BUF".to_string(),
			});
		}

		let fd = self.fds[0].try_clone().map_err(|e| TextureImportError::PlatformError {
			message: format!("Failed to duplicate DMA-BUF file descriptor: {e}"),
		})?;

		let hal_texture = unsafe {
			let Some(hal_device) = device.as_hal::<Vulkan>() else {
				return Err(TextureImportError::HardwareUnavailable {
					reason: "Device is not using Vulkan backend".to_string(),
				});
			};
			hal_device.texture_from_dmabuf_fd(
				fd,
				&wgpu::hal::TextureDescriptor {
					label: Some("CEF DMA-BUF Texture"),
					size: wgpu::Extent3d {
						width: self.width,
						height: self.height,
						depth_or_array_layers: 1,
					},
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: wgpu_format(self.format)?,
					usage: TextureUses::COPY_SRC | TextureUses::RESOURCE,
					memory_flags: wgpu::hal::MemoryFlags::empty(),
					view_formats: vec![],
				},
				self.modifier,
				u64::from(self.strides[0]),
				u64::from(self.offsets[0]),
			)?
		};

		let texture = unsafe {
			device.create_texture_from_hal::<Vulkan>(
				hal_texture,
				&texture_descriptor(self.width, self.height, self.format, "CEF DMA-BUF Texture")?,
				TextureUses::COPY_SRC | TextureUses::RESOURCE,
			)
		};
		let ownership = ownership_transfer(device, &texture)?;
		tracing::trace!("Successfully imported DMA-BUF texture via Vulkan");
		Ok(ImportedTexture { texture, ownership: Some(ownership) })
	}
}

impl DmaBufImporter {
	pub fn from_parts(fds: Vec<std::os::fd::OwnedFd>, strides: Vec<u32>, offsets: Vec<u32>, modifier: u64, width: u32, height: u32, format: cef_color_type_t) -> Self {
		Self {
			fds,
			format,
			modifier,
			width,
			height,
			strides,
			offsets,
		}
	}
}

fn ownership_transfer(device: &wgpu::Device, texture: &wgpu::Texture) -> Result<OwnershipTransfer, TextureImportError> {
	let (raw_device, local_queue_family, external_queue_family, image) = unsafe {
		let (Some(hal_device), Some(hal_texture)) = (device.as_hal::<Vulkan>(), texture.as_hal::<Vulkan>()) else {
			return Err(TextureImportError::HardwareUnavailable {
				reason: "Device is not using Vulkan backend".to_string(),
			});
		};
		let external_queue_family = if hal_device.enabled_device_extensions().contains(&ash::ext::queue_family_foreign::NAME) {
			vk::QUEUE_FAMILY_FOREIGN_EXT
		} else {
			vk::QUEUE_FAMILY_EXTERNAL
		};
		(hal_device.raw_device().clone(), hal_device.queue_family_index(), external_queue_family, hal_texture.raw_handle())
	};

	let barrier = vk::ImageMemoryBarrier::default()
		.image(image)
		.subresource_range(vk::ImageSubresourceRange {
			aspect_mask: vk::ImageAspectFlags::COLOR,
			base_mip_level: 0,
			level_count: 1,
			base_array_layer: 0,
			layer_count: 1,
		})
		.old_layout(vk::ImageLayout::GENERAL)
		.new_layout(vk::ImageLayout::GENERAL);

	let acquire = record_barrier(
		device,
		&raw_device,
		barrier
			.src_queue_family_index(external_queue_family)
			.dst_queue_family_index(local_queue_family)
			.dst_access_mask(vk::AccessFlags::MEMORY_READ),
		vk::PipelineStageFlags::TOP_OF_PIPE,
		vk::PipelineStageFlags::ALL_COMMANDS,
	)?;
	let release = record_barrier(
		device,
		&raw_device,
		barrier
			.src_queue_family_index(local_queue_family)
			.dst_queue_family_index(external_queue_family)
			.src_access_mask(vk::AccessFlags::MEMORY_READ),
		vk::PipelineStageFlags::ALL_COMMANDS,
		vk::PipelineStageFlags::BOTTOM_OF_PIPE,
	)?;

	Ok(OwnershipTransfer {
		acquire,
		release,
		state: TextureUses::COPY_SRC | TextureUses::RESOURCE,
	})
}

fn record_barrier(
	device: &wgpu::Device,
	raw_device: &ash::Device,
	barrier: vk::ImageMemoryBarrier<'_>,
	src_stage: vk::PipelineStageFlags,
	dst_stage: vk::PipelineStageFlags,
) -> Result<wgpu::CommandBuffer, TextureImportError> {
	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
		label: Some("CEF DMA-BUF Ownership Transfer"),
	});
	unsafe {
		encoder.as_hal_mut::<Vulkan, _, _>(|hal_encoder| {
			let hal_encoder = hal_encoder.ok_or_else(|| TextureImportError::HardwareUnavailable {
				reason: "Command encoder is not using Vulkan backend".to_string(),
			})?;
			raw_device.cmd_pipeline_barrier(hal_encoder.raw_handle(), src_stage, dst_stage, vk::DependencyFlags::empty(), &[], &[], &[barrier]);
			Ok::<_, TextureImportError>(())
		})?;
	}
	Ok(encoder.finish())
}
