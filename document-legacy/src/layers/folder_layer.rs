use super::layer_info::LegacyLayer;
use crate::document::LayerId;
use crate::DocumentError;

use serde::{Deserialize, Serialize};

/// A layer that encapsulates other layers, including potentially more folders.
/// The contained layers are rendered in the same order they are stored.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct FolderLegacyLayer {
	/// The IDs of the [Layer]s contained within the Folder
	pub layer_ids: Vec<LayerId>,
	/// The [Layer]s contained in the folder
	pub layers: Vec<LegacyLayer>,
}

impl FolderLegacyLayer {
	pub fn layer(&self, layer_id: LayerId) -> Option<&LegacyLayer> {
		let index = self.layer_ids.iter().position(|x| *x == layer_id).ok_or_else(|| DocumentError::LayerNotFound([layer_id].into())).ok()?;
		Some(&self.layers[index])
	}

	pub fn layer_mut(&mut self, layer_id: LayerId) -> Option<&mut LegacyLayer> {
		let index = self.layer_ids.iter().position(|x| *x == layer_id).ok_or_else(|| DocumentError::LayerNotFound([layer_id].into())).ok()?;
		Some(&mut self.layers[index])
	}
}
