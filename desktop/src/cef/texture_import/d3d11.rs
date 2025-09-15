//! Windows D3D11 shared texture import implementation

use super::common::{format, texture, vulkan};
use super::{TextureImportError, TextureImportResult, TextureImporter};
use ash::vk;
use cef::{AcceleratedPaintInfo, sys::cef_color_type_t};
use std::os::raw::c_void;
use wgpu::hal::api;

pub struct D3D11Importer {
	pub handle: *mut c_void,
	pub format: cef_color_type_t,
	pub width: u32,
	pub height: u32,
}

impl TextureImporter for D3D11Importer {
	fn new(info: &AcceleratedPaintInfo) -> Self {
		Self {
			handle: info.shared_texture_handle,
			format: *info.format.as_ref(),
			width: info.extra.coded_size.width as u32,
			height: info.extra.coded_size.height as u32,
		}
	}

	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult {
		// Try hardware acceleration first
		if self.supports_hardware_acceleration(device) {
			// Try D3D12 first (most efficient on Windows)
			if vulkan::is_d3d12_backend(device) {
				match self.import_via_d3d12(device) {
					Ok(texture) => {
						tracing::info!("Successfully imported D3D11 shared texture via D3D12");
						return Ok(texture);
					}
					Err(e) => {
						tracing::warn!("Failed to import D3D11 via D3D12: {}, trying Vulkan fallback", e);
					}
				}
			}

			// Try Vulkan as fallback
			if vulkan::is_vulkan_backend(device) {
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
		}

		// Fallback to CPU texture
		texture::create_fallback(device, self.width, self.height, self.format, "CEF D3D11 Texture (fallback)")
	}

	fn supports_hardware_acceleration(&self, device: &wgpu::Device) -> bool {
		// Check if handle is valid
		if self.handle.is_null() {
			return false;
		}

		// Check if wgpu is using D3D12 or Vulkan backend
		vulkan::is_d3d12_backend(device) || vulkan::is_vulkan_backend(device)
	}
}

impl D3D11Importer {
	fn import_via_d3d12(&self, device: &wgpu::Device) -> TextureImportResult {
		// Get wgpu's D3D12 device
		use wgpu::hal::api;
		let hal_texture = unsafe {
			device.as_hal::<api::Dx12, _, _>(|device| {
				let Some(device) = device else {
					return Err(TextureImportError::HardwareUnavailable {
						reason: "Device is not using D3D12 backend".to_string(),
					});
				};

				// Import D3D11 shared handle directly into D3D12 resource
				let d3d12_resource = self.import_d3d11_handle_to_d3d12(device)?;

				// Wrap D3D12 resource in wgpu-hal texture
				let hal_texture = <api::Dx12 as wgpu::hal::Api>::Device::texture_from_raw(
					d3d12_resource,
					format::cef_to_wgpu(self.format)?,
					wgpu::TextureDimension::D2,
					wgpu::Extent3d {
						width: self.width,
						height: self.height,
						depth_or_array_layers: 1,
					},
					1, // mip_level_count
					1, // sample_count
				);

				Ok(hal_texture)
			})
		}?;

		// Import hal texture into wgpu
		let texture = unsafe {
			device.create_texture_from_hal::<api::Dx12>(
				hal_texture,
				&wgpu::TextureDescriptor {
					label: Some("CEF D3D11→D3D12 Shared Texture"),
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

				// Import D3D11 shared handle into Vulkan
				let vk_image = self.import_d3d11_handle_to_vulkan(device)?;

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
					label: Some("CEF D3D11 Shared Texture"),
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

	fn import_d3d11_handle_to_vulkan(&self, hal_device: &<api::Vulkan as wgpu::hal::Api>::Device) -> Result<vk::Image, TextureImportError> {
		// Get raw Vulkan handles
		let device = hal_device.raw_device();
		let _instance = hal_device.shared_instance().raw_instance();

		// Validate dimensions
		if self.width == 0 || self.height == 0 {
			return Err(TextureImportError::InvalidHandle("Invalid D3D11 texture dimensions".to_string()));
		}

		// Create external memory image info
		let mut external_memory_info = vk::ExternalMemoryImageCreateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE);

		// Create image create info
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
			.tiling(vk::ImageTiling::OPTIMAL)
			.usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::COLOR_ATTACHMENT)
			.sharing_mode(vk::SharingMode::EXCLUSIVE)
			.push_next(&mut external_memory_info);

		// Create the image
		let image = unsafe {
			device.create_image(&image_create_info, None).map_err(|e| TextureImportError::VulkanError {
				operation: format!("Failed to create Vulkan image: {:?}", e),
			})?
		};

		// Get memory requirements
		let memory_requirements = unsafe { device.get_image_memory_requirements(image) };

		// Import D3D11 handle
		let mut import_memory_win32 = vk::ImportMemoryWin32HandleInfoKHR::default()
			.handle_type(vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE)
			.handle(self.handle as isize);

		// Find a suitable memory type
		let memory_properties = unsafe { hal_device.shared_instance().raw_instance().get_physical_device_memory_properties(hal_device.raw_physical_device()) };

		let memory_type_index =
			vulkan::find_memory_type_index(memory_requirements.memory_type_bits, vk::MemoryPropertyFlags::empty(), &memory_properties).ok_or_else(|| TextureImportError::VulkanError {
				operation: "Failed to find suitable memory type for D3D11 texture".to_string(),
			})?;

		let allocate_info = vk::MemoryAllocateInfo::default()
			.allocation_size(memory_requirements.size)
			.memory_type_index(memory_type_index)
			.push_next(&mut import_memory_win32);

		let device_memory = unsafe {
			device.allocate_memory(&allocate_info, None).map_err(|e| TextureImportError::VulkanError {
				operation: format!("Failed to allocate memory for D3D11 texture: {:?}", e),
			})?
		};

		// Bind memory to image
		unsafe {
			device.bind_image_memory(image, device_memory, 0).map_err(|e| TextureImportError::VulkanError {
				operation: format!("Failed to bind memory to image: {:?}", e),
			})?;
		}

		Ok(image)
	}

	fn import_d3d11_handle_to_d3d12(&self, hal_device: &<wgpu::hal::api::Dx12 as wgpu::hal::Api>::Device) -> Result<windows::Win32::Graphics::Direct3D12::ID3D12Resource, TextureImportError> {
		use windows::Win32::Graphics::Direct3D12::*;

		// Get D3D12 device from wgpu-hal
		let d3d12_device = hal_device.raw_device();

		// Validate dimensions
		if self.width == 0 || self.height == 0 {
			return Err(TextureImportError::InvalidHandle("Invalid D3D11 texture dimensions".to_string()));
		}

		// Open D3D11 shared handle on D3D12 device
		unsafe {
			let mut shared_resource: Option<ID3D12Resource> = None;
			d3d12_device
				.OpenSharedHandle(windows::Win32::Foundation::HANDLE(self.handle), &mut shared_resource)
				.map_err(|e| TextureImportError::PlatformError {
					message: format!("Failed to open D3D11 shared handle on D3D12: {:?}", e),
				})?;

			shared_resource.ok_or_else(|| TextureImportError::InvalidHandle("Failed to get D3D12 resource from shared handle".to_string()))
		}
	}
}
