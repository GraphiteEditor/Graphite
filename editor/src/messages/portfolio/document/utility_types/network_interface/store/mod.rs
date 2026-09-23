use super::*;
use graph_craft::runtime_delta::RuntimeDelta;

mod accessors;
mod cursor;

pub use cursor::NodeLocator;
pub(crate) use cursor::{NetworkMut, NodeMut, SelectionHistoryMut};

/// Holds one of the two parallel trees so that only this module can write it.
///
/// `Deref` passes reads straight through, so a query reads the tree as though the interface held it
/// directly. There is deliberately no `DerefMut` and the field is private to this module, so the only
/// route to a `&mut` is the store API below, and a write that skips it does not compile rather than
/// silently going unrecorded.
///
/// Transparent to serde, so wrapping a field does not change the document format.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Guarded<T>(T);

impl<T> std::ops::Deref for Guarded<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.0
	}
}

impl<T> Guarded<T> {
	fn new(value: T) -> Self {
		Self(value)
	}

	fn get_mut(&mut self) -> &mut T {
		&mut self.0
	}
}

// The store: the only writer of the node graph and its parallel metadata tree, keeping the two halves
// in step by construction.
impl NodeNetworkInterface {
	/// Builds an interface around a freshly-assembled pair of trees, for a document arriving from
	/// storage or from an older format rather than being edited into existence. Nothing is recorded: the
	/// trees are the document, not a set of changes to it.
	pub(super) fn from_trees(network: NodeNetwork, network_metadata: NodeNetworkMetadata) -> Self {
		Self {
			network: Guarded::new(MemoNetwork::new(network)),
			network_metadata: Guarded::new(network_metadata),
			..Default::default()
		}
	}

	/// Inserts an input and its metadata at `index`, clamped to the end. Returns whether the node was found.
	pub(crate) fn insert_input_slot(&mut self, locator: NodeLocator, index: usize, input: NodeInput, metadata: InputMetadata) -> bool {
		let Some(mut node) = self.node_mut(locator) else {
			log::error!("Could not get node {} in insert_input_slot", locator.node_id);
			return false;
		};

		let index = index.min(node.node.inputs.len());
		node.node.inputs.insert(index, input);
		node.metadata.input_metadata.insert(index.min(node.metadata.input_metadata.len()), metadata);
		node.emit_inputs();
		true
	}

	/// Removes the input at `index` together with its metadata, returning both.
	pub(crate) fn remove_input_slot(&mut self, locator: NodeLocator, index: usize) -> Option<(NodeInput, InputMetadata)> {
		let Some(mut node) = self.node_mut(locator) else {
			log::error!("Could not get node {} in remove_input_slot", locator.node_id);
			return None;
		};

		if index >= node.node.inputs.len() {
			log::error!("Input index {index} out of bounds in remove_input_slot for node {}", locator.node_id);
			return None;
		}

		let input = node.node.inputs.remove(index);
		let metadata = (index < node.metadata.input_metadata.len()).then(|| node.metadata.input_metadata.remove(index));
		node.emit_inputs();
		Some((input, metadata.unwrap_or_default()))
	}

	/// Moves the input at `from` to `to`, carrying its metadata with it.
	///
	/// Records the removal and the insertion separately, so the list is briefly one slot short. Both
	/// land in the same batch, where the second supersedes the first.
	pub(crate) fn move_input_slot(&mut self, locator: NodeLocator, from: usize, to: usize) -> bool {
		let Some((input, metadata)) = self.remove_input_slot(locator, from) else { return false };
		self.insert_input_slot(locator, to, input, metadata)
	}

	/// Inserts an export at `index`, clamped to the end, along with the name the encapsulating node shows
	/// for it. The document network has no encapsulating node, so it carries no name.
	pub(crate) fn insert_export_slot(&mut self, network_path: &[NodeId], index: usize, export: NodeInput, output_name: String) -> bool {
		let Some(network) = self.network_graph_mut(network_path) else {
			log::error!("Could not get nested network in insert_export_slot");
			return false;
		};

		let index = index.min(network.exports.len());
		network.exports.insert(index, export);

		if let Some(encapsulating_node_metadata) = self.encapsulating_node_metadata_mut(network_path) {
			let output_names = &mut encapsulating_node_metadata.persistent_metadata.output_names;
			output_names.insert(index.min(output_names.len()), output_name);
		}
		self.emit_exports_from(network_path, index, None);
		true
	}

