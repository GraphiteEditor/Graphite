use std::collections::{HashMap, HashSet};

use crate::{AttributeDelta, NetworkId, Node, NodeId, NodeInput, Registry, RegistryDelta};

/// Computes the minimal set of deltas to transform `from` into `to`
pub fn compute_deltas(from: &Registry, to: &Registry) -> Vec<RegistryDelta> {
	let mut deltas = Vec::new();

	// Find all node IDs in both registries
	let from_node_ids: HashSet<NodeId> = from.node_instances.keys().copied().collect();
	let to_node_ids: HashSet<NodeId> = to.node_instances.keys().copied().collect();

	// 1. Find removed nodes
	for &node_id in from_node_ids.difference(&to_node_ids) {
		deltas.push(RegistryDelta::RemoveNode { node_id });
	}

	// 2. Find added nodes
	for &node_id in to_node_ids.difference(&from_node_ids) {
		let node = to.node_instances[&node_id].clone();
		deltas.push(RegistryDelta::AddNode { node_id, node });
	}

	// 3. Find modified nodes (nodes that exist in both)
	for &node_id in from_node_ids.intersection(&to_node_ids) {
		let from_node = &from.node_instances[&node_id];
		let to_node = &to.node_instances[&node_id];

		// If implementation changed, we need to remove and re-add the node
		// (since we don't have a ChangeImplementation delta variant)
		if !nodes_have_same_implementation(from_node, to_node) {
			deltas.push(RegistryDelta::RemoveNode { node_id });
			deltas.push(RegistryDelta::AddNode {
				node_id,
				node: to_node.clone(),
			});
			continue;
		}

		// Check for input changes
		for (input_idx, (from_input, to_input)) in from_node.inputs.iter().zip(to_node.inputs.iter()).enumerate() {
			if !inputs_equal(from_input, to_input) {
				deltas.push(RegistryDelta::ChangeNodeInput {
					node_id,
					input_idx,
					new_input: to_input.clone(),
				});
			}
		}

		// Handle input count changes
		if from_node.inputs.len() != to_node.inputs.len() {
			// If input count changed, we need to remove and re-add the node
			deltas.push(RegistryDelta::RemoveNode { node_id });
			deltas.push(RegistryDelta::AddNode {
				node_id,
				node: to_node.clone(),
			});
			continue;
		}

		// Check for attribute changes
		let attribute_deltas = compute_attribute_deltas(&from_node.attributes, &to_node.attributes);
		for delta in attribute_deltas {
			deltas.push(RegistryDelta::ChangeNodeAttribute {
				node_id,
				delta,
			});
		}

		// Check for input attribute changes
		for (input_idx, (from_attrs, to_attrs)) in from_node.inputs_attributes.iter().zip(to_node.inputs_attributes.iter()).enumerate() {
			let input_attr_deltas = compute_attribute_deltas(from_attrs, to_attrs);
			for delta in input_attr_deltas {
				deltas.push(RegistryDelta::ChangeNodeInputAttribute {
					node_id,
					input_idx,
					delta,
				});
			}
		}
	}

	// 4. Handle network changes
	let from_network_ids: HashSet<NetworkId> = from.networks.keys().copied().collect();
	let to_network_ids: HashSet<NetworkId> = to.networks.keys().copied().collect();

	// Find removed networks
	for &network_id in from_network_ids.difference(&to_network_ids) {
		deltas.push(RegistryDelta::RemoveNetwork { network: network_id });
	}

	// Find added or modified networks
	for &network_id in &to_network_ids {
		let to_network = &to.networks[&network_id];

		// Check if this is a new network or if exports changed
		if let Some(from_network) = from.networks.get(&network_id) {
			if from_network.exports != to_network.exports {
				deltas.push(RegistryDelta::SetNetwork {
					network: network_id,
					network_output_nodes: to_network.exports.clone(),
				});
			}
		} else {
			// New network
			deltas.push(RegistryDelta::SetNetwork {
				network: network_id,
				network_output_nodes: to_network.exports.clone(),
			});
		}
	}

	deltas
}

/// Check if two nodes have the same implementation
fn nodes_have_same_implementation(a: &Node, b: &Node) -> bool {
	match (&a.implementation, &b.implementation) {
		(crate::Implementation::ProtoNode(a_id), crate::Implementation::ProtoNode(b_id)) => a_id == b_id,
		(crate::Implementation::Network(a_id), crate::Implementation::Network(b_id)) => a_id == b_id,
		_ => false,
	}
}

