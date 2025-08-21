//! Linux DMA-BUF texture import implementation

use super::common::{format, texture, vulkan};
use super::{TextureImportError, TextureImportResult, TextureImporter};
use ash::vk;
use cef::{AcceleratedPaintInfo, sys::cef_color_type_t};
use wgpu::hal::api;

pub(crate) struct DmaBufImporter {
	fds: Vec<std::os::fd::RawFd>,
	format: cef_color_type_t,
	modifier: u64,
	width: u32,
	height: u32,
	strides: Vec<u32>,
	offsets: Vec<u32>,
}

impl TextureImporter for DmaBufImporter {
	fn new(info: &AcceleratedPaintInfo) -> Self {
		Self {
			fds: extract_fds_from_info(info),
			format: *info.format.as_ref(),
			modifier: info.modifier,
			width: info.extra.coded_size.width as u32,
			height: info.extra.coded_size.height as u32,
			strides: extract_strides_from_info(info),
			offsets: extract_offsets_from_info(info),
		}
	}

	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult {
		// Try hardware acceleration first
		if self.supports_hardware_acceleration(device) {
			match self.import_via_vulkan(device) {
				Ok(texture) => {
					tracing::info!("Successfully imported DMA-BUF texture via Vulkan");
					return Ok(texture);
				}
				Err(e) => {
					tracing::warn!("Failed to import DMA-BUF via Vulkan: {}, falling back to CPU texture", e);
				}
			}
		}

		// Fallback to CPU texture
		texture::create_fallback(device, self.width, self.height, self.format, "CEF DMA-BUF Texture (fallback)")
	}

	fn supports_hardware_acceleration(&self, device: &wgpu::Device) -> bool {
		// Check if we have valid file descriptors
		if self.fds.is_empty() {
			return false;
		}

		for &fd in &self.fds {
			if fd < 0 {
				return false;
			}
			// Check if file descriptor is valid
			let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
			if flags == -1 {
				return false;
			}
		}

		// Check if wgpu is using Vulkan backend
		vulkan::is_vulkan_backend(device)
	}
}

impl DmaBufImporter {
	fn import_via_vulkan(&self, device: &wgpu::Device) -> TextureImportResult {
		// Get wgpu's Vulkan instance and device
		use wgpu::{TextureUses, wgc::api::Vulkan};
		let hal_texture = unsafe {
			device.as_hal::<api::Vulkan, _, _>(|device| {
				let Some(device) = device else {
					return Err(TextureImportError::HardwareUnavailable {
						reason: "Device is not using Vulkan backend".to_string(),
					});
				};

				// Create VkImage from DMA-BUF using external memory
				let vk_image = self.create_vulkan_image_from_dmabuf(device)?;

				// Wrap VkImage in wgpu-hal texture
				let hal_texture = <api::Vulkan as wgpu::hal::Api>::Device::texture_from_raw(
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
						format: format::cef_to_wgpu(self.format)?,
						usage: TextureUses::COPY_DST | TextureUses::RESOURCE,
						memory_flags: wgpu::hal::MemoryFlags::empty(),
						view_formats: vec![],
					},
					None, // drop_callback
				);

				Ok(hal_texture)
			})
		}?;

		// Import hal texture into wgpu
		let texture = unsafe {
			device.create_texture_from_hal::<Vulkan>(
				hal_texture,
				&wgpu::TextureDescriptor {
					label: Some("CEF DMA-BUF Texture"),
					size: wgpu::Extent3d {
						width: self.width,
						height: self.height,
						depth_or_array_layers: 1,
					},
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: format::cef_to_wgpu(self.format)?,
					usage: wgpu::TextureUsages::TEXTURE_BINDING,
					view_formats: &[],
				},
			)
		};

