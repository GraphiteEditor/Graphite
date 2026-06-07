use std::collections::{HashMap, HashSet};

use crate::{AttributeDelta, ExportSlot, NetworkId, Node, NodeId, NodeInput, Registry, RegistryDelta, ResourceEntry, ResourceId, TimeStamp};

/// Minimal set of deltas to transform `from` into `to`.
///
/// Emits timestamp-less op shapes; the caller (`Document::commit_local` or equivalent) wraps each
/// in a `Delta` with a fresh clock tick.
pub fn compute_deltas(from: &Registry, to: &Registry) -> Vec<RegistryDelta> {
	let mut deltas = Vec::new();

	let from_network_ids: HashSet<NetworkId> = from.networks.keys().copied().collect();
	let to_network_ids: HashSet<NetworkId> = to.networks.keys().copied().collect();

	// AddNetwork before any AddNode that references it.
	for &network_id in to_network_ids.difference(&from_network_ids) {
		deltas.push(RegistryDelta::AddNetwork {
			network: network_id,
			contents: to.networks[&network_id].clone(),
		});
	}

	let from_node_ids: HashSet<NodeId> = from.node_instances.keys().copied().collect();
	let to_node_ids: HashSet<NodeId> = to.node_instances.keys().copied().collect();

	for &node_id in from_node_ids.difference(&to_node_ids) {
		deltas.push(RegistryDelta::RemoveNode {
			node_id,
			snapshot: from.node_instances[&node_id].clone(),
		});
	}

	for &node_id in to_node_ids.difference(&from_node_ids) {
		deltas.push(RegistryDelta::AddNode {
			node_id,
			node: to.node_instances[&node_id].clone(),
		});
	}

	for &node_id in from_node_ids.intersection(&to_node_ids) {
		let from_node = &from.node_instances[&node_id];
		let to_node = &to.node_instances[&node_id];

		// No `ChangeImplementation` op; the only path is remove + re-add. Same for input-count and
		// containing-network changes (a moved node has no in-place op either).
		let structural_change = !nodes_have_same_implementation(from_node, to_node) || from_node.inputs.len() != to_node.inputs.len() || from_node.network != to_node.network;
		if structural_change {
			deltas.push(RegistryDelta::RemoveNode { node_id, snapshot: from_node.clone() });
			deltas.push(RegistryDelta::AddNode { node_id, node: to_node.clone() });
			continue;
		}

		// Compare by value, ignoring the per-slot timestamp. Timestamps are derived from the diff
		// (assigned by the caller via clock.tick), not part of the diff itself: a slot whose value
		// is unchanged but whose timestamp differs should not emit a delta.
		for (input_idx, (from_slot, to_slot)) in from_node.inputs.iter().zip(&to_node.inputs).enumerate() {
			if from_slot.input != to_slot.input {
				deltas.push(RegistryDelta::ChangeNodeInput {
					node_id,
					input_idx,
					new_input: to_slot.input.clone(),
				});
			}
		}

		for delta in compute_attribute_deltas(&from_node.attributes, &to_node.attributes) {
			deltas.push(RegistryDelta::ChangeNodeAttribute { node_id, delta });
		}

		for (input_idx, (from_attrs, to_attrs)) in from_node.inputs_attributes.iter().zip(&to_node.inputs_attributes).enumerate() {
			for delta in compute_attribute_deltas(from_attrs, to_attrs) {
				deltas.push(RegistryDelta::ChangeNodeInputAttribute { node_id, input_idx, delta });
			}
		}
	}

	for &network_id in from_network_ids.difference(&to_network_ids) {
		deltas.push(RegistryDelta::RemoveNetwork {
			network: network_id,
			snapshot: from.networks[&network_id].clone(),
		});
	}

	for &network_id in from_network_ids.intersection(&to_network_ids) {
		let from_network = &from.networks[&network_id];
		let to_network = &to.networks[&network_id];

		let max_len = from_network.exports.len().max(to_network.exports.len());
		for slot_idx in 0..max_len {
			let from_slot = from_network.exports.get(slot_idx);
			let to_slot = to_network.exports.get(slot_idx);

			let from_target = from_slot.and_then(|s| s.target.as_ref());
			let to_target = to_slot.and_then(|s| s.target.as_ref());
			if from_target != to_target {
				deltas.push(RegistryDelta::SetExport {
					network: network_id,
					slot: slot_idx as u32,
					target: to_target.cloned(),
				});
			}
		}

		// Per-network attributes (`ui::nav::*`, ...).
		for delta in compute_attribute_deltas(&from_network.attributes, &to_network.attributes) {
			deltas.push(RegistryDelta::ChangeNetworkAttribute { network: network_id, delta });
		}
	}

	// Document-level attributes (`ui::doc::*`, format version, ...).
	for delta in compute_attribute_deltas(&from.attributes, &to.attributes) {
		deltas.push(RegistryDelta::ChangeDocumentAttribute { delta });
	}

	// Public library export list (whole-list LWW).
	if from.exported_nodes != to.exported_nodes {
		deltas.push(RegistryDelta::SetExportedNodes { nodes: to.exported_nodes.clone() });
	}

	compute_resource_deltas(from, to, &mut deltas);

	deltas
}

