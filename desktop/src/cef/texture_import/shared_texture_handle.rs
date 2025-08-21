use cef::AcceleratedPaintInfo;

use super::{TextureImportError, TextureImportResult, TextureImporter};

pub(crate) enum SharedTextureHandle {
	#[cfg(target_os = "linux")]
	DmaBuf(super::dmabuf::DmaBufImporter),
	#[cfg(target_os = "windows")]
	D3D11(super::d3d11::D3D11Importer),
	#[cfg(target_os = "macos")]
	IOSurface(super::iosurface::IOSurfaceImporter),
	Unsupported,
}

impl SharedTextureHandle {
	pub(crate) fn new(info: &AcceleratedPaintInfo) -> Self {
		// Extract DMA-BUF information
		#[cfg(target_os = "linux")]
		return Self::DmaBuf(super::dmabuf::DmaBufImporter::new(info));

		// Extract D3D11 shared handle with texture metadata
		#[cfg(target_os = "windows")]
		return Self::D3D11(super::d3d11::D3D11Importer::new(info));

		// Extract IOSurface handle with texture metadata
		#[cfg(target_os = "macos")]
		return Self::IOSurface(super::iosurface::IOSurfaceImporter::new(info));

		#[allow(unreachable_code)]
		Self::Unsupported
	}

	/// Import a texture using the appropriate platform-specific importer
	pub(crate) fn import_texture(self, device: &wgpu::Device) -> TextureImportResult {
		match self {
			#[cfg(target_os = "linux")]
			SharedTextureHandle::DmaBuf(importer) => importer.import_to_wgpu(device),
			#[cfg(target_os = "windows")]
			SharedTextureHandle::D3D11(importer) => importer.import_to_wgpu(device),
			#[cfg(target_os = "macos")]
			SharedTextureHandle::IOSurface(importer) => importer.import_to_wgpu(device),
			SharedTextureHandle::Unsupported => Err(TextureImportError::UnsupportedPlatform),
		}
	}
}