		Ok(texture)
	}

	fn create_vulkan_image_from_dmabuf(&self, hal_device: &<api::Vulkan as wgpu::hal::Api>::Device) -> Result<vk::Image, TextureImportError> {
		// Get raw Vulkan handles
		let device = hal_device.raw_device();
		let _instance = hal_device.shared_instance().raw_instance();

		// Validate dimensions
		if self.width == 0 || self.height == 0 {
			return Err(TextureImportError::InvalidHandle("Invalid DMA-BUF dimensions".to_string()));
		}

		// Create external memory image
		let image_create_info = vk::ImageCreateInfo::default()
			.image_type(vk::ImageType::TYPE_2D)
			.format(format::cef_to_vulkan(self.format)?)
			.extent(vk::Extent3D {
				width: self.width,
				height: self.height,
				depth: 1,
			})
			.mip_levels(1)
			.array_layers(1)
			.samples(vk::SampleCountFlags::TYPE_1)
			.tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
			.usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::COLOR_ATTACHMENT)
			.sharing_mode(vk::SharingMode::EXCLUSIVE);

		// Set up DRM format modifier
		let plane_layouts = self.create_subresource_layouts()?;
		let mut drm_format_modifier = vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
			.drm_format_modifier(self.modifier)
			.plane_layouts(&plane_layouts);

		let image_create_info = image_create_info.push_next(&mut drm_format_modifier);

		// Create the image
		let image = unsafe {
			device.create_image(&image_create_info, None).map_err(|e| TextureImportError::VulkanError {
				operation: format!("Failed to create Vulkan image: {e:?}"),
			})?
		};

		// Import memory from DMA-BUF
		let memory_requirements = unsafe { device.get_image_memory_requirements(image) };

		// Duplicate the file descriptor to avoid ownership issues
		let dup_fd = unsafe { libc::dup(self.fds[0]) };
		if dup_fd == -1 {
			return Err(TextureImportError::PlatformError {
				message: "Failed to duplicate DMA-BUF file descriptor".to_string(),
			});
		}

		let mut import_memory_fd = vk::ImportMemoryFdInfoKHR::default().handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT).fd(dup_fd);

		// Find a suitable memory type
		let memory_properties = unsafe { hal_device.shared_instance().raw_instance().get_physical_device_memory_properties(hal_device.raw_physical_device()) };

		let memory_type_index =
			vulkan::find_memory_type_index(memory_requirements.memory_type_bits, vk::MemoryPropertyFlags::empty(), &memory_properties).ok_or_else(|| TextureImportError::VulkanError {
				operation: "Failed to find suitable memory type for DMA-BUF".to_string(),
			})?;

		let allocate_info = vk::MemoryAllocateInfo::default()
			.allocation_size(memory_requirements.size)
			.memory_type_index(memory_type_index)
			.push_next(&mut import_memory_fd);

		let device_memory = unsafe {
			device.allocate_memory(&allocate_info, None).map_err(|e| TextureImportError::VulkanError {
				operation: format!("Failed to allocate memory for DMA-BUF: {e:?}"),
			})?
		};

		// Bind memory to image
		unsafe {
			device.bind_image_memory(image, device_memory, 0).map_err(|e| TextureImportError::VulkanError {
				operation: format!("Failed to bind memory to image: {e:?}"),
			})?;
		}

		Ok(image)
	}

	fn create_subresource_layouts(&self) -> Result<Vec<vk::SubresourceLayout>, TextureImportError> {
		let mut layouts = Vec::new();

		for i in 0..self.fds.len() {
			layouts.push(vk::SubresourceLayout {
				offset: self.offsets.get(i).copied().unwrap_or(0) as u64,
				size: 0, // Will be calculated by driver
				row_pitch: self.strides.get(i).copied().unwrap_or(0) as u64,
				array_pitch: 0,
				depth_pitch: 0,
			});
		}

		Ok(layouts)
	}
}

fn extract_fds_from_info(info: &cef::AcceleratedPaintInfo) -> Vec<std::os::fd::RawFd> {
	let plane_count = info.plane_count as usize;
	let mut fds = Vec::with_capacity(plane_count);

	for i in 0..plane_count {
		if let Some(plane) = info.planes.get(i) {
			fds.push(plane.fd);
		}
	}

	fds
}

fn extract_strides_from_info(info: &cef::AcceleratedPaintInfo) -> Vec<u32> {
	let plane_count = info.plane_count as usize;
	let mut strides = Vec::with_capacity(plane_count);

	for i in 0..plane_count {
		if let Some(plane) = info.planes.get(i) {
			strides.push(plane.stride);
		}
	}

	strides
}

fn extract_offsets_from_info(info: &cef::AcceleratedPaintInfo) -> Vec<u32> {
	let plane_count = info.plane_count as usize;
	let mut offsets = Vec::with_capacity(plane_count);

	for i in 0..plane_count {
		if let Some(plane) = info.planes.get(i) {
			offsets.push(plane.offset as u32);
		}
	}

	offsets
}
