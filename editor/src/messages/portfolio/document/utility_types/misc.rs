pub use super::layer_panel::{LayerMetadata, LayerPanelEntry};

use document_legacy::color::Color;
use document_legacy::document::Document as DocumentLegacy;
use document_legacy::LayerId;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone)]
pub struct DocumentSave {
	pub document: DocumentLegacy,
	pub layer_metadata: HashMap<Vec<LayerId>, LayerMetadata>,
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
	Average,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TargetDocument {
	Artboard,
	Artwork,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DocumentMode {
	DesignMode,
	SelectMode,
	GuideMode,
}

impl fmt::Display for DocumentMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			DocumentMode::DesignMode => write!(f, "Design Mode"),
			DocumentMode::SelectMode => write!(f, "Select Mode"),
			DocumentMode::GuideMode => write!(f, "Guide Mode"),
		}
	}
}

impl DocumentMode {
	pub fn icon_name(&self) -> String {
		match self {
			DocumentMode::DesignMode => "ViewportDesignMode".to_string(),
			DocumentMode::SelectMode => "ViewportSelectMode".to_string(),
			DocumentMode::GuideMode => "ViewportGuideMode".to_string(),
		}
	}
}

pub enum DocumentRenderMode<'a> {
	Root,
	OnlyBelowLayerInFolder(&'a [LayerId]),
	LayerCutout(&'a [LayerId], Color),
}
