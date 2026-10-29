use super::{InputConnector, NodeNetworkInterface, NodeNetworkMetadata};
use graph_craft::document::{DocumentNodeImplementation, NetworkId, NodeId, NodeInput, NodeNetwork};
use std::collections::{HashMap, HashSet};

impl NodeNetworkInterface {
	/// Checks the structural invariants of the document network, its parallel metadata tree, and its redirect dots at every nesting level.
	/// Returns a description of each violation found, or an empty list if the interface is internally consistent.
	/// Intended for tests and debugging; violations indicate a desync bug in the mutation methods, not a user error.
	// O(nodes + wires) summed across all nesting levels, plus an acyclicity walk per network
	pub fn validate_invariants(&self) -> Vec<String> {
		let mut violations = Vec::new();
		self.validate_definition_tables(&mut violations);

		let mut root = DocumentRoot {
			network: self.document_network(),
			metadata: &self.network_metadata,
			definition_import_counts: HashMap::new(),
		};
		validate_network(&mut root, self.document_network(), &self.network_metadata, &mut Vec::new(), 0, &mut violations);
		self.validate_outward_wires(&mut Vec::new(), &mut HashSet::new(), &mut violations);
		self.validate_dots_stay_on_wires(&mut violations);
		violations
	}

	/// The definitions table and its metadata table must contain exactly the same definitions.
	fn validate_definition_tables(&self, violations: &mut Vec<String>) {
		let definitions = &self.document_network().definitions;
		let definition_metadata = &self.network_metadata.persistent_metadata.definitions;

		for id in definitions.keys().filter(|id| !definition_metadata.contains_key(id)) {
			violations.push(format!("Definition {id:?} has no metadata entry"));
		}
		for id in definition_metadata.keys().filter(|id| !definitions.contains_key(id)) {
			violations.push(format!("Metadata entry for definition {id:?} has no corresponding definition"));
		}
	}

	/// A redirect dot an edit took a wire from must be gone by the time the edit is done if that left it orphaned.
	fn validate_dots_stay_on_wires(&self, violations: &mut Vec<String>) {
		for (network_path, lost_wires) in &self.dots_that_lost_a_wire {
			for (dot, side) in lost_wires.iter().filter(|(dot, side)| self.is_orphaned_dot(dot, *side, network_path)) {
				violations.push(format!("Redirect dot {dot} in network {network_path:?} lost its {side:?} wire to an edit that left it behind"));
			}
		}
	}

	/// A loaded outward wire cache must match a fresh computation from the network.
	fn validate_outward_wires(&self, path: &mut Vec<NodeId>, validated_definitions: &mut HashSet<NetworkId>, violations: &mut Vec<String>) {
		let Some(network) = self.nested_network(path) else { return };

		if let Some(computed) = self.compute_outward_wires(path)
			&& let Some(network_metadata) = self.network_metadata(path)
		{
			network_metadata.transient_metadata.outward_wires.with_loaded(|cached| {
				let outputs: HashSet<_> = cached.keys().chain(computed.keys()).collect();
				for output in outputs {
					let (cached_inputs, computed_inputs) = (cached.get(output), computed.get(output));
					if cached_inputs.map(|inputs| wire_counts(inputs)) != computed_inputs.map(|inputs| wire_counts(inputs)) {
						violations.push(format!(
							"Outward wires of {output:?} in network {path:?} are cached as {cached_inputs:?} but the network holds {computed_inputs:?}"
						));
					}
				}
			});
		}

		for (node_id, node) in &network.nodes {
			let descend = match node.implementation {
				DocumentNodeImplementation::Network(_) => true,
				DocumentNodeImplementation::Definition(id) => validated_definitions.insert(id),
				_ => false,
			};
			if descend {
				path.push(*node_id);
				self.validate_outward_wires(path, validated_definitions, violations);
				path.pop();
			}
		}
	}
}

/// How many times each input appears, since the cache lists an output's consumers in no particular order.
fn wire_counts(inputs: &[InputConnector]) -> HashMap<InputConnector, usize> {
	let mut counts = HashMap::new();
	for input in inputs {
		*counts.entry(*input).or_insert(0) += 1;
	}
	counts
}

struct DocumentRoot<'a> {
	network: &'a NodeNetwork,
	metadata: &'a NodeNetworkMetadata,
	definition_import_counts: HashMap<NetworkId, usize>,
}

fn validate_network(root: &mut DocumentRoot, network: &NodeNetwork, network_metadata: &NodeNetworkMetadata, path: &mut Vec<NodeId>, import_count: usize, violations: &mut Vec<String>) {
	let (document_network, document_metadata) = (root.network, root.metadata);

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
			Some(target) if *output_index >= document_network.implementation_output_count(&target.implementation) => {
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

		let validate_output_names = |export_count: usize, violations: &mut Vec<String>| {
			if persistent.output_names.len() != export_count {
				violations.push(format!(
					"Node {node_id} in network {path:?} has {export_count} exports but {} output names",
					persistent.output_names.len()
				));
			}
		};

		// Nested network metadata must exist exactly for nodes implemented as networks
		match (&node.implementation, &persistent.network_metadata) {
			(DocumentNodeImplementation::Network(nested_network), Some(nested_metadata)) => {
				validate_output_names(nested_network.exports.len(), violations);

				path.push(*node_id);
				validate_network(root, nested_network, nested_metadata, path, node.inputs.len(), violations);
				path.pop();
			}
			(DocumentNodeImplementation::Network(_), None) => {
				violations.push(format!("Network node {node_id} in network {path:?} is missing its nested network metadata"));
			}
			(_, Some(_)) => {
				violations.push(format!("Non-network node {node_id} in network {path:?} has nested network metadata"));
			}
			(DocumentNodeImplementation::Definition(id), None) => {
				let Some(definition) = document_network.definitions.get(id) else {
					violations.push(format!("Node {node_id} in network {path:?} references nonexistent definition {id:?}"));
					continue;
				};
				validate_output_names(definition.exports.len(), violations);

				match root.definition_import_counts.get(id) {
					Some(&validated_import_count) if validated_import_count != node.inputs.len() => violations.push(format!(
						"Node {node_id} in network {path:?} has {} inputs but another instance of definition {id:?} has {validated_import_count}",
						node.inputs.len()
					)),
					Some(_) => {}
					None => {
						root.definition_import_counts.insert(*id, node.inputs.len());

						let Some(definition_metadata) = document_metadata.persistent_metadata.definitions.get(id) else {
							continue;
						};
						path.push(*node_id);
						validate_network(root, definition, definition_metadata, path, node.inputs.len(), violations);
						path.pop();
					}
				}
			}
			(_, None) => {}
		}
	}
}
