//! macOS IOSurface texture import implementation

use super::common::{format, texture};
use super::{TextureImportError, TextureImportResult, TextureImporter};
use cef::{AcceleratedPaintInfo, sys::cef_color_type_t};
use metal::foreign_types::ForeignType;
use metal::{MTLPixelFormat, MTLTextureType, MTLTextureUsage, Texture};
use std::os::raw::c_void;
use wgpu::hal::api;

pub struct IOSurfaceImporter {
	pub handle: *mut c_void,
	pub format: cef_color_type_t,
	pub width: u32,
	pub height: u32,
}

impl TextureImporter for IOSurfaceImporter {
	fn new(info: &AcceleratedPaintInfo) -> Self {
		Self {
			handle: info.shared_texture_io_surface,
			format: *info.format.as_ref(),
			width: info.extra.coded_size.width as u32,
			height: info.extra.coded_size.height as u32,
		}
	}

	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult {
		// Try hardware acceleration first
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

		// Fallback to CPU texture
		texture::create_fallback(device, self.width, self.height, self.format, "CEF IOSurface Texture (fallback)")
	}

	fn supports_hardware_acceleration(&self, device: &wgpu::Device) -> bool {
		// Check if handle is valid
		if self.handle.is_null() {
			return false;
		}

		// Check if wgpu is using Metal backend
		self.is_metal_backend(device)
	}
}

impl IOSurfaceImporter {
	fn import_via_metal(&self, device: &wgpu::Device) -> TextureImportResult {
		// Get wgpu's Metal device
		use wgpu::wgc::api::Metal;
		let hal_texture = unsafe {
			device.as_hal::<api::Metal, _, _>(|hal_device| {
				let Some(hal_device) = hal_device else {
					return Err(TextureImportError::HardwareUnavailable {
						reason: "Device is not using Metal backend".to_string(),
					});
				};

				// Import IOSurface handle into Metal texture
				let metal_texture = self.import_iosurface_to_metal_texture(hal_device)?;

				// Wrap Metal texture in wgpu-hal texture
				// texture_from_raw signature: (texture, format, texture_type, mip_levels, sample_count, copy_extent)
				let hal_texture = <api::Metal as wgpu::hal::Api>::Device::texture_from_raw(
					metal_texture,
					format::cef_to_wgpu(self.format)?,
					MTLTextureType::D2,
					1, // mip_level_count
					1, // sample_count
					wgpu::hal::CopyExtent {
						width: self.width,
						height: self.height,
						depth: 1,
					},
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

	fn import_iosurface_to_metal_texture(&self, hal_device: &<api::Metal as wgpu::hal::Api>::Device) -> Result<Texture, TextureImportError> {
		// Validate dimensions
		if self.width == 0 || self.height == 0 {
			return Err(TextureImportError::InvalidHandle("Invalid IOSurface texture dimensions".to_string()));
		}

		// Get the Metal device from wgpu-hal
		let metal_device = hal_device.raw_device();

		// Convert CEF format to Metal pixel format
		let metal_format = self.cef_to_metal_format(self.format)?;

		// Create Metal texture from IOSurface using objc runtime
		// We need to use raw objc because the metal crate doesn't expose IOSurface creation directly
		#[allow(unexpected_cfgs)] // Suppress objc crate internal cfg warnings
		unsafe {
			use objc::runtime::Object;
			use objc::{class, msg_send, sel, sel_impl};

			let iosurface = self.handle;

			// Create texture descriptor using NSObject/Objective-C
			let descriptor_class = class!(MTLTextureDescriptor);
			let descriptor: *mut Object = msg_send![descriptor_class, new];

			// Set descriptor properties
			let _: () = msg_send![descriptor, setTextureType: MTLTextureType::D2];
			let _: () = msg_send![descriptor, setPixelFormat: metal_format];
			let _: () = msg_send![descriptor, setWidth: self.width as u64];
			let _: () = msg_send![descriptor, setHeight: self.height as u64];
			let _: () = msg_send![descriptor, setDepth: 1u64];
			let _: () = msg_send![descriptor, setMipmapLevelCount: 1u64];
			let _: () = msg_send![descriptor, setSampleCount: 1u64];
			let _: () = msg_send![descriptor, setArrayLength: 1u64];
			let _: () = msg_send![descriptor, setUsage: MTLTextureUsage::ShaderRead.bits()];

			// Get device pointer
			let device_ptr = metal_device.lock().as_ptr();

			// Call newTextureWithDescriptor:iosurface:plane:
			let metal_texture: *mut Object = msg_send![device_ptr, newTextureWithDescriptor:descriptor iosurface:iosurface plane:0u64];

			// Release the descriptor
			let _: () = msg_send![descriptor, release];

			if metal_texture.is_null() {
				return Err(TextureImportError::PlatformError {
					message: "Failed to create Metal texture from IOSurface".to_string(),
				});
			}

			// Cast to correct type and wrap in metal::Texture
			let mtl_texture = metal_texture as *mut metal::MTLTexture;
			Ok(Texture::from_ptr(mtl_texture))
		}
	}

	fn cef_to_metal_format(&self, format: cef_color_type_t) -> Result<MTLPixelFormat, TextureImportError> {
		match format {
			cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(MTLPixelFormat::BGRA8Unorm_sRGB),
			cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(MTLPixelFormat::RGBA8Unorm_sRGB),
			_ => Err(TextureImportError::UnsupportedFormat { format }),
		}
	}

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
