use super::layer_info::LegacyLayerType;
use crate::document::LayerId;

use serde::{Deserialize, Serialize};

/// A layer that encapsulates other layers, including potentially more folders.
/// The contained layers are rendered in the same order they are stored.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct FolderLegacyLayer {
	/// The [Layer]s contained in the folder
	pub layers: Vec<LegacyLayerType>,
}

impl FolderLegacyLayer {
	pub fn layer(&self, layer_id: LayerId) -> Option<&LegacyLayerType> {
		None
	}

	pub fn layer_mut(&mut self, layer_id: LayerId) -> Option<&mut LegacyLayerType> {
		None
	}
}
