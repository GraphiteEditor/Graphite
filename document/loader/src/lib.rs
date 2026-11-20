//! Opens Graphite documents through the format steps and node migrations this crate links, so every consumer reads documents the way the current build writes them.

use document_container::AnyContainer;
use document_container::archive::ArchiveFormat;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{NetworkMetadataEntry, NodeMetadataEntry};
use graph_craft::document::{DocumentNodeImplementation, NodeId, NodeNetwork};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

// Referenced so the linker keeps their self-registering migrations
use migration_compiled as _;
use migration_formats as _;
use migration_nodes as _;

/// A document's runtime network, with the file it was opened from.
pub struct LoadedDocument {
	pub network: NodeNetwork,
	/// The path of every layer in `network`, once for each instance of the networks holding it.
	pub layers: Vec<Vec<NodeId>>,
	pub origin: DocumentOrigin,
}

pub enum DocumentOrigin {
	/// A `.gdd`, with the metadata of the nodes and networks in its registry.
	Gdd {
		gdd: Box<GddV1>,
		nodes: Vec<NodeMetadataEntry>,
		networks: Vec<NetworkMetadataEntry>,
	},
	/// The JSON text of a legacy document.
	Legacy(String),
}

impl LoadedDocument {
	pub fn gdd(&self) -> Option<&GddV1> {
		match &self.origin {
			DocumentOrigin::Gdd { gdd, .. } => Some(gdd),
			DocumentOrigin::Legacy(_) => None,
		}
	}
}

/// Open a `.gdd` from its archive bytes into memory, running the migrations this crate links.
pub async fn open_gdd(archive: &[u8]) -> Result<GddV1, Box<dyn Error>> {
	let container = AnyContainer::Memory(MemoryBackend::new());
	let host = migration_compiled::CompilerHost::default();
	let gdd = GddV1::open_from_archive_with(archive, container, GddV1Layout, None, Some(&host))
		.await
		.map_err(|error| format!("Failed to open document: {error}"))?;
	Ok(gdd)
}

/// Open `path` as a `.gdd` if it is named `.gdd` or starts with archive magic bytes, otherwise as a legacy JSON document.
pub async fn open_document(path: &Path) -> Result<LoadedDocument, Box<dyn Error>> {
	let bytes = std::fs::read(path).map_err(|error| format!("Failed to read document {}: {error}", path.display()))?;

	// A file named `.gdd` stays on the archive path, so a damaged one reports the archive error
	let named_gdd = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("gdd"));
	if !named_gdd && ArchiveFormat::detect(&bytes).is_none() {
		let document_string = String::from_utf8(bytes).map_err(|error| format!("Document {} is neither a .gdd archive nor text: {error}", path.display()))?;
		let parse_error = |error| format!("Failed to parse legacy document {}: {error}", path.display());
		let document: serde_json::Value = serde_json::from_str(&document_string).map_err(parse_error)?;
		let network = legacy_network(&document).map_err(parse_error)?;
		let layers = legacy_layer_paths(&network, &document);
		return Ok(LoadedDocument {
			network,
			layers,
			origin: DocumentOrigin::Legacy(document_string),
		});
	}

	let gdd = open_gdd(&bytes).await?;
	let declarations = gdd.declarations(&gdd).await;
	let (network, nodes, networks) = gdd.registry().to_runtime_with_full_metadata(&declarations, gdd.blobs())?;
	let layers = document_format::layers::layer_paths(&network, &nodes);
	Ok(LoadedDocument {
		network,
		layers,
		origin: DocumentOrigin::Gdd { gdd: Box::new(gdd), nodes, networks },
	})
}

fn legacy_network(document: &serde_json::Value) -> Result<NodeNetwork, serde_json::Error> {
	let network = document["network_interface"]["network"].to_string();
	serde_json::from_str(&network)
}

/// The path of every node a legacy document's metadata marks as a layer.
fn legacy_layer_paths(network: &NodeNetwork, document: &serde_json::Value) -> Vec<Vec<NodeId>> {
	let mut layer_paths = Vec::new();
	document_format::layers::for_each_network_instance(network, |network_path, _| {
		let Some(network_metadata) = legacy_network_metadata(network, document, network_path) else {
			return;
		};
		let is_layer = |node_metadata: &serde_json::Value| node_metadata["node_type_metadata"]["Layer"].is_object();
		let layers = legacy_node_metadata(network_metadata).into_iter().filter(|(_, node_metadata)| is_layer(node_metadata));
		layer_paths.extend(layers.map(|(node_id, _)| [network_path, &[node_id]].concat()));
	});
	layer_paths
}

