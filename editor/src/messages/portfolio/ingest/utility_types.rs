use crate::messages::frontend::utility_types::FileFilter;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use document_container::archive::ArchiveFormat;
use graph_craft::document::NodeId;
use image::ImageFormat;
use std::ffi::OsStr;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(from_wasm_abi))]
pub enum IngestAction {
	Open,
	Import,
	Paste,
	DropOnCanvas { mouse: (f64, f64) },
	DropOnLayers { parent: LayerNodeIdentifier, insert_index: usize },
	ResourceInput { node_id: NodeId, input_index: usize },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeHint {
	pub mime: String,
	pub extension: String,
}

impl TypeHint {
	pub fn new(mime: &str, path: impl AsRef<Path>) -> Self {
		Self {
			mime: mime.into(),
			extension: path.as_ref().extension().and_then(OsStr::to_str).unwrap_or_default().into(),
		}
	}
}

impl From<TypeHint> for DataType {
	fn from(hint: TypeHint) -> Self {
		match Self::from_mime(&hint.mime) {
			Self::Unknown => Self::from_extension(&hint.extension),
			mime => mime,
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DataType {
	GraphiteLegacy,
	Gdd,
	Svg,
	Raster(ImageFormat),
	#[default]
	Unknown,
}

impl DataType {
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
			"graphite" => Self::GraphiteLegacy,
			"gdd" => Self::Gdd,
			"svg" => Self::Svg,
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
		let Ok(text) = std::str::from_utf8(data) else { return Self::Unknown };
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
			Self::Unknown => return None,
		})
	}

	pub fn extensions(self) -> &'static [&'static str] {
		match self {
			Self::GraphiteLegacy => &["graphite"],
			Self::Gdd => &["gdd"],
			Self::Svg => &["svg"],
			Self::Raster(format) => format.extensions_str(),
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
}

impl From<TypeFilter> for FileFilter {
	fn from(filter: TypeFilter) -> Self {
		Self {
			name: filter.name,
			extensions: filter.types.iter().flat_map(|data_type| data_type.extensions()).map(|extension| extension.to_string()).collect(),
			mimes: filter.types.iter().filter_map(|data_type| data_type.mime()).map(str::to_string).collect(),
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
	fn data_type_from_hint() {
		let hint = |extension: &str, mime: &str| TypeHint {
			mime: mime.into(),
			extension: extension.into(),
		};
		assert_eq!(DataType::from(hint(".JPEG", "")), DataType::Raster(ImageFormat::Jpeg));
		assert_eq!(DataType::from(hint("", "image/svg+xml")), DataType::Svg);
		assert_eq!(DataType::from(hint("", "application/graphite+json")), DataType::GraphiteLegacy);
		assert_eq!(DataType::from(hint("gdd", "image/png")), DataType::Raster(ImageFormat::Png));
		assert_eq!(DataType::from(hint("csv", "text/csv")), DataType::Unknown);
		assert_eq!(DataType::from(TypeHint::default()), DataType::Unknown);
		assert_eq!(DataType::from_path("photo.jpg"), DataType::Raster(ImageFormat::Jpeg));
	}

	#[test]
	fn type_filter_to_file_filter() {
		let filter = FileFilter::from(TypeFilter::image());
		assert!(filter.extensions.iter().any(|extension| extension == "jpeg") && filter.extensions.iter().any(|extension| extension == "webp"));
		assert!(filter.extensions.last().is_some_and(|extension| extension == "svg"));
		assert!(filter.mimes.contains(&"image/jpeg".to_string()) && filter.mimes.contains(&"image/svg+xml".to_string()));
	}
}
