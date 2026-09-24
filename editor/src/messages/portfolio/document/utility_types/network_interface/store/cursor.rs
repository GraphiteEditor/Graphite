//! The recorded-write cursors: the only handles through which a node or a network is edited.
//!
//! Each setter writes its field and records a delta naming exactly what it wrote, so a write cannot
//! reach the document without also reaching the delta buffer.

use super::*;
use graph_craft::runtime_delta::RuntimeDelta;

/// A network's selection undo and redo stacks, borrowed together since every operation on one is
/// paired with one on the other.
pub(crate) struct SelectionHistoryMut<'a> {
	pub(crate) undo: &'a mut VecDeque<SelectedNodes>,
	pub(crate) redo: &'a mut VecDeque<SelectedNodes>,
}

/// Addresses one node: the network it lives in, and its ID within that network.
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
	pub(super) node: &'a mut DocumentNode,
	pub(super) metadata: &'a mut DocumentNodePersistentMetadata,
	deltas: NodeDeltas<'a>,
}

impl<'a> NodeMut<'a> {
	pub(super) fn new(node: &'a mut DocumentNode, metadata: &'a mut DocumentNodePersistentMetadata, deltas: &'a mut Vec<EditorDelta>, locator: NodeLocator<'a>) -> Self {
		Self {
			node,
			metadata,
			deltas: NodeDeltas { deltas, locator },
		}
	}
}

/// Where a node's writes record what they changed, carrying the node's address so each setter names
/// what it wrote without its caller repeating it.
struct NodeDeltas<'a> {
	pub(super) deltas: &'a mut Vec<EditorDelta>,
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

	/// Records a change to the network this node owns, addressed by the node's path extended with its ID.
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

	/// Records a snapshot for a node inside the network this one owns, for metadata that arrived with a
	/// new nested network rather than describing this node.
	fn snapshot_owned(&mut self, node_id: NodeId, metadata: Box<DocumentNodePersistentMetadata>) {
		self.deltas.push(EditorDelta::NodeMetadataSnapshot {
			network_path: self.locator.owned_network_path(),
			node_id,
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

	/// Whether the node is displayed as a layer or a node, together with its position.
	pub(crate) fn set_node_type(&mut self, node_type: NodeTypePersistentMetadata) -> bool {
		let changed = self.metadata.node_type_metadata != node_type;
		self.metadata.node_type_metadata = node_type;
		if changed {
			let change = NodeMetadataChange::NodeType(self.metadata.node_type_metadata.clone());
			self.deltas.metadata(change);
		}
		changed
	}

	/// The definition this node's nested network was instantiated from. Only network nodes carry one.
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
		let changed = input_metadata.persistent_metadata.widget_override != widget_override;
		input_metadata.persistent_metadata.widget_override = widget_override;
		if changed {
			let change = NodeMetadataChange::WidgetOverride {
				index: input_index,
				widget_override: input_metadata.persistent_metadata.widget_override.clone(),
			};
			self.deltas.metadata(change);
		}
		changed
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

	/// The encapsulating node's names for every one of the network's exports, replaced together since
	/// they are stored as one array.
	#[cfg_attr(not(test), expect(dead_code, reason = "reached only through replay, which has no caller until deltas from another peer arrive"))]
	pub(crate) fn set_output_names(&mut self, output_names: Vec<String>) -> bool {
		let changed = self.metadata.output_names != output_names;
		self.metadata.output_names = output_names;
		if changed {
			let change = NodeMetadataChange::OutputNames(self.metadata.output_names.clone());
			self.deltas.metadata(change);
		}
		changed
	}

	/// The metadata array the node's inputs index, one entry per input.
	#[cfg_attr(not(test), expect(dead_code, reason = "reached only through replay, which has no caller until deltas from another peer arrive"))]
	pub(crate) fn input_metadata(&self) -> &[InputMetadata] {
		&self.metadata.input_metadata
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
	///
	/// Entries past the last input are left alone, not truncated: nothing indexes them, and this runs on
	/// every open, so dropping them would destroy user-authored names and overrides with no way back.
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

		// Only what the new implementation brought with it. A swap leaves the node's own metadata alone,
		// and restating it would assert values this peer did not write, clobbering a concurrent rename.
		let owned: Vec<_> = self
			.metadata
			.network_metadata
			.iter()
			.flat_map(|network_metadata| &network_metadata.persistent_metadata.node_metadata)
			.map(|(child_id, child)| (*child_id, Box::new(child.persistent_metadata.clone())))
			.collect();
		for (child_id, metadata) in owned {
			self.deltas.snapshot_owned(child_id, metadata);
		}
	}

	/// Replaces the node's whole persistent metadata, including that of everything nested under it,
	/// which is the write a metadata snapshot describes.
	#[cfg_attr(not(test), expect(dead_code, reason = "reached only through replay, which has no caller until deltas from another peer arrive"))]
	pub(crate) fn replace_metadata(&mut self, metadata: DocumentNodePersistentMetadata) {
		*self.metadata = metadata;

		let snapshot = Box::new(self.metadata.clone());
		self.deltas.snapshot(snapshot);
	}

	/// Replaces the metadata array the node's inputs index, leaving the inputs themselves alone.
	#[cfg_attr(not(test), expect(dead_code, reason = "reached only through replay, which has no caller until deltas from another peer arrive"))]
	pub(crate) fn set_input_metadata(&mut self, input_metadata: Vec<InputMetadata>) {
		self.metadata.input_metadata = input_metadata;
		self.emit_input_metadata();
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
	pub(super) fn emit_inputs(&mut self) {
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
	pub(super) metadata: &'a mut NodeNetworkPersistentMetadata,
	pub(super) deltas: &'a mut Vec<EditorDelta>,
	pub(super) network_path: &'a [NodeId],
}

impl<'a> NetworkMut<'a> {
	pub(super) fn new(metadata: &'a mut NodeNetworkPersistentMetadata, deltas: &'a mut Vec<EditorDelta>, network_path: &'a [NodeId]) -> Self {
		Self { metadata, deltas, network_path }
	}
}

impl NetworkMut<'_> {
	/// Which node the network renders instead of its export. This write is not recorded.
	pub(crate) fn set_previewing(&mut self, previewing: Previewing) -> bool {
		let changed = self.metadata.previewing != previewing;
		self.metadata.previewing = previewing;
		changed
	}

	/// The definition this network was instantiated from.
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
