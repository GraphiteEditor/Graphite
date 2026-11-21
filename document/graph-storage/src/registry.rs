use crate::{Attributes, Implementation, NetworkId, NodeId, ROOT_NETWORK, Registry, ResourceStore, Tombstone};
use std::collections::{HashMap, HashSet};

impl Registry {
	/// The node under `id`, live or as removed, for a reference to a node the runtime cannot hold.
	#[cfg(any(feature = "conversion", test))]
	pub(crate) fn node_or_removed(&self, id: NodeId) -> Option<&crate::Node> {
		self.node_instances.get(&id).or_else(|| self.removed_nodes.get(&id).map(|mark| &mark.content))
	}

	/// The network under `id`, live or as it was when removed.
	#[cfg(any(feature = "conversion", test))]
	pub(crate) fn network_or_removed(&self, id: NetworkId) -> Option<&crate::Network> {
		self.networks.get(&id).or_else(|| self.removed_networks.get(&id).map(|mark| &mark.content))
	}

	/// The live entities alone, for a document starting a history of its own: tombstones only order removals against ops
	/// that share this one.
	pub fn without_tombstones(&self) -> Registry {
		Registry {
			removed_nodes: HashMap::new(),
			removed_networks: HashMap::new(),
			removed_resources: HashMap::new(),
			..self.clone()
		}
	}

	/// The registry as `to_runtime` renders it. A removed network the root still reaches counts as present.
	pub(crate) fn as_rendered(&self) -> Registry {
		let mut nodes_by_network: HashMap<NetworkId, Vec<&crate::Node>> = HashMap::new();
		for node in self.node_instances.values() {
			nodes_by_network.entry(node.network).or_default().push(node);
		}

		let mut rendered = self.clone();
		let mut reached = HashSet::from([ROOT_NETWORK]);
		let mut pending = vec![ROOT_NETWORK];
		while let Some(network) = pending.pop() {
			for node in nodes_by_network.get(&network).into_iter().flatten() {
				if let Implementation::Network(id) = node.implementation
					&& reached.insert(id)
				{
					pending.push(id);
					if let Some(mark) = rendered.removed_networks.remove(&id) {
						rendered.networks.insert(id, mark.content);
					}
				}
			}
		}
		rendered
	}

	/// Whether both registries removed the same entities at the same times. Registries equal in value but not in their
	/// removal marks resolve later ops differently.
	pub fn removal_marks_equal(&self, other: &Self) -> bool {
		fn same<K: std::hash::Hash + Eq, T>(a: &HashMap<K, Tombstone<T>>, b: &HashMap<K, Tombstone<T>>) -> bool {
			a.len() == b.len()
				&& a.iter()
					.all(|(id, mark)| b.get(id).is_some_and(|other| other.timestamp == mark.timestamp && other.placeholder == mark.placeholder))
		}
		same(&self.removed_nodes, &other.removed_nodes) && same(&self.removed_networks, &other.removed_networks) && same(&self.removed_resources, &other.removed_resources)
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
}

pub(crate) fn attributes_value_equal(a: &Attributes, b: &Attributes) -> bool {
	if a.live().count() != b.live().count() {
		return false;
	}
	a.live().all(|(key, value)| b.get(key).is_some_and(|other| !other.deleted && value.value == other.value))
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
