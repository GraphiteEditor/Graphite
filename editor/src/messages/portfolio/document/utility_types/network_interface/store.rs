use super::*;

/// Addresses one node: the network it lives in, and its ID within that network.
///
/// The store is the only place node identity is used for addressing, so this is the one type that
/// changes when identity stops being derived from the node's location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeLocator<'a> {
	pub node_id: NodeId,
	pub network_path: &'a [NodeId],
}

impl<'a> NodeLocator<'a> {
	pub fn new(node_id: NodeId, network_path: &'a [NodeId]) -> Self {
		Self { node_id, network_path }
	}

	/// The path of the network this node owns, if it implements one.
	pub fn owned_network_path(&self) -> Vec<NodeId> {
		[self.network_path, &[self.node_id]].concat()
	}
}

/// The graph and metadata halves of one node, resolved together so the two parallel trees are never
/// reached separately and cannot drift apart.
pub(crate) struct NodeMut<'a> {
	pub node: &'a mut DocumentNode,
	pub metadata: &'a mut DocumentNodePersistentMetadata,
}

// The store: the only writer of the node graph and its parallel metadata tree. Every write keeps the
// two halves in step by construction, so the invariants checked by `validate_invariants` hold without
// each caller restating them.
impl NodeNetworkInterface {
	/// Both halves of a node, or `None` if either is missing.
	pub(crate) fn node_mut(&mut self, locator: NodeLocator) -> Option<NodeMut<'_>> {
		let node = self.network.network_mut().nested_network_mut(locator.network_path)?.nodes.get_mut(&locator.node_id)?;
		let metadata = self
			.network_metadata
			.nested_metadata_mut(locator.network_path)?
			.persistent_metadata
			.node_metadata
			.get_mut(&locator.node_id)?;

		Some(NodeMut {
			node,
			metadata: &mut metadata.persistent_metadata,
		})
	}

	/// Inserts an input and its metadata at `index`, clamped to the end. Returns whether the node was found.
	pub(crate) fn insert_input_slot(&mut self, locator: NodeLocator, index: usize, input: NodeInput, metadata: InputMetadata) -> bool {
		let Some(node) = self.node_mut(locator) else {
			log::error!("Could not get node {} in insert_input_slot", locator.node_id);
			return false;
		};

		let index = index.min(node.node.inputs.len());
		node.node.inputs.insert(index, input);
		node.metadata.input_metadata.insert(index.min(node.metadata.input_metadata.len()), metadata);
		true
	}

	/// Removes the input at `index` together with its metadata, returning both.
	pub(crate) fn remove_input_slot(&mut self, locator: NodeLocator, index: usize) -> Option<(NodeInput, InputMetadata)> {
		let Some(node) = self.node_mut(locator) else {
			log::error!("Could not get node {} in remove_input_slot", locator.node_id);
			return None;
		};

		if index >= node.node.inputs.len() {
			log::error!("Input index {index} out of bounds in remove_input_slot for node {}", locator.node_id);
			return None;
		}

		let input = node.node.inputs.remove(index);
		let metadata = (index < node.metadata.input_metadata.len()).then(|| node.metadata.input_metadata.remove(index));
		Some((input, metadata.unwrap_or_default()))
	}

	/// Moves the input at `from` to `to`, carrying its metadata with it.
	pub(crate) fn move_input_slot(&mut self, locator: NodeLocator, from: usize, to: usize) -> bool {
		let Some((input, metadata)) = self.remove_input_slot(locator, from) else { return false };
		self.insert_input_slot(locator, to, input, metadata)
	}

	/// Inserts an export at `index`, clamped to the end, along with the name the encapsulating node shows
	/// for it. The document network has no encapsulating node, so it carries no name.
	pub(crate) fn insert_export_slot(&mut self, network_path: &[NodeId], index: usize, export: NodeInput, output_name: String) -> bool {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in insert_export_slot");
			return false;
		};

		let index = index.min(network.exports.len());
		network.exports.insert(index, export);

		if let Some(encapsulating_node_metadata) = self.encapsulating_node_metadata_mut(network_path) {
			let output_names = &mut encapsulating_node_metadata.persistent_metadata.output_names;
			output_names.insert(index.min(output_names.len()), output_name);
		}
		true
	}

	/// Removes the export at `index` together with the encapsulating node's name for it.
	pub(crate) fn remove_export_slot(&mut self, network_path: &[NodeId], index: usize) -> Option<NodeInput> {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in remove_export_slot");
			return None;
		};

		if index >= network.exports.len() {
			log::error!("Export index {index} out of bounds in remove_export_slot");
			return None;
		}
		let export = network.exports.remove(index);

		if let Some(encapsulating_node_metadata) = self.encapsulating_node_metadata_mut(network_path) {
			let output_names = &mut encapsulating_node_metadata.persistent_metadata.output_names;
			if index < output_names.len() {
				output_names.remove(index);
			}
		}
		Some(export)
	}

	/// Moves the export at `from` to `to`, carrying the encapsulating node's name for it.
	pub(crate) fn move_export_slot(&mut self, network_path: &[NodeId], from: usize, to: usize) -> bool {
		let name = self
			.encapsulating_node_metadata(network_path)
			.and_then(|metadata| metadata.persistent_metadata.output_names.get(from).cloned())
			.unwrap_or_default();
		let Some(export) = self.remove_export_slot(network_path, from) else { return false };
		self.insert_export_slot(network_path, to, export, name)
	}

	/// Drops the encapsulating node's link to the definition it was instantiated from, which no longer
	/// describes it once its signature is edited.
	pub(crate) fn clear_encapsulating_reference(&mut self, network_path: &[NodeId]) {
		if let Some(encapsulating_node_metadata) = self.encapsulating_node_metadata_mut(network_path)
			&& let Some(network_metadata) = encapsulating_node_metadata.persistent_metadata.network_metadata.as_mut()
		{
			network_metadata.persistent_metadata.reference = None;
		}
	}
}
