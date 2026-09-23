use super::*;
use graph_craft::runtime_delta::RuntimeDelta;

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
	deltas: NodeDeltas<'a>,
}

/// Where a node's writes record what they changed, carrying the node's address so each setter names
/// what it wrote without its caller repeating it.
struct NodeDeltas<'a> {
	deltas: &'a mut Vec<EditorDelta>,
	locator: NodeLocator<'a>,
}

impl NodeDeltas<'_> {
	fn metadata(&mut self, change: NodeMetadataChange) {
		self.deltas.push(EditorDelta::NodeMetadata {
			network_path: self.locator.network_path.to_vec(),
			node_id: self.locator.node_id,
			change,
		});
	}

	/// The nested network a node owns is addressed by the node's own path extended with its ID, which
	/// is the path a scoped conversion resolves that network's ID from.
	fn owned_network(&mut self, change: NetworkMetadataChange) {
		self.deltas.push(EditorDelta::NetworkMetadata {
			network_path: self.locator.owned_network_path(),
			change,
		});
	}

	fn graph(&mut self, delta: RuntimeDelta) {
		self.deltas.push(EditorDelta::Graph(delta));
	}

	fn input_metadata(&mut self, input_metadata: Vec<InputMetadata>) {
		self.deltas.push(EditorDelta::NodeInputMetadata {
			network_path: self.locator.network_path.to_vec(),
			node_id: self.locator.node_id,
			input_metadata,
		});
	}

	fn snapshot(&mut self, metadata: Box<DocumentNodePersistentMetadata>) {
		self.deltas.push(EditorDelta::NodeMetadataSnapshot {
			network_path: self.locator.network_path.to_vec(),
			node_id: self.locator.node_id,
			metadata,
		});
	}

	fn network_path(&self) -> Vec<NodeId> {
		self.locator.network_path.to_vec()
	}

	fn node_id(&self) -> NodeId {
		self.locator.node_id
	}
}

