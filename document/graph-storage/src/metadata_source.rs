//! Lets `from_runtime` read editor-side per-node metadata without depending on the editor crate.
//! The editor implements this on `NodeNetworkInterface`; tests pass [`NoMetadata`].
//!
//! `network_path` is the chain of runtime local `NodeId`s from the root down to (but not including)
//! the queried node, matching `NodeNetworkInterface::node_metadata(node_id, network_path)`.

use std::collections::HashMap;

use core_types::uuid::NodeId as RuntimeNodeId;

/// One node's editor-side metadata, produced by `Registry::to_runtime_with_metadata`. One entry per
/// node, since every node carries an identity to restore even when it carries no `ui::*` attribute.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeMetadataEntry {
	pub network_path: Vec<RuntimeNodeId>,
	pub local_id: RuntimeNodeId,
	/// The node's storage identity. Restoring it pins the node to this identity, so a later conversion
	/// keeps it instead of re-deriving one from the node's location.
	pub storage_id: crate::NodeId,
	pub position: Option<Position>,
	pub is_layer: bool,
	pub display_name: Option<String>,
	pub locked: bool,
	pub pinned: bool,
	/// Always sized to match the runtime node's `inputs.len()`; absent slots use `Default`. The rebuild
	/// returns an error if this length does not match the node's input count.
	pub input_metadata: Vec<InputMetadataEntry>,
	pub output_names: Vec<String>,
}

/// Per-network metadata. Separate from `NodeMetadataEntry` since these are
/// properties of a network, not of any node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkMetadataEntry {
	/// Owning-node chain from the root to (and including) the node containing this network.
	/// Empty = root network.
	pub network_path: Vec<RuntimeNodeId>,
	/// Stable storage id of this network. Lets the editor associate per-network, per-peer view state
	/// (the node-graph nav, in `session.json`) with a network across reparenting.
	pub network_id: crate::NetworkId,
	/// Matches the runtime's `NodeNetworkPersistentMetadata::reference`: the definition lineage tag.
	pub reference: Option<String>,
	/// The display order of the network's pinned nodes.
	pub pinned_order: Vec<RuntimeNodeId>,
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

	/// The storage identity this node is pinned to, if it has one. A node the source does not name
	/// falls back to a hash of its location.
	fn storage_node_id(&self, _network_path: &[RuntimeNodeId], _local_id: RuntimeNodeId) -> Option<crate::NodeId> {
		None
	}

	/// The definition the network was instantiated from, if it still names one.
	fn reference(&self, _network_path: &[RuntimeNodeId]) -> Option<&str> {
		None
	}

	/// The display order of the network's pinned nodes.
	fn pinned_order(&self, _network_path: &[RuntimeNodeId]) -> Vec<RuntimeNodeId> {
		Vec::new()
	}
}

/// No-op metadata source. Use when there's nothing to attach (synthetic networks, CLI tools).
pub struct NoMetadata;

impl NodeMetadataSource for NoMetadata {}

/// Unified storage-side position. The valid variants depend on `attr::node::ui::IS_LAYER`:
/// layers use `Absolute` or `Stack`; non-layer nodes use `Absolute` or `Chain`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Position {
	Absolute([i32; 2]),
	Chain,
	Stack(u32),
}
