use crate::document::LayerData;
use document_core::{layers::LayerDataTypes, LayerId};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LayerPanelEntry {
	pub name: String,
	pub visible: bool,
	pub layer_type: LayerType,
	pub layer_data: LayerData,
	pub path: Vec<LayerId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
