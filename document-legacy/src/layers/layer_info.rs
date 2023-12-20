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

// ===========
// LegacyLayer
// ===========

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct LegacyLayer {
	/// The user-given name of the layer.
	pub name: Option<String>,
	/// The type of layer, such as folder or shape.
	pub data: LegacyLayerType,
}

impl LegacyLayer {
	/// Iterate over the layers encapsulated by this layer.
	/// If the [Layer type](Layer::data) is not a folder, the only item in the iterator will be the layer itself.
	/// If the [Layer type](Layer::data) wraps a [Folder](LegacyLayerType::Folder), the iterator will recursively yield all the layers contained in the folder as well as potential sub-folders.
	pub fn iter(&self) -> LegacyLayerIter<'_> {
		LegacyLayerIter { stack: vec![self] }
	}

	/// Get a mutable reference to the Folder wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LegacyLayerType::Folder`.
	pub fn as_folder_mut(&mut self) -> Result<&mut FolderLegacyLayer, DocumentError> {
		match &mut self.data {
			LegacyLayerType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotFolder),
		}
	}

	/// Get a reference to the Folder wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LegacyLayerType::Folder`.
	pub fn as_folder(&self) -> Result<&FolderLegacyLayer, DocumentError> {
		match &self.data {
			LegacyLayerType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotFolder),
		}
	}
}

// ===============
// LegacyLayerIter
// ===============

/// An iterator over the layers encapsulated by this layer.
/// See [Layer::iter] for more information.
#[derive(Debug, Default)]
pub struct LegacyLayerIter<'a> {
	pub stack: Vec<&'a LegacyLayer>,
}

impl<'a> Iterator for LegacyLayerIter<'a> {
	type Item = &'a LegacyLayer;

	fn next(&mut self) -> Option<Self::Item> {
		match self.stack.pop() {
			Some(layer) => {
				if let LegacyLayerType::Folder(folder) = &layer.data {
					let layers = folder.layers.as_slice();
					self.stack.extend(layers);
				};
				Some(layer)
			}
			None => None,
		}
	}
}
