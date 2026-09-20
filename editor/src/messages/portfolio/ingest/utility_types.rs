use crate::consts::{FILE_EXTENSION, GDD_FILE_EXTENSION};
use crate::messages::frontend::utility_types::FileFilter;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::DocumentId;
use document_container::archive::ArchiveFormat;
use graph_craft::document::NodeId;
use graphene_std::raster_nodes::color_lookup_table::LUT_FILE_EXTENSIONS;
use image::ImageFormat;
use std::ffi::OsStr;
use std::path::Path;

/// How many leading bytes are inspected to recognize a text format.
const SNIFFED_TEXT_LENGTH: usize = 4096;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(from_wasm_abi))]
pub enum IngestAction {
	Open,
	Import,
	Paste,
	DropOnCanvas {
		mouse: (f64, f64),
	},
	DropOnLayers {
		parent: LayerNodeIdentifier,
		insert_index: u32,
	},
	ResourceInput {
		document_id: DocumentId,
		node_id: NodeId,
		input_index: u32,
		/// The types this input takes, where none means any file.
		#[cfg_attr(feature = "wasm", tsify(type = "unknown"))]
		accepted_types: Vec<DataType>,
	},
}

impl IngestAction {
	/// Sends a file to a node's resource input, which takes only the types that its filters list.
	pub fn resource_input(document_id: DocumentId, node_id: NodeId, input_index: usize, filters: &[TypeFilter]) -> Self {
		Self::ResourceInput {
			document_id,
			node_id,
			input_index: input_index as u32,
			accepted_types: filters.iter().flat_map(|filter| filter.types.iter().copied()).collect(),
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DataType {
	GraphiteLegacy,
	Gdd,
	Svg,
	Raster(ImageFormat),
	Lut,
	#[default]
	Unknown,
}

impl DataType {
	/// The content decides, and the MIME type then the file name only settle what it leaves unknown.
	pub fn detect(data: &[u8], mime_type: &str, path: Option<&Path>) -> Self {
		match Self::from_content(data) {
			Self::Unknown => match Self::from_mime(mime_type) {
				Self::Unknown => path.map_or(Self::Unknown, Self::from_path),
				data_type => data_type,
			},
			data_type => data_type,
		}
	}

	pub fn from_mime(mime: &str) -> Self {
		match mime.to_ascii_lowercase().as_str() {
			"application/graphite+json" => Self::GraphiteLegacy,
			"application/vnd.graphite.document" => Self::Gdd,
			"image/svg+xml" => Self::Svg,
			mime => ImageFormat::from_mime_type(mime).map_or(Self::Unknown, Self::Raster),
		}
	}

	pub fn from_extension(extension: &str) -> Self {
		match extension.trim_start_matches('.').to_ascii_lowercase().as_str() {
			FILE_EXTENSION => Self::GraphiteLegacy,
			GDD_FILE_EXTENSION => Self::Gdd,
			"svg" => Self::Svg,
			extension if LUT_FILE_EXTENSIONS.contains(&extension) => Self::Lut,
			extension => ImageFormat::from_extension(extension).map_or(Self::Unknown, Self::Raster),
		}
	}

	pub fn from_path(path: impl AsRef<Path>) -> Self {
		path.as_ref().extension().and_then(OsStr::to_str).map_or(Self::Unknown, Self::from_extension)
	}

	pub fn from_content(data: &[u8]) -> Self {
		if ArchiveFormat::detect(data).is_some() {
			return Self::Gdd;
		}
		if let Ok(format) = image::guess_format(data) {
			return Self::Raster(format);
		}

		// Only the head is read as text, where a character split by the cut is the one invalid sequence tolerated
		let head = &data[..data.len().min(SNIFFED_TEXT_LENGTH)];
		let text = match std::str::from_utf8(head) {
			Ok(text) => text,
			Err(error) if error.error_len().is_none() => std::str::from_utf8(&head[..error.valid_up_to()]).unwrap_or_default(),
			Err(_) => return Self::Unknown,
		};

		let text = text.trim_start_matches('\u{feff}').trim_start();
		if text.starts_with('{') {
			Self::GraphiteLegacy
		} else if text.starts_with('<') && text.contains("<svg") {
			Self::Svg
		} else {
			Self::Unknown
		}
	}

	pub fn mime(self) -> Option<&'static str> {
		Some(match self {
			Self::GraphiteLegacy => "application/graphite+json",
			Self::Gdd => "application/vnd.graphite.document",
			Self::Svg => "image/svg+xml",
			Self::Raster(format) => format.to_mime_type(),
			// The LUT formats share no MIME type
			Self::Lut | Self::Unknown => return None,
		})
	}

	pub fn extensions(self) -> &'static [&'static str] {
		match self {
			Self::GraphiteLegacy => &[FILE_EXTENSION],
			Self::Gdd => &[GDD_FILE_EXTENSION],
			Self::Svg => &["svg"],
			Self::Raster(format) => format.extensions_str(),
			Self::Lut => LUT_FILE_EXTENSIONS,
			Self::Unknown => &[],
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeFilter {
	pub name: String,
	pub types: Vec<DataType>,
}

impl TypeFilter {
	pub fn documents() -> Self {
		Self {
			name: "Graphite Document".into(),
			types: vec![DataType::Gdd, DataType::GraphiteLegacy],
		}
	}

	pub fn raster() -> Self {
		Self {
			name: "Image".into(),
			types: ImageFormat::all().filter(ImageFormat::reading_enabled).map(DataType::Raster).collect(),
		}
	}

	pub fn image() -> Self {
		let mut images = Self::raster();
		images.types.push(DataType::Svg);
		images
	}

	pub fn lut() -> Self {
		Self {
			name: "LUT".into(),
			types: vec![DataType::Lut],
		}
	}
}

impl From<TypeFilter> for FileFilter {
	fn from(filter: TypeFilter) -> Self {
		Self {
			name: filter.name,
			extensions: filter.types.iter().flat_map(|data_type| data_type.extensions()).map(|extension| extension.to_string()).collect(),
			mime_types: filter.types.iter().filter_map(|data_type| data_type.mime()).map(str::to_string).collect(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use graphene_std::Color;
	use graphene_std::raster::Image;

	#[test]
	fn data_type_from_content() {
		assert_eq!(DataType::from_content(&Image::new(1, 1, Color::WHITE).to_png()), DataType::Raster(ImageFormat::Png));
		assert_eq!(DataType::from_content(b"PK\x03\x04"), DataType::Gdd);
		assert_eq!(DataType::from_content(b"{\"network_interface\":{}}"), DataType::GraphiteLegacy);
		assert_eq!(
			DataType::from_content("\u{feff}<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>".as_bytes()),
			DataType::Svg
		);
		assert_eq!(DataType::from_content(b"just text"), DataType::Unknown);
		assert_eq!(DataType::from_content(&[0xff, 0xfe, 0x00]), DataType::Unknown);
	}

	#[test]
	fn data_type_from_content_reads_only_the_head() {
		let late_svg = format!("<!--{}--><svg/>", " ".repeat(SNIFFED_TEXT_LENGTH));
		assert_eq!(DataType::from_content(late_svg.as_bytes()), DataType::Unknown);

		// The three-byte euro sign straddles the cut, which must not hide the text before it
		let straddling = format!("<svg>{}€", " ".repeat(SNIFFED_TEXT_LENGTH - 6));
		assert_eq!(DataType::from_content(straddling.as_bytes()), DataType::Svg);
	}

	#[test]
	fn data_type_detect() {
		let detect = |mime_type: &str, path: &str| DataType::detect(&[], mime_type, Some(Path::new(path)));
		assert_eq!(detect("", "photo.JPEG"), DataType::Raster(ImageFormat::Jpeg));
		assert_eq!(detect("image/svg+xml", ""), DataType::Svg);
		assert_eq!(detect("application/graphite+json", ""), DataType::GraphiteLegacy);
		assert_eq!(detect("image/png", "document.gdd"), DataType::Raster(ImageFormat::Png));
		assert_eq!(detect("", "grade.CUBE"), DataType::Lut);
		assert_eq!(detect("application/vnd.iccprofile", "profile.icm"), DataType::Lut);
		assert_eq!(detect("text/csv", "table.csv"), DataType::Unknown);
		assert_eq!(DataType::detect(&[], "", None), DataType::Unknown);

		let png = Image::new(1, 1, Color::WHITE).to_png();
		assert_eq!(DataType::detect(&png, "image/svg+xml", Some(Path::new("drawing.svg"))), DataType::Raster(ImageFormat::Png));
	}

	#[test]
	fn type_filter_to_file_filter() {
		let filter = FileFilter::from(TypeFilter::image());
		assert!(filter.extensions.iter().any(|extension| extension == "jpeg") && filter.extensions.iter().any(|extension| extension == "png"));
		for enabled in ["webp", "tiff", "ico", "tga", "hdr", "exr"] {
			assert!(filter.extensions.iter().any(|extension| extension == enabled), "{enabled} should be offered");
		}
		assert!(filter.extensions.last().is_some_and(|extension| extension == "svg"));
		assert!(filter.mime_types.contains(&"image/jpeg".to_string()) && filter.mime_types.contains(&"image/svg+xml".to_string()));

		// LUTs are picked by extension alone
		let filter = FileFilter::from(TypeFilter::lut());
		assert!(filter.extensions.iter().any(|extension| extension == "cube") && filter.mime_types.is_empty());
	}
}