	/// Removes the export at `index` together with the encapsulating node's name for it.
	pub(crate) fn remove_export_slot(&mut self, network_path: &[NodeId], index: usize) -> Option<NodeInput> {
		let Some(network) = self.network_graph_mut(network_path) else {
			log::error!("Could not get nested network in remove_export_slot");
			return None;
		};

		if index >= network.exports.len() {
			log::error!("Export index {index} out of bounds in remove_export_slot");
			return None;
		}
		let export = network.exports.remove(index);
		// The list shrank by one, so what was the last index now has no slot and has to be cleared.
		let vacated = network.exports.len();

		if let Some(encapsulating_node_metadata) = self.encapsulating_node_metadata_mut(network_path) {
			let output_names = &mut encapsulating_node_metadata.persistent_metadata.output_names;
			if index < output_names.len() {
				output_names.remove(index);
			}
		}
		self.emit_exports_from(network_path, index, Some(vacated));
		Some(export)
	}

	/// Records every export slot from `index` on, plus a clear for `vacated` when the list shrank.
	///
	/// Export slots are addressed by index, so inserting or removing one changes the meaning of every
	/// later slot and each has to be restated. The encapsulating node's names for them ride along as one
	/// whole-array metadata write.
	fn emit_exports_from(&mut self, network_path: &[NodeId], index: usize, vacated: Option<usize>) {
		let Some(network) = self.network.network().nested_network(network_path) else { return };
		let exports: Vec<_> = network.exports.iter().skip(index).cloned().collect();

		for (offset, export) in exports.into_iter().enumerate() {
			self.deltas.push(EditorDelta::Graph(RuntimeDelta::SetExport {
				network_path: network_path.to_vec(),
				export_index: index + offset,
				input: Some(export),
			}));
		}
		if let Some(vacated) = vacated {
			self.deltas.push(EditorDelta::Graph(RuntimeDelta::SetExport {
				network_path: network_path.to_vec(),
				export_index: vacated,
				input: None,
			}));
		}

		let Some((&node_id, parent_path)) = network_path.split_last() else { return };
		let Some(metadata) = self.encapsulating_node_metadata(network_path) else { return };
		let change = NodeMetadataChange::OutputNames(metadata.persistent_metadata.output_names.clone());
		self.deltas.push(EditorDelta::NodeMetadata {
			network_path: parent_path.to_vec(),
			node_id,
			change,
		});
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

	/// Writes the input at `connector`, returning the input it replaced.
	pub(crate) fn set_input_slot(&mut self, connector: &InputConnector, network_path: &[NodeId], input: NodeInput) -> Option<NodeInput> {
		let Some(network) = self.network_graph_mut(network_path) else {
			log::error!("Could not get nested network in set_input_slot");
			return None;
		};

		let Some(slot) = input_slot_mut(network, connector) else {
			log::error!("Could not get input {connector:?} in set_input_slot");
			return None;
		};

		if *slot == input {
			return Some(input);
		}
		let previous = std::mem::replace(slot, input.clone());

		self.deltas.push(input_write_delta(connector, network_path, input));
		Some(previous)
	}

	/// Edits the value at `connector` in place, recording the value the edit leaves behind.
	///
	/// For a change expressed as an operation on the existing value rather than as a replacement of it,
	/// which avoids copying the value out and back. The recorded delta still carries the whole result,
	/// since the storage op is a whole-value write.
	pub(crate) fn edit_input_value(&mut self, connector: &InputConnector, network_path: &[NodeId], edit: impl FnOnce(&mut TaggedValue)) -> bool {
		let Some(network) = self.network_graph_mut(network_path) else {
			log::error!("Could not get nested network in edit_input_value");
			return false;
		};

		let Some(slot) = input_slot_mut(network, connector) else {
			log::error!("Could not get input {connector:?} in edit_input_value");
			return false;
		};
		let Some(mut value) = slot.as_value_mut() else {
			log::error!("Input {connector:?} is not a value in edit_input_value");
			return false;
		};

		edit(&mut value);
		drop(value);

		let input = slot.clone();
		self.deltas.push(input_write_delta(connector, network_path, input));
		true
	}

	/// Inserts a node and its metadata, returning the entry it replaced.
	pub(crate) fn insert_node_entry(&mut self, locator: NodeLocator, template: NodeTemplate) -> Option<NodeTemplate> {
		if !self.network_pair_exists(locator.network_path) {
			log::error!("Could not get network {:?} in insert_node_entry", locator.network_path);
			return None;
		}

		let (document_node, persistent_metadata) = template.into_parts();
		let node = Box::new(document_node.clone());
		let metadata = Box::new(persistent_metadata.clone());

		let previous_node = self
			.network
			.get_mut()
			.network_mut()
			.nested_network_mut(locator.network_path)?
			.nodes
			.insert(locator.node_id, document_node);
		let previous_metadata = self.network_metadata.get_mut().nested_metadata_mut(locator.network_path)?.persistent_metadata.node_metadata.insert(
			locator.node_id,
			DocumentNodeMetadata {
				persistent_metadata,
				transient_metadata: DocumentNodeTransientMetadata::default(),
			},
		);

		// Overwriting an existing entry replaces the node and everything nested under it, which is what
		// distinguishes the two from the storage side.
		let network_path = locator.network_path.to_vec();
		let node_id = locator.node_id;
		self.deltas.push(EditorDelta::Graph(match previous_node.is_some() {
			true => RuntimeDelta::ReplaceNode {
				network_path: network_path.clone(),
				node_id,
				node,
			},
			false => RuntimeDelta::AddNode {
				network_path: network_path.clone(),
				node_id,
				node,
			},
		}));
		self.deltas.push(EditorDelta::NodeMetadataSnapshot { network_path, node_id, metadata });

		previous_node
			.zip(previous_metadata)
			.map(|(node, metadata)| NodeTemplate::from_parts(node, metadata.persistent_metadata))
	}

	/// Removes a node and its metadata, returning them joined as a template.
	pub(crate) fn remove_node_entry(&mut self, locator: NodeLocator) -> Option<NodeTemplate> {
		if !self.network_pair_exists(locator.network_path) {
			log::error!("Could not get network {:?} in remove_node_entry", locator.network_path);
			return None;
		}

		let node = self.network.get_mut().network_mut().nested_network_mut(locator.network_path)?.nodes.remove(&locator.node_id);
		let metadata = self
			.network_metadata
			.get_mut()
			.nested_metadata_mut(locator.network_path)?
			.persistent_metadata
			.node_metadata
			.remove(&locator.node_id);

		if node.is_some() {
			self.deltas.push(EditorDelta::Graph(RuntimeDelta::RemoveNode {
				network_path: locator.network_path.to_vec(),
				node_id: locator.node_id,
			}));
		}

		node.zip(metadata).map(|(node, metadata)| NodeTemplate::from_parts(node, metadata.persistent_metadata))
	}

	/// Whether both trees hold the network, checked before a write so a missing one cannot leave the two
	/// halves out of step.
	fn network_pair_exists(&self, network_path: &[NodeId]) -> bool {
		self.network.network().nested_network(network_path).is_some() && self.network_metadata.nested_metadata(network_path).is_some()
	}

	/// Takes what the writes since the last drain changed, in write order, leaving the buffer empty.
	pub(crate) fn take_deltas(&mut self) -> Vec<EditorDelta> {
		std::mem::take(&mut self.deltas)
	}

	/// Drops what the writes since the last drain changed, for writes that are not edits to the document.
	pub(crate) fn discard_deltas(&mut self) {
		self.deltas.clear();
	}

	/// Drops the network's link to the definition it was instantiated from, which no longer describes it
	/// once its signature is edited.
	pub(crate) fn clear_encapsulating_reference(&mut self, network_path: &[NodeId]) {
		if let Some(mut network) = self.network_mut(network_path) {
			network.set_reference(None);
		}
	}
}

/// The slot `connector` addresses, whether it is a node's input or one of the network's exports.
fn input_slot_mut<'a>(network: &'a mut NodeNetwork, connector: &InputConnector) -> Option<&'a mut NodeInput> {
	match connector {
		InputConnector::Node { node_id, input_index } => network.nodes.get_mut(node_id).and_then(|node| node.inputs.get_mut(*input_index)),
		InputConnector::Export(export_index) => network.exports.get_mut(*export_index),
	}
}

/// What a whole-value write to `connector` records, which is a different delta for an input than for
/// an export.
fn input_write_delta(connector: &InputConnector, network_path: &[NodeId], input: NodeInput) -> EditorDelta {
	EditorDelta::Graph(match connector {
		InputConnector::Node { node_id, input_index } => RuntimeDelta::SetInput {
			network_path: network_path.to_vec(),
			node_id: *node_id,
			input_index: *input_index,
			input,
		},
		InputConnector::Export(export_index) => RuntimeDelta::SetExport {
			network_path: network_path.to_vec(),
			export_index: *export_index,
			input: Some(input),
		},
	})
}
