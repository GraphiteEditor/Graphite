//! macOS IOSurface texture import implementation

#[cfg(target_os = "macos")]
use std::os::raw::c_void;

#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use core_foundation::base::{CFType, TCFType};
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use objc2_io_surface::{IOSurface, IOSurfaceRef};
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use objc2_metal::{MTLDevice, MTLPixelFormat, MTLTexture, MTLTextureDescriptor, MTLTextureType, MTLTextureUsage};
#[cfg(all(feature = "accelerated_paint", target_os = "macos"))]
use wgpu::hal::api;

use super::common::{TextureImportError, TextureImportResult, TextureImporter, format, texture};
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
				match self.import_via_metal(device) {
					Ok(texture) => {
						tracing::trace!("Successfully imported IOSurface texture via Metal");
						return Ok(texture);
					}
					Err(e) => {
						tracing::warn!("Failed to import IOSurface via Metal: {}, falling back to CPU texture", e);
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

			// Check if wgpu is using Metal backend
			self.is_metal_backend(device)
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
	fn import_via_metal(&self, device: &wgpu::Device) -> TextureImportResult {
		// Get wgpu's Metal device
		use wgpu::{hal::Api, wgc::api::Metal};
		let hal_texture = unsafe {
			device.as_hal::<api::Metal, _, _>(|device| {
				let Some(device) = device else {
					return Err(TextureImportError::HardwareUnavailable {
						reason: "Device is not using Metal backend".to_string(),
					});
				};

				// Import IOSurface handle into Metal texture
				let metal_texture = self.import_iosurface_to_metal(device)?;

				// Wrap Metal texture in wgpu-hal texture
				let hal_texture = <api::Metal as wgpu::hal::Api>::Device::texture_from_raw(
					metal_texture,
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
						usage: wgpu::hal::TextureUses::RESOURCE,
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
			device.create_texture_from_hal::<Metal>(
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
	fn import_iosurface_to_metal(&self, hal_device: &<api::Metal as wgpu::hal::Api>::Device) -> Result<<api::Metal as wgpu::hal::Api>::Texture, TextureImportError> {
		// Validate dimensions
		if self.width == 0 || self.height == 0 {
			return Err(TextureImportError::InvalidHandle("Invalid IOSurface texture dimensions".to_string()));
		}

		// Convert handle to IOSurface
		let iosurface = unsafe {
			let cf_type = CFType::wrap_under_get_rule(self.handle as IOSurfaceRef);
			IOSurface::from(cf_type)
		};

		// Get the Metal device from wgpu-hal
		let metal_device = hal_device.raw_device();

		// Convert CEF format to Metal pixel format
		let metal_format = self.cef_to_metal_format(self.format)?;

		// Create Metal texture descriptor
		let texture_descriptor = MTLTextureDescriptor::new();
		texture_descriptor.setTextureType(MTLTextureType::Type2D);
		texture_descriptor.setPixelFormat(metal_format);
		texture_descriptor.setWidth(self.width as usize);
		texture_descriptor.setHeight(self.height as usize);
		texture_descriptor.setDepth(1);
		texture_descriptor.setMipmapLevelCount(1);
		texture_descriptor.setSampleCount(1);
		texture_descriptor.setUsage(MTLTextureUsage::ShaderRead);

		// Create Metal texture from IOSurface
		let metal_texture = unsafe { metal_device.newTextureWithDescriptor_iosurface_plane(&texture_descriptor, &iosurface, 0) };

		let Some(metal_texture) = metal_texture else {
			return Err(TextureImportError::PlatformError {
				message: "Failed to create Metal texture from IOSurface".to_string(),
			});
		};

		tracing::trace!("Successfully created Metal texture from IOSurface");
		Ok(metal_texture)
	}

	#[cfg(feature = "accelerated_paint")]
	fn cef_to_metal_format(&self, format: cef_color_type_t) -> Result<MTLPixelFormat, TextureImportError> {
		match format {
			cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(MTLPixelFormat::BGRA8Unorm_sRGB),
			cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(MTLPixelFormat::RGBA8Unorm_sRGB),
			_ => Err(TextureImportError::UnsupportedFormat { format }),
		}
	}

	#[cfg(feature = "accelerated_paint")]
	fn is_metal_backend(&self, device: &wgpu::Device) -> bool {
		use wgpu::hal::api;
		let mut is_metal = false;
		unsafe {
			device.as_hal::<api::Metal, _, _>(|device| {
				is_metal = device.is_some();
			});
		}
		is_metal
	}
}
