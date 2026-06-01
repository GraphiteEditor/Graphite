use std::path::PathBuf;

use graph_craft::application_io::resource::ResourceHash;

use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::utility_types::WorkspacePanelLayout;
use crate::messages::prelude::*;

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentInfo {
	pub id: DocumentId,
	pub name: String,
	#[serde(default)]
	#[cfg_attr(feature = "wasm", tsify(type = "unknown"))]
	pub resources: Option<Box<[ResourceHash]>>,
	#[serde(default)]
	pub path: Option<PathBuf>,
	pub is_saved: bool,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints, from_wasm_abi))]
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

	pub fn to_extension(self) -> &'static str {
		match self {
			FileType::Png => "png",
			FileType::Jpg => "jpg",
			FileType::Svg => "svg",
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

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationExport {
	pub fps: f64,
	pub start_seconds: f64,
	pub total_frames: u32,
}

impl Default for AnimationExport {
	fn default() -> Self {
		Self {
			fps: 30.,
			start_seconds: 0.,
			total_frames: 30,
		}
	}
}

impl AnimationExport {
	pub fn frame_time_seconds(&self, frame_index: u32) -> f64 {
		self.start_seconds + frame_index as f64 / self.fps
	}
}

/// One frame of a multi-frame animation export, in playback order.
/// `Svg` is text that the frontend can save directly (for SVG export) or rasterize via canvas (for PNG/JPG export).
/// `Bytes` is already-encoded image bytes from the GPU export path, ready to write directly.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExportAnimationFrame {
	Svg(String),
	Bytes(serde_bytes::ByteBuf),
}

/// Replace characters that have special meaning in filesystem paths (or are otherwise unsafe in filenames) with `_`.
/// Used to neutralize user-controlled strings — document names, artboard names — before they become export filenames,
/// so a name like `..\evil` or `foo/bar` can't escape the export folder or create nested zip entries.
pub fn sanitize_filename_component(name: &str) -> String {
	let cleaned: String = name
		.chars()
		.map(|c| match c {
			'/' | '\\' | ':' | '*' | '?' | '<' | '>' | '|' | '"' | '\0' => '_',
			c if c.is_control() => '_',
			c => c,
		})
		.collect();
	// Strip leading/trailing dots and whitespace; Windows treats trailing `.` / ` ` as hidden and `..` is the parent dir.
	let trimmed = cleaned.trim().trim_matches('.').trim();
	if trimmed.is_empty() { "Untitled".to_string() } else { trimmed.to_string() }
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EyedropperPreviewImage {
	pub data: serde_bytes::ByteBuf,
	pub width: u32,
	pub height: u32,
}
