use crate::document::LayerData;
use graphene::layers::{BlendMode, LayerDataType};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerPanelEntry {
	pub name: String,
	pub visible: bool,
	pub blend_mode: BlendMode,
	pub opacity: f64,
	pub layer_type: LayerType,
	pub layer_data: LayerData,
	// TODO: Instead of turning the u64 into (u32, u32)s here, do that in the WASM translation layer
	pub path: Vec<(u32, u32)>,
	pub thumbnail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LayerType {
	Folder,
	Shape,
}

impl fmt::Display for LayerType {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let name = match self {
			LayerType::Folder => "Folder",
			LayerType::Shape => "Shape",
		};

		formatter.write_str(name)
	}
}

impl From<&LayerDataType> for LayerType {
	fn from(data: &LayerDataType) -> Self {
		use LayerDataType::*;
		match data {
			Folder(_) => LayerType::Folder,
			Shape(_) => LayerType::Shape,
		}
	}
}
