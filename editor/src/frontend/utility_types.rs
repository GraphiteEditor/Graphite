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
pub enum ExportType {
	#[default]
	Svg,
	Png,
	Jpeg,
}

impl ExportType {
	pub fn to_mime(self) -> &'static str {
		match self {
			ExportType::Svg => "image/svg+xml",
			ExportType::Png => "image/png",
			ExportType::Jpeg => "image/jpeg",
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, Deserialize, PartialEq, Default, Serialize)]
pub enum ExportArea {
	#[default]
	All,
	Artboard(LayerId),
}
