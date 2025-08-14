#[cfg(target_os = "windows")]
use std::os::raw::c_void;

#[cfg(all(feature = "accelerated_paint", target_os = "windows"))]
use ash::vk;
#[cfg(all(feature = "accelerated_paint", target_os = "windows"))]
use wgpu::hal::api;
#[cfg(all(feature = "accelerated_paint", target_os = "windows"))]
use windows::Win32::Graphics::{Direct3D11::*, Dxgi::*};

#[cfg(target_os = "windows")]
pub struct D3D11SharedTexture {
	pub handle: *mut c_void,
	pub width: u32,
	pub height: u32,
	pub format: cef::sys::cef_color_type_t,
}

#[cfg(target_os = "windows")]
impl D3D11SharedTexture {
	pub fn import_to_wgpu(&self, device: &wgpu::Device) -> Result<wgpu::Texture, String> {
		tracing::debug!("D3D11 shared texture import requested: {}x{} handle={:p}", self.width, self.height, self.handle);

		// Try to import via Vulkan, fallback to CPU texture on failure
		#[cfg(feature = "accelerated_paint")]
		{
			match self.import_via_vulkan(device) {
				Ok(texture) => {
					tracing::info!("Successfully imported D3D11 shared texture via Vulkan");
					return Ok(texture);
				}
				Err(e) => {
					tracing::warn!("Failed to import D3D11 via Vulkan: {}, falling back to CPU texture", e);
				}
			}
		}

		// Fallback: create empty CPU texture with same dimensions
		self.create_fallback_texture(device)
	}

	#[cfg(feature = "accelerated_paint")]
	fn import_via_vulkan(&self, device: &wgpu::Device) -> Result<wgpu::Texture, String> {
		// Validate handle
		if self.handle.is_null() {
			return Err("D3D11 shared texture handle is null".to_string());
		}

		// Get wgpu's Vulkan instance and device
		use wgpu::{TextureUses, wgc::api::Vulkan};
		let hal_texture = unsafe {
			device.as_hal::<api::Vulkan, _, _>(|device| {
				let Some(device) = device else {
					return Err("Device is not using Vulkan backend".to_string());
				};

				// Import D3D11 shared handle into Vulkan
				let vk_image = self
					.import_d3d11_handle_to_vulkan(device)
					.map_err(|e| format!("Failed to create Vulkan image from D3D11 handle: {}", e))?;

				// Wrap VkImage in wgpu-hal texture
				let hal_texture = <api::Vulkan as wgpu::hal::Api>::Device::texture_from_raw(
					vk_image,
					&wgpu::hal::TextureDescriptor {
						label: Some("CEF D3D11 Shared Texture"),
						size: wgpu::Extent3d {
							width: self.width,
							height: self.height,
							depth_or_array_layers: 1,
						},
						mip_level_count: 1,
						sample_count: 1,
						dimension: wgpu::TextureDimension::D2,
						format: self.cef_to_hal_format()?,
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
					label: Some("CEF D3D11 Shared Texture"),
					size: wgpu::Extent3d {
						width: self.width,
						height: self.height,
						depth_or_array_layers: 1,
					},
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: self.cef_to_wgpu_format()?,
					usage: wgpu::TextureUsages::TEXTURE_BINDING,
					view_formats: &[],
				},
			)
		};

		Ok(texture)
	}

	#[cfg(feature = "accelerated_paint")]
	fn import_d3d11_handle_to_vulkan(&self, hal_device: &<api::Vulkan as wgpu::hal::Api>::Device) -> Result<vk::Image, String> {
		// Get raw Vulkan handles
		let device = hal_device.raw_device();
		let _instance = hal_device.shared_instance().raw_instance();

		// Validate dimensions
		if self.width == 0 || self.height == 0 {
			return Err("Invalid D3D11 texture dimensions".to_string());
		}

		// Create external memory image info
		let mut external_memory_info = vk::ExternalMemoryImageCreateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE);

		// Create image create info
		let image_create_info = vk::ImageCreateInfo::default()
			.image_type(vk::ImageType::TYPE_2D)
			.format(self.cef_to_vk_format()?)
			.extent(vk::Extent3D {
				width: self.width,
				height: self.height,
				depth: 1,
			})
			.mip_levels(1)
			.array_layers(1)
			.samples(vk::SampleCountFlags::TYPE_1)
			.tiling(vk::ImageTiling::OPTIMAL)
			.usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::COLOR_ATTACHMENT)
			.sharing_mode(vk::SharingMode::EXCLUSIVE)
			.push_next(&mut external_memory_info);

		// Create the image
		let image = unsafe { device.create_image(&image_create_info, None).map_err(|e| format!("Failed to create Vulkan image: {:?}", e))? };

		// Get memory requirements
		let memory_requirements = unsafe { device.get_image_memory_requirements(image) };

		// Import D3D11 handle
		let mut import_memory_win32 = vk::ImportMemoryWin32HandleInfoKHR::default()
			.handle_type(vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE)
			.handle(self.handle as isize);

		// Find a suitable memory type
		let memory_properties = unsafe { hal_device.shared_instance().raw_instance().get_physical_device_memory_properties(hal_device.raw_physical_device()) };

		let memory_type_index =
			find_memory_type_index(memory_requirements.memory_type_bits, vk::MemoryPropertyFlags::empty(), &memory_properties).ok_or("Failed to find suitable memory type for D3D11 texture")?;

		let allocate_info = vk::MemoryAllocateInfo::default()
			.allocation_size(memory_requirements.size)
			.memory_type_index(memory_type_index)
			.push_next(&mut import_memory_win32);

		let device_memory = unsafe {
			device
				.allocate_memory(&allocate_info, None)
				.map_err(|e| format!("Failed to allocate memory for D3D11 texture: {:?}", e))?
		};

		// Bind memory to image
		unsafe {
			device.bind_image_memory(image, device_memory, 0).map_err(|e| format!("Failed to bind memory to image: {:?}", e))?;
		}

		Ok(image)
	}