impl NodeMut<'_> {
	/// Places the node at an absolute grid position, keeping whether it is displayed as a layer or a node.
	pub(crate) fn set_absolute_position(&mut self, position: IVec2) -> bool {
		let node_type = match self.metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(_) => NodeTypePersistentMetadata::layer(position),
			NodeTypePersistentMetadata::Node(_) => NodeTypePersistentMetadata::node(position),
		};
		self.set_node_type(node_type)
	}

	/// Offsets a node that is absolutely positioned, leaving a stack or chain node where it is.
	pub(crate) fn shift_absolute_position(&mut self, shift: IVec2) -> bool {
		let shifted = match self.metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::Absolute(position),
			}) => NodeTypePersistentMetadata::layer(position + shift),
			NodeTypePersistentMetadata::Node(ref node) => match node.position {
				NodePosition::Absolute(position) => NodeTypePersistentMetadata::node(position + shift),
				NodePosition::Chain => return false,
			},
			NodeTypePersistentMetadata::Layer(_) => return false,
		};
		self.set_node_type(shifted)
	}

	/// Stacks a layer `y_offset` below the sibling it feeds. Nodes are never stacked.
	pub(crate) fn set_stack_position(&mut self, y_offset: u32) -> bool {
		if !self.metadata.is_layer() {
			return false;
		}
		self.set_node_type(NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
			position: LayerPosition::Stack(y_offset),
		}))
	}

	/// Chains a node to the left of the layer it feeds. Layers are never chained.
	pub(crate) fn set_chain_position(&mut self) -> bool {
		if self.metadata.is_layer() {
			return false;
		}
		self.set_node_type(NodeTypePersistentMetadata::Node(NodePersistentMetadata::new(NodePosition::Chain)))
	}

	/// Whether the node is displayed as a layer, which decides the kinds of position it can hold.
	pub(crate) fn is_layer(&self) -> bool {
		self.metadata.is_layer()
	}

	/// Whether the node is rendered, which the compiler reads to replace it with a passthrough.
	pub(crate) fn set_visible(&mut self, visible: bool) -> bool {
		let changed = self.node.visible != visible;
		self.node.visible = visible;
		if changed {
			let delta = RuntimeDelta::SetVisibility {
				network_path: self.deltas.network_path(),
				node_id: self.deltas.node_id(),
				visible,
			};
			self.deltas.graph(delta);
		}
		changed
	}

	/// The type of argument the node can be evaluated with.
	pub(crate) fn set_call_argument(&mut self, call_argument: Type) -> bool {
		let changed = self.node.call_argument != call_argument;
		self.node.call_argument = call_argument;
		if changed {
			let delta = RuntimeDelta::SetCallArgument {
				network_path: self.deltas.network_path(),
				node_id: self.deltas.node_id(),
				call_argument: self.node.call_argument.clone(),
			};
			self.deltas.graph(delta);
		}
		changed
	}

	/// The Extract and Inject annotations the node declares for the Context.
	pub(crate) fn set_context_features(&mut self, context_features: ContextDependencies) -> bool {
		let changed = self.node.context_features != context_features;
		self.node.context_features = context_features;
		if changed {
			let delta = RuntimeDelta::SetContextFeatures {
				network_path: self.deltas.network_path(),
				node_id: self.deltas.node_id(),
				context_features: self.node.context_features,
			};
			self.deltas.graph(delta);
		}
		changed
	}

	/// The user-chosen name for this instance, empty when it has none.
	pub(crate) fn set_display_name(&mut self, display_name: String) -> bool {
		let changed = self.metadata.display_name != display_name;
		self.metadata.display_name = display_name;
		if changed {
			let change = NodeMetadataChange::DisplayName(self.metadata.display_name.clone());
			self.deltas.metadata(change);
		}
		changed
	}

	pub(crate) fn set_locked(&mut self, locked: bool) -> bool {
		let changed = self.metadata.locked != locked;
		self.metadata.locked = locked;
		if changed {
			self.deltas.metadata(NodeMetadataChange::Locked(locked));
		}
		changed
	}

	pub(crate) fn set_pinned(&mut self, pinned: bool) -> bool {
		let changed = self.metadata.pinned != pinned;
		self.metadata.pinned = pinned;
		if changed {
			self.deltas.metadata(NodeMetadataChange::Pinned(pinned));
		}
		changed
	}

	/// Whether the node is displayed as a layer or a node, together with its position, which are one
	/// choice: a layer and a node do not have the same kinds of position.
	pub(crate) fn set_node_type(&mut self, node_type: NodeTypePersistentMetadata) -> bool {
		let changed = self.metadata.node_type_metadata != node_type;
		self.metadata.node_type_metadata = node_type;
		if changed {
			let change = NodeMetadataChange::NodeType(self.metadata.node_type_metadata.clone());
			self.deltas.metadata(change);
		}
		changed
	}

	/// The definition this node's nested network was instantiated from, dropped once the node is edited
	/// away from it. Only network nodes carry one.
	pub(crate) fn set_reference(&mut self, reference: Option<String>) -> bool {
		let Some(network_metadata) = self.metadata.network_metadata.as_mut() else { return false };
		let changed = network_metadata.persistent_metadata.reference != reference;
		network_metadata.persistent_metadata.reference = reference;
		if changed {
			let change = NetworkMetadataChange::Reference(network_metadata.persistent_metadata.reference.clone());
			self.deltas.owned_network(change);
		}
		changed
	}

	pub(crate) fn set_input_name(&mut self, input_index: usize, input_name: String) -> bool {
		let Some(input_metadata) = self.metadata.input_metadata.get_mut(input_index) else { return false };
		let changed = input_metadata.persistent_metadata.input_name != input_name;
		input_metadata.persistent_metadata.input_name = input_name;
		if changed {
			let change = NodeMetadataChange::InputName {
				index: input_index,
				name: input_metadata.persistent_metadata.input_name.clone(),
			};
			self.deltas.metadata(change);
		}
		changed
	}

	/// The identifier of the widget override the properties panel uses for this input, or `None` for the
	/// widget generated from its type.
	pub(crate) fn set_widget_override(&mut self, input_index: usize, widget_override: Option<String>) -> bool {
		let Some(input_metadata) = self.metadata.input_metadata.get_mut(input_index) else { return false };
		input_metadata.persistent_metadata.widget_override = widget_override;

		let change = NodeMetadataChange::WidgetOverride {
			index: input_index,
			widget_override: input_metadata.persistent_metadata.widget_override.clone(),
		};
		self.deltas.metadata(change);
		true
	}

	pub(crate) fn set_output_name(&mut self, output_index: usize, output_name: String) -> bool {
		let Some(existing) = self.metadata.output_names.get_mut(output_index) else { return false };
		let changed = *existing != output_name;
		*existing = output_name;
		if changed {
			let change = NodeMetadataChange::OutputNames(self.metadata.output_names.clone());
			self.deltas.metadata(change);
		}
		changed
	}

	/// Grows `output_names` to one entry per export, which an older document may be missing.
	pub(crate) fn resize_output_names(&mut self, number_of_exports: usize) {
		if self.metadata.output_names.len() == number_of_exports {
			return;
		}
		self.metadata.output_names.resize(number_of_exports, String::new());

		let change = NodeMetadataChange::OutputNames(self.metadata.output_names.clone());
		self.deltas.metadata(change);
	}

	/// Appends input metadata until there is one entry per input, filling from `defaults` where it has an
	/// entry for the added index. Restores the parallel-array invariant for an older document.
	pub(crate) fn pad_input_metadata(&mut self, number_of_inputs: usize, defaults: impl Fn(usize) -> Option<InputMetadata>) {
		if self.metadata.input_metadata.len() >= number_of_inputs {
			return;
		}
		for added_input_index in self.metadata.input_metadata.len()..number_of_inputs {
			self.metadata.input_metadata.push(defaults(added_input_index).unwrap_or_default());
		}
		self.emit_input_metadata();
	}

	/// Swaps in a new implementation together with the nested network metadata that belongs to it.
	///
	/// Replaces the node and everything nested under it, since the nested network goes with the
	/// implementation.
	pub(crate) fn replace_implementation(&mut self, implementation: DocumentNodeImplementation, network_metadata: Option<NodeNetworkMetadata>) {
		self.node.implementation = implementation;
		self.metadata.network_metadata = network_metadata;

		let delta = RuntimeDelta::ReplaceNode {
			network_path: self.deltas.network_path(),
			node_id: self.deltas.node_id(),
			node: Box::new(self.node.clone()),
		};
		self.deltas.graph(delta);
		let metadata = Box::new(self.metadata.clone());
		self.deltas.snapshot(metadata);
	}

	/// Swaps in new inputs together with their metadata, returning the inputs replaced.
	pub(crate) fn replace_inputs(&mut self, inputs: Vec<NodeInput>, input_metadata: Vec<InputMetadata>) -> Vec<NodeInput> {
		self.metadata.input_metadata = input_metadata;
		let previous = std::mem::replace(&mut self.node.inputs, inputs);

		self.emit_inputs();
		previous
	}

	/// Records the node's input list and the metadata array indexed by it, which change together
	/// whenever the number or order of slots does.
	fn emit_inputs(&mut self) {
		let delta = RuntimeDelta::SetInputs {
			network_path: self.deltas.network_path(),
			node_id: self.deltas.node_id(),
			inputs: self.node.inputs.clone(),
		};
		self.deltas.graph(delta);
		self.emit_input_metadata();
	}

	fn emit_input_metadata(&mut self) {
		let input_metadata = self.metadata.input_metadata.clone();
		self.deltas.input_metadata(input_metadata);
	}
}

