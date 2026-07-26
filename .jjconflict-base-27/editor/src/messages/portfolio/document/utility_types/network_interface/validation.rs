use super::{NodeNetworkInterface, NodeNetworkMetadata};
use graph_craft::document::{DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork};

impl NodeNetworkInterface {
	/// Checks the structural invariants between the document network and its parallel metadata tree at every nesting level.
	/// Returns a description of each violation found, or an empty list if the interface is internally consistent.
	/// Intended for tests and debugging; violations indicate a desync bug in the mutation methods, not a user error.
	// O(nodes + wires) summed across all nesting levels, plus an acyclicity walk per network
	pub fn validate_invariants(&self) -> Vec<String> {
		let mut violations = Vec::new();
		validate_network(self.document_network(), &self.network_metadata, &mut Vec::new(), 0, &mut violations);
		violations
	}
}

fn validate_network(network: &NodeNetwork, network_metadata: &NodeNetworkMetadata, path: &mut Vec<NodeId>, import_count: usize, violations: &mut Vec<String>) {
	let node_metadata = &network_metadata.persistent_metadata.node_metadata;

	// The network and its metadata tree must contain exactly the same set of nodes
	for node_id in network.nodes.keys() {
		if !node_metadata.contains_key(node_id) {
			violations.push(format!("Node {node_id} in network {path:?} has no metadata entry"));
		}
	}
	for node_id in node_metadata.keys() {
		if !network.nodes.contains_key(node_id) {
			violations.push(format!("Metadata entry {node_id} in network {path:?} has no corresponding node"));
		}
	}

	// The pinned display order may only reference existing nodes
	for node_id in &network_metadata.persistent_metadata.pinned_node_order {
		if !network.nodes.contains_key(node_id) {
			violations.push(format!("Pinned node order in network {path:?} references nonexistent node {node_id}"));
		}
	}

	// Every wire must reference an existing endpoint within this network
	let validate_input = |input: &NodeInput, location: &str, violations: &mut Vec<String>| match input {
		NodeInput::Node { node_id, output_index, .. } => match network.nodes.get(node_id) {
			None => violations.push(format!("{location} in network {path:?} references nonexistent node {node_id}")),
			Some(target) if *output_index >= target.implementation.output_count() => {
				violations.push(format!("{location} in network {path:?} references nonexistent output {output_index} of node {node_id}"));
			}
			_ => {}
		},
		NodeInput::Import { import_index, .. } if *import_index >= import_count => {
			violations.push(format!("{location} in network {path:?} references nonexistent import {import_index} of {import_count}"));
		}
		_ => {}
	};

	for (node_id, node) in &network.nodes {
		for input in &node.inputs {
			validate_input(input, &format!("Input of node {node_id}"), violations);
		}
	}
	for (export_index, export) in network.exports.iter().enumerate() {
		validate_input(export, &format!("Export {export_index}"), violations);
	}

	if !network.is_acyclic() {
		violations.push(format!("Network {path:?} contains a cycle"));
	}

	for (node_id, node) in &network.nodes {
		let Some(metadata) = node_metadata.get(node_id) else { continue };
		let persistent = &metadata.persistent_metadata;

		// Input metadata is a parallel array to the node's inputs and the lengths must stay in sync
		if persistent.input_metadata.len() != node.inputs.len() {
			violations.push(format!(
				"Node {node_id} in network {path:?} has {} inputs but {} input metadata entries",
				node.inputs.len(),
				persistent.input_metadata.len()
			));
		}

		// Nested network metadata must exist exactly for nodes implemented as networks
		match (&node.implementation, &persistent.network_metadata) {
			(DocumentNodeImplementation::Network(nested_network), Some(nested_metadata)) => {
				if persistent.output_names.len() != nested_network.exports.len() {
					violations.push(format!(
						"Node {node_id} in network {path:?} has {} exports but {} output names",
						nested_network.exports.len(),
						persistent.output_names.len()
					));
				}

				path.push(*node_id);
				validate_network(nested_network, nested_metadata, path, node.inputs.len(), violations);
				path.pop();
			}
			(DocumentNodeImplementation::Network(_), None) => {
				violations.push(format!("Network node {node_id} in network {path:?} is missing its nested network metadata"));
			}
			(_, Some(_)) => {
				violations.push(format!("Non-network node {node_id} in network {path:?} has nested network metadata"));
			}
			(_, None) => {}
		}
	}
}
