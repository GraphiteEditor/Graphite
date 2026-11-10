use crate::consts::{FILE_EXTENSION, GDD_FILE_EXTENSION};
use crate::messages::frontend::utility_types::FileFilter;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{ChainNodeCopy, CopiedDefinition, NodeTemplate};
use crate::messages::prelude::DocumentId;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use document_container::archive::ArchiveFormat;
use glam::DAffine2;
use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceId};
use graph_craft::document::NodeId;
use graphene_std::raster_nodes::color_lookup_table::LUT_FILE_EXTENSIONS;
use graphene_std::vector::VectorModification;
use image::ImageFormat;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const GDD_MIME: &str = "application/vnd.graphite.document";
const GRAPHITE_LEGACY_MIME: &str = "application/graphite+json";
const SVG_MIME: &str = "image/svg+xml";
const TEXT_MIME: &str = "text/plain";

const GRAPHITE_PREFIX: &str = "graphite: ";
const SNIFFED_TEXT_LENGTH: usize = 4096;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum IngestAction {
	Open,
	OpenUnique,
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
	Text,
	#[default]
	Unknown,
}

impl DataType {
	pub fn detect(data: &[u8], mime: Option<&str>, path: Option<&Path>) -> Self {
		match Self::from_content(data) {
			Self::Unknown => match mime.map_or(Self::Unknown, Self::from_mime) {
				Self::Unknown => path.map_or(Self::Unknown, Self::from_path),
				data_type => data_type,
			},
			data_type => data_type,
		}
	}

