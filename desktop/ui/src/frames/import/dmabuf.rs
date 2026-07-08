use super::{TextureImportError, TextureImportResult, TextureImporter, texture_descriptor, wgpu_format};
use ash::vk;
use cef::sys::cef_color_type_t;
use wgpu::hal::api;

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
		if self.fds.is_empty() {
			return Err(TextureImportError::InvalidHandle("No DMA-BUF plane fds".to_string()));
		}
		let texture = self.import_via_vulkan(device)?;
		tracing::trace!("Successfully imported DMA-BUF texture via Vulkan");
		Ok(texture)
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

	fn import_via_vulkan(&self, device: &wgpu::Device) -> TextureImportResult {
		use wgpu::{TextureUses, wgc::api::Vulkan};
		let hal_texture = unsafe {
			let hal_device_guard = device.as_hal::<api::Vulkan>();
			let Some(hal_device) = hal_device_guard else {
				return Err(TextureImportError::HardwareUnavailable {
					reason: "Device is not using Vulkan backend".to_string(),
				});
			};

			let (vk_image, device_memory) = self.create_vulkan_image_from_dmabuf(&hal_device)?;

			let hal_texture = <api::Vulkan as wgpu::hal::Api>::Device::texture_from_raw(
				&hal_device,
				vk_image,
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
					usage: TextureUses::COPY_DST | TextureUses::COPY_SRC | TextureUses::RESOURCE,
					memory_flags: wgpu::hal::MemoryFlags::empty(),
					view_formats: vec![],
				},
				None,
				wgpu::hal::vulkan::TextureMemory::Dedicated(device_memory),
			);

			Ok::<_, TextureImportError>(hal_texture)
		}?;

		let texture = unsafe { device.create_texture_from_hal::<Vulkan>(hal_texture, &texture_descriptor(self.width, self.height, self.format, "CEF DMA-BUF Texture")?) };

		Ok(texture)
	}

	fn create_vulkan_image_from_dmabuf(&self, hal_device: &<api::Vulkan as wgpu::hal::Api>::Device) -> Result<(vk::Image, vk::DeviceMemory), TextureImportError> {
		let device = hal_device.raw_device();
		let _instance = hal_device.shared_instance().raw_instance();

		if self.width == 0 || self.height == 0 {
			return Err(TextureImportError::InvalidHandle("Invalid DMA-BUF dimensions".to_string()));
		}

		let image_create_info = vk::ImageCreateInfo::default()
			.image_type(vk::ImageType::TYPE_2D)
			.format(vulkan_format(self.format)?)
			.extent(vk::Extent3D {
				width: self.width,
				height: self.height,
				depth: 1,
			})
			.mip_levels(1)
			.array_layers(1)
			.samples(vk::SampleCountFlags::TYPE_1)
			.tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
			.usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
			.sharing_mode(vk::SharingMode::EXCLUSIVE);

		// Set up DRM format modifier
		let plane_layouts = self.create_subresource_layouts();
		let mut drm_format_modifier = vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
			.drm_format_modifier(self.modifier)
			.plane_layouts(&plane_layouts);

		let image_create_info = image_create_info.push_next(&mut drm_format_modifier);

		let image = unsafe {
			device.create_image(&image_create_info, None).map_err(|e| TextureImportError::VulkanError {
				operation: format!("Failed to create Vulkan image: {e:?}"),
			})?
		};

		let memory_requirements = unsafe { device.get_image_memory_requirements(image) };

		// Duplicate the file descriptor
		let dup_fd = unsafe { libc::dup(std::os::fd::AsRawFd::as_raw_fd(&self.fds[0])) };
		if dup_fd == -1 {
			// SAFETY: the image was created above and never bound or returned.
			unsafe { device.destroy_image(image, None) };
			return Err(TextureImportError::PlatformError {
				message: "Failed to duplicate DMA-BUF file descriptor".to_string(),
			});
		}

		let mut import_memory_fd = vk::ImportMemoryFdInfoKHR::default().handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT).fd(dup_fd);

		let memory_properties = unsafe { hal_device.shared_instance().raw_instance().get_physical_device_memory_properties(hal_device.raw_physical_device()) };

		let Some(memory_type_index) = find_memory_type_index(memory_requirements.memory_type_bits, vk::MemoryPropertyFlags::empty(), &memory_properties) else {
			// SAFETY: import failed and the fd is still ours, need to clean up the image and close the fd
			unsafe {
				device.destroy_image(image, None);
				libc::close(dup_fd);
			}
			return Err(TextureImportError::VulkanError {
				operation: "Failed to find suitable memory type for DMA-BUF".to_string(),
			});
		};

		let allocate_info = vk::MemoryAllocateInfo::default()
			.allocation_size(memory_requirements.size)
			.memory_type_index(memory_type_index)
			.push_next(&mut import_memory_fd);

		let device_memory = match unsafe { device.allocate_memory(&allocate_info, None) } {
			Ok(memory) => memory,
			Err(e) => {
				// SAFETY: import failed and the fd is still ours, need to clean up the image and close the fd
				unsafe {
					device.destroy_image(image, None);
					libc::close(dup_fd);
				}
				return Err(TextureImportError::VulkanError {
					operation: format!("Failed to allocate memory for DMA-BUF: {e:?}"),
				});
			}
		};

		if let Err(e) = unsafe { device.bind_image_memory(image, device_memory, 0) } {
			// SAFETY: import failed, need to clean up the image and free the memory
			unsafe {
				device.destroy_image(image, None);
				device.free_memory(device_memory, None);
			}
			return Err(TextureImportError::VulkanError {
				operation: format!("Failed to bind memory to image: {e:?}"),
			});
		}

		Ok((image, device_memory))
	}

	fn create_subresource_layouts(&self) -> Vec<vk::SubresourceLayout> {
		(0..self.fds.len())
			.map(|i| vk::SubresourceLayout {
				offset: self.offsets.get(i).copied().unwrap_or(0) as u64,
				size: 0, // Will be calculated by driver
				row_pitch: self.strides.get(i).copied().unwrap_or(0) as u64,
				array_pitch: 0,
				depth_pitch: 0,
			})
			.collect()
	}
}

fn vulkan_format(format: cef_color_type_t) -> Result<vk::Format, TextureImportError> {
	match format {
		cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(vk::Format::B8G8R8A8_UNORM),
		cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(vk::Format::R8G8B8A8_UNORM),
		_ => Err(TextureImportError::UnsupportedFormat { format }),
	}
}

fn find_memory_type_index(type_filter: u32, properties: vk::MemoryPropertyFlags, mem_properties: &vk::PhysicalDeviceMemoryProperties) -> Option<u32> {
	(0..mem_properties.memory_type_count).find(|&i| (type_filter & (1 << i)) != 0 && mem_properties.memory_types[i as usize].property_flags.contains(properties))
}
