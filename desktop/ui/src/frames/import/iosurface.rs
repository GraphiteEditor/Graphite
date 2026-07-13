use super::{TextureImportError, TextureImportResult, TextureImporter, texture_descriptor};
use cef::sys::cef_color_type_t;
use objc2::rc::Retained;
use objc2_io_surface::IOSurfaceRef;
use objc2_metal::{MTLDevice, MTLPixelFormat, MTLStorageMode, MTLTextureDescriptor, MTLTextureType, MTLTextureUsage};
use wgpu::TextureDescriptor;

use std::os::raw::c_void;

pub struct IOSurfaceImporter {
	pub handle: *mut c_void,
	pub format: cef_color_type_t,
	pub width: u32,
	pub height: u32,
}

impl TextureImporter for IOSurfaceImporter {
	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult {
		let texture = self.import_via_metal(device)?;
		tracing::trace!("Successfully imported IOSurface texture via Metal");
		Ok(texture)
	}
}

impl IOSurfaceImporter {
	pub fn from_parts(handle: *mut c_void, width: u32, height: u32, format: cef_color_type_t) -> Self {
		Self { handle, format, width, height }
	}

	fn get_metal_desc(&self, texture_desc: &TextureDescriptor) -> Result<Retained<MTLTextureDescriptor>, TextureImportError> {
		if self.width == 0 || self.height == 0 {
			return Err(TextureImportError::InvalidHandle("Invalid IOSurface texture dimensions".to_string()));
		}

		let metal_desc = MTLTextureDescriptor::new();
		unsafe {
			metal_desc.setWidth(texture_desc.size.width as _);
			metal_desc.setHeight(texture_desc.size.height as _);
			metal_desc.setArrayLength(texture_desc.array_layer_count() as _);
			metal_desc.setMipmapLevelCount(texture_desc.mip_level_count as _);
			metal_desc.setSampleCount(texture_desc.sample_count as _);
			metal_desc.setTextureType(MTLTextureType::Type2D);
			metal_desc.setPixelFormat(match texture_desc.format {
				wgpu::TextureFormat::Rgba8Unorm => MTLPixelFormat::RGBA8Unorm,
				wgpu::TextureFormat::Bgra8Unorm => MTLPixelFormat::BGRA8Unorm,
				_ => unimplemented!(),
			});
			metal_desc.setUsage(MTLTextureUsage::ShaderRead);
			metal_desc.setStorageMode(MTLStorageMode::Managed);
		}

		Ok(metal_desc)
	}

	fn import_via_metal(&self, device: &wgpu::Device) -> TextureImportResult {
		let io_surface = std::ptr::NonNull::new(self.handle.cast::<IOSurfaceRef>()).ok_or(TextureImportError::InvalidHandle("Invalid IOSurface handle".to_string()))?;

		let texture_desc = texture_descriptor(self.width, self.height, self.format, "Cef Texture")?;
		let hal_tex = {
			let metal_desc = self.get_metal_desc(&texture_desc)?;

			let texture = unsafe {
				let hal_device_guard = device.as_hal::<wgpu::wgc::api::Metal>();
				let Some(hal_device) = hal_device_guard else {
					return Err(TextureImportError::InvalidHandle("Failed to get Metal device from wgpu".to_string()));
				};

				let texture = hal_device
					.raw_device()
					.newTextureWithDescriptor_iosurface_plane(metal_desc.as_ref(), io_surface.as_ref(), 0)
					.ok_or(TextureImportError::InvalidHandle("Invalid IOSurface handle".to_string()))?;

				let hal_tex = <wgpu::wgc::api::Metal as wgpu::hal::Api>::Device::texture_from_raw(
					texture,
					texture_desc.format,
					MTLTextureType::Type2D,
					texture_desc.array_layer_count(),
					texture_desc.mip_level_count,
					wgpu::hal::CopyExtent {
						width: texture_desc.size.width,
						height: texture_desc.size.height,
						depth: texture_desc.array_layer_count(),
					},
				);

				Ok::<_, TextureImportError>(hal_tex)
			}?;
			texture
		};

		Ok(unsafe { device.create_texture_from_hal::<wgpu::wgc::api::Metal>(hal_tex, &texture_desc) })
	}
}