	fn cef_to_vk_format(&self) -> Result<vk::Format, String> {
		use cef::sys::cef_color_type_t;
		match self.format {
			cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(vk::Format::B8G8R8A8_UNORM),
			cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(vk::Format::R8G8B8A8_UNORM),
			_ => Err(format!("Unsupported CEF format for Vulkan: {:?}", self.format)),
		}
	}

	fn cef_to_hal_format(&self) -> Result<wgpu::TextureFormat, String> {
		use cef::sys::cef_color_type_t;
		match self.format {
			cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(wgpu::TextureFormat::Bgra8UnormSrgb),
			cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(wgpu::TextureFormat::Rgba8UnormSrgb),
			_ => Err(format!("Unsupported CEF format for HAL: {:?}", self.format)),
		}
	}

	fn cef_to_wgpu_format(&self) -> Result<wgpu::TextureFormat, String> {
		self.cef_to_hal_format()
	}

	fn create_fallback_texture(&self, device: &wgpu::Device) -> Result<wgpu::Texture, String> {
		let format = self.cef_to_wgpu_format()?;
		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("CEF D3D11 Texture (fallback)"),
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

		tracing::warn!("Using fallback CPU texture - D3D11 hardware acceleration not available");
		Ok(texture)
	}
}

#[cfg(all(feature = "accelerated_paint", target_os = "windows"))]
fn find_memory_type_index(type_filter: u32, properties: vk::MemoryPropertyFlags, mem_properties: &vk::PhysicalDeviceMemoryProperties) -> Option<u32> {
	for i in 0..mem_properties.memory_type_count {
		if (type_filter & (1 << i)) != 0 && mem_properties.memory_types[i as usize].property_flags.contains(properties) {
			return Some(i);
		}
	}
	None
}
