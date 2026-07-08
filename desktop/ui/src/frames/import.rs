use cef::sys::cef_color_type_t;

#[cfg(target_os = "windows")]
pub(crate) mod d3d11;
#[cfg(target_os = "linux")]
pub(crate) mod dmabuf;
#[cfg(target_os = "macos")]
pub(crate) mod iosurface;

pub(crate) type TextureImportResult = Result<wgpu::Texture, TextureImportError>;

#[derive(Debug, thiserror::Error)]
pub(crate) enum TextureImportError {
	#[error("Invalid texture handle: {0}")]
	InvalidHandle(String),
	#[error("Unsupported texture format: {format:?}")]
	UnsupportedFormat { format: cef_color_type_t },
	#[error("Hardware acceleration not available: {reason}")]
	HardwareUnavailable { reason: String },
	#[error("Vulkan operation failed: {operation}")]
	VulkanError { operation: String },
	#[error("Platform-specific error: {message}")]
	PlatformError { message: String },
}

impl From<wgpu::hal::DeviceError> for TextureImportError {
	fn from(e: wgpu::hal::DeviceError) -> Self {
		TextureImportError::PlatformError {
			message: format!("wgpu-hal DeviceError: {:?}", e),
		}
	}
}

pub(crate) trait TextureImporter {
	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult;
}

fn wgpu_format(format: cef_color_type_t) -> Result<wgpu::TextureFormat, TextureImportError> {
	match format {
		cef_color_type_t::CEF_COLOR_TYPE_BGRA_8888 => Ok(wgpu::TextureFormat::Bgra8Unorm),
		cef_color_type_t::CEF_COLOR_TYPE_RGBA_8888 => Ok(wgpu::TextureFormat::Rgba8Unorm),
		_ => Err(TextureImportError::UnsupportedFormat { format }),
	}
}

fn texture_descriptor(width: u32, height: u32, format: cef_color_type_t, label: &'static str) -> Result<wgpu::TextureDescriptor<'static>, TextureImportError> {
	Ok(wgpu::TextureDescriptor {
		label: Some(label),
		size: wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu_format(format)?,
		usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
		view_formats: &[],
	})
}
