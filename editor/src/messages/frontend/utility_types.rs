use std::path::PathBuf;

use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::utility_types::WorkspacePanelLayout;
use crate::messages::prelude::*;

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentInfo {
	pub id: DocumentId,
	pub name: String,
	#[serde(default)]
	pub path: Option<PathBuf>,
	pub is_saved: bool,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints, from_wasm_abi))]
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PersistedState {
	pub documents: Vec<DocumentInfo>,
	pub current_document: Option<DocumentId>,
	#[serde(default)]
	pub workspace_layout: Option<WorkspacePanelLayout>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MouseCursorIcon {
	#[default]
	Default,
	None,
	ZoomIn,
	ZoomOut,
	Grabbing,
	Crosshair,
	Text,
	Move,
	NSResize,
	EWResize,
	NESWResize,
	NWSEResize,
	Rotate,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(from_wasm_abi))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FileType {
	#[default]
	Png,
	Jpg,
	Webp,
	Tiff,
	Bmp,
	Tga,
	Ico,
	Svg,
}

impl FileType {
	pub fn extension(self) -> &'static str {
		match self {
			FileType::Png => "png",
			FileType::Jpg => "jpg",
			FileType::Webp => "webp",
			FileType::Tiff => "tiff",
			FileType::Bmp => "bmp",
			FileType::Tga => "tga",
			FileType::Ico => "ico",
			FileType::Svg => "svg",
		}
	}

	pub fn file_filter(self) -> FileFilter {
		let name = match self {
			FileType::Png => "PNG Image",
			FileType::Jpg => "JPEG Image",
			FileType::Webp => "WEBP Image",
			FileType::Tiff => "TIFF Image",
			FileType::Bmp => "BMP Image",
			FileType::Tga => "TGA Image",
			FileType::Ico => "ICO Image",
			FileType::Svg => "SVG Image",
		};
		FileFilter {
			name: name.into(),
			extensions: vec![self.extension().into()],
			mime_types: Vec::new(),
		}
	}

	/// Encodes 8-bit RGBA pixels with straight alpha as a file of this raster type.
	pub fn encode(self, width: u32, height: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
		use image::buffer::ConvertBuffer;
		use image::{ImageFormat, RgbImage, RgbaImage};

		let Some(mut image) = RgbaImage::from_raw(width, height, rgba) else {
			return Err("Failed to create image buffer for export".to_string());
		};

		let format = match self {
			FileType::Png => ImageFormat::Png,
			FileType::Jpg => ImageFormat::Jpeg,
			FileType::Webp => ImageFormat::WebP,
			FileType::Tiff => ImageFormat::Tiff,
			FileType::Bmp => ImageFormat::Bmp,
			FileType::Tga => ImageFormat::Tga,
			FileType::Ico => ImageFormat::Ico,
			FileType::Svg => return Err("SVG cannot be exported from an image buffer".to_string()),
		};
		if self == FileType::Ico && (width > 256 || height > 256) {
			return Err("An ICO image can be at most 256 pixels wide and tall. Lower the scale factor or choose a smaller export area.".to_string());
		}

		let mut encoded = Vec::new();
		let mut cursor = std::io::Cursor::new(&mut encoded);

		let result = if self == FileType::Jpg {
			// Composite onto a white background since JPG doesn't support transparency
			for pixel in image.pixels_mut() {
				let [r, g, b, a] = pixel.0;
				let alpha = a as f32 / 255.;
				let blend = |channel: u8| (channel as f32 * alpha + 255. * (1. - alpha)).round() as u8;
				*pixel = image::Rgba([blend(r), blend(g), blend(b), 255]);
			}

			let image: RgbImage = image.convert();
			image.write_to(&mut cursor, format)
		} else {
			image.write_to(&mut cursor, format)
		};
		result.map_err(|error| format!("Failed to encode {self:?}: {error}"))?;

		Ok(encoded)
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExportBounds {
	#[default]
	AllArtwork,
	Selection,
	Artboard(LayerNodeIdentifier),
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EyedropperPreviewImage {
	pub data: serde_bytes::ByteBuf,
	pub width: u32,
	pub height: u32,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RasterizedImage {
	pub id: u64,
	pub width: u32,
	pub height: u32,
	pub pixels: serde_bytes::ByteBuf,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FileFilter {
	pub name: String,
	pub extensions: Vec<String>,
	#[serde(rename = "mimeTypes")]
	pub mime_types: Vec<String>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FileDialogOptions {
	pub filters: Vec<FileFilter>,
	pub multiple: bool,
}

#[cfg(test)]
mod tests {
	use super::*;
	use graphene_std::raster::Image;

	#[test]
	fn every_raster_file_type_encodes_an_image_that_reads_back() {
		// Opaque red, then fully transparent
		let pixels = vec![255, 0, 0, 255, 0, 0, 0, 0];

		for file_type in [FileType::Png, FileType::Jpg, FileType::Webp, FileType::Tiff, FileType::Bmp, FileType::Tga, FileType::Ico] {
			let encoded = file_type.encode(2, 1, pixels.clone()).unwrap_or_else(|error| panic!("{file_type:?}: {error}"));
			let decoded = Image::from_encoded(&encoded).unwrap_or_else(|| panic!("{file_type:?} should read back"));
			assert_eq!((decoded.width, decoded.height), (2, 1), "{file_type:?}");

			// Only JPG lacks transparency, so it lands on white
			let transparent = decoded.to_flat_u8().0[4..8].to_vec();
			if file_type == FileType::Jpg {
				assert!(transparent.iter().all(|&channel| channel > 250), "{transparent:?}");
			} else {
				assert_eq!(transparent[3], 0, "{file_type:?}");
			}
		}
	}

	#[test]
	fn file_types_that_cannot_hold_the_image_say_so() {
		assert!(FileType::Svg.encode(1, 1, vec![0; 4]).is_err());
		assert!(FileType::Png.encode(2, 2, vec![0; 4]).is_err());

		let too_large = FileType::Ico.encode(257, 1, vec![0; 257 * 4]).unwrap_err();
		assert!(too_large.contains("256 pixels"), "{too_large}");
		assert!(FileType::Ico.encode(256, 1, vec![0; 256 * 4]).is_ok());
	}
}
