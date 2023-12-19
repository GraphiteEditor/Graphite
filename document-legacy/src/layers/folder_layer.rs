use super::layer_info::{LegacyLayer, LegacyLayerType};
use crate::{DocumentError, LayerId};

use serde::{Deserialize, Serialize};

/// A layer that encapsulates other layers, including potentially more folders.
/// The contained layers are rendered in the same order they are stored.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct FolderLegacyLayer {
	/// The ID that will be assigned to the next layer that is added to the folder
	next_assignment_id: LayerId,
	/// The IDs of the [Layer]s contained within the Folder
	pub layer_ids: Vec<LayerId>,
	/// The [Layer]s contained in the folder
	pub layers: Vec<LegacyLayer>,
}

impl FolderLegacyLayer {
	/// Returns a list of [LayerId]s in the folder.
	pub fn list_layers(&self) -> &[LayerId] {
		self.layer_ids.as_slice()
	}

	/// Get references to all the [Layer]s in the folder.
	pub fn layers(&self) -> &[LegacyLayer] {
		self.layers.as_slice()
	}

	pub fn layer(&self, id: LayerId) -> Option<&LegacyLayer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&self.layers[pos])
	}

	pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut LegacyLayer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&mut self.layers[pos])
	}

	/// Tries to find the index of a layer with the given [LayerId] within the folder.
	/// This operation will fail if no layer with a matching ID is present in the folder.
	pub fn position_of_layer(&self, layer_id: LayerId) -> Result<usize, DocumentError> {
		self.layer_ids.iter().position(|x| *x == layer_id).ok_or_else(|| DocumentError::LayerNotFound([layer_id].into()))
	}

	/// Tries to get a reference to a folder with the given [LayerId].
	/// This operation will return `None` if either no layer with `id` exists
	/// in the folder, or the layer with matching ID is not a folder.
	pub fn folder(&self, id: LayerId) -> Option<&FolderLegacyLayer> {
		match self.layer(id) {
			Some(LegacyLayer {
				data: LegacyLayerType::Folder(folder),
				..
			}) => Some(folder),
			_ => None,
		}
	}
}
