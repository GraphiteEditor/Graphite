use crate::LayerId;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerPanelEntry {
	pub name: String,
	pub visible: bool,
	pub layer_type: LayerType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayerType {
	Folder,
	Shape,
}

impl fmt::Display for LayerType {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let name = match self {
			LayerType::Folder => "folder",
			LayerType::Shape => "shape",
		};

		formatter.write_str(name)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
// TODO - Make Copy when possible
pub enum DocumentResponse {
	DocumentChanged,
	CollapseFolder { path: Vec<LayerId> },
	ExpandFolder { path: Vec<LayerId>, children: Vec<LayerPanelEntry> },
}

impl fmt::Display for DocumentResponse {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let name = match self {
			DocumentResponse::DocumentChanged { .. } => "DocumentChanged",
			DocumentResponse::CollapseFolder { .. } => "CollapseFolder",
			DocumentResponse::ExpandFolder { .. } => "ExpandFolder",
		};

		formatter.write_str(name)
	}
}
