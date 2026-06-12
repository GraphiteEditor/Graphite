use crate::{Attributes, Network, NetworkId, Node, NodeId, PeerId, ResourceId, ResourceStore, SourceKey, TimeStamp, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Registry {
	pub node_instances: HashMap<NodeId, Node>,
	pub networks: HashMap<NetworkId, Network>,
	/// Content-addressable resources (images, fonts, eventually proto-node declarations) referenced
	/// by `ResourceId`. See [`ResourceStore`].
	pub resources: ResourceStore,
	/// Append-only mapping from per-device `PeerId` to per-human `UserId`.
	/// Registered by each device's first contribution via `RegistryDelta::RegisterPeer`.
	pub peer_users: HashMap<PeerId, UserId>,
	pub attributes: Attributes,
}

impl Registry {
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
	if a.len() != b.len() {
		return false;
	}
	a.iter().all(|(key, value)| b.get(key).is_some_and(|other| value.value == other.value))
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
				&& entry.sources.len() == other.sources.len()
				&& entry.sources.iter().all(|(key, value)| other.source(key).is_some_and(|other_value| value.source == other_value.source))
		})
	})
}

/// Stable identity for any timestamped slot in a `Registry`. Used by `order_consistent`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum TimestampKey {
	NodeInput(NodeId, usize),
	NodeInputAttribute(NodeId, usize, String),
	NodeAttribute(NodeId, String),
	NetworkExport(NetworkId, usize),
	NetworkAttribute(NetworkId, String),
	DocumentAttribute(String),
	ResourceHash(ResourceId),
	ResourceSource(ResourceId, SourceKey),
}

fn collect_timestamps(registry: &Registry) -> HashMap<TimestampKey, TimeStamp> {
	let mut out = HashMap::new();
	for (node_id, node) in &registry.node_instances {
		for (i, slot) in node.inputs.iter().enumerate() {
			out.insert(TimestampKey::NodeInput(*node_id, i), slot.timestamp);
			for (key, value) in &slot.attributes {
				out.insert(TimestampKey::NodeInputAttribute(*node_id, i, key.clone()), value.timestamp);
			}
		}
		for (key, value) in &node.attributes {
			out.insert(TimestampKey::NodeAttribute(*node_id, key.clone()), value.timestamp);
		}
	}
	for (network_id, network) in &registry.networks {
		for (i, slot) in network.exports.iter().enumerate() {
			out.insert(TimestampKey::NetworkExport(*network_id, i), slot.timestamp);
		}
		for (key, value) in &network.attributes {
			out.insert(TimestampKey::NetworkAttribute(*network_id, key.clone()), value.timestamp);
		}
	}
	for (key, value) in &registry.attributes {
		out.insert(TimestampKey::DocumentAttribute(key.clone()), value.timestamp);
	}
	for (id, entry) in &registry.resources {
		out.insert(TimestampKey::ResourceHash(*id), entry.hash_timestamp);
		for (source_key, source_value) in &entry.sources {
			out.insert(TimestampKey::ResourceSource(*id, *source_key), source_value.timestamp);
		}
	}
	out
}
