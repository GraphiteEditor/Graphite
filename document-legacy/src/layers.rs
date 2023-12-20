use core::fmt;
use serde::{Deserialize, Serialize};

// ===============
// LegacyLayerType
// ===============

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum LegacyLayerType {
	Folder(Vec<LegacyLayerType>),
	Layer(graph_craft::document::NodeNetwork),
}

impl Default for LegacyLayerType {
	fn default() -> Self {
		LegacyLayerType::Layer(Default::default())
	}
}

impl LegacyLayerType {
	/// Iterate over the layers encapsulated by this layer.
	/// If the [Layer type](Layer::data) is not a folder, the only item in the iterator will be the layer itself.
	/// If the [Layer type](Layer::data) wraps a [Folder](LegacyLayerType::Folder), the iterator will recursively yield all the layers contained in the folder as well as potential sub-folders.
	pub fn iter(&self) -> LegacyLayerTypeIter<'_> {
		LegacyLayerTypeIter { stack: vec![self] }
	}
}

// =========================
// LayerDataTypeDiscriminant
// =========================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, specta::Type)]
pub enum LayerDataTypeDiscriminant {
	Folder,
	Layer,
}

impl fmt::Display for LayerDataTypeDiscriminant {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			LayerDataTypeDiscriminant::Folder => write!(f, "Folder"),
			LayerDataTypeDiscriminant::Layer => write!(f, "Layer"),
		}
	}
}

impl From<&LegacyLayerType> for LayerDataTypeDiscriminant {
	fn from(data: &LegacyLayerType) -> Self {
		use LegacyLayerType::*;

		match data {
			Folder(_) => LayerDataTypeDiscriminant::Folder,
			Layer(_) => LayerDataTypeDiscriminant::Layer,
		}
	}
}

// ===================
// LegacyLayerTypeIter
// ===================

/// An iterator over the layers encapsulated by this layer.
/// See [Layer::iter] for more information.
#[derive(Debug, Default)]
pub struct LegacyLayerTypeIter<'a> {
	pub stack: Vec<&'a LegacyLayerType>,
}

impl<'a> Iterator for LegacyLayerTypeIter<'a> {
	type Item = &'a LegacyLayerType;

	fn next(&mut self) -> Option<Self::Item> {
		self.stack.pop().map(|layer| {
			if let LegacyLayerType::Folder(folder) = layer {
				self.stack.extend(folder.as_slice());
			}
			layer
		})
	}
}
