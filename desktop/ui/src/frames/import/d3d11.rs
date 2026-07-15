use super::{TextureImportError, TextureImportResult, TextureImporter, texture_descriptor, wgpu_format};
use cef::sys::cef_color_type_t;
use std::os::raw::c_void;
use wgpu::hal::api;

pub struct D3D11Importer {
	pub handle: *mut c_void,
	pub format: cef_color_type_t,
	pub width: u32,
	pub height: u32,
}

impl TextureImporter for D3D11Importer {
	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult {
		if self.handle.is_null() {
			return Err(TextureImportError::InvalidHandle("Null D3D11 shared texture handle".to_string()));
		}

		let is_d3d12_backend = unsafe { device.as_hal::<api::Dx12>().is_some() };

		if is_d3d12_backend {
			let texture = self.import_via_d3d12(device)?;
			return Ok(texture);
		}

		let texture = self.import_via_vulkan(device)?;
		tracing::trace!("Successfully imported D3D11 shared texture via Vulkan");
		Ok(texture)
	}
}

impl D3D11Importer {
	pub fn from_parts(handle: u64, width: u32, height: u32, format: cef_color_type_t) -> Self {
		Self {
			handle: handle as *mut c_void,
			format,
			width,
			height,
		}
	}

	fn import_via_d3d12(&self, device: &wgpu::Device) -> TextureImportResult {
		use wgpu::hal::api;
		let hal_texture = unsafe {
			let hal_device_guard = device.as_hal::<api::Dx12>();
			let Some(hal_device) = hal_device_guard else {
				return Err(TextureImportError::HardwareUnavailable {
					reason: "Device is not using D3D12 backend".to_string(),
				});
			};

			let d3d12_resource = self.import_d3d11_handle_to_d3d12(&hal_device)?;

			let hal_texture = <api::Dx12 as wgpu::hal::Api>::Device::texture_from_raw(
				d3d12_resource,
				wgpu_format(self.format)?,
				wgpu::TextureDimension::D2,
				wgpu::Extent3d {
					width: self.width,
					height: self.height,
					depth_or_array_layers: 1,
				},
				1, // mip_level_count
				1, // sample_count
			);

			Ok::<_, TextureImportError>(hal_texture)
		}?;

		let texture = unsafe { device.create_texture_from_hal::<api::Dx12>(hal_texture, &texture_descriptor(self.width, self.height, self.format, "CEF D3D11→D3D12 Shared Texture")?) };

		Ok(texture)
	}

	fn import_via_vulkan(&self, device: &wgpu::Device) -> TextureImportResult {
		use wgpu::{TextureUses, wgc::api::Vulkan};
		let hal_texture = unsafe {
			let hal_device_guard = device.as_hal::<Vulkan>();
			let Some(hal_device) = hal_device_guard else {
				return Err(TextureImportError::HardwareUnavailable {
					reason: "Device is not using Vulkan backend".to_string(),
				});
			};

			let hal_texture = <Vulkan as wgpu::hal::Api>::Device::texture_from_d3d11_shared_handle(
				&hal_device,
				windows::Win32::Foundation::HANDLE(self.handle),
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
					format: wgpu_format(self.format)?,
					usage: TextureUses::COPY_DST | TextureUses::COPY_SRC | TextureUses::RESOURCE,
					memory_flags: wgpu::hal::MemoryFlags::empty(),
					view_formats: vec![],
				},
			)
			.map_err(|e| TextureImportError::PlatformError {
				message: format!("Failed to import D3D11 shared handle into Vulkan: {:?}", e),
			})?;

			Ok::<_, TextureImportError>(hal_texture)
		}?;

		let texture = unsafe { device.create_texture_from_hal::<Vulkan>(hal_texture, &texture_descriptor(self.width, self.height, self.format, "CEF D3D11 Shared Texture")?) };

		Ok(texture)
	}

	fn import_d3d11_handle_to_d3d12(&self, hal_device: &<wgpu::hal::api::Dx12 as wgpu::hal::Api>::Device) -> Result<windows::Win32::Graphics::Direct3D12::ID3D12Resource, TextureImportError> {
		use windows::Win32::Graphics::Direct3D12::*;

		let d3d12_device = hal_device.raw_device();

		if self.width == 0 || self.height == 0 {
			return Err(TextureImportError::InvalidHandle("Invalid D3D11 texture dimensions".to_string()));
		}

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