/// The metadata of the network at this path in a legacy document.
/// Older documents keep a held network's metadata inside its node's metadata, newer ones keep a definition's in a table.
fn legacy_network_metadata<'a>(network: &NodeNetwork, document: &'a serde_json::Value, network_path: &[NodeId]) -> Option<&'a serde_json::Value> {
	let document_network_metadata = &document["network_interface"]["network_metadata"];
	let definitions = &document_network_metadata["persistent_metadata"]["definitions"];

	let mut current = network;
	let mut network_metadata = document_network_metadata;
	for node_id in network_path {
		let node = current.nodes.get(node_id)?;
		network_metadata = match node.implementation {
			DocumentNodeImplementation::Definition(id) => &definitions[id.0.to_string()],
			DocumentNodeImplementation::Network(_) => &legacy_node_metadata(network_metadata).remove(node_id)?["network_metadata"],
			_ => return None,
		};
		current = network.implementation_network(&node.implementation)?;
	}

	Some(network_metadata)
}

/// The metadata of each node in a legacy network, by ID.
fn legacy_node_metadata(network_metadata: &serde_json::Value) -> HashMap<NodeId, &serde_json::Value> {
	let entries = network_metadata["persistent_metadata"]["node_metadata"].as_array().into_iter().flatten();
	entries.filter_map(|entry| Some((NodeId(entry[0].as_u64()?), &entry[1]["persistent_metadata"]))).collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use graph_craft::document::{DocumentNode, NetworkId};

	fn node(implementation: DocumentNodeImplementation) -> DocumentNode {
		DocumentNode { implementation, ..Default::default() }
	}

	/// A network holding the nodes 3 and 4.
	fn inner_network() -> NodeNetwork {
		NodeNetwork {
			nodes: [3, 4].map(|id| (NodeId(id), DocumentNode::default())).into_iter().collect(),
			..Default::default()
		}
	}

	fn sorted_legacy_layer_paths(network: &NodeNetwork, document: &str) -> Vec<Vec<NodeId>> {
		let document = serde_json::from_str(document).expect("The test document should be valid JSON");
		let mut layer_paths = legacy_layer_paths(network, &document);
		layer_paths.sort();
		layer_paths
	}

	#[test]
	fn a_layer_inside_a_definition_is_found_through_each_instance() {
		let definition = NetworkId(9);
		let network = NodeNetwork {
			nodes: [1, 2].map(|id| (NodeId(id), node(DocumentNodeImplementation::Definition(definition)))).into_iter().collect(),
			definitions: [(definition, inner_network())].into_iter().collect(),
			..Default::default()
		};
		let document = r#"{ "network_interface": { "network_metadata": { "persistent_metadata": {
			"node_metadata": [[1, { "persistent_metadata": { "node_type_metadata": { "Layer": {} } } }], [2, { "persistent_metadata": { "node_type_metadata": { "Node": {} } } }]],
			"definitions": { "9": { "persistent_metadata": {
				"node_metadata": [[3, { "persistent_metadata": { "node_type_metadata": { "Layer": {} } } }], [4, { "persistent_metadata": { "node_type_metadata": { "Node": {} } } }]]
			} } }
		} } } }"#;

		let expected = vec![vec![NodeId(1)], vec![NodeId(1), NodeId(3)], vec![NodeId(2), NodeId(3)]];
		assert_eq!(sorted_legacy_layer_paths(&network, document), expected);
	}

	#[test]
	fn a_layer_inside_a_held_network_is_found() {
		let network = NodeNetwork {
			nodes: [(NodeId(1), node(DocumentNodeImplementation::Network(inner_network())))].into_iter().collect(),
			..Default::default()
		};
		let document = r#"{ "network_interface": { "network_metadata": { "persistent_metadata": {
			"node_metadata": [[1, { "persistent_metadata": {
				"node_type_metadata": { "Layer": {} },
				"network_metadata": { "persistent_metadata": {
					"node_metadata": [[3, { "persistent_metadata": { "node_type_metadata": { "Layer": {} } } }], [4, { "persistent_metadata": { "node_type_metadata": { "Node": {} } } }]]
				} }
			} }]]
		} } } }"#;

		assert_eq!(sorted_legacy_layer_paths(&network, document), vec![vec![NodeId(1)], vec![NodeId(1), NodeId(3)]]);
	}
}
