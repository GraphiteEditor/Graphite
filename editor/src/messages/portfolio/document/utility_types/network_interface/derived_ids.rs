//! IDs derived from the network that holds a node, so the nodes of a copied network get IDs unique across the document.

use super::*;

/// The ID the node `node_id` gets in a copy of its network under the ID `network`.
pub(super) fn derived_node_id(network: NetworkId, node_id: NodeId) -> NodeId {
	NodeId(truncated_hash(b"network node", &[network.0.to_le_bytes(), node_id.0.to_le_bytes()].concat()))
}

/// Gives each node of a network copied under the ID `network_id` its derived ID, in the graph and in the metadata naming it.
pub(super) fn rekey_nodes(network_id: NetworkId, network: &mut NodeNetwork, metadata: &mut NodeNetworkPersistentMetadata) {
	let derived = |node_id| derived_node_id(network_id, node_id);
	network.map_ids(derived);

	metadata.node_metadata = std::mem::take(&mut metadata.node_metadata)
		.into_iter()
		.map(|(node_id, node_metadata)| (derived(node_id), node_metadata))
		.collect();
	metadata.pinned_node_order.iter_mut().for_each(|node_id| *node_id = derived(*node_id));

	for selection in metadata.selection_undo_history.iter_mut().chain(metadata.selection_redo_history.iter_mut()) {
		selection.0.iter_mut().for_each(|node_id| *node_id = derived(*node_id));
	}
	if let Previewing::Yes { previewed }
	| Previewing::LegacyRewired {
		root_node_to_restore: Some(previewed),
	} = &mut metadata.previewing
	{
		previewed.node_id = derived(previewed.node_id);
	}
}

/// Truncated blake3 under a domain tag, like graph-storage's derived IDs.
pub(super) fn truncated_hash(domain: &[u8], key: &[u8]) -> u64 {
	let mut hasher = blake3::Hasher::new();
	hasher.update(domain);
	hasher.update(key);
	let mut truncated = [0_u8; 8];
	truncated.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
	u64::from_le_bytes(truncated)
}
