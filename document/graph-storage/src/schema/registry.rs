use super::{Attributes, Network, NetworkId, Node, NodeId, PeerId, ResourceEntry, ResourceStore, TimeStamp, UserId};
use graphene_resource::ResourceId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The live document. Every field, existence included, is last-writer-wins on a timestamp, so the registry depends on the
/// set of ops applied and not their order. A removed entity keeps a tombstone: an older op lands on it, a newer one
/// revives the entity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Registry {
	pub node_instances: HashMap<NodeId, Node>,
	pub networks: HashMap<NetworkId, Network>,
	/// Content-addressable resources (images, fonts, proto-node declarations) referenced
	/// by `ResourceId`. See [`ResourceStore`].
	pub resources: ResourceStore,
	/// Which person each device is, from `RegistryDelta::RegisterPeer`, so undo and authorship scope by person.
	pub peer_users: HashMap<PeerId, PeerRegistration>,
	pub attributes: Attributes,
	/// Tombstones of removed nodes, so an op on one is ordered against its removal whichever arrives first.
	#[serde(default)]
	pub removed_nodes: HashMap<NodeId, Tombstone<Node>>,
	/// Tombstones of removed networks; see [`removed_nodes`](Self::removed_nodes).
	#[serde(default)]
	pub removed_networks: HashMap<NetworkId, Tombstone<Network>>,
	/// Tombstones of removed resources; see [`removed_nodes`](Self::removed_nodes).
	#[serde(default)]
	pub removed_resources: HashMap<ResourceId, Tombstone<ResourceEntry>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerRegistration {
	pub user: UserId,
	pub timestamp: TimeStamp,
}

/// A removed entity's content, for reviving it, and its removal stamp.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tombstone<T> {
	pub content: T,
	pub timestamp: TimeStamp,
	/// Nothing has added the entity yet: it stays dead until an addition folds in, the content holding writes that came first.
	#[serde(default)]
	pub placeholder: bool,
}
