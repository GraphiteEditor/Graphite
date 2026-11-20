use cef::sys::cef_color_type_t;

use super::resample::Resampler;

#[cfg(target_os = "windows")]
pub(crate) mod d3d11;
#[cfg(target_os = "linux")]
pub(crate) mod dmabuf;
#[cfg(target_os = "macos")]
pub(crate) mod iosurface;

pub(crate) type TextureImportResult = Result<ImportedTexture, TextureImportError>;

#[derive(Debug, thiserror::Error)]
pub(crate) enum TextureImportError {
	#[error("Invalid texture handle: {0}")]
	InvalidHandle(String),
	#[error("Unsupported texture format: {format:?}")]
	UnsupportedFormat { format: cef_color_type_t },
	#[cfg(not(target_os = "macos"))]
	#[error("Hardware acceleration not available: {reason}")]
	HardwareUnavailable { reason: String },
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

#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct ContentRect {
	pub(crate) x: u32,
	pub(crate) y: u32,
	pub(crate) width: u32,
	pub(crate) height: u32,
	pub(crate) source_width: u32,
	pub(crate) source_height: u32,
}

#[derive(Clone, Copy)]
pub(crate) enum ContentMapping {
	Identity,
	Scaled(ContentRect),
}

impl ContentRect {
	pub(crate) fn mapping(self, width: u32, height: u32) -> ContentMapping {
		let valid = self.width > 0
			&& self.height > 0
			&& self.source_width > 0
			&& self.source_height > 0
			&& self.x.checked_add(self.width).is_some_and(|right| right <= width)
			&& self.y.checked_add(self.height).is_some_and(|bottom| bottom <= height);
		let full = self.x == 0 && self.y == 0 && (self.width, self.height) == (width, height) && (self.source_width, self.source_height) == (width, height);
		if valid && !full { ContentMapping::Scaled(self) } else { ContentMapping::Identity }
	}
}

impl TryFrom<&cef::AcceleratedPaintInfo> for ContentRect {
	type Error = TextureImportError;

	fn try_from(info: &cef::AcceleratedPaintInfo) -> Result<Self, Self::Error> {
		let invalid = || TextureImportError::InvalidHandle("Failed to create content rect".into());
		let content = &info.extra.content_rect;
		let width = u32::try_from(content.width).ok().filter(|&width| width > 0).ok_or_else(invalid)?;
		let height = u32::try_from(content.height).ok().filter(|&height| height > 0).ok_or_else(invalid)?;
		let source = &info.extra.source_size;
		let (source_width, source_height) = if info.extra.has_source_size != 0 && source.width > 0 && source.height > 0 {
			(source.width as u32, source.height as u32)
		} else {
			(width, height)
		};
		Ok(Self {
			x: content.x.max(0) as u32,
			y: content.y.max(0) as u32,
			width,
			height,
			source_width,
			source_height,
		})
	}
}

pub(crate) trait TextureImporter {
	fn import_to_wgpu(&self, device: &wgpu::Device) -> TextureImportResult;
}

pub(crate) struct ImportedTexture {
	texture: wgpu::Texture,
	ownership: Option<OwnershipTransfer>,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct OwnershipTransfer {
	acquire: wgpu::CommandBuffer,
	release: wgpu::CommandBuffer,
	state: wgpu::TextureUses,
}

impl From<wgpu::Texture> for ImportedTexture {
	fn from(texture: wgpu::Texture) -> Self {
		Self { texture, ownership: None }
	}
}

impl ImportedTexture {
	pub(super) fn copy(self, device: &wgpu::Device, queue: &wgpu_sync::Queue, resampler: &Resampler, content_rect: ContentRect) -> wgpu::Texture {
		let imported = self.texture;
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("CEF Frame Copy Encoder"),
		});
		let output = match content_rect.mapping(imported.width(), imported.height()) {
			ContentMapping::Identity => {
				let output = device.create_texture(&wgpu::TextureDescriptor {
					label: Some("CEF Imported Frame Copy"),
					size: imported.size(),
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: imported.format(),
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
					view_formats: &[],
				});
				encoder.copy_texture_to_texture(
					wgpu::TexelCopyTextureInfo {
						texture: &imported,
						mip_level: 0,
						origin: wgpu::Origin3d::ZERO,
						aspect: wgpu::TextureAspect::All,
					},
					wgpu::TexelCopyTextureInfo {
						texture: &output,
						mip_level: 0,
						origin: wgpu::Origin3d::ZERO,
						aspect: wgpu::TextureAspect::All,
					},
					imported.size(),
				);
				output
			}
			ContentMapping::Scaled(content_rect) => {
				let output = device.create_texture(&wgpu::TextureDescriptor {
					label: Some("CEF Imported Scaled Frame Copy"),
					size: wgpu::Extent3d {
						width: content_rect.source_width,
						height: content_rect.source_height,
						depth_or_array_layers: 1,
					},
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: imported.format(),
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
					view_formats: &[],
				});
				let size = wgpu::Extent3d {
					width: content_rect.width,
					height: content_rect.height,
					depth_or_array_layers: 1,
				};
				resampler.encode(
					&mut encoder,
					&imported,
					wgpu::Origin3d {
						x: content_rect.x,
						y: content_rect.y,
						z: 0,
					},
					size,
					&output,
				);
				output
			}
		};

		match self.ownership {
			Some(OwnershipTransfer { acquire, release, state }) => {
				encoder.transition_resources(
					std::iter::empty(),
					std::iter::once(wgpu::TextureTransition {
						texture: &imported,
						selector: None,
						state,
					}),
				);
				queue.submit([acquire, encoder.finish(), release]);
			}
			None => {
				queue.submit([encoder.finish()]);
			}
		}

		output
	}
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
