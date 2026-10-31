//! Format version 2, frozen by `cargo run migration snapshot`. Never edit it: old documents decode through these definitions.

pub use super::v1::{
	AttributeDelta, AttributeValue, Attributes, Delta, ExportSlot, HotOp, HotOpId, HotSequence, Implementation, InputSlot, Network, NetworkField, NetworkId, Node, NodeField, NodeId, NodeInput,
	NonFinitePriority, PeerId, PeerRegistration, Prior, Priority, Registry, RegistryDelta, ResourceEntry, ResourceStore, RetiredMarks, Rev, SessionState, SourceKey, SourceValue, TimeStamp, Tombstone,
	UserId, Value,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtoNode {
	pub identifier: String,
	#[serde(default = "ProtoNode::first_version", skip_serializing_if = "ProtoNode::is_first_version")]
	pub version: u32,
	pub attributes: Attributes,
}

impl ProtoNode {
	pub fn first_version() -> u32 {
		1
	}
	pub fn is_first_version(version: &u32) -> bool {
		*version == 1
	}
}
