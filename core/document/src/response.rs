use crate::{
	layers::{Layer, LayerDataTypes},
	LayerId,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerPanelEntry {
	pub name: String,
	pub visible: bool,
	pub layer_type: LayerType,
	pub collapsed: bool,
	pub path: Vec<LayerId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayerType {
	Folder,
	Shape,
	Circle,
	Rect,
	Line,
	PolyLine,
	Ellipse,
}

impl fmt::Display for LayerType {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let name = match self {
			LayerType::Folder => "Folder",
			LayerType::Shape => "Shape",
			LayerType::Rect => "Rectangle",
			LayerType::Line => "Line",
			LayerType::Circle => "Circle",
			LayerType::PolyLine => "Polyline",
			LayerType::Ellipse => "Ellipse",
		};

		formatter.write_str(name)
	}
}

impl From<&LayerDataTypes> for LayerType {
	fn from(data: &LayerDataTypes) -> Self {
		use LayerDataTypes::*;
		match data {
			Folder(_) => LayerType::Folder,
			Shape(_) => LayerType::Shape,
			Circle(_) => LayerType::Circle,
			Rect(_) => LayerType::Rect,
			Line(_) => LayerType::Line,
			PolyLine(_) => LayerType::PolyLine,
			Ellipse(_) => LayerType::Ellipse,
		}
	}
}

impl LayerPanelEntry {
	pub fn from_layer(layer: &Layer, path: Vec<LayerId>) -> Self {
		let layer_type: LayerType = (&layer.data).into();
		let name = layer.name.clone().unwrap_or_else(|| format!("Unnamed {}", layer_type));
		let collapsed = if let LayerDataTypes::Folder(f) = &layer.data { f.collapsed } else { true };
		Self {
			name,
			visible: layer.visible,
			layer_type,
			collapsed,
			path,
		}
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
