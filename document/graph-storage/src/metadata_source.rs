//! Lets `from_runtime` read editor-side per-node metadata without depending on the editor crate.
//! The editor implements this on `NodeNetworkInterface`; tests pass [`NoMetadata`].
//!
//! `network_path` is the chain of runtime local `NodeId`s from the root down to (but not including)
//! the queried node, matching `NodeNetworkInterface::node_metadata(node_id, network_path)`.

use std::collections::HashMap;

use core_types::uuid::NodeId as RuntimeNodeId;

use crate::Position;

/// One node's editor-side metadata, produced by `Registry::to_runtime_with_metadata`.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeMetadataEntry {
	pub network_path: Vec<RuntimeNodeId>,
	pub local_id: RuntimeNodeId,
	pub position: Option<Position>,
	pub is_layer: bool,
	pub display_name: Option<String>,
	pub locked: bool,
	pub pinned: bool,
	/// Always sized to match the runtime node's `inputs.len()`; absent slots use `Default`.
	/// Rebuild errors on length mismatch.
	pub input_metadata: Vec<InputMetadataEntry>,
	pub output_names: Vec<String>,
}

impl NodeMetadataEntry {
	pub fn is_empty(&self) -> bool {
		self.position.is_none()
			&& !self.is_layer
			&& self.display_name.is_none()
			&& !self.locked
			&& !self.pinned
			&& self.output_names.is_empty()
			&& self.input_metadata.iter().all(InputMetadataEntry::is_empty)
	}
}

/// Per-network metadata (navigation, previewing). Separate from `NodeMetadataEntry` since these are
/// properties of a network, not of any node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkMetadataEntry {
	/// Owning-node chain from the root to (and including) the node containing this network.
	/// Empty = root network.
	pub network_path: Vec<RuntimeNodeId>,
	/// Stable storage id of this network. Lets the editor associate per-network, per-peer view state
	/// (node-graph nav + previewing, in `session.json`) with a network across reparenting.
	pub network_id: crate::NetworkId,
	/// Matches the runtime's `NodeNetworkPersistentMetadata::reference` — definition lineage tag.
	pub reference: Option<String>,
}

impl NetworkMetadataEntry {
	pub fn is_empty(&self) -> bool {
		self.reference.is_none()
	}
}

/// Per-input editor metadata. Mirrors `InputPersistentMetadata` but wraps strings in `Option` so
/// unset (`""` on the runtime side) is distinguishable from an explicit empty string.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputMetadataEntry {
	pub input_name: Option<String>,
	pub input_description: Option<String>,
	pub widget_override: Option<String>,
	/// Reassembled from `ui::input_data::<sub_key>` attributes.
	pub input_data: HashMap<String, serde_json::Value>,
}

impl InputMetadataEntry {
	pub fn is_empty(&self) -> bool {
		self.input_name.is_none() && self.input_description.is_none() && self.widget_override.is_none() && self.input_data.is_empty()
	}
}

/// Editor-side metadata source. Methods default to "no data" so implementors only override what
/// they carry. Returns are JSON-shaped where the underlying types live editor-side (PTZ, etc.).
pub trait NodeMetadataSource {
	fn position(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId) -> Option<Position> {
		None
	}
	fn is_layer(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId) -> bool {
		false
	}
	fn display_name(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId) -> Option<&str> {
		None
	}
	fn locked(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId) -> bool {
		false
	}
	fn pinned(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId) -> bool {
		false
	}
	/// Empty vec = no overrides. Stored as a single `ui::output_names` attribute (whole-vec LWW).
	fn output_names(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId) -> Vec<String> {
		Vec::new()
	}

	fn input_name(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId, _input_index: usize) -> Option<&str> {
		None
	}
	fn input_description(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId, _input_index: usize) -> Option<&str> {
		None
	}
	fn widget_override(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId, _input_index: usize) -> Option<&str> {
		None
	}
	/// Returns owned to stay object-safe. Each entry is stored as `ui::input_data::<key>` for per-key LWW.
	fn input_data(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId, _input_index: usize) -> HashMap<String, serde_json::Value> {
		HashMap::new()
	}

	fn reference(&self, _network_path: &[RuntimeNodeId]) -> Option<&str> {
		None
	}
}

/// No-op metadata source. Use when there's nothing to attach (synthetic networks, CLI tools).
pub struct NoMetadata;

impl NodeMetadataSource for NoMetadata {}