	pub fn from_mime(mime: &str) -> Self {
		match mime.to_ascii_lowercase().as_str() {
			GRAPHITE_LEGACY_MIME => Self::GraphiteLegacy,
			GDD_MIME => Self::Gdd,
			SVG_MIME => Self::Svg,
			TEXT_MIME => Self::Text,
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
			Self::GraphiteLegacy => GRAPHITE_LEGACY_MIME,
			Self::Gdd => GDD_MIME,
			Self::Svg => SVG_MIME,
			Self::Raster(format) => format.to_mime_type(),
			Self::Text => TEXT_MIME,
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
			Self::Text | Self::Unknown => &[],
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

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct DataTransfer(pub Vec<DataTransferItem>);

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DataTransferItem {
	pub data: TransferData,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub path: Option<PathBuf>,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TransferData {
	Text(String),
	GraphiteText(String),
	Svg(String),
	Raster { format: ImageFormat, data: Vec<u8> },
	Document(Vec<u8>),
	LegacyDocument(String),
	Lut(Vec<u8>),
	Other(Vec<u8>),
}

impl TransferData {
	pub fn data_type(&self) -> DataType {
		match self {
			Self::Svg(_) => DataType::Svg,
			Self::Raster { format, .. } => DataType::Raster(*format),
			Self::Document(_) => DataType::Gdd,
			Self::LegacyDocument(_) => DataType::GraphiteLegacy,
			Self::Lut(_) => DataType::Lut,
			Self::Text(_) | Self::GraphiteText(_) => DataType::Text,
			Self::Other(_) => DataType::Unknown,
		}
	}

	pub fn into_bytes(self) -> Vec<u8> {
		match self {
			Self::Text(text) | Self::GraphiteText(text) | Self::Svg(text) | Self::LegacyDocument(text) => text.into_bytes(),
			Self::Raster { data, .. } | Self::Document(data) | Self::Lut(data) | Self::Other(data) => data,
		}
	}
}

pub enum RawData {
	Text(String),
	Bytes(Vec<u8>),
}

impl RawData {
	pub fn into_string(self) -> Option<String> {
		match self {
			Self::Text(text) => Some(text),
			Self::Bytes(bytes) => String::from_utf8(bytes).ok(),
		}
	}

	pub fn into_bytes(self) -> Vec<u8> {
		match self {
			Self::Text(text) => text.into_bytes(),
			Self::Bytes(bytes) => bytes,
		}
	}
}

impl DataTransferItem {
	pub fn from_raw(data_type: DataType, data: RawData) -> Option<Self> {
		let data = match data_type {
			DataType::Text => {
				let text = data.into_string()?;
				match text.strip_prefix(GRAPHITE_PREFIX) {
					Some(graphite) => TransferData::GraphiteText(graphite.to_string()),
					None => TransferData::Text(text),
				}
			}
			DataType::Svg => TransferData::Svg(data.into_string()?),
			DataType::Raster(format) => TransferData::Raster { format, data: data.into_bytes() },
			_ => return None,
		};
		Some(data.into())
	}

	pub fn from_file(path: PathBuf, mime: Option<&str>, data: Vec<u8>) -> Self {
		let text = |data: Vec<u8>, into: fn(String) -> TransferData| String::from_utf8(data).map_or_else(|error| TransferData::Other(error.into_bytes()), into);
		let data = match DataType::detect(&data, mime, Some(&path)) {
			DataType::GraphiteLegacy => text(data, TransferData::LegacyDocument),
			DataType::Gdd => TransferData::Document(data),
			DataType::Svg => text(data, TransferData::Svg),
			DataType::Raster(format) => TransferData::Raster { format, data },
			DataType::Lut => TransferData::Lut(data),
			DataType::Text | DataType::Unknown => TransferData::Other(data),
		};
		Self { data, path: Some(path) }
	}

	pub fn into_raw(self) -> Option<(DataType, RawData)> {
		match self.data {
			TransferData::Text(text) => Some((DataType::Text, RawData::Text(text))),
			TransferData::GraphiteText(data) => Some((DataType::Text, RawData::Text(format!("{GRAPHITE_PREFIX}{data}")))),
			TransferData::Svg(svg) => Some((DataType::Svg, RawData::Text(svg))),
			TransferData::Raster { format, data } => Some((DataType::Raster(format), RawData::Bytes(data))),
			TransferData::Document(_) | TransferData::LegacyDocument(_) | TransferData::Lut(_) | TransferData::Other(_) => None,
		}
	}
}

impl From<TransferData> for DataTransferItem {
	fn from(data: TransferData) -> Self {
		Self { data, path: None }
	}
}

impl From<DataTransferItem> for DataTransfer {
	fn from(item: DataTransferItem) -> Self {
		Self(vec![item])
	}
}

impl From<TransferData> for DataTransfer {
	fn from(data: TransferData) -> Self {
		DataTransferItem::from(data).into()
	}
}

impl FromIterator<DataTransferItem> for DataTransfer {
	fn from_iter<T: IntoIterator<Item = DataTransferItem>>(iter: T) -> Self {
		Self(iter.into_iter().collect())
	}
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ApplicationDataItem {
	Layer(ApplicationDataLayer),
	Nodes(Vec<(NodeId, NodeTemplate)>),
	ChainNodes(Vec<ChainNodeCopy>),
	Vector(Vec<ApplicationDataVector>),
	Resource(ApplicationDataResource),
	Definition(Box<CopiedDefinition>),
}

pub type ApplicationDataVector = (LayerNodeIdentifier, VectorModification, DAffine2);

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ApplicationDataLayer {
	pub nodes: Vec<(NodeId, NodeTemplate)>,
	pub visible: bool,
	pub locked: bool,
	pub collapsed: bool,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ApplicationDataResource {
	pub id: ResourceId,
	pub hash: Option<ResourceHash>,
	pub sources: Vec<DataSource>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub data: Option<ResourceData>,
}

#[derive(PartialEq, Clone)]
pub struct ResourceData(pub Vec<u8>);
impl std::fmt::Debug for ResourceData {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ResourceData").field("len", &self.0.len()).finish()
	}
}
impl serde::Serialize for ResourceData {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&BASE64.encode(&self.0))
	}
}
impl<'de> serde::Deserialize<'de> for ResourceData {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		let encoded = String::deserialize(deserializer)?;
		Ok(Self(BASE64.decode(&encoded).map_err(serde::de::Error::custom)?))
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
		let detect = |mime: Option<&str>, path: &str| DataType::detect(&[], mime, Some(Path::new(path)));
		assert_eq!(detect(None, "photo.JPEG"), DataType::Raster(ImageFormat::Jpeg));
		assert_eq!(detect(Some("image/svg+xml"), ""), DataType::Svg);
		assert_eq!(detect(Some("application/graphite+json"), ""), DataType::GraphiteLegacy);
		assert_eq!(detect(Some("image/png"), "document.gdd"), DataType::Raster(ImageFormat::Png));
		assert_eq!(detect(None, "grade.CUBE"), DataType::Lut);
		assert_eq!(detect(Some("application/vnd.iccprofile"), "profile.icm"), DataType::Lut);
		assert_eq!(detect(Some("text/csv"), "table.csv"), DataType::Unknown);
		assert_eq!(DataType::detect(&[], None, None), DataType::Unknown);

		let png = Image::new(1, 1, Color::WHITE).to_png();
		assert_eq!(DataType::detect(&png, Some("image/svg+xml"), Some(Path::new("drawing.svg"))), DataType::Raster(ImageFormat::Png));
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

	#[test]
	fn graphite_data_travels_as_prefixed_text() {
		let (data_type, data) = DataTransferItem::from(TransferData::GraphiteText("[]".to_string())).into_raw().unwrap();
		assert_eq!(data_type, DataType::Text);
		assert_eq!(data.into_string().as_deref(), Some("graphite: []"));

		let item = |text: &[u8]| DataTransferItem::from_raw(DataType::Text, RawData::Bytes(text.to_vec())).map(|item| item.data);
		assert_eq!(item(b"graphite: []"), Some(TransferData::GraphiteText("[]".to_string())));
		assert_eq!(item(b"graphite"), Some(TransferData::Text("graphite".to_string())));
	}

	#[test]
	fn raw_data_lifts_by_mime_type() {
		let item = |mime: &str, data: &[u8]| DataTransferItem::from_raw(DataType::from_mime(mime), RawData::Bytes(data.to_vec())).map(|item| item.data);
		assert_eq!(item(SVG_MIME, b"<svg/>"), Some(TransferData::Svg("<svg/>".to_string())));
		assert_eq!(
			item("image/png", &[1, 2]),
			Some(TransferData::Raster {
				format: ImageFormat::Png,
				data: vec![1, 2]
			})
		);
		assert_eq!(item("text/html", b"<b>bold</b>"), None);
	}

	#[test]
	fn files_are_detected_by_content_first() {
		let png = Image::new(1, 1, Color::WHITE).to_png();
		let file = DataTransferItem::from_file("photo.jpg".into(), Some("image/jpeg"), png.clone());
		assert_eq!(file.data, TransferData::Raster { format: ImageFormat::Png, data: png });
		assert_eq!(file.path, Some(PathBuf::from("photo.jpg")));

		assert_eq!(
			DataTransferItem::from_file("document.gdd".into(), None, b"PK\x03\x04".to_vec()).data,
			TransferData::Document(b"PK\x03\x04".to_vec())
		);
		assert_eq!(DataTransferItem::from_file("grade.cube".into(), None, b"LUT".to_vec()).data, TransferData::Lut(b"LUT".to_vec()));
		assert_eq!(DataTransferItem::from_file("notes.txt".into(), None, b"notes".to_vec()).data, TransferData::Other(b"notes".to_vec()));
	}

	#[test]
	fn data_transfer_round_trips_through_serde() {
		let transfer: DataTransfer = [
			DataTransferItem::from(TransferData::GraphiteText("[]".to_string())),
			DataTransferItem::from_file("drawing.svg".into(), None, b"<svg/>".to_vec()),
		]
		.into_iter()
		.collect();

		let json = serde_json::to_string(&transfer).unwrap();
		assert_eq!(serde_json::from_str::<DataTransfer>(&json).unwrap(), transfer);
	}
}
