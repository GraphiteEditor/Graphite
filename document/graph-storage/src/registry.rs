use crate::{Attributes, Network, NetworkId, Node, NodeId, PeerId, ResourceEntry, ResourceId, ResourceStore, SourceKey, TimeStamp, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The live document. Every field, existence included, is last-writer-wins on a timestamp, so the registry depends on the
/// set of ops applied and not their order. A removed entity keeps a tombstone: an older op lands on it, a newer one
/// revives the entity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Registry {
	pub node_instances: HashMap<NodeId, Node>,
	pub networks: HashMap<NetworkId, Network>,
	/// Content-addressable resources (images, fonts, eventually proto-node declarations) referenced
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

/// A device's registration to a person, and when it was made.
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

impl Registry {
	/// The node under `id`, live or as removed, for a reference to a node the runtime cannot hold.
	#[cfg(any(feature = "conversion", test))]
	pub(crate) fn node_or_removed(&self, id: NodeId) -> Option<&Node> {
		self.node_instances.get(&id).or_else(|| self.removed_nodes.get(&id).map(|mark| &mark.content))
	}

	/// The network under `id`, live or as it was when removed.
	#[cfg(any(feature = "conversion", test))]
	pub(crate) fn network_or_removed(&self, id: NetworkId) -> Option<&Network> {
		self.networks.get(&id).or_else(|| self.removed_networks.get(&id).map(|mark| &mark.content))
	}

	/// True if both registries agree on every value-bearing field, ignoring per-slot and
	/// per-attribute timestamps. Mirrors `compute_deltas`'s value-only semantics, so unchanged
	/// state at a stamped slot doesn't count as drift. `peer_users` is excluded: it isn't diffed by
	/// `compute_deltas` (the mapping is injected on the commit path via `RegisterPeer`, never by a
	/// fresh `from_runtime` conversion), so a committed registry and a fresh conversion legitimately
	/// differ there without it counting as drift.
	pub fn value_equal(&self, other: &Self) -> bool {
		if !resources_value_equal(&self.resources, &other.resources) {
			return false;
		}
		if !attributes_value_equal(&self.attributes, &other.attributes) {
			return false;
		}

		if self.node_instances.len() != other.node_instances.len() {
			return false;
		}
		for (id, node) in &self.node_instances {
			let Some(other_node) = other.node_instances.get(id) else { return false };
			if !node.value_equal(other_node) {
				return false;
			}
		}

		if self.networks.len() != other.networks.len() {
			return false;
		}
		for (id, network) in &self.networks {
			let Some(other_network) = other.networks.get(id) else { return false };
			if !network.value_equal(other_network) {
				return false;
			}
		}

		true
	}

	/// True if the relative timestamp order on every shared timestamped slot agrees across
	/// the two registries. Catches LWW-bookkeeping bugs that `value_equal` deliberately ignores.
	///
	/// For every pair of shared keys (a, b), checks that `self[a].cmp(self[b])` and
	/// `other[a].cmp(other[b])` are compatible: `Equal` on either side is always compatible;
	/// otherwise both sides must agree on direction. Equality on one side imposes no order, so
	/// a registry with all-equal timestamps trivially passes against any other.
	///
	/// Slots present in only one registry are skipped. O(N²) in the number of shared timestamped
	/// slots; intended for debug-only use.
	pub fn order_consistent(&self, other: &Self) -> bool {
		let self_stamps = collect_timestamps(self);
		let other_stamps = collect_timestamps(other);

		let shared: Vec<(TimestampKey, TimeStamp, TimeStamp)> = self_stamps.into_iter().filter_map(|(key, ts)| other_stamps.get(&key).map(|other_ts| (key, ts, *other_ts))).collect();

		for i in 0..shared.len() {
			for j in (i + 1)..shared.len() {
				let self_order = shared[i].1.cmp(&shared[j].1);
				let other_order = shared[i].2.cmp(&shared[j].2);
				use std::cmp::Ordering::*;
				let compatible = matches!((self_order, other_order), (Equal, _) | (_, Equal) | (Less, Less) | (Greater, Greater));
				if !compatible {
					return false;
				}
			}
		}
		true
	}
}

pub(crate) fn attributes_value_equal(a: &Attributes, b: &Attributes) -> bool {
	if crate::attributes::live(a).count() != crate::attributes::live(b).count() {
		return false;
	}
	crate::attributes::live(a).all(|(key, value)| b.get(key).is_some_and(|other| !other.deleted && value.value == other.value))
}

/// Value-level resource comparison: same resolved hashes and same source chains (keyed by
/// `SourceKey`, comparing source bodies), ignoring LWW timestamps. Mirrors `attributes_value_equal`.
pub(crate) fn resources_value_equal(a: &ResourceStore, b: &ResourceStore) -> bool {
	if a.len() != b.len() {
		return false;
	}
	a.iter().all(|(id, entry)| {
		b.get(id).is_some_and(|other| {
			entry.hash == other.hash
				&& entry.live_sources().count() == other.live_sources().count()
				&& entry.live_sources().all(|(key, value)| other.source(key).is_some_and(|other_value| value.source == other_value.source))
		})
	})
}

/// Stable identity for any timestamp in a `Registry`, tombstoned content and existence stamps included. Used by
/// `order_consistent`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum TimestampKey {
	/// A whole-entity stamp under its field name: presence, a shape or floor, a removal.
	Node(NodeId, &'static str),
	NodeInput(NodeId, usize),
	NodeInputFloor(NodeId, usize),
	NodeInputAttribute(NodeId, usize, String),
	NodeAttribute(NodeId, String),
	Network(NetworkId, &'static str),
	NetworkExport(NetworkId, usize),
	NetworkAttribute(NetworkId, String),
	DocumentAttribute(String),
	Resource(ResourceId, &'static str),
	ResourceSource(ResourceId, SourceKey),
	PeerRegistration(PeerId),
}

fn collect_timestamps(registry: &Registry) -> HashMap<TimestampKey, TimeStamp> {
	let mut out = HashMap::new();
	let nodes = registry.node_instances.iter().chain(registry.removed_nodes.iter().map(|(id, mark)| (id, &mark.content)));
	for (&id, node) in nodes {
		for (field, timestamp) in [
			("presence", node.presence),
			("network", node.network_timestamp),
			("inputs", node.inputs_timestamp),
			("implementation", node.implementation_timestamp),
			("attributes", node.attributes_timestamp),
		] {
			out.insert(TimestampKey::Node(id, field), timestamp);
		}
		for (i, slot) in node.inputs.iter().enumerate() {
			out.insert(TimestampKey::NodeInput(id, i), slot.timestamp);
			out.insert(TimestampKey::NodeInputFloor(id, i), slot.attributes_timestamp);
			for (key, value) in &slot.attributes {
				out.insert(TimestampKey::NodeInputAttribute(id, i, key.clone()), value.timestamp);
			}
		}
		for (key, value) in &node.attributes {
			out.insert(TimestampKey::NodeAttribute(id, key.clone()), value.timestamp);
		}
	}
	let networks = registry.networks.iter().chain(registry.removed_networks.iter().map(|(id, mark)| (id, &mark.content)));
	for (&id, network) in networks {
		for (field, timestamp) in [("presence", network.presence), ("exports", network.exports_timestamp), ("attributes", network.attributes_timestamp)] {
			out.insert(TimestampKey::Network(id, field), timestamp);
		}
		for (i, slot) in network.exports.iter().enumerate() {
			out.insert(TimestampKey::NetworkExport(id, i), slot.timestamp);
		}
		for (key, value) in &network.attributes {
			out.insert(TimestampKey::NetworkAttribute(id, key.clone()), value.timestamp);
		}
	}
	for (key, value) in &registry.attributes {
		out.insert(TimestampKey::DocumentAttribute(key.clone()), value.timestamp);
	}
	let resources = registry.resources.iter().chain(registry.removed_resources.iter().map(|(id, mark)| (id, &mark.content)));
	for (&id, entry) in resources {
		for (field, timestamp) in [("presence", entry.presence), ("hash", entry.hash_timestamp), ("sources", entry.sources_timestamp)] {
			out.insert(TimestampKey::Resource(id, field), timestamp);
		}
		for (source_key, source_value) in &entry.sources {
			out.insert(TimestampKey::ResourceSource(id, *source_key), source_value.timestamp);
		}
	}
	for (&id, mark) in &registry.removed_nodes {
		out.insert(TimestampKey::Node(id, "removed"), mark.timestamp);
	}
	for (&id, mark) in &registry.removed_networks {
		out.insert(TimestampKey::Network(id, "removed"), mark.timestamp);
	}
	for (&id, mark) in &registry.removed_resources {
		out.insert(TimestampKey::Resource(id, "removed"), mark.timestamp);
	}
	for (&peer, registration) in &registry.peer_users {
		out.insert(TimestampKey::PeerRegistration(peer), registration.timestamp);
	}
	out
}
