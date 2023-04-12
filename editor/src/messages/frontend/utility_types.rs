use document_legacy::LayerId;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub struct FrontendDocumentDetails {
	#[serde(rename = "isAutoSaved")]
	pub is_auto_saved: bool,
	#[serde(rename = "isSaved")]
	pub is_saved: bool,
	pub name: String,
	pub id: u64,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub struct FrontendImageData {
	pub path: Vec<LayerId>,
	pub mime: String,
	#[serde(skip)]
	pub image_data: std::sync::Arc<Vec<u8>>,
	pub transform: Option<[f64; 6]>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, specta::Type)]
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, specta::Type)]
pub enum FileType {
	#[default]
	Png,
	Jpg,
	Svg,
}

impl FileType {
	pub fn to_mime(self) -> &'static str {
		match self {
			FileType::Png => "image/png",
			FileType::Jpg => "image/jpeg",
			FileType::Svg => "image/svg+xml",
		}
	}
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, specta::Type)]
pub enum ExportBounds {
	#[default]
	AllArtwork,
	Selection,
	Artboard(LayerId),
}