/// Diff the resource store, emitting whole-entry add/remove for resources that appear or vanish and
/// fine-grained hash/source ops for resources present in both. Value-only: per-entry and per-source
/// timestamps are derived by the caller, so an unchanged resource emits nothing.
fn compute_resource_deltas(from: &Registry, to: &Registry, deltas: &mut Vec<RegistryDelta>) {
	let from_ids: HashSet<ResourceId> = from.resources.keys().copied().collect();
	let to_ids: HashSet<ResourceId> = to.resources.keys().copied().collect();

	for &id in from_ids.difference(&to_ids) {
		deltas.push(RegistryDelta::RemoveResource {
			id,
			snapshot: from.resources[&id].clone(),
		});
	}

	for &id in to_ids.difference(&from_ids) {
		deltas.push(RegistryDelta::AddResource { id, entry: to.resources[&id].clone() });
	}

	for &id in from_ids.intersection(&to_ids) {
		diff_resource_entry(id, &from.resources[&id], &to.resources[&id], deltas);
	}
}

/// Per-entry diff for a resource present in both registries: hash change, then source chain
/// additions/changes/removals.
fn diff_resource_entry(id: ResourceId, from: &ResourceEntry, to: &ResourceEntry, deltas: &mut Vec<RegistryDelta>) {
	if from.hash != to.hash {
		deltas.push(RegistryDelta::SetResourceHash { id, hash: to.hash });
	}

	for (key, _) in &from.sources {
		if to.source(key).is_none() {
			deltas.push(RegistryDelta::RemoveSource { id, key: *key });
		}
	}

	// Compare source bodies only; the per-source timestamp is derived from the diff, not part of it.
	for (key, to_source) in &to.sources {
		if from.source(key).is_none_or(|from_source| from_source.source != to_source.source) {
			deltas.push(RegistryDelta::AddSource {
				id,
				key: *key,
				source: to_source.source.clone(),
			});
		}
	}
}

fn nodes_have_same_implementation(a: &Node, b: &Node) -> bool {
	use crate::Implementation::*;
	match (&a.implementation, &b.implementation) {
		(ProtoNode(a_id), ProtoNode(b_id)) => a_id == b_id,
		(Network(a_id), Network(b_id)) => a_id == b_id,
		_ => false,
	}
}

