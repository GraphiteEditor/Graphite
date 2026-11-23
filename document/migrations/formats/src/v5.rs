//! Format version 5. No stored type changed: nodes in a chain or a stack store their position as an offset relative to the nodes
//! reading them, so the `ui::position` attribute no longer holds `Chain` or `Stack`. Never edit it: old documents decode through these definitions.

pub use super::v4::{
	AttributeDelta, AttributeValue, Attributes, BlobHash, Delta, ExportSlot, HotOp, HotOpId, HotSequence, Implementation, InputSlot, Network, NetworkField, NetworkId, Node, NodeField, NodeId,
	NodeInput, NonFinitePriority, PeerId, PeerRegistration, Prior, Priority, ProtoNode, Registry, RegistryDelta, ResourceEntry, ResourceStore, RetiredMarks, Rev, SessionState, SourceKey, SourceValue,
	TimeStamp, Tombstone, UserId, Value,
};
