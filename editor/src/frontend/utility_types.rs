use graphene::LayerId;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub struct FrontendDocumentDetails {
	pub is_saved: bool,
	pub name: String,
	pub id: u64,
}

#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub struct FrontendImageData {
	pub path: Vec<LayerId>,
	pub mime: String,
	pub image_data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, Deserialize, PartialEq, Default, Serialize)]
pub enum MouseCursorIcon {
	#[default]
	Default,
	ZoomIn,
	ZoomOut,
	Grabbing,
	Crosshair,
	Text,
	NSResize,
	EWResize,
	NESWResize,
	NWSEResize,
}

#[derive(Clone, Copy, Debug, Eq, Deserialize, PartialEq, Default, Serialize)]
pub enum FileType {
	#[default]
	Svg,
	Png,
	Jpg,
}

impl FileType {
	pub fn to_mime(self) -> &'static str {
		match self {
			FileType::Svg => "image/svg+xml",
			FileType::Png => "image/png",
			FileType::Jpg => "image/jpg",
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, Deserialize, PartialEq, Default, Serialize)]
pub enum ExportBounds {
	#[default]
	All,
	Artboard(LayerId),
}
