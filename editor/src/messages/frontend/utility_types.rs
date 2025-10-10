use std::path::PathBuf;

use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

#[derive(PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OpenDocument {
	pub id: DocumentId,
	pub details: DocumentDetails,
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DocumentDetails {
	pub name: String,
	pub path: Option<PathBuf>,
	#[serde(rename = "isSaved")]
	pub is_saved: bool,
	#[serde(rename = "isAutoSaved")]
	pub is_auto_saved: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ExportBounds {
	#[default]
	AllArtwork,
	Selection,
	Artboard(LayerNodeIdentifier),
}
