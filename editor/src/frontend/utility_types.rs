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
		Self::Default
	}
}
