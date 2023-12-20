use super::folder_layer::FolderLegacyLayer;
use super::layer_layer::LayerLegacyLayer;
use crate::DocumentError;

use core::fmt;
use serde::{Deserialize, Serialize};

// ===============
// LegacyLayerType
// ===============

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Represents different types of layers.
pub enum LegacyLayerType {
	/// A layer that wraps a [FolderLegacyLayer] struct.
	Folder(FolderLegacyLayer),
	/// A layer that wraps an [LayerLegacyLayer] struct.
	Layer(LayerLegacyLayer),
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

	/// Get a mutable reference to the Folder wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LegacyLayerType::Folder`.
	pub fn as_folder_mut(&mut self) -> Result<&mut FolderLegacyLayer, DocumentError> {
		match self {
			LegacyLayerType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotFolder),
		}
	}

	/// Get a reference to the Folder wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LegacyLayerType::Folder`.
	pub fn as_folder(&self) -> Result<&FolderLegacyLayer, DocumentError> {
		match self {
			LegacyLayerType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotFolder),
		}
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
				let layers = folder.layers.as_slice();
				self.stack.extend(layers);
			}
			layer
		})
	}
}
