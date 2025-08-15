#[cfg(target_os = "macos")]
use std::os::raw::c_void;

#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use ash::vk;
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use wgpu::hal::api;
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use objc2_io_surface::{IOSurface, IOSurfaceRef};
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use objc2_metal::{MTLDevice, MTLTexture, MTLTextureDescriptor, MTLPixelFormat};
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use core_foundation::base::{CFType, TCFType};

#[cfg(target_os = "macos")]
pub struct IOSurfaceTexture {
	pub handle: *mut c_void,
	pub width: u32,
	pub height: u32,
	pub format: cef::sys::cef_color_type_t,
}

#[cfg(target_os = "macos")]
impl IOSurfaceTexture {
	pub fn import_to_wgpu(&self, device: &wgpu::Device) -> Result<wgpu::Texture, String> {
		tracing::debug!(
			"IOSurface texture import requested: {}x{} handle={:p}",
			self.width,
			self.height,
			self.handle
		);

		// Try to import via Metal/Vulkan, fallback to CPU texture on failure
		#[cfg(feature = "accelerated_paint")]
		{
			match self.import_via_vulkan(device) {
				Ok(texture) => {
					tracing::info!("Successfully imported IOSurface texture via Vulkan");
					return Ok(texture);
				}
				Err(e) => {
					tracing::warn!("Failed to import IOSurface via Vulkan: {}, falling back to CPU texture", e);
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
			return Err("IOSurface handle is null".to_string());
		}

		// Get wgpu's Vulkan instance and device
		use wgpu::{TextureUses, wgc::api::Vulkan};
		let hal_texture = unsafe {
			device.as_hal::<api::Vulkan, _, _>(|device| {
				let Some(device) = device else {
					return Err("Device is not using Vulkan backend".to_string());
				};

				// Import IOSurface handle into Vulkan via Metal
				let vk_image = self.import_iosurface_to_vulkan(device).map_err(|e| format!("Failed to create Vulkan image from IOSurface: {}", e))?;

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
					label: Some("CEF IOSurface Texture"),
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
	fn import_iosurface_to_vulkan(&self, hal_device: &<api::Vulkan as wgpu::hal::Api>::Device) -> Result<vk::Image, String> {
		// Get raw Vulkan handles
		let device = hal_device.raw_device();
		let _instance = hal_device.shared_instance().raw_instance();

		// Validate dimensions
		if self.width == 0 || self.height == 0 {
			return Err("Invalid IOSurface texture dimensions".to_string());
		}

		// Convert handle to IOSurface
		let iosurface = unsafe {
			let cf_type = CFType::wrap_under_get_rule(self.handle as IOSurfaceRef);
			IOSurface::from(cf_type)
		};

		// Create Metal texture from IOSurface
		let mtl_texture = self.create_metal_texture_from_iosurface(&iosurface)?;

		// Import Metal texture into Vulkan using VK_EXT_metal_objects
		self.import_metal_texture_to_vulkan(device, &mtl_texture)
	}

	#[cfg(feature = "accelerated_paint")]
	fn create_metal_texture_from_iosurface(&self, iosurface: &IOSurface) -> Result<MTLTexture, String> {
		// Get Metal device (this would need to be obtained from wgpu-hal)
		// For now, we'll create a simple fallback
		tracing::warn!("Metal texture creation from IOSurface not fully implemented");
		Err("Metal texture creation not available".to_string())
	}

	#[cfg(feature = "accelerated_paint")]
	fn import_metal_texture_to_vulkan(&self, device: vk::Device, _mtl_texture: &MTLTexture) -> Result<vk::Image, String> {
		// Create external memory image info for Metal objects
		let mut external_memory_info = vk::ExternalMemoryImageCreateInfo::default()
			.handle_types(vk::ExternalMemoryHandleTypeFlags::MTLTEXTURE_EXT);

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
		let image = unsafe {
			device.create_image(&image_create_info, None)
				.map_err(|e| format!("Failed to create Vulkan image: {:?}", e))?
		};

		// Note: The actual Metal-to-Vulkan import would require VK_EXT_metal_objects
		// and proper Metal texture handle extraction, which is complex and not
		// fully supported in the current objc2 bindings
		tracing::warn!("Metal-to-Vulkan texture import not fully implemented");
		
		Ok(image)
	}

	fn cef_to_vk_format(&self) -> Result<vk::Format, String> {
		use cef::sys::cef_color_type_t;
		match self.format {
			// macOS IOSurfaces are typically BGRA
			cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(vk::Format::B8G8R8A8_UNORM),
			cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(vk::Format::R8G8B8A8_UNORM),
			_ => Err(format!("Unsupported CEF format for Vulkan: {:?}", self.format)),
		}
	}

	fn cef_to_hal_format(&self) -> Result<wgpu::TextureFormat, String> {
		use cef::sys::cef_color_type_t;
		match self.format {
			// macOS IOSurfaces are typically BGRA with sRGB
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
			label: Some("CEF IOSurface Texture (fallback)"),
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

		tracing::warn!("Using fallback CPU texture - IOSurface hardware acceleration not available");
		Ok(texture)
	}
}