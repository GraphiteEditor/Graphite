//! Common utilities and traits for texture import across platforms

use crate::cef::texture_import::*;
use cef::sys::cef_color_type_t;
use wgpu::Device;

/// Common format conversion utilities
pub mod format {
	use super::*;

	/// Convert CEF color type to wgpu texture format
	pub fn cef_to_wgpu(format: cef_color_type_t) -> Result<wgpu::TextureFormat, TextureImportError> {
		match format {
			cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(wgpu::TextureFormat::Bgra8UnormSrgb),
			cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(wgpu::TextureFormat::Rgba8UnormSrgb),
			_ => Err(TextureImportError::UnsupportedFormat { format }),
		}
	}

	#[cfg(not(target_os = "macos"))]
	/// Convert CEF color type to Vulkan format
	pub fn cef_to_vulkan(format: cef_color_type_t) -> Result<ash::vk::Format, TextureImportError> {
		match format {
			cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(ash::vk::Format::B8G8R8A8_UNORM),
			cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(ash::vk::Format::R8G8B8A8_UNORM),
			_ => Err(TextureImportError::UnsupportedFormat { format }),
		}
	}
}

/// Common texture creation utilities
pub mod texture {
	use super::*;

	/// Create a fallback CPU texture with the given dimensions and format
	pub fn create_fallback(device: &Device, width: u32, height: u32, format: cef_color_type_t, label: &str) -> TextureImportResult {
		let wgpu_format = format::cef_to_wgpu(format)?;

		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some(label),
			size: wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu_format,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});

		tracing::warn!(
			"Using fallback CPU texture for CEF rendering ({}x{}, {:?}) - hardware acceleration failed or unavailable. Consider checking GPU driver support.",
			width,
			height,
			format
		);
		Ok(texture)
	}
}

/// Common Vulkan utilities
#[cfg(not(target_os = "macos"))]
pub mod vulkan {
	use super::*;
	use ash::vk;

	/// Find a suitable memory type index for Vulkan allocation
	pub fn find_memory_type_index(type_filter: u32, properties: vk::MemoryPropertyFlags, mem_properties: &vk::PhysicalDeviceMemoryProperties) -> Option<u32> {
		(0..mem_properties.memory_type_count).find(|&i| (type_filter & (1 << i)) != 0 && mem_properties.memory_types[i as usize].property_flags.contains(properties))
	}

	/// Check if the wgpu device is using Vulkan backend
	pub fn is_vulkan_backend(device: &Device) -> bool {
		use wgpu::hal::api;
		let mut is_vulkan = false;
		unsafe {
			device.as_hal::<api::Vulkan, _, _>(|device| {
				is_vulkan = device.is_some();
			});
		}
		is_vulkan
	}

	/// Check if the wgpu device is using D3D12 backend
	#[cfg(target_os = "windows")]
	pub fn is_d3d12_backend(device: &Device) -> bool {
		use wgpu::hal::api;
		let mut is_d3d12 = false;
		unsafe {
			device.as_hal::<api::Dx12, _, _>(|device| {
				is_d3d12 = device.is_some();
			});
		}
		is_d3d12
	}
}