/// The persistent metadata of one network, resolved once so a run of writes walks the tree a single time.
///
/// Metadata only: the graph half of a network is its export slots, which are addressed through
/// [`NodeNetworkInterface::set_input_slot`] like any other input.
pub(crate) struct NetworkMut<'a> {
	metadata: &'a mut NodeNetworkPersistentMetadata,
	deltas: &'a mut Vec<EditorDelta>,
	network_path: &'a [NodeId],
}

impl NetworkMut<'_> {
	/// Which node the network renders instead of its export, and what the export reconnects to when the
	/// preview ends.
	pub(crate) fn set_previewing(&mut self, previewing: Previewing) -> bool {
		let changed = self.metadata.previewing != previewing;
		self.metadata.previewing = previewing;
		if changed {
			self.emit(NetworkMetadataChange::Previewing(previewing));
		}
		changed
	}

	/// The definition this network was instantiated from, dropped once it is edited away from it.
	pub(crate) fn set_reference(&mut self, reference: Option<String>) -> bool {
		let changed = self.metadata.reference != reference;
		self.metadata.reference = reference;
		if changed {
			let change = NetworkMetadataChange::Reference(self.metadata.reference.clone());
			self.emit(change);
		}
		changed
	}

	/// The display order of pinned nodes in the Properties panel.
	pub(crate) fn set_pinned_order(&mut self, pinned_node_order: Vec<NodeId>) -> bool {
		let changed = self.metadata.pinned_node_order != pinned_node_order;
		self.metadata.pinned_node_order = pinned_node_order;
		if changed {
			let change = NetworkMetadataChange::PinnedOrder(self.metadata.pinned_node_order.clone());
			self.emit(change);
		}
		changed
	}

	/// Appends a newly pinned node to the display order, or drops one that is no longer pinned.
	pub(crate) fn record_pinned(&mut self, node_id: NodeId, pinned: bool) -> bool {
		let order = &mut self.metadata.pinned_node_order;
		let changed = match pinned {
			true if !order.contains(&node_id) => {
				order.push(node_id);
				true
			}
			true => false,
			false => {
				let before = order.len();
				order.retain(|id| *id != node_id);
				order.len() != before
			}
		};

		if changed {
			let change = NetworkMetadataChange::PinnedOrder(self.metadata.pinned_node_order.clone());
			self.emit(change);
		}
		changed
	}

	/// Drops every node the order names that is no longer in the network.
	pub(crate) fn retain_pinned(&mut self, surviving: impl Fn(&NodeId) -> bool) -> bool {
		let before = self.metadata.pinned_node_order.len();
		self.metadata.pinned_node_order.retain(&surviving);

		let changed = self.metadata.pinned_node_order.len() != before;
		if changed {
			let change = NetworkMetadataChange::PinnedOrder(self.metadata.pinned_node_order.clone());
			self.emit(change);
		}
		changed
	}

	fn emit(&mut self, change: NetworkMetadataChange) {
		self.deltas.push(EditorDelta::NetworkMetadata {
			network_path: self.network_path.to_vec(),
			change,
		});
	}

	/// The transform from node graph space to viewport space.
	pub(crate) fn set_navigation_transform(&mut self, transform: DAffine2) -> bool {
		let changed = self.metadata.navigation_metadata.node_graph_to_viewport != transform;
		self.metadata.navigation_metadata.node_graph_to_viewport = transform;
		changed
	}

	/// The width of the node graph in viewport space.
	pub(crate) fn set_navigation_width(&mut self, node_graph_width: f64) -> bool {
		let changed = self.metadata.navigation_metadata.node_graph_width != node_graph_width;
		self.metadata.navigation_metadata.node_graph_width = node_graph_width;
		changed
	}
}

