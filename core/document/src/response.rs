use crate::LayerId;
use std::fmt;

#[derive(Debug, Clone)]
pub struct LayerPanelEntry {
	pub name: String,
	pub visible: bool,
	pub layer_type: LayerType,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
#[repr(C)]
// TODO - Make Copy when possible
pub enum DocumentResponse {
	UpdateCanvas { document: String },
	CollapseFolder { path: Vec<LayerId> },
	ExpandFolder { path: Vec<LayerId>, children: Vec<LayerPanelEntry> },
}

impl fmt::Display for DocumentResponse {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let name = match self {
			DocumentResponse::UpdateCanvas { .. } => "UpdateCanvas",
			DocumentResponse::CollapseFolder { .. } => "CollapseFolder",
			DocumentResponse::ExpandFolder { .. } => "ExpandFolder",
		};

		formatter.write_str(name)
	}
}
