use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub struct FrontendDocumentDetails {
	pub is_saved: bool,
	pub name: String,
	pub id: u64,
}

#[derive(Clone, Copy, Debug, Eq, Deserialize, PartialEq, Serialize)]
pub enum FrontendMouseCursor {
	Default,
	ZoomIn,
	ZoomOut,
	Grabbing,
	Crosshair,
}
