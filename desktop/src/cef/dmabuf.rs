use std::os::fd::RawFd;

#[cfg(target_os = "linux")]
pub struct DmaBufTexture {
	pub fds: Vec<RawFd>,
	pub format: u32,
	pub modifier: u64,
	pub width: u32,
	pub height: u32,
	pub strides: Vec<u32>,
	pub offsets: Vec<u32>,
}

#[cfg(target_os = "linux")]
impl DmaBufTexture {
	pub fn import_to_wgpu(&self, device: &wgpu::Device) -> Result<wgpu::Texture, String> {
		// For now, return an error - full DMA-BUF support requires complex Vulkan integration
		// This would need to:
		// 1. Use wgpu's Vulkan backend to access VkDevice  
		// 2. Create VkImage from DMA-BUF using VK_EXT_image_drm_format_modifier
		// 3. Import the VkImage into wgpu using create_texture_from_hal
		
		tracing::debug!("DMA-BUF import requested: {}x{} format={:#x} modifier={:#x} planes={}", 
			self.width, self.height, self.format, self.modifier, self.fds.len());
			
		// For now, create a fallback CPU-based texture with the same dimensions
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
		
		// Note: This fallback texture is empty - a real implementation would
		// need to either import the DMA-BUF directly or copy the contents
		tracing::warn!("DMA-BUF import created fallback texture - hardware acceleration not fully implemented");
		
		Ok(texture)
	}
}

#[cfg(target_os = "linux")]
fn drm_format_to_wgpu(drm_format: u32) -> Result<wgpu::TextureFormat, String> {
	// Based on OBS's drm-format.cpp
	match drm_format {
		0x34325241 => Ok(wgpu::TextureFormat::Bgra8UnormSrgb), // DRM_FORMAT_ARGB8888
		0x34324241 => Ok(wgpu::TextureFormat::Rgba8UnormSrgb), // DRM_FORMAT_ABGR8888
		_ => Err(format!("Unsupported DRM format: {:#x}", drm_format)),
	}
}
