//! Lets `from_runtime` read editor-side per-node metadata without depending on the editor crate.
//! The editor implements this on `NodeNetworkInterface`; tests pass [`NoMetadata`].
//!
//! `network` is the location of the network holding the queried node, or of the queried network itself.

use std::collections::BTreeMap;

use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::document::NetworkLocation;

use crate::{NetworkReference, Position, Value};

/// One node's editor-side metadata, produced by `Registry::to_runtime_with_metadata`. One entry per
/// node, since every node carries an identity to restore even when it carries no `ui::*` attribute.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeMetadataEntry {
	pub network: NetworkLocation,
	pub local_id: RuntimeNodeId,
	pub position: Option<Position>,
	pub is_layer: bool,
	pub display_name: Option<String>,
	pub locked: bool,
	pub pinned: bool,
}

/// Per-network metadata. Separate from `NodeMetadataEntry` since these are
/// properties of a network, not of any node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkMetadataEntry {
	pub network: NetworkLocation,
	/// Stable storage id of this network. Lets the editor associate per-network, per-peer view state
	/// (the node-graph nav, in `session.json`) with a network across reparenting.
	pub network_id: crate::NetworkId,
	/// Matches the runtime's `NodeNetworkPersistentMetadata::reference`: the definition lineage tag.
	pub reference: Option<String>,
	/// The display order of the network's pinned nodes.
	pub pinned_order: Vec<RuntimeNodeId>,
	pub import_metadata: Vec<InputMetadataEntry>,
	pub export_names: Vec<String>,
	pub export_descriptions: Vec<String>,
	/// The document's pixels per inch, where it differs from the default.
	pub pixels_per_inch: Option<f64>,
}

/// Editor metadata for one import of a subgraph. Mirrors `InputPersistentMetadata` but wraps strings in `Option` so
/// unset (`""` on the runtime side) is distinguishable from an explicit empty string.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InputMetadataEntry {
	pub input_name: Option<String>,
	pub input_description: Option<String>,
	pub widget_override: Option<String>,
	pub input_data: BTreeMap<String, Value>,
}

/// Editor-side metadata source. Methods default to "no data" so implementors only override what
/// they carry. Returns are type-erased as [`Value`] where the underlying types live editor-side (PTZ, etc.).
pub trait NodeMetadataSource {
	fn position(&self, _network: NetworkLocation, _local_id: RuntimeNodeId) -> Option<Position> {
		None
	}
	fn is_layer(&self, _network: NetworkLocation, _local_id: RuntimeNodeId) -> bool {
		false
	}
	fn display_name(&self, _network: NetworkLocation, _local_id: RuntimeNodeId) -> Option<&str> {
		None
	}
	fn locked(&self, _network: NetworkLocation, _local_id: RuntimeNodeId) -> bool {
		false
	}
	fn pinned(&self, _network: NetworkLocation, _local_id: RuntimeNodeId) -> bool {
		false
	}
	/// The definition the network was instantiated from, if it still names one.
	fn reference(&self, _network: NetworkLocation) -> Option<NetworkReference> {
		None
	}

	/// The display order of the network's pinned nodes.
	fn pinned_order(&self, _network: NetworkLocation) -> Vec<RuntimeNodeId> {
		Vec::new()
	}

	/// The network's pixels per inch, where it differs from the default.
	fn pixels_per_inch(&self, _network: NetworkLocation) -> Option<f64> {
		None
	}

	fn import_metadata(&self, _network: NetworkLocation) -> Vec<InputMetadataEntry> {
		Vec::new()
	}

	fn export_names(&self, _network: NetworkLocation) -> Vec<String> {
		Vec::new()
	}

	fn export_descriptions(&self, _network: NetworkLocation) -> Vec<String> {
		Vec::new()
	}
}

/// No-op metadata source. Use when there's nothing to attach (synthetic networks, CLI tools).
pub struct NoMetadata;

impl NodeMetadataSource for NoMetadata {}
