use graphene::LayerId;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Clone, Deserialize, Serialize, Debug)]
pub struct FrontendDocumentDetails {
	pub is_saved: bool,
	pub name: String,
	pub id: u64,
}

#[derive(PartialEq, Eq, Clone, Deserialize, Serialize, Debug)]
pub struct FrontendImageData {
	pub path: Vec<LayerId>,
	pub mime: String,
	pub image_data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, Deserialize, PartialEq, Serialize)]
pub enum MouseCursorIcon {
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

impl Default for MouseCursorIcon {
	fn default() -> Self {
		MouseCursorIcon::Default
	}
}

#[derive(Clone, Copy, Debug, Eq, Deserialize, PartialEq, Serialize)]
pub enum FileType {
	Svg,
	Png,
	Jpg,
}

impl Default for FileType {
	fn default() -> Self {
		FileType::Svg
	}
}

impl FileType {
	pub fn to_mime(self) -> &'static str {
		match self {
			FileType::Svg => "image/svg+xml",
			FileType::Png => "image/png",
			FileType::Jpg => "image/jpeg",
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, Deserialize, PartialEq, Serialize)]
pub enum ExportBounds {
	AllArtwork,
	Selection,
	Artboard(LayerId),
}

impl Default for ExportBounds {
	fn default() -> Self {
		ExportBounds::AllArtwork
	}
}
