//! Finds the path of every layer in a runtime network, once for each instance of the networks holding it.

use std::collections::HashSet;

use document_graph_storage::NodeMetadataEntry;
use graph_craft::document::{DocumentNodeImplementation, NetworkLocation, NodeId, NodeNetwork};

/// The path of every node its metadata marks as a layer.
pub fn layer_paths(network: &NodeNetwork, node_metadata: &[NodeMetadataEntry]) -> Vec<Vec<NodeId>> {
	let layers: HashSet<(NetworkLocation, NodeId)> = node_metadata.iter().filter(|node| node.is_layer).map(|node| (node.network, node.local_id)).collect();

	let mut layer_paths = Vec::new();
	for_each_network_instance(network, |network_path, nested| {
		let Some(location) = network.locate(network_path) else { return };
		let nested_layers = nested.nodes.keys().filter(|node_id| layers.contains(&(location, **node_id)));
		layer_paths.extend(nested_layers.map(|node_id| [network_path, &[*node_id]].concat()));
	});
	layer_paths
}

/// Visits every network with its path, once for each instance of it.
/// A definition is not entered again from inside itself.
pub fn for_each_network_instance(network: &NodeNetwork, mut visit: impl FnMut(&[NodeId], &NodeNetwork)) {
	let mut stack = vec![(Vec::new(), Vec::new())];
	while let Some((network_path, enclosing_definitions)) = stack.pop() {
		let Some(nested) = network.nested_network(&network_path) else { continue };
		visit(&network_path, nested);

		for (node_id, node) in &nested.nodes {
			let mut enclosing_definitions = enclosing_definitions.clone();
			match node.implementation {
				DocumentNodeImplementation::Definition(id) if enclosing_definitions.contains(&id) => continue,
				DocumentNodeImplementation::Definition(id) => enclosing_definitions.push(id),
				DocumentNodeImplementation::Network(_) => {}
				_ => continue,
			}
			stack.push(([network_path.as_slice(), &[*node_id]].concat(), enclosing_definitions));
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use graph_craft::document::{DocumentNode, NetworkId};

	const DEFINITION: NetworkId = NetworkId(9);

	fn node_metadata(network: NetworkLocation, local_id: u64, is_layer: bool) -> NodeMetadataEntry {
		NodeMetadataEntry {
			network,
			local_id: NodeId(local_id),
			position: None,
			is_layer,
			display_name: None,
			locked: false,
			pinned: false,
		}
	}

	#[test]
	fn a_layer_inside_a_definition_is_found_through_each_instance() {
		let instance = DocumentNode {
			implementation: DocumentNodeImplementation::Definition(DEFINITION),
			..Default::default()
		};
		let definition = NodeNetwork {
			nodes: [3, 4].map(|id| (NodeId(id), DocumentNode::default())).into_iter().collect(),
			..Default::default()
		};
		let network = NodeNetwork {
			nodes: [1, 2].map(|id| (NodeId(id), instance.clone())).into_iter().collect(),
			definitions: [(DEFINITION, definition)].into_iter().collect(),
			..Default::default()
		};
		let metadata = [(None, 1, true), (None, 2, false), (Some(DEFINITION), 3, true), (Some(DEFINITION), 4, false)];
		let metadata: Vec<_> = metadata.into_iter().map(|(location, local_id, is_layer)| node_metadata(location, local_id, is_layer)).collect();

		let mut layer_paths = layer_paths(&network, &metadata);
		layer_paths.sort();

		assert_eq!(layer_paths, vec![vec![NodeId(1)], vec![NodeId(1), NodeId(3)], vec![NodeId(2), NodeId(3)]]);
	}

	#[test]
	fn a_definition_holding_itself_is_entered_once() {
		let recursive = DocumentNode {
			implementation: DocumentNodeImplementation::Definition(DEFINITION),
			..Default::default()
		};
		let network = NodeNetwork {
			nodes: [(NodeId(1), recursive.clone())].into_iter().collect(),
			definitions: [(
				DEFINITION,
				NodeNetwork {
					nodes: [(NodeId(2), recursive)].into_iter().collect(),
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		};

		let mut network_paths = Vec::new();
		for_each_network_instance(&network, |network_path, _| network_paths.push(network_path.to_vec()));

		assert_eq!(network_paths, vec![vec![], vec![NodeId(1)]]);
	}
}
