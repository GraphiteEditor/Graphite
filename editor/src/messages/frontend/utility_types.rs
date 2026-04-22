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

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
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

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
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