/// Check if two inputs are equal
fn inputs_equal(a: &NodeInput, b: &NodeInput) -> bool {
	match (a, b) {
		(NodeInput::Node { node_id: a_id, output_index: a_idx }, NodeInput::Node { node_id: b_id, output_index: b_idx }) => {
			a_id == b_id && a_idx == b_idx
		}
		(NodeInput::Value { raw_value: a_val, exposed: a_exp }, NodeInput::Value { raw_value: b_val, exposed: b_exp }) => {
			a_val == b_val && a_exp == b_exp
		}
		(NodeInput::Scope(a), NodeInput::Scope(b)) => a == b,
		(NodeInput::Import { import_idx: a_idx }, NodeInput::Import { import_idx: b_idx }) => a_idx == b_idx,
		(NodeInput::Reflection, NodeInput::Reflection) => true,
		_ => false,
	}
}

/// Compute attribute deltas between two attribute maps
fn compute_attribute_deltas(from: &crate::Attributes, to: &crate::Attributes) -> Vec<AttributeDelta> {
	let mut deltas = Vec::new();

	// Find all keys
	let from_keys: HashSet<&String> = from.keys().collect();
	let to_keys: HashSet<&String> = to.keys().collect();

	// Removed attributes
	for key in from_keys.difference(&to_keys) {
		deltas.push(AttributeDelta::Remove { key: (*key).clone() });
	}

	// Added or modified attributes
	for key in &to_keys {
		let to_value = &to[*key];

		if let Some(from_value) = from.get(*key) {
			// Check if value changed (comparing both value and timestamp)
			if from_value != to_value {
				deltas.push(AttributeDelta::Set {
					key: (*key).clone(),
					value: to_value.clone(),
				});
			}
		} else {
			// New attribute
			deltas.push(AttributeDelta::Set {
				key: (*key).clone(),
				value: to_value.clone(),
			});
		}
	}

	deltas
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Implementation, Network, Node, ProtoNode};

	#[test]
	fn test_compute_deltas_empty() {
		let registry = Registry {
			node_declarations: HashMap::new(),
			node_instances: HashMap::new(),
			networks: HashMap::new(),
			exported_nodes: vec![],
		};

		let deltas = compute_deltas(&registry, &registry);
		assert_eq!(deltas.len(), 0, "No deltas should be generated for identical registries");
	}

	#[test]
	fn test_compute_deltas_add_node() {
		let from = Registry {
			node_declarations: HashMap::new(),
			node_instances: HashMap::new(),
			networks: HashMap::new(),
			exported_nodes: vec![],
		};

		let mut to = from.clone();
		let node = Node {
			implementation: Implementation::ProtoNode(1),
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
	fn test_compute_deltas_remove_node() {
		let mut from = Registry {
			node_declarations: HashMap::new(),
			node_instances: HashMap::new(),
			networks: HashMap::new(),
			exported_nodes: vec![],
		};

		let node = Node {
			implementation: Implementation::ProtoNode(1),
			inputs: vec![],
			inputs_attributes: vec![],
			attributes: HashMap::new(),
			network: 0,
		};
		from.node_instances.insert(42, node);

		let to = Registry {
			node_declarations: HashMap::new(),
			node_instances: HashMap::new(),
			networks: HashMap::new(),
			exported_nodes: vec![],
		};

		let deltas = compute_deltas(&from, &to);
		assert_eq!(deltas.len(), 1);
		assert!(matches!(deltas[0], RegistryDelta::RemoveNode { node_id: 42 }));
	}

	#[test]
	fn test_compute_deltas_modify_attribute() {
		let mut from = Registry {
			node_declarations: HashMap::new(),
			node_instances: HashMap::new(),
			networks: HashMap::new(),
			exported_nodes: vec![],
		};

		let mut node = Node {
			implementation: Implementation::ProtoNode(1),
			inputs: vec![],
			inputs_attributes: vec![],
			attributes: HashMap::new(),
			network: 0,
		};
		node.attributes.insert("test".to_string(), (serde_json::json!("old"), 0));
		from.node_instances.insert(42, node);

		let mut to = from.clone();
		to.node_instances.get_mut(&42).unwrap()
			.attributes.insert("test".to_string(), (serde_json::json!("new"), 1));

		let deltas = compute_deltas(&from, &to);
		assert_eq!(deltas.len(), 1);
		assert!(matches!(
			&deltas[0],
			RegistryDelta::ChangeNodeAttribute { node_id: 42, delta: AttributeDelta::Set { key, .. } } if key == "test"
		));
	}

	#[test]
	fn test_compute_deltas_network_changes() {
		let mut from = Registry {
			node_declarations: HashMap::new(),
			node_instances: HashMap::new(),
			networks: HashMap::new(),
			exported_nodes: vec![],
		};
		from.networks.insert(0, Network { exports: vec![1, 2] });

		let mut to = from.clone();
		to.networks.get_mut(&0).unwrap().exports = vec![1, 2, 3];

		let deltas = compute_deltas(&from, &to);
		assert_eq!(deltas.len(), 1);
		assert!(matches!(
			&deltas[0],
			RegistryDelta::SetNetwork { network: 0, network_output_nodes } if network_output_nodes == &vec![1, 2, 3]
		));
	}
}