fn compute_attribute_deltas(from: &crate::Attributes, to: &crate::Attributes) -> Vec<AttributeDelta> {
	let mut deltas = Vec::new();

	for key in from.keys() {
		if !to.contains_key(key) {
			deltas.push(AttributeDelta { key: key.clone(), value: None });
		}
	}

	// Compare by `value` only; the per-entry `timestamp` is derived from the diff, not part of it.
	for (key, to_value) in to {
		if from.get(key).is_none_or(|from_value| from_value.value != to_value.value) {
			deltas.push(AttributeDelta {
				key: key.clone(),
				value: Some(to_value.value.clone()),
			});
		}
	}

	deltas
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Implementation, Network, Node};

	#[test]
	fn test_compute_deltas_empty() {
		let registry = Registry::default();

		let deltas = compute_deltas(&registry, &registry);
		assert_eq!(deltas.len(), 0, "No deltas should be generated for identical registries");
	}

	#[test]
	fn test_compute_deltas_add_node() {
		let from = Registry::default();

		let mut to = from.clone();
		let node = Node {
			implementation: Implementation::ProtoNode(ResourceId::new()),
			inputs: vec![],
			inputs_attributes: vec![],
			attributes: HashMap::new(),
			network: 0,
		};
		to.node_instances.insert(42, node);

		let deltas = compute_deltas(&from, &to);
		assert_eq!(deltas.len(), 1);
		assert!(matches!(deltas[0], RegistryDelta::AddNode { node_id: 42, .. }));
	}

	#[test]
	fn test_compute_deltas_change_network_attribute() {
		use crate::{AttributesExt, TimeStamp};

		let mut from = Registry::default();
		from.networks.insert(0, Network::default());

		let mut to = from.clone();
		to.networks.get_mut(&0).unwrap().attributes.set("ui::nav::width", serde_json::json!(640.0), TimeStamp::ORIGIN);

		let deltas = compute_deltas(&from, &to);
		assert_eq!(deltas.len(), 1, "a changed per-network attribute must emit one delta");
		assert!(
			matches!(&deltas[0], RegistryDelta::ChangeNetworkAttribute { network: 0, delta } if delta.key == "ui::nav::width"),
			"expected ChangeNetworkAttribute for ui::nav::width, got {:?}",
			deltas[0]
		);
	}

	#[test]
	fn test_compute_deltas_remove_node() {
		let mut from = Registry::default();

		let node = Node {
			implementation: Implementation::ProtoNode(ResourceId::new()),
			inputs: vec![],
			inputs_attributes: vec![],
			attributes: HashMap::new(),
			network: 0,
		};
		from.node_instances.insert(42, node);

		let to = Registry::default();

		let deltas = compute_deltas(&from, &to);
		assert_eq!(deltas.len(), 1);
		assert!(matches!(deltas[0], RegistryDelta::RemoveNode { node_id: 42, .. }));
	}

	#[test]
	fn test_compute_deltas_modify_attribute() {
		let mut from = Registry::default();

		let mut node = Node {
			implementation: Implementation::ProtoNode(ResourceId::new()),
			inputs: vec![],
			inputs_attributes: vec![],
			attributes: HashMap::new(),
			network: 0,
		};
		let stamp = |counter: u64| TimeStamp { counter, peer: crate::PeerId(0) };
		node.attributes.insert(
			"test".to_string(),
			crate::Value {
				value: serde_json::json!("old"),
				timestamp: stamp(0),
			},
		);
		from.node_instances.insert(42, node);

		let mut to = from.clone();
		to.node_instances.get_mut(&42).unwrap().attributes.insert(
			"test".to_string(),
			crate::Value {
				value: serde_json::json!("new"),
				timestamp: stamp(1),
			},
		);

		let deltas = compute_deltas(&from, &to);
		assert_eq!(deltas.len(), 1);
		assert!(matches!(
			&deltas[0],
			RegistryDelta::ChangeNodeAttribute { node_id: 42, delta: AttributeDelta { key, value: Some(_) } } if key == "test"
		));
	}

	/// Document-level attributes (the `Registry.attributes` bucket) must diff into
	/// `ChangeDocumentAttribute` deltas, so a document-scoped attribute change reaches the commit path.
	/// (Per-peer `ui::doc::*` view settings live in `session.json`, not here.)
	#[test]
	fn test_compute_deltas_document_attribute() {
		let stamp = |counter: u64| TimeStamp { counter, peer: crate::PeerId(0) };
		let from = Registry::default();

		let mut to = from.clone();
		to.attributes.insert(
			"doc::test_attribute".to_string(),
			crate::Value {
				value: serde_json::json!("value"),
				timestamp: stamp(1),
			},
		);

		let deltas = compute_deltas(&from, &to);
		assert_eq!(deltas.len(), 1);
		assert!(matches!(
			&deltas[0],
			RegistryDelta::ChangeDocumentAttribute { delta: AttributeDelta { key, value: Some(_) } } if key == "doc::test_attribute"
		));
	}

	#[test]
	fn test_compute_deltas_network_changes() {
		let make_slot = |id: u64| ExportSlot {
			target: Some(NodeInput::Node { node_id: id, output_index: 0 }),
			timestamp: TimeStamp::ORIGIN,
		};

		let mut from = Registry::default();
		from.networks.insert(
			0,
			Network {
				exports: vec![make_slot(1), make_slot(2)],
				..Default::default()
			},
		);

		let mut to = from.clone();
		to.networks.get_mut(&0).unwrap().exports.push(make_slot(3));

		let deltas = compute_deltas(&from, &to);
		// Only slot 2 changed (added). Slots 0 and 1 are unchanged so they don't emit ops.
		assert_eq!(deltas.len(), 1);
		assert!(matches!(
			&deltas[0],
			RegistryDelta::SetExport {
				network: 0,
				slot: 2,
				target: Some(NodeInput::Node { node_id: 3, .. }),
				..
			}
		));
	}
}
