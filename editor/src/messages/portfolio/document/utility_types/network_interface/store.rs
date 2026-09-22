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
///
/// Holds the halves directly rather than the interface, so a write cannot reach back out to query the
/// graph or unload a cache: what a write invalidates is declared, not performed here.
pub(crate) struct NodeMut<'a> {
	node: &'a mut DocumentNode,
	metadata: &'a mut DocumentNodePersistentMetadata,
}

impl NodeMut<'_> {
	/// Whether the node is rendered, which the compiler reads to replace it with a passthrough.
	pub(crate) fn set_visible(&mut self, visible: bool) -> bool {
		let changed = self.node.visible != visible;
		self.node.visible = visible;
		changed
	}

	/// The type of argument the node can be evaluated with.
	pub(crate) fn set_call_argument(&mut self, call_argument: Type) {
		self.node.call_argument = call_argument;
	}

	/// The Extract and Inject annotations the node declares for the Context.
	pub(crate) fn set_context_features(&mut self, context_features: ContextDependencies) {
		self.node.context_features = context_features;
	}

	/// The user-chosen name for this instance, empty when it has none.
	pub(crate) fn set_display_name(&mut self, display_name: String) -> bool {
		let changed = self.metadata.display_name != display_name;
		self.metadata.display_name = display_name;
		changed
	}

	pub(crate) fn set_locked(&mut self, locked: bool) -> bool {
		let changed = self.metadata.locked != locked;
		self.metadata.locked = locked;
		changed
	}

	pub(crate) fn set_pinned(&mut self, pinned: bool) -> bool {
		let changed = self.metadata.pinned != pinned;
		self.metadata.pinned = pinned;
		changed
	}

	/// Whether the node is displayed as a layer or a node, together with its position, which are one
	/// choice: a layer and a node do not have the same kinds of position.
	pub(crate) fn set_node_type(&mut self, node_type: NodeTypePersistentMetadata) -> bool {
		let changed = self.metadata.node_type_metadata != node_type;
		self.metadata.node_type_metadata = node_type;
		changed
	}

	/// The definition this node's nested network was instantiated from, dropped once the node is edited
	/// away from it. Only network nodes carry one.
	pub(crate) fn set_reference(&mut self, reference: Option<String>) -> bool {
		let Some(network_metadata) = self.metadata.network_metadata.as_mut() else { return false };
		let changed = network_metadata.persistent_metadata.reference != reference;
		network_metadata.persistent_metadata.reference = reference;
		changed
	}

	pub(crate) fn set_input_name(&mut self, input_index: usize, input_name: String) -> bool {
		let Some(input_metadata) = self.metadata.input_metadata.get_mut(input_index) else { return false };
		let changed = input_metadata.persistent_metadata.input_name != input_name;
		input_metadata.persistent_metadata.input_name = input_name;
		changed
	}

	/// The identifier of the widget override the properties panel uses for this input, or `None` for the
	/// widget generated from its type.
	pub(crate) fn set_widget_override(&mut self, input_index: usize, widget_override: Option<String>) -> bool {
		let Some(input_metadata) = self.metadata.input_metadata.get_mut(input_index) else { return false };
		input_metadata.persistent_metadata.widget_override = widget_override;
		true
	}

	pub(crate) fn set_output_name(&mut self, output_index: usize, output_name: String) -> bool {
		let Some(existing) = self.metadata.output_names.get_mut(output_index) else { return false };
		let changed = *existing != output_name;
		*existing = output_name;
		changed
	}

	/// Grows `output_names` to one entry per export, which an older document may be missing.
	pub(crate) fn resize_output_names(&mut self, number_of_exports: usize) {
		self.metadata.output_names.resize(number_of_exports, String::new());
	}

	/// Appends input metadata until there is one entry per input, filling from `defaults` where it has an
	/// entry for the added index. Restores the parallel-array invariant for an older document.
	pub(crate) fn pad_input_metadata(&mut self, number_of_inputs: usize, defaults: impl Fn(usize) -> Option<InputMetadata>) {
		for added_input_index in self.metadata.input_metadata.len()..number_of_inputs {
			self.metadata.input_metadata.push(defaults(added_input_index).unwrap_or_default());
		}
	}

	/// Swaps in a new implementation together with the nested network metadata that belongs to it.
	pub(crate) fn replace_implementation(&mut self, implementation: DocumentNodeImplementation, network_metadata: Option<NodeNetworkMetadata>) {
		self.node.implementation = implementation;
		self.metadata.network_metadata = network_metadata;
	}

	/// Swaps in new inputs together with their metadata, returning the inputs replaced.
	pub(crate) fn replace_inputs(&mut self, inputs: Vec<NodeInput>, input_metadata: Vec<InputMetadata>) -> Vec<NodeInput> {
		self.metadata.input_metadata = input_metadata;
		std::mem::replace(&mut self.node.inputs, inputs)
	}
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

	/// Writes the input at `connector`, returning the input it replaced.
	pub(crate) fn set_input_slot(&mut self, connector: &InputConnector, network_path: &[NodeId], input: NodeInput) -> Option<NodeInput> {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_input_slot");
			return None;
		};

		let slot = match connector {
			InputConnector::Node { node_id, input_index } => network.nodes.get_mut(node_id).and_then(|node| node.inputs.get_mut(*input_index)),
			InputConnector::Export(export_index) => network.exports.get_mut(*export_index),
		};
		let Some(slot) = slot else {
			log::error!("Could not get input {connector:?} in set_input_slot");
			return None;
		};

		Some(std::mem::replace(slot, input))
	}

	/// Inserts a node and its metadata, returning the entry it replaced.
	pub(crate) fn insert_node_entry(&mut self, locator: NodeLocator, template: NodeTemplate) -> Option<NodeTemplate> {
		if !self.network_pair_exists(locator.network_path) {
			log::error!("Could not get network {:?} in insert_node_entry", locator.network_path);
			return None;
		}

		let (document_node, persistent_metadata) = template.into_parts();
		let previous_node = self.network.network_mut().nested_network_mut(locator.network_path)?.nodes.insert(locator.node_id, document_node);
		let previous_metadata = self.network_metadata.nested_metadata_mut(locator.network_path)?.persistent_metadata.node_metadata.insert(
			locator.node_id,
			DocumentNodeMetadata {
				persistent_metadata,
				transient_metadata: DocumentNodeTransientMetadata::default(),
			},
		);

		previous_node.zip(previous_metadata).map(|(node, metadata)| NodeTemplate::from_parts(node, metadata.persistent_metadata))
	}

	/// Removes a node and its metadata, returning them joined as a template.
	pub(crate) fn remove_node_entry(&mut self, locator: NodeLocator) -> Option<NodeTemplate> {
		if !self.network_pair_exists(locator.network_path) {
			log::error!("Could not get network {:?} in remove_node_entry", locator.network_path);
			return None;
		}

		let node = self.network.network_mut().nested_network_mut(locator.network_path)?.nodes.remove(&locator.node_id);
		let metadata = self
			.network_metadata
			.nested_metadata_mut(locator.network_path)?
			.persistent_metadata
			.node_metadata
			.remove(&locator.node_id);

		node.zip(metadata).map(|(node, metadata)| NodeTemplate::from_parts(node, metadata.persistent_metadata))
	}

	/// Whether both trees hold the network, checked before a write so a missing one cannot leave the two
	/// halves out of step.
	fn network_pair_exists(&self, network_path: &[NodeId]) -> bool {
		self.network.network().nested_network(network_path).is_some() && self.network_metadata.nested_metadata(network_path).is_some()
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
