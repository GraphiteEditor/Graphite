//! Format version 3, frozen by `cargo run migration snapshot`. Never edit it: old documents decode through these definitions.

pub use super::v2::{
	AttributeDelta, AttributeValue, Attributes, Delta, ExportSlot, HotOp, HotOpId, HotSequence, Implementation, InputSlot, Network, NetworkField, NetworkId, Node, NodeField, NodeId, NodeInput,
	NonFinitePriority, PeerId, PeerRegistration, Prior, Priority, ProtoNode, Registry, RegistryDelta, ResourceEntry, ResourceStore, Rev, SourceKey, SourceValue, TimeStamp, Tombstone, UserId, Value,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetiredMarks {
	pub retired_up_to: HashMap<PeerId, HotSequence>,
	pub retired_runs: HashMap<PeerId, Vec<(HotSequence, HotSequence)>>,
	#[serde(default)]
	pub taken_back: HashMap<PeerId, Vec<(HotSequence, HotSequence)>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionState {
	#[serde(default)]
	pub peer_id: PeerId,
	#[serde(default)]
	pub user_id: UserId,
	#[serde(default)]
	pub head_rev: Option<Rev>,
	#[serde(default)]
	pub last_broadcast_rev: Option<Rev>,
	#[serde(default)]
	pub redo_stack: Vec<Rev>,
	#[serde(default)]
	pub next_node_counter: u64,
	#[serde(default)]
	pub last_hot_sequence: HotSequence,
	#[serde(default)]
	pub view_settings: BTreeMap<String, Value>,
	#[serde(default)]
	pub network_view_settings: BTreeMap<NetworkId, BTreeMap<String, Value>>,
	#[serde(default)]
	pub retired_marks: RetiredMarks,
	#[serde(default)]
	pub clock_counter: u64,
	#[serde(default)]
	pub shared: bool,
}
