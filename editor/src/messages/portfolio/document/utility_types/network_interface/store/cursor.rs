//! The write cursors: the only handles through which a node or a network is edited.
//!
//! Each setter reports whether it changed anything, so a caller invalidates only what it actually
//! wrote.

use super::*;

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
}

impl<'a> NodeMut<'a> {
	pub(super) fn new(node: &'a mut DocumentNode, metadata: &'a mut DocumentNodePersistentMetadata) -> Self {
		Self { node, metadata }
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
		changed
	}

	/// The type of argument the node can be evaluated with.
	pub(crate) fn set_call_argument(&mut self, call_argument: Type) -> bool {
		let changed = self.node.call_argument != call_argument;
		self.node.call_argument = call_argument;
		changed
	}

	/// The Extract and Inject annotations the node declares for the Context.
	pub(crate) fn set_context_features(&mut self, context_features: ContextDependencies) -> bool {
		let changed = self.node.context_features != context_features;
		self.node.context_features = context_features;
		changed
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

	/// Whether the node is displayed as a layer or a node, together with its position.
	pub(crate) fn set_node_type(&mut self, node_type: NodeTypePersistentMetadata) -> bool {
		let changed = self.metadata.node_type_metadata != node_type;
		self.metadata.node_type_metadata = node_type;
		changed
	}

	/// The definition this node's nested network was instantiated from. Only network nodes carry one.
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
		let changed = input_metadata.persistent_metadata.widget_override != widget_override;
		input_metadata.persistent_metadata.widget_override = widget_override;
		changed
	}

	pub(crate) fn set_output_name(&mut self, output_index: usize, output_name: String) -> bool {
		let Some(existing) = self.metadata.output_names.get_mut(output_index) else { return false };
		let changed = *existing != output_name;
		*existing = output_name;
		changed
	}

	/// Grows `output_names` to one entry per export, which an older document may be missing.
	pub(crate) fn resize_output_names(&mut self, number_of_exports: usize) {
		if self.metadata.output_names.len() == number_of_exports {
			return;
		}
		self.metadata.output_names.resize(number_of_exports, String::new());
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
	}

	/// Swaps in a new implementation together with the nested network metadata that belongs to it.
	///
	/// Replaces the node and everything nested under it, since the nested network goes with the
	/// implementation.
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

/// The persistent metadata of one network, resolved once so a run of writes walks the tree a single time.
///
/// Metadata only: the graph half of a network is its export slots, which are addressed through
/// [`NodeNetworkInterface::set_input_slot`] like any other input.
pub(crate) struct NetworkMut<'a> {
	pub(super) metadata: &'a mut NodeNetworkPersistentMetadata,
}

impl<'a> NetworkMut<'a> {
	pub(super) fn new(metadata: &'a mut NodeNetworkPersistentMetadata) -> Self {
		Self { metadata }
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
		changed
	}

	/// The display order of pinned nodes in the Properties panel.
	pub(crate) fn set_pinned_order(&mut self, pinned_node_order: Vec<NodeId>) -> bool {
		let changed = self.metadata.pinned_node_order != pinned_node_order;
		self.metadata.pinned_node_order = pinned_node_order;
		changed
	}

	/// Appends a newly pinned node to the display order, or drops one that is no longer pinned.
	pub(crate) fn record_pinned(&mut self, node_id: NodeId, pinned: bool) -> bool {
		let order = &mut self.metadata.pinned_node_order;
		match pinned {
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
		}
	}

	/// Drops every node the order names that is no longer in the network.
	pub(crate) fn retain_pinned(&mut self, surviving: impl Fn(&NodeId) -> bool) -> bool {
		let before = self.metadata.pinned_node_order.len();
		self.metadata.pinned_node_order.retain(&surviving);

		self.metadata.pinned_node_order.len() != before
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
