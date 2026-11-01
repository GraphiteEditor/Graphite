use super::*;
use graph_craft::document::OwnedNetworkLocation;
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
	pub(super) fn from_trees(network: NodeNetwork, mut network_metadata: NodeNetworkMetadata) -> Self {
		network_metadata.move_interface_metadata_into_networks(&network);
		Self {
			network: Guarded::new(MemoNetwork::new(network)),
			network_metadata: Guarded::new(network_metadata),
			..Default::default()
		}
	}

	/// Inserts an import at `index` into the network at `network_path`: its metadata once, and `input` on every node the network implements.
	pub(crate) fn insert_import_slot(&mut self, network_path: &[NodeId], index: usize, input: NodeInput, metadata: InputMetadata) -> bool {
		let encapsulating_nodes = self.encapsulating_nodes(network_path);
		if encapsulating_nodes.is_empty() {
			log::error!("Cannot insert an import into the document network");
			return false;
		}
		let Some(mut network) = self.network_mut(network_path) else {
			log::error!("Could not get network {network_path:?} in insert_import_slot");
			return false;
		};
		network.insert_import_metadata(index, metadata);

		for (path, node_id) in &encapsulating_nodes {
			let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, path)) else {
				log::error!("Could not get node {node_id} in insert_import_slot");
				continue;
			};
			node.insert_input(index, input.clone());
		}
		true
	}

	/// Removes the import at `index` from the network at `network_path` and from every node the network implements, returning its metadata.
	pub(crate) fn remove_import_slot(&mut self, network_path: &[NodeId], index: usize) -> Option<InputMetadata> {
		for (path, node_id) in self.encapsulating_nodes(network_path) {
			let Some(mut node) = self.node_mut(NodeLocator::new(node_id, &path)) else {
				log::error!("Could not get node {node_id} in remove_import_slot");
				continue;
			};
			if node.remove_input(index).is_none() {
				log::error!("Input index {index} out of bounds in remove_import_slot for node {node_id}");
			}
		}

		let Some(mut network) = self.network_mut(network_path) else {
			log::error!("Could not get network {network_path:?} in remove_import_slot");
			return None;
		};
		network.remove_import_metadata(index)
	}

	/// Moves the import at `from` to `to`, carrying its metadata and each node's input with it.
	pub(crate) fn move_import_slot(&mut self, network_path: &[NodeId], from: usize, to: usize) -> bool {
		let encapsulating_nodes = self.encapsulating_nodes(network_path);
		if encapsulating_nodes.is_empty() {
			return false;
		}

		for (path, node_id) in &encapsulating_nodes {
			let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, path)) else {
				log::error!("Could not get node {node_id} in move_import_slot");
				continue;
			};
			let Some(input) = node.remove_input(from) else {
				log::error!("Could not remove input {from} of node {node_id} in move_import_slot");
				return false;
			};
			node.insert_input(to, input);
		}

		let Some(mut network) = self.network_mut(network_path) else { return false };
		let Some(metadata) = network.remove_import_metadata(from) else { return false };
		network.insert_import_metadata(to, metadata);
		true
	}

	/// Inserts an export at `index`, clamped to the end, along with the name and description the encapsulating
	/// node shows for it. The document network has no encapsulating node, so it carries neither.
	pub(crate) fn insert_export_slot(&mut self, network_path: &[NodeId], index: usize, export: NodeInput, output_name: String, output_description: String) -> bool {
		let Some(location) = self.locate(network_path) else {
			log::error!("Could not locate network {network_path:?} in insert_export_slot");
			return false;
		};
		self.insert_export_slot_at(location, index, export, output_name, output_description)
	}

	/// Inserts an export into the network at `location`, like [`Self::insert_export_slot`].
	pub(crate) fn insert_export_slot_at(&mut self, location: NetworkLocation, index: usize, export: NodeInput, output_name: String, output_description: String) -> bool {
		let Some(network) = self.network_graph_mut(location) else {
			log::error!("Could not get nested network in insert_export_slot");
			return false;
		};

		let index = index.min(network.exports.len());
		network.exports.insert(index, export);

		let mut descriptions_changed = false;
		if let Some(persistent_metadata) = self.subnetwork_metadata_mut(location) {
			persistent_metadata.export_names.insert(index.min(persistent_metadata.export_names.len()), output_name);

			// A missing description is empty, so only a non-empty one past the end needs the gap filled
			let output_descriptions = &mut persistent_metadata.export_descriptions;
			descriptions_changed = !output_description.is_empty() || (index <= output_descriptions.len() && has_description(output_descriptions));
			if index <= output_descriptions.len() {
				output_descriptions.insert(index, output_description);
			} else if !output_description.is_empty() {
				output_descriptions.resize(index, String::new());
				output_descriptions.push(output_description);
			}
		}
		self.emit_exports_from(location, index, None, descriptions_changed);
		true
	}

	/// Removes the export at `index` together with the encapsulating node's name and description for it.
	pub(crate) fn remove_export_slot(&mut self, network_path: &[NodeId], index: usize) -> Option<NodeInput> {
		let Some(location) = self.locate(network_path) else {
			log::error!("Could not locate network {network_path:?} in remove_export_slot");
			return None;
		};
		self.remove_export_slot_at(location, index)
	}

	/// Removes the export at `index` from the network at `location`, like [`Self::remove_export_slot`].
	pub(crate) fn remove_export_slot_at(&mut self, location: NetworkLocation, index: usize) -> Option<NodeInput> {
		let Some(network) = self.network_graph_mut(location) else {
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

		let mut descriptions_changed = false;
		if let Some(persistent_metadata) = self.subnetwork_metadata_mut(location) {
			if index < persistent_metadata.export_names.len() {
				persistent_metadata.export_names.remove(index);
			}
			descriptions_changed = index < persistent_metadata.export_descriptions.len() && has_description(&persistent_metadata.export_descriptions);
			if index < persistent_metadata.export_descriptions.len() {
				persistent_metadata.export_descriptions.remove(index);
			}
		}
		self.emit_exports_from(location, index, Some(vacated), descriptions_changed);
		Some(export)
	}

	/// Records every export slot from `index` on, plus a clear for `vacated` when the list shrank.
	///
	/// Export slots are addressed by index, so inserting or removing one changes the meaning of every
	/// later slot and each has to be restated. The encapsulating node's names, and its descriptions unless
	/// they're all empty, ride along as whole-array metadata writes.
	fn emit_exports_from(&mut self, location: NetworkLocation, index: usize, vacated: Option<usize>, descriptions_changed: bool) {
		let Some(network) = self.network.network().located_network(location) else { return };
		let exports: Vec<_> = network.exports.iter().skip(index).cloned().collect();

		for (offset, export) in exports.into_iter().enumerate() {
			self.deltas.push(EditorDelta::Graph(RuntimeDelta::SetExport {
				network: location.to_owned_location(),
				export_index: index + offset,
				input: Some(export),
			}));
		}
		if let Some(vacated) = vacated {
			self.deltas.push(EditorDelta::Graph(RuntimeDelta::SetExport {
				network: location.to_owned_location(),
				export_index: vacated,
				input: None,
			}));
		}

		if location.is_root() {
			return;
		}
		let Some(metadata) = self.network_metadata.nested_metadata(location) else { return };
		let export_names = NetworkMetadataChange::ExportNames(metadata.persistent_metadata.export_names.clone());
		let export_descriptions = descriptions_changed.then(|| NetworkMetadataChange::ExportDescriptions(metadata.persistent_metadata.export_descriptions.clone()));
		for change in std::iter::once(export_names).chain(export_descriptions) {
			self.deltas.push(EditorDelta::NetworkMetadata {
				network: location.to_owned_location(),
				change,
			});
		}
	}

	/// Moves the export at `from` to `to`, carrying the encapsulating node's name and description for it.
	pub(crate) fn move_export_slot(&mut self, network_path: &[NodeId], from: usize, to: usize) -> bool {
		let (name, description) = self
			.nested_metadata(network_path)
			.filter(|_| !network_path.is_empty())
			.map(|metadata| {
				let persistent_metadata = &metadata.persistent_metadata;
				(
					persistent_metadata.export_names.get(from).cloned().unwrap_or_default(),
					persistent_metadata.export_descriptions.get(from).cloned().unwrap_or_default(),
				)
			})
			.unwrap_or_default();
		let Some(export) = self.remove_export_slot(network_path, from) else { return false };
		self.insert_export_slot(network_path, to, export, name, description)
	}

	/// Writes the input at `connector`, returning the input it replaced.
	pub(crate) fn set_input_slot(&mut self, connector: &InputConnector, network_path: &[NodeId], input: NodeInput) -> Option<NodeInput> {
		let Some(location) = self.locate(network_path) else {
			log::error!("Could not locate network {network_path:?} in set_input_slot");
			return None;
		};
		self.set_input_slot_at(connector, location, input)
	}

	/// Writes the input at `connector` in the network at `location`, like [`Self::set_input_slot`].
	pub(crate) fn set_input_slot_at(&mut self, connector: &InputConnector, location: NetworkLocation, input: NodeInput) -> Option<NodeInput> {
		let Some(network) = self.network_graph_mut(location) else {
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

		self.deltas.push(input_write_delta(connector, location, input));
		Some(previous)
	}

	/// Edits the value at `connector` in place, recording the value the edit leaves behind.
	///
	/// For a change expressed as an operation on the existing value rather than as a replacement of it,
	/// which avoids copying the value out and back. The recorded delta still carries the whole result,
	/// since the storage op is a whole-value write.
	pub(crate) fn edit_input_value(&mut self, connector: &InputConnector, network_path: &[NodeId], edit: impl FnOnce(&mut TaggedValue) -> bool) -> bool {
		let Some(location) = self.locate(network_path) else {
			log::error!("Could not locate network {network_path:?} in edit_input_value");
			return false;
		};
		let Some(network) = self.network_graph_mut(location) else {
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

		// The edit reports whether it changed anything, rather than this comparing before and after.
		// Comparing the values would mean copying one out, which is the copy this method exists to avoid,
		// and comparing their memoized hashes would let a collision silently drop a real edit.
		let changed = edit(&mut value);
		drop(value);

		// An edit that changes nothing still carries a timestamp, which would let it supersede a concurrent
		// peer's real edit to the same input.
		if !changed {
			return true;
		}

		let input = slot.clone();
		self.deltas.push(input_write_delta(connector, location, input));
		true
	}

	/// Inserts a node and its metadata, returning the entry it replaced.
	pub(crate) fn insert_node_entry(&mut self, locator: NodeLocator, template: NodeTemplate) -> Option<NodeTemplate> {
		let Some(location) = self.locate(locator.network_path) else {
			log::error!("Could not locate network {:?} in insert_node_entry", locator.network_path);
			return None;
		};
		self.insert_node_entry_at(location, locator.node_id, template)
	}

	/// Inserts the node `node_id` into the network at `location`, like [`Self::insert_node_entry`].
	pub(crate) fn insert_node_entry_at(&mut self, location: NetworkLocation, node_id: NodeId, template: NodeTemplate) -> Option<NodeTemplate> {
		if !self.network_pair_exists(location) {
			log::error!("Could not get network {location:?} in insert_node_entry");
			return None;
		}

		let (mut document_node, mut persistent_metadata) = template.into_parts();
		self.lift_into_table(&mut document_node.implementation, &mut persistent_metadata.network_metadata);
		if let DocumentNodeImplementation::Definition(id) = document_node.implementation {
			self.insert_carried_definition(id);
		}
		let node = Box::new(document_node.clone());
		let metadata = Box::new(persistent_metadata.clone());

		let previous_node = self.network.get_mut().network_mut().located_network_mut(location)?.nodes.insert(node_id, document_node);
		let previous_metadata = self.network_metadata.get_mut().nested_metadata_mut(location)?.persistent_metadata.node_metadata.insert(
			node_id,
			DocumentNodeMetadata {
				persistent_metadata,
				transient_metadata: DocumentNodeTransientMetadata::default(),
			},
		);

		// Overwriting an existing entry replaces the node and everything nested under it, which is what
		// distinguishes the two from the storage side.
		let network = location.to_owned_location();
		self.deltas.push(EditorDelta::Graph(match previous_node.is_some() {
			true => RuntimeDelta::ReplaceNode {
				network: network.clone(),
				node_id,
				node,
			},
			false => RuntimeDelta::AddNode {
				network: network.clone(),
				node_id,
				node,
			},
		}));
		self.deltas.push(EditorDelta::NodeMetadataSnapshot { network, node_id, metadata });

		previous_node
			.zip(previous_metadata)
			.map(|(node, metadata)| NodeTemplate::from_parts(node, metadata.persistent_metadata))
	}

	/// Removes a node and its metadata, returning them joined as a template.
	pub(crate) fn remove_node_entry(&mut self, locator: NodeLocator) -> Option<NodeTemplate> {
		let Some(location) = self.locate(locator.network_path) else {
			log::error!("Could not locate network {:?} in remove_node_entry", locator.network_path);
			return None;
		};
		self.remove_node_entry_at(location, locator.node_id)
	}

	/// Removes the node `node_id` from the network at `location`, like [`Self::remove_node_entry`].
	pub(crate) fn remove_node_entry_at(&mut self, location: NetworkLocation, node_id: NodeId) -> Option<NodeTemplate> {
		if !self.network_pair_exists(location) {
			log::error!("Could not get network {location:?} in remove_node_entry");
			return None;
		}

		let node = self.network.get_mut().network_mut().located_network_mut(location)?.nodes.remove(&node_id);
		let metadata = self.network_metadata.get_mut().nested_metadata_mut(location)?.persistent_metadata.node_metadata.remove(&node_id);

		if node.is_some() {
			self.deltas.push(EditorDelta::Graph(RuntimeDelta::RemoveNode {
				network: location.to_owned_location(),
				node_id,
			}));
		}

		node.zip(metadata).map(|(node, metadata)| NodeTemplate::from_parts(node, metadata.persistent_metadata))
	}

	/// Swaps in a new implementation for the node, first moving an inline network it brings into the definitions table with the node's interface.
	pub(crate) fn replace_node_implementation(&mut self, locator: NodeLocator, mut implementation: DocumentNodeImplementation, mut network_metadata: Option<NodeNetworkMetadata>) {
		let Some(location) = self.locate(locator.network_path) else {
			log::error!("Could not locate network {:?} in replace_node_implementation", locator.network_path);
			return;
		};

		if matches!(implementation, DocumentNodeImplementation::Network(_)) {
			let Some(mut node) = self.node_mut_at(location, locator.node_id) else {
				log::error!("Could not get node {} in replace_node_implementation", locator.node_id);
				return;
			};
			let previous = node.interface();
			let interface = &mut network_metadata.get_or_insert_default().persistent_metadata;
			interface.import_metadata = previous.input_metadata.clone();
			interface.export_names = previous.output_names.clone();
			interface.export_descriptions = previous.output_descriptions.clone();

			self.lift_into_table(&mut implementation, &mut network_metadata);
		}
		if let DocumentNodeImplementation::Definition(id) = implementation {
			self.insert_carried_definition(id);
		}

		let Some(mut node) = self.node_mut_at(location, locator.node_id) else {
			log::error!("Could not get node {} in replace_node_implementation", locator.node_id);
			return;
		};
		node.replace_implementation(implementation);
	}

	/// Moves an inline network `implementation` brings, and every inline network nested in it, into the definitions table under fresh IDs.
	/// Each entry is recorded before the entries and nodes that name it.
	fn lift_into_table(&mut self, implementation: &mut DocumentNodeImplementation, network_metadata: &mut Option<NodeNetworkMetadata>) {
		if !matches!(implementation, DocumentNodeImplementation::Network(_)) {
			return;
		}
		let DocumentNodeImplementation::Network(mut network) = std::mem::replace(implementation, DocumentNodeImplementation::Extract) else {
			return;
		};
		let mut metadata = network_metadata.take().unwrap_or_default();

		for (node_id, node) in &mut network.nodes {
			let node_metadata = metadata.persistent_metadata.node_metadata.entry(*node_id).or_default();
			self.lift_into_table(&mut node.implementation, &mut node_metadata.persistent_metadata.network_metadata);
		}

		let id = NetworkId::new();
		self.insert_definition(id, network, metadata);
		*implementation = DocumentNodeImplementation::Definition(id);
	}

	/// Adds the definitions table entry `id` with its metadata, recording the network and then the metadata of it and its nodes.
	pub(crate) fn insert_definition(&mut self, id: NetworkId, network: NodeNetwork, metadata: NodeNetworkMetadata) {
		let mut named: Vec<NetworkId> = network
			.nodes
			.values()
			.filter_map(|node| match node.implementation {
				DocumentNodeImplementation::Definition(named) => Some(named),
				_ => None,
			})
			.collect();
		named.sort();
		for named in named {
			self.insert_carried_definition(named);
		}

		let location = OwnedNetworkLocation::definition(id);
		self.deltas.push(EditorDelta::Graph(RuntimeDelta::AddNetwork {
			id,
			network: Box::new(network.clone()),
		}));

		let mut node_ids: Vec<NodeId> = metadata.persistent_metadata.node_metadata.keys().copied().collect();
		node_ids.sort();
		for node_id in node_ids {
			let node_metadata = Box::new(metadata.persistent_metadata.node_metadata[&node_id].persistent_metadata.clone());
			self.deltas.push(EditorDelta::NodeMetadataSnapshot {
				network: location.clone(),
				node_id,
				metadata: node_metadata,
			});
		}

		let persistent_metadata = &metadata.persistent_metadata;
		let changes = persistent_metadata
			.reference
			.clone()
			.map(|reference| NetworkMetadataChange::Reference(Some(reference)))
			.into_iter()
			.chain([
				NetworkMetadataChange::PinnedOrder(persistent_metadata.pinned_node_order.clone()),
				NetworkMetadataChange::ImportMetadata(persistent_metadata.import_metadata.clone()),
				NetworkMetadataChange::ExportNames(persistent_metadata.export_names.clone()),
				NetworkMetadataChange::ExportDescriptions(persistent_metadata.export_descriptions.clone()),
			]);
		for change in changes {
			self.deltas.push(EditorDelta::NetworkMetadata { network: location.clone(), change });
		}

		self.network.get_mut().network_mut().definitions.insert(id, network);
		self.network_metadata.get_mut().persistent_metadata.definitions.insert(id, metadata);
	}

	/// Takes the table entries a paste carries, each listed after the entries it names, and returns the new ID of every entry that has to be renamed.
	///
	/// An entry the document already holds with the same content is reused. One it holds with different content goes in under a fresh ID.
	/// Nothing is added yet: each entry goes in once a node names it, so it lands in the same transaction as that node.
	pub(crate) fn carry_definitions(&mut self, definitions: Vec<CopiedDefinition>) -> HashMap<NetworkId, NetworkId> {
		let mut renamed = HashMap::new();
		let mut carried = HashMap::new();
		for CopiedDefinition { id, mut network, metadata } in definitions {
			for node in network.nodes.values_mut() {
				if let DocumentNodeImplementation::Definition(named) = &mut node.implementation
					&& let Some(new_id) = renamed.get(named)
				{
					*named = *new_id;
				}
			}

			let held = self.document_network().definitions.get(&id).zip(self.network_metadata.persistent_metadata.definitions.get(&id));
			let id = match held {
				Some((held_network, held_metadata)) if *held_network != network || !same_content(&held_metadata.persistent_metadata, &metadata.persistent_metadata) => {
					let new_id = NetworkId::new();
					renamed.insert(id, new_id);
					new_id
				}
				_ => id,
			};
			carried.insert(id, CopiedDefinition { id, network, metadata });
		}

		self.carried_definitions = carried;
		renamed
	}

	/// Adds the carried table entry `id` if the document does not hold it.
	fn insert_carried_definition(&mut self, id: NetworkId) {
		if self.document_network().definitions.contains_key(&id) {
			return;
		}
		let Some(CopiedDefinition { network, metadata, .. }) = self.carried_definitions.get(&id).cloned() else {
			return;
		};
		self.insert_definition(id, network, metadata);
	}

	/// Drops the definitions table entry `id` from both trees.
	pub(crate) fn remove_definition(&mut self, id: NetworkId) {
		let removed = self.network.get_mut().network_mut().definitions.remove(&id).is_some();
		self.network_metadata.get_mut().persistent_metadata.definitions.remove(&id);
		if removed {
			self.deltas.push(EditorDelta::Graph(RuntimeDelta::RemoveNetwork { id }));
		}
	}

	/// Whether both trees hold the network, checked before a write so a missing one cannot leave the two
	/// halves out of step.
	fn network_pair_exists(&self, location: NetworkLocation) -> bool {
		self.network.network().located_network(location).is_some() && self.network_metadata.nested_metadata(location).is_some()
	}

	/// Takes what the writes since the last drain changed, in write order, leaving the buffer empty
	/// after reading it for the nodes an open transaction has written to.
	pub(crate) fn take_deltas(&mut self) -> Vec<EditorDelta> {
		self.scan_transaction_edited_nodes();
		self.transaction_edited_nodes_scanned = 0;
		std::mem::take(&mut self.deltas)
	}

	/// Drops what the writes since the last drain changed, for writes that are not edits to the document,
	/// after reading it for the nodes an open transaction has written to.
	/// The redirect dots those writes took wires from are forgotten too, so no later edit removes them.
	pub(crate) fn discard_deltas(&mut self) {
		self.scan_transaction_edited_nodes();
		self.transaction_edited_nodes_scanned = 0;
		self.deltas.clear();
		self.dots_that_lost_a_wire.clear();
	}

	/// Records the open transaction's bookkeeping, so writes that are undone before [`Self::restore_transaction_checkpoint`] don't count toward it.
	pub fn transaction_checkpoint(&mut self) -> TransactionCheckpoint {
		self.scan_transaction_edited_nodes();

		TransactionCheckpoint {
			status: self.transaction_status,
			edited_nodes: self.transaction_edited_nodes.clone(),
		}
	}

	/// Puts back the open transaction's bookkeeping from [`Self::transaction_checkpoint`] once the writes made since then are undone, leaving their deltas unread.
	pub fn restore_transaction_checkpoint(&mut self, checkpoint: TransactionCheckpoint) {
		if self.transaction_status == TransactionStatus::Finished {
			return;
		}

		self.transaction_status = checkpoint.status;
		self.transaction_edited_nodes_changed |= self.transaction_edited_nodes != checkpoint.edited_nodes;
		self.transaction_edited_nodes = checkpoint.edited_nodes;
		self.transaction_edited_nodes_scanned = self.deltas.len();
	}

	/// Reads the open transaction's writes since the last call, returning whether that changed which nodes it has written to.
	pub fn update_transaction_edited_nodes(&mut self) -> bool {
		self.scan_transaction_edited_nodes();
		std::mem::take(&mut self.transaction_edited_nodes_changed)
	}

	/// Notes the document network nodes that the unread writes of an open transaction added, or set an input value of.
	fn scan_transaction_edited_nodes(&mut self) {
		if self.transaction_status != TransactionStatus::Finished {
			let new_deltas = self.deltas.get(self.transaction_edited_nodes_scanned..).unwrap_or_default();
			let edited_nodes = new_deltas
				.iter()
				.filter_map(|delta| match delta {
					EditorDelta::Graph(RuntimeDelta::AddNode { network, node_id, .. } | RuntimeDelta::ReplaceNode { network, node_id, .. }) if network.as_location().is_root() => Some(*node_id),
					// Judged by what the input holds now, since splicing in a node briefly disconnects a wire to a placeholder value
					EditorDelta::Graph(RuntimeDelta::SetInput { network, node_id, input_index, .. }) if network.as_location().is_root() => {
						let input = self.input_from_connector(&InputConnector::node_at_index(*node_id, *input_index), &[]);
						input.is_some_and(|input| input.as_value().is_some()).then_some(*node_id)
					}
					_ => None,
				})
				.collect::<Vec<_>>();

			for node_id in edited_nodes {
				self.transaction_edited_nodes_changed |= self.transaction_edited_nodes.insert(node_id);
			}
		}
		self.transaction_edited_nodes_scanned = self.deltas.len();
	}

	pub(super) fn clear_transaction_edited_nodes(&mut self) {
		if !self.transaction_edited_nodes.is_empty() {
			self.transaction_edited_nodes.clear();
			self.transaction_edited_nodes_changed = true;
		}
	}

	/// Drops the network's link to the definition it was instantiated from, which no longer describes it
	/// once its signature is edited.
	pub(crate) fn clear_encapsulating_reference(&mut self, network_path: &[NodeId]) {
		if let Some(mut network) = self.network_mut(network_path) {
			network.set_reference(None);
		}
	}
}

/// Whether two networks' metadata describe the same content, ignoring this peer's view of them.
fn same_content(first: &NodeNetworkPersistentMetadata, second: &NodeNetworkPersistentMetadata) -> bool {
	let content = |metadata: &NodeNetworkPersistentMetadata| NodeNetworkPersistentMetadata {
		previewing: Previewing::No,
		navigation_metadata: NavigationMetadata::default(),
		selection_undo_history: VecDeque::new(),
		selection_redo_history: VecDeque::new(),
		..metadata.clone()
	};
	content(first) == content(second)
}

/// Whether any of `descriptions` says anything, since storage keeps the whole list then, but not a list that's all empty.
fn has_description(descriptions: &[String]) -> bool {
	descriptions.iter().any(|description| !description.is_empty())
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
fn input_write_delta(connector: &InputConnector, location: NetworkLocation, input: NodeInput) -> EditorDelta {
	EditorDelta::Graph(match connector {
		InputConnector::Node { node_id, input_index } => RuntimeDelta::SetInput {
			network: location.to_owned_location(),
			node_id: *node_id,
			input_index: *input_index,
			input,
		},
		InputConnector::Export(export_index) => RuntimeDelta::SetExport {
			network: location.to_owned_location(),
			export_index: *export_index,
			input: Some(input),
		},
	})
}
