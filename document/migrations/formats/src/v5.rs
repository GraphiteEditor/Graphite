//! Format version 5. No stored type changed: every node another node reads stores its position as an offset from the top left corner
//! of its readers, so the `ui::position` attribute no longer holds `Chain` or `Stack`. Never edit it: old documents decode through these definitions.

pub use super::v4::{
	AttributeDelta, AttributeValue, Attributes, BlobHash, Delta, ExportSlot, HotOp, HotOpId, HotSequence, Implementation, InputSlot, Network, NetworkField, NetworkId, Node, NodeField, NodeId,
	NodeInput, NonFinitePriority, PeerId, PeerRegistration, Prior, Priority, ProtoNode, Registry, RegistryDelta, ResourceEntry, ResourceStore, RetiredMarks, Rev, SessionState, SourceKey, SourceValue,
	TimeStamp, Tombstone, UserId, Value,
};
