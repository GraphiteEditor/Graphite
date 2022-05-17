pub use super::layer_panel::{layer_panel_entry, LayerMetadata, LayerPanelEntry, RawBuffer};
use graphene::document::Document as GrapheneDocument;
use graphene::LayerId;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

pub type DocumentSave = (GrapheneDocument, HashMap<Vec<LayerId>, LayerMetadata>);

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
	Average,
}

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TargetDocument {
	Artboard,
	Artwork,
}

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DocumentMode {
	DesignMode,
	SelectMode,
	GuideMode,
}

impl fmt::Display for DocumentMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let text = match self {
			DocumentMode::DesignMode => "Design Mode".to_string(),
			DocumentMode::SelectMode => "Select Mode".to_string(),
			DocumentMode::GuideMode => "Guide Mode".to_string(),
		};
		write!(f, "{}", text)
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
