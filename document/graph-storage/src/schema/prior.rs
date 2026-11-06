use super::{AttributeValue, ExportSlot, Implementation, InputSlot, Network, NetworkId, Node, NodeId, PeerId, PeerRegistration, ResourceEntry, TimeStamp, Tombstone};
use graphene_resource::ResourceId;
use serde::{Deserialize, Serialize};

/// What a slot held before a delta wrote it, stamps included. Undo puts it back bypassing LWW, so the registry is again
/// the fold of history up to the delta's parent. Never sent or applied as an op.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Prior {
	/// The whole entity, live, tombstoned, or absent. Restoring an absent one leaves no tombstone, so undoing an addition erases it.
	Node {
		id: NodeId,
		node: Node,
	},
	RemovedNode {
		id: NodeId,
		tombstone: Tombstone<Node>,
	},
	AbsentNode {
		id: NodeId,
	},
	Network {
		id: NetworkId,
		network: Network,
	},
	RemovedNetwork {
		id: NetworkId,
		tombstone: Tombstone<Network>,
	},
	AbsentNetwork {
		id: NetworkId,
	},
	Resource {
		id: ResourceId,
		resource: ResourceEntry,
	},
	RemovedResource {
		id: ResourceId,
		tombstone: Tombstone<ResourceEntry>,
	},
	AbsentResource {
		id: ResourceId,
	},
	/// One field of a live node, and the node's presence, which every write moves.
	NodeField {
		id: NodeId,
		presence: TimeStamp,
		field: NodeField,
	},
	/// One field of a live network, and its presence.
	NetworkField {
		id: NetworkId,
		presence: TimeStamp,
		field: NetworkField,
	},
	DocumentAttribute {
		key: String,
		previous: Option<AttributeValue>,
	},
	PeerUser {
		peer: PeerId,
		previous: Option<PeerRegistration>,
	},
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeField {
	/// Only the presence moved, by a reference from another entity's op.
	Presence,
	Attribute {
		key: String,
		previous: Option<AttributeValue>,
	},
	Input {
		index: u32,
		previous: InputSlot,
	},
	InputAttribute {
		index: u32,
		key: String,
		previous: Option<AttributeValue>,
	},
	Inputs {
		previous: Vec<InputSlot>,
		inputs_timestamp: TimeStamp,
	},
	Implementation {
		previous: Implementation,
		implementation_timestamp: TimeStamp,
	},
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NetworkField {
	/// Only the presence moved, by a node added to or implemented by it.
	Presence,
	Attribute {
		key: String,
		previous: Option<AttributeValue>,
	},
	Exports {
		previous: Vec<ExportSlot>,
	},
}
