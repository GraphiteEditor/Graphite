use serde::{Deserialize, Serialize};

// ================
// LayerLegacyLayer
// ================

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct LayerLegacyLayer {
	/// The document node network that this layer contains
	pub network: graph_craft::document::NodeNetwork,
}