// The store: the only writer of the node graph and its parallel metadata tree. Every write keeps the
// two halves in step by construction, so the invariants checked by `validate_invariants` hold without
// each caller restating them.
impl NodeNetworkInterface {
	/// The persistent metadata of a network, or `None` if the network is missing.
	pub(crate) fn network_mut<'a>(&'a mut self, network_path: &'a [NodeId]) -> Option<NetworkMut<'a>> {
		let metadata = &mut self.network_metadata.nested_metadata_mut(network_path)?.persistent_metadata;
		Some(NetworkMut {
			metadata,
			deltas: &mut self.deltas,
			network_path,
		})
	}

	/// Both halves of a node, or `None` if either is missing.
	pub(crate) fn node_mut<'a>(&'a mut self, locator: NodeLocator<'a>) -> Option<NodeMut<'a>> {
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
			deltas: NodeDeltas { deltas: &mut self.deltas, locator },
		})
	}

	/// Inserts an input and its metadata at `index`, clamped to the end. Returns whether the node was found.
	pub(crate) fn insert_input_slot(&mut self, locator: NodeLocator, index: usize, input: NodeInput, metadata: InputMetadata) -> bool {
		let Some(mut node) = self.node_mut(locator) else {
			log::error!("Could not get node {} in _input_slot", locator.node_id);
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
			log::error!("Could not get node {} in _input_slot", locator.node_id);
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

		let slot = match connector {
			InputConnector::Node { node_id, input_index } => network.nodes.get_mut(node_id).and_then(|node| node.inputs.get_mut(*input_index)),
			InputConnector::Export(export_index) => network.exports.get_mut(*export_index),
		};
		let Some(slot) = slot else {
			log::error!("Could not get input {connector:?} in set_input_slot");
			return None;
		};

		if *slot == input {
			return Some(input);
		}
		let previous = std::mem::replace(slot, input.clone());

		self.deltas.push(EditorDelta::Graph(match connector {
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
		}));

		Some(previous)
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

		let previous_node = self.network.network_mut().nested_network_mut(locator.network_path)?.nodes.insert(locator.node_id, document_node);
		let previous_metadata = self.network_metadata.nested_metadata_mut(locator.network_path)?.persistent_metadata.node_metadata.insert(
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

		let node = self.network.network_mut().nested_network_mut(locator.network_path)?.nodes.remove(&locator.node_id);
		let metadata = self
			.network_metadata
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

	/// Drops the network's link to the definition it was instantiated from, which no longer describes it
	/// once its signature is edited.
	pub(crate) fn clear_encapsulating_reference(&mut self, network_path: &[NodeId]) {
		if let Some(mut network) = self.network_mut(network_path) {
			network.set_reference(None);
		}
	}
}
