use std::os::fd::RawFd;

#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
use ash::vk;
#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
use wgpu::hal::api;
#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
extern crate libc;

#[cfg(target_os = "linux")]
pub struct DmaBufTexture {
	pub fds: Vec<RawFd>,
	pub format: cef::sys::cef_color_type_t,
	pub modifier: u64,
	pub width: u32,
	pub height: u32,
	pub strides: Vec<u32>,
	pub offsets: Vec<u32>,
}

#[cfg(target_os = "linux")]
impl DmaBufTexture {
	pub fn import_to_wgpu(&self, device: &wgpu::Device) -> Result<wgpu::Texture, String> {
		tracing::debug!(
			"DMA-BUF import requested: {}x{} format={:?} modifier={:#x} planes={}",
			self.width,
			self.height,
			self.format,
			self.modifier,
			self.fds.len()
		);

		// Try to import via Vulkan, fallback to CPU texture on failure
		#[cfg(feature = "accelerated_paint")]
		{
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

		// Fallback: create empty CPU texture with same dimensions
		let format = drm_format_to_wgpu(self.format)?;
		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("CEF DMA-BUF Texture (fallback)"),
			size: wgpu::Extent3d {
				width: self.width,
				height: self.height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});

		tracing::warn!("Using fallback CPU texture - DMA-BUF hardware acceleration not available");
		Ok(texture)
	}

	#[cfg(feature = "accelerated_paint")]
	fn import_via_vulkan(&self, device: &wgpu::Device) -> Result<wgpu::Texture, String> {
		// Validate file descriptors before proceeding
		if self.fds.is_empty() {
			return Err("No DMA-BUF file descriptors provided".to_string());
		}
		
		for &fd in &self.fds {
			if fd < 0 {
				return Err(format!("Invalid file descriptor: {}", fd));
			}
			// Check if file descriptor is valid by testing with fcntl
			let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
			if flags == -1 {
				return Err(format!("File descriptor {} is not valid", fd));
			}
		}
		
		// Get wgpu's Vulkan instance and device
		use wgpu::{TextureUses, wgc::api::Vulkan};
		let hal_texture = unsafe {
			device.as_hal::<api::Vulkan, _, _>(|device| {
				let Some(device) = device else {
					return Err("Device is not using Vulkan backend".to_string());
				};

				// Create VkImage from DMA-BUF using external memory
				let vk_image = self.create_vulkan_image_from_dmabuf(device).map_err(|e| format!("Failed to create Vulkan image from DMA-BUF: {}", e))?;

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
							format: cef_to_hal_format(self.format)?,
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
					format: drm_format_to_wgpu(self.format)?,
					usage: wgpu::TextureUsages::TEXTURE_BINDING,
					view_formats: &[],
				},
			)
		};

		Ok(texture)
	}

	#[cfg(feature = "accelerated_paint")]
	fn create_vulkan_image_from_dmabuf(&self, hal_device: &<api::Vulkan as wgpu::hal::Api>::Device) -> Result<vk::Image, String> {
		// Get raw Vulkan handles
		let device = hal_device.raw_device();
		let _instance = hal_device.shared_instance().raw_instance();

		// Validate dimensions
		if self.width == 0 || self.height == 0 {
			return Err("Invalid DMA-BUF dimensions".to_string());
		}
		
		// Create external memory image
		let image_create_info = vk::ImageCreateInfo::default()
			.image_type(vk::ImageType::TYPE_2D)
			.format(cef_to_vk_format(self.format)?)
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
		let image = unsafe { device.create_image(&image_create_info, None).map_err(|e| format!("Failed to create Vulkan image: {:?}", e))? };

		// Import memory from DMA-BUF
		let memory_requirements = unsafe { device.get_image_memory_requirements(image) };

		// Duplicate the file descriptor to avoid ownership issues
		// CEF owns the original FDs and will close them when the AcceleratedPaintInfo is destroyed
		let dup_fd = unsafe { libc::dup(self.fds[0]) };
		if dup_fd == -1 {
			return Err("Failed to duplicate DMA-BUF file descriptor".to_string());
		}
		
		let mut import_memory_fd = vk::ImportMemoryFdInfoKHR::default().handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT).fd(dup_fd);

		// Find a suitable memory type
		let memory_properties = unsafe { hal_device.shared_instance().raw_instance().get_physical_device_memory_properties(hal_device.raw_physical_device()) };
		let memory_type_index = find_memory_type_index(memory_requirements.memory_type_bits, vk::MemoryPropertyFlags::empty(), &memory_properties)
			.ok_or("Failed to find suitable memory type for DMA-BUF")?;

		let allocate_info = vk::MemoryAllocateInfo::default()
			.allocation_size(memory_requirements.size)
			.memory_type_index(memory_type_index)
			.push_next(&mut import_memory_fd);

		let device_memory = unsafe { device.allocate_memory(&allocate_info, None).map_err(|e| format!("Failed to allocate memory for DMA-BUF: {:?}", e))? };

		// Bind memory to image
		unsafe {
			device.bind_image_memory(image, device_memory, 0).map_err(|e| format!("Failed to bind memory to image: {:?}", e))?;
		}

		Ok(image)
	}

	#[cfg(feature = "accelerated_paint")]
	fn create_subresource_layouts(&self) -> Result<Vec<vk::SubresourceLayout>, String> {
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

#[cfg(target_os = "linux")]
fn drm_format_to_wgpu(drm_format: cef::sys::cef_color_type_t) -> Result<wgpu::TextureFormat, String> {
	// Based on OBS's drm-format.cpp

	use cef::sys::cef_color_type_t;
	match drm_format {
		cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(wgpu::TextureFormat::Bgra8UnormSrgb), // DRM_FORMAT_ARGB8888
		cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(wgpu::TextureFormat::Rgba8UnormSrgb), // DRM_FORMAT_ABGR8888
		_ => Err(format!("Unsupported DRM format: {:?}", drm_format)),
	}
}

#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
fn cef_to_vk_format(cef_format: cef::sys::cef_color_type_t) -> Result<vk::Format, String> {
	use cef::sys::cef_color_type_t;
	match cef_format {
		cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(vk::Format::B8G8R8A8_UNORM),
		cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(vk::Format::R8G8B8A8_UNORM),
		_ => Err(format!("Unsupported CEF format for Vulkan: {:?}", cef_format)),
	}
}

#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
fn cef_to_hal_format(cef_format: cef::sys::cef_color_type_t) -> Result<wgpu::TextureFormat, String> {
	use cef::sys::cef_color_type_t;
	match cef_format {
		cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(wgpu::TextureFormat::Bgra8UnormSrgb),
		cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(wgpu::TextureFormat::Rgba8UnormSrgb),
		_ => Err(format!("Unsupported CEF format for HAL: {:?}", cef_format)),
	}
}

#[cfg(all(feature = "accelerated_paint", target_os = "linux"))]
fn find_memory_type_index(
	type_filter: u32,
	properties: vk::MemoryPropertyFlags,
	mem_properties: &vk::PhysicalDeviceMemoryProperties,
) -> Option<u32> {
	for i in 0..mem_properties.memory_type_count {
		if (type_filter & (1 << i)) != 0 && mem_properties.memory_types[i as usize].property_flags.contains(properties) {
			return Some(i);
		}
	}
	None
}
