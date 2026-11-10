//! The catalog's networks, shared under fixed IDs and forked before any write.

use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{network_node_identifiers, resolve_network_node_type};

thread_local! {
	// Per thread because the metadata holds caches that are not `Sync`
	static CATALOG: Catalog = Catalog::build();
}

struct Catalog {
	entries: HashMap<NetworkId, CopiedDefinition>,
	/// Each catalog network as its template carries it, to recognize an unmodified one on insertion.
	inline: HashMap<&'static str, (NodeNetwork, NodeNetworkPersistentMetadata)>,
}

impl Catalog {
	fn build() -> Self {
		let mut inline = HashMap::new();
		for identifier in network_node_identifiers() {
			let Some(definition) = resolve_network_node_type(identifier) else { continue };
			let (node, metadata) = definition.default_node_template().into_parts();
			if let (DocumentNodeImplementation::Network(network), Some(network_metadata)) = (node.implementation, metadata.network_metadata) {
				inline.insert(identifier, (network, network_metadata.persistent_metadata));
			}
		}

		let mut catalog = Self { entries: HashMap::new(), inline };
		let identifiers: Vec<&'static str> = catalog.inline.keys().copied().collect();
		for identifier in identifiers {
			let (network, persistent_metadata) = catalog.inline[identifier].clone();
			let metadata = NodeNetworkMetadata {
				persistent_metadata,
				..Default::default()
			};
			catalog.lift(network_id(identifier.as_bytes()), network, metadata);
		}
		catalog
	}

	/// Nodes get IDs derived from `id` and their template ID, and nested inline networks get IDs derived from `id` and the node holding them.
	fn lift(&mut self, id: NetworkId, mut network: NodeNetwork, mut metadata: NodeNetworkMetadata) {
		let derived = |template_id| derived_node_id(id, template_id);
		network.map_ids(derived);
		let persistent_metadata = &mut metadata.persistent_metadata;
		persistent_metadata.node_metadata = std::mem::take(&mut persistent_metadata.node_metadata)
			.into_iter()
			.map(|(template_id, node_metadata)| (derived(template_id), node_metadata))
			.collect();
		persistent_metadata.pinned_node_order.iter_mut().for_each(|node_id| *node_id = derived(*node_id));

		for (node_id, node) in &mut network.nodes {
			if !matches!(node.implementation, DocumentNodeImplementation::Network(_)) {
				continue;
			}
			let DocumentNodeImplementation::Network(nested) = std::mem::replace(&mut node.implementation, DocumentNodeImplementation::Extract) else {
				continue;
			};
			let node_metadata = metadata.persistent_metadata.node_metadata.entry(*node_id).or_default();
			let nested_metadata = node_metadata.persistent_metadata.network_metadata.take().unwrap_or_default();

			let nested_id = match self.catalog_id_of(&nested, &nested_metadata.persistent_metadata) {
				Some(catalog_id) => catalog_id,
				None => {
					let nested_id = network_id(&[id.0.to_le_bytes(), node_id.0.to_le_bytes()].concat());
					self.lift(nested_id, nested, nested_metadata);
					nested_id
				}
			};
			node.implementation = DocumentNodeImplementation::Definition(nested_id);
		}

		self.entries.insert(id, CopiedDefinition { id, network, metadata });
	}

	fn catalog_id_of(&self, network: &NodeNetwork, metadata: &NodeNetworkPersistentMetadata) -> Option<NetworkId> {
		let identifier = metadata.reference.as_deref()?;
		let (shipped_network, shipped_metadata) = self.inline.get(identifier)?;
		(shipped_network == network && same_content(shipped_metadata, metadata)).then(|| network_id(identifier.as_bytes()))
	}
}

/// The fixed ID of an unmodified catalog network.
pub(super) fn catalog_id_of(network: &NodeNetwork, metadata: &NodeNetworkPersistentMetadata) -> Option<NetworkId> {
	CATALOG.with(|catalog| catalog.catalog_id_of(network, metadata))
}

pub(super) fn entry(id: NetworkId) -> Option<CopiedDefinition> {
	CATALOG.with(|catalog| catalog.entries.get(&id).cloned())
}

/// Whether `id` is a catalog entry, which writes fork first.
pub(crate) fn is_protected(id: NetworkId) -> bool {
	CATALOG.with(|catalog| catalog.entries.contains_key(&id))
}

/// The ID the node with `template_id` has in the shipped network of `identifier`.
#[cfg(test)]
pub(crate) fn shipped_node_id(identifier: &str, template_id: NodeId) -> NodeId {
	derived_node_id(network_id(identifier.as_bytes()), template_id)
}

fn network_id(key: &[u8]) -> NetworkId {
	NetworkId(truncated_hash(b"catalog network", key))
}

/// The ID of the node with `template_id` in the catalog network `network`, unique across the catalog.
fn derived_node_id(network: NetworkId, template_id: NodeId) -> NodeId {
	NodeId(truncated_hash(b"catalog node", &[network.0.to_le_bytes(), template_id.0.to_le_bytes()].concat()))
}

/// Truncated blake3 under a domain tag, like graph-storage's derived IDs.
fn truncated_hash(domain: &[u8], key: &[u8]) -> u64 {
	let mut hasher = blake3::Hasher::new();
	hasher.update(domain);
	hasher.update(key);
	let mut truncated = [0_u8; 8];
	truncated.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
	u64::from_le_bytes(truncated)
}
