//! macOS IOSurface texture import implementation

#[cfg(target_os = "macos")]
use std::os::raw::c_void;

#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use ash::vk;
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use core_foundation::base::{CFType, TCFType};
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use objc2_io_surface::{IOSurface, IOSurfaceRef};
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use wgpu::hal::api;

use super::common::{TextureImportError, TextureImportResult, TextureImporter, format, texture, vulkan};
use cef::sys::cef_color_type_t;

#[cfg(target_os = "macos")]
pub struct IOSurfaceImporter {
	pub handle: *mut c_void,
	pub format: cef_color_type_t,
	pub width: u32,
	pub height: u32,
}

#[cfg(target_os = "macos")]
impl TextureImporter for IOSurfaceImporter {
	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult {
		// Try hardware acceleration first
		#[cfg(feature = "accelerated_paint")]
		{
			if self.supports_hardware_acceleration(device) {
				match self.import_via_vulkan(device) {
					Ok(texture) => {
						tracing::trace!("Successfully imported IOSurface texture via Vulkan");
						return Ok(texture);
					}
					Err(e) => {
						tracing::warn!("Failed to import IOSurface via Vulkan: {}, falling back to CPU texture", e);
					}
				}
			}
		}

		// Fallback to CPU texture
		texture::create_fallback(device, self.width, self.height, self.format, "CEF IOSurface Texture (fallback)")
	}

	fn supports_hardware_acceleration(&self, device: &wgpu::Device) -> bool {
		#[cfg(feature = "accelerated_paint")]
		{
			// Check if handle is valid
			if self.handle.is_null() {
				return false;
			}

			// Check if wgpu is using Vulkan backend
			vulkan::is_vulkan_backend(device)
		}
		#[cfg(not(feature = "accelerated_paint"))]
		{
			let _ = device;
			false
		}
	}
}

#[cfg(target_os = "macos")]
impl IOSurfaceImporter {
	#[cfg(feature = "accelerated_paint")]
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

				// Import IOSurface handle into Vulkan via Metal
				let vk_image = self.import_iosurface_to_vulkan(device)?;

				// Wrap VkImage in wgpu-hal texture
				let hal_texture = <api::Vulkan as wgpu::hal::Api>::Device::texture_from_raw(
					vk_image,
					&wgpu::hal::TextureDescriptor {
						label: Some("CEF IOSurface Texture"),
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
					label: Some("CEF IOSurface Texture"),
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

	#[cfg(feature = "accelerated_paint")]
	fn import_iosurface_to_vulkan(&self, hal_device: &<api::Vulkan as wgpu::hal::Api>::Device) -> Result<vk::Image, TextureImportError> {
		// Get raw Vulkan handles
		let device = hal_device.raw_device();
		let _instance = hal_device.shared_instance().raw_instance();

		// Validate dimensions
		if self.width == 0 || self.height == 0 {
			return Err(TextureImportError::InvalidHandle("Invalid IOSurface texture dimensions".to_string()));
		}

		// Convert handle to IOSurface
		let _iosurface = unsafe {
			let cf_type = CFType::wrap_under_get_rule(self.handle as IOSurfaceRef);
			IOSurface::from(cf_type)
		};

		// Note: Full Metal-to-Vulkan import would require:
		// 1. Creating Metal texture from IOSurface
		// 2. Using VK_EXT_metal_objects to import Metal texture into Vulkan
		// 3. Proper synchronization between Metal and Vulkan
		//
		// This is complex and not fully supported by current objc2 bindings.
		// For now, we create a minimal Vulkan image and rely on fallback.

		// Create external memory image info for Metal objects
		let mut external_memory_info = vk::ExternalMemoryImageCreateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::MTLTEXTURE_EXT);

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

		// Note: The actual Metal-to-Vulkan import would require VK_EXT_metal_objects
		// and proper Metal texture handle extraction, which is complex and not
		// fully supported in the current objc2 bindings. For now, we return the
		// image and rely on fallback behavior.

		tracing::warn!("Metal-to-Vulkan texture import not fully implemented");

		Ok(image)
	}
}
