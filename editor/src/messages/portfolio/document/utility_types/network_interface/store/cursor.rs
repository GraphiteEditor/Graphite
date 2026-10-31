//! The recorded-write cursors: the only handles through which a node or a network is edited.
//!
//! Each setter writes its field and records a delta naming exactly what it wrote, so a write cannot
//! reach the document without also reaching the delta buffer.

use super::*;
use graph_craft::document::OwnedNetworkLocation;
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
}

/// The graph and metadata halves of one node, resolved together so the two parallel trees are never
/// reached separately and cannot drift apart.
///
/// Holds the halves directly rather than the interface, so a write cannot reach back out to query the
/// graph or unload a cache: what a write invalidates is declared, not performed here.
pub(crate) struct NodeMut<'a> {
	pub(super) node: &'a mut DocumentNode,
	pub(super) metadata: &'a mut DocumentNodePersistentMetadata,
	definition_metadata: Option<&'a mut NodeNetworkPersistentMetadata>,
	deltas: NodeDeltas<'a>,
}

impl<'a> NodeMut<'a> {
	pub(super) fn new(
		node: &'a mut DocumentNode,
		metadata: &'a mut DocumentNodePersistentMetadata,
		definition_metadata: Option<&'a mut NodeNetworkPersistentMetadata>,
		deltas: &'a mut Vec<EditorDelta>,
		network: NetworkLocation<'a>,
		node_id: NodeId,
	) -> Self {
		Self {
			node,
			metadata,
			definition_metadata,
			deltas: NodeDeltas { deltas, network, node_id },
		}
	}
}

pub(crate) struct InterfaceMut<'a> {
	pub(crate) input_metadata: &'a mut Vec<InputMetadata>,
	pub(crate) output_names: &'a mut Vec<String>,
	pub(crate) output_descriptions: &'a mut Vec<String>,
}

impl<'m> InterfaceMut<'m> {
	fn of_network(metadata: &'m mut NodeNetworkPersistentMetadata) -> Self {
		Self {
			input_metadata: &mut metadata.import_metadata,
			output_names: &mut metadata.export_names,
			output_descriptions: &mut metadata.export_descriptions,
		}
	}

	fn of_node(metadata: &'m mut DocumentNodePersistentMetadata, definition_metadata: &'m mut Option<&mut NodeNetworkPersistentMetadata>) -> Self {
		if let Some(definition_metadata) = definition_metadata {
			return Self::of_network(definition_metadata);
		}
		if let Some(network_metadata) = &mut metadata.network_metadata {
			return Self::of_network(&mut network_metadata.persistent_metadata);
		}
		Self {
			input_metadata: &mut metadata.input_metadata,
			output_names: &mut metadata.output_names,
			output_descriptions: &mut metadata.output_descriptions,
		}
	}
}

/// Where a node's writes record what they changed, carrying the node's address so each setter names
/// what it wrote without its caller repeating it.
struct NodeDeltas<'a> {
	pub(super) deltas: &'a mut Vec<EditorDelta>,
	network: NetworkLocation<'a>,
	node_id: NodeId,
}

impl NodeDeltas<'_> {
	fn metadata(&mut self, change: NodeMetadataChange) {
		self.deltas.push(EditorDelta::NodeMetadata {
			network: self.network(),
			node_id: self.node_id,
			change,
		});
	}

	/// Records a change to the network the node implements with `implementation`.
	fn owned_network(&mut self, implementation: &DocumentNodeImplementation, change: NetworkMetadataChange) {
		self.deltas.push(EditorDelta::NetworkMetadata {
			network: self.network.implemented_by(self.node_id, implementation),
			change,
		});
	}

	fn graph(&mut self, delta: RuntimeDelta) {
		self.deltas.push(EditorDelta::Graph(delta));
	}

	fn input_metadata(&mut self, input_metadata: Vec<InputMetadata>) {
		self.deltas.push(EditorDelta::NodeInputMetadata {
			network: self.network(),
			node_id: self.node_id,
			input_metadata,
		});
	}

	fn snapshot(&mut self, metadata: Box<DocumentNodePersistentMetadata>) {
		self.deltas.push(EditorDelta::NodeMetadataSnapshot {
			network: self.network(),
			node_id: self.node_id,
			metadata,
		});
	}

	fn network(&self) -> OwnedNetworkLocation {
		self.network.to_owned_location()
	}

	fn node_id(&self) -> NodeId {
		self.node_id
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
				network: self.deltas.network(),
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
				network: self.deltas.network(),
				node_id: self.deltas.node_id(),
				call_argument: self.node.call_argument.clone(),
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

	pub(crate) fn interface(&mut self) -> InterfaceMut<'_> {
		InterfaceMut::of_node(self.metadata, &mut self.definition_metadata)
	}

	pub(crate) fn set_input_name(&mut self, input_index: usize, input_name: String) -> bool {
		let Some(input_metadata) = self.interface().input_metadata.get_mut(input_index) else {
			return false;
		};
		let changed = input_metadata.persistent_metadata.input_name != input_name;
		input_metadata.persistent_metadata.input_name = input_name;
		if changed {
			let change = NodeMetadataChange::InputName {
				index: input_index,
				name: input_metadata.persistent_metadata.input_name.clone(),
			};
			self.record_input_change(change);
		}
		changed
	}

	pub(crate) fn set_input_description(&mut self, input_index: usize, input_description: String) -> bool {
		let Some(input_metadata) = self.interface().input_metadata.get_mut(input_index) else {
			return false;
		};
		let changed = input_metadata.persistent_metadata.input_description != input_description;
		input_metadata.persistent_metadata.input_description = input_description;
		if changed {
			self.emit_input_metadata();
		}
		changed
	}

	/// The identifier of the widget override the Inspector uses for this input, or `None` for the
	/// widget generated from its type.
	pub(crate) fn set_widget_override(&mut self, input_index: usize, widget_override: Option<String>) -> bool {
		let Some(input_metadata) = self.interface().input_metadata.get_mut(input_index) else {
			return false;
		};
		let changed = input_metadata.persistent_metadata.widget_override != widget_override;
		input_metadata.persistent_metadata.widget_override = widget_override;
		if changed {
			let change = NodeMetadataChange::WidgetOverride {
				index: input_index,
				widget_override: input_metadata.persistent_metadata.widget_override.clone(),
			};
			self.record_input_change(change);
		}
		changed
	}

	pub(crate) fn set_output_name(&mut self, output_index: usize, output_name: String) -> bool {
		let output_names = self.interface().output_names;
		let Some(existing) = output_names.get_mut(output_index) else { return false };
		let changed = *existing != output_name;
		*existing = output_name;
		if changed {
			let output_names = output_names.clone();
			self.record_output_names(output_names);
		}
		changed
	}

	/// The encapsulating node's names for every one of the network's exports, replaced together since
	/// they are stored as one array.
	pub(crate) fn set_output_names(&mut self, output_names: Vec<String>) -> bool {
		let existing = self.interface().output_names;
		let changed = *existing != output_names;
		*existing = output_names;
		if changed {
			let output_names = existing.clone();
			self.record_output_names(output_names);
		}
		changed
	}

	pub(crate) fn set_output_description(&mut self, output_index: usize, output_description: String) -> bool {
		let interface = self.interface();
		if output_index >= interface.output_names.len() {
			return false;
		}

		let output_descriptions = interface.output_descriptions;
		if output_descriptions.get(output_index).map_or("", String::as_str) == output_description {
			return false;
		}

		if output_index >= output_descriptions.len() {
			output_descriptions.resize(output_index + 1, String::new());
		}
		output_descriptions[output_index] = output_description;

		let output_descriptions = output_descriptions.clone();
		self.record_output_descriptions(output_descriptions);
		true
	}

	/// The encapsulating node's descriptions for the network's exports, replaced together since they are stored as one array.
	pub(crate) fn set_output_descriptions(&mut self, output_descriptions: Vec<String>) -> bool {
		let existing = self.interface().output_descriptions;
		let changed = *existing != output_descriptions;
		*existing = output_descriptions;
		if changed {
			let output_descriptions = existing.clone();
			self.record_output_descriptions(output_descriptions);
		}
		changed
	}

	fn interface_on_network(&self) -> bool {
		self.definition_metadata.is_some() || self.metadata.network_metadata.is_some()
	}

	fn record_input_change(&mut self, change: NodeMetadataChange) {
		match self.interface_on_network() {
			true => self.emit_input_metadata(),
			false => self.deltas.metadata(change),
		}
	}

	fn record_output_names(&mut self, output_names: Vec<String>) {
		match self.interface_on_network() {
			true => self.deltas.owned_network(&self.node.implementation, NetworkMetadataChange::ExportNames(output_names)),
			false => self.deltas.metadata(NodeMetadataChange::OutputNames(output_names)),
		}
	}

	fn record_output_descriptions(&mut self, output_descriptions: Vec<String>) {
		match self.interface_on_network() {
			true => self.deltas.owned_network(&self.node.implementation, NetworkMetadataChange::ExportDescriptions(output_descriptions)),
			false => self.deltas.metadata(NodeMetadataChange::OutputDescriptions(output_descriptions)),
		}
	}

	/// The metadata array the node's inputs index, one entry per input.
	pub(crate) fn input_metadata(&self) -> &[InputMetadata] {
		if let Some(definition_metadata) = &self.definition_metadata {
			return &definition_metadata.import_metadata;
		}
		if let Some(network_metadata) = &self.metadata.network_metadata {
			return &network_metadata.persistent_metadata.import_metadata;
		}
		&self.metadata.input_metadata
	}

	/// Grows `output_names` to one entry per export, which an older document may be missing.
	pub(crate) fn resize_output_names(&mut self, number_of_exports: usize) {
		let output_names = self.interface().output_names;
		if output_names.len() == number_of_exports {
			return;
		}
		output_names.resize(number_of_exports, String::new());

		let output_names = output_names.clone();
		self.record_output_names(output_names);
	}

	/// Appends input metadata until there is one entry per input, filling from `defaults` where it has an
	/// entry for the added index. Restores the parallel-array invariant for an older document.
	/// Entries past the last input are kept, since they can hold user-authored names and overrides.
	pub(crate) fn pad_input_metadata(&mut self, number_of_inputs: usize, defaults: impl Fn(usize) -> Option<InputMetadata>) {
		let input_metadata = self.interface().input_metadata;
		if input_metadata.len() >= number_of_inputs {
			return;
		}
		for added_input_index in input_metadata.len()..number_of_inputs {
			input_metadata.push(defaults(added_input_index).unwrap_or_default());
		}
		self.emit_input_metadata();
	}

	/// Swaps in a new implementation, which never carries an inline network since the store moves one into the definitions table first.
	///
	/// A table entry keeps the interface it already has. Any other implementation takes over the node's previous interface.
	pub(crate) fn replace_implementation(&mut self, implementation: DocumentNodeImplementation) {
		let carried = (!matches!(implementation, DocumentNodeImplementation::Definition(_))).then(|| {
			let previous = self.interface();
			(previous.input_metadata.clone(), previous.output_names.clone(), previous.output_descriptions.clone())
		});

		self.node.implementation = implementation;
		self.metadata.network_metadata = None;
		self.definition_metadata = None;
		self.metadata.input_metadata.clear();
		self.metadata.output_names.clear();
		self.metadata.output_descriptions.clear();

		if let Some((input_metadata, output_names, output_descriptions)) = carried {
			let interface = self.interface();
			*interface.input_metadata = input_metadata;
			*interface.output_names = output_names;
			*interface.output_descriptions = output_descriptions;
		}

		let delta = RuntimeDelta::ReplaceNode {
			network: self.deltas.network(),
			node_id: self.deltas.node_id(),
			node: Box::new(self.node.clone()),
		};
		self.deltas.graph(delta);
	}

	/// Replaces the node's whole persistent metadata, including that of everything nested under it,
	/// which is the write a metadata snapshot describes.
	pub(crate) fn replace_metadata(&mut self, metadata: DocumentNodePersistentMetadata) {
		*self.metadata = metadata;

		let snapshot = Box::new(self.metadata.clone());
		self.deltas.snapshot(snapshot);
	}

	/// Replaces the metadata array the node's inputs index, leaving the inputs themselves alone.
	pub(crate) fn set_input_metadata(&mut self, input_metadata: Vec<InputMetadata>) {
		*self.interface().input_metadata = input_metadata;
		self.emit_input_metadata();
	}

	/// Swaps in new inputs together with their metadata, returning the inputs replaced.
	pub(crate) fn replace_inputs(&mut self, inputs: Vec<NodeInput>, input_metadata: Vec<InputMetadata>) -> Vec<NodeInput> {
		*self.interface().input_metadata = input_metadata;
		let previous = std::mem::replace(&mut self.node.inputs, inputs);

		self.emit_inputs();
		previous
	}

	/// Records the node's input list and the metadata array indexed by it, which change together
	/// whenever the number or order of slots does.
	pub(super) fn emit_inputs(&mut self) {
		self.emit_graph_inputs();
		self.emit_input_metadata();
	}

	/// Inserts an input at `index`, clamped to the end, leaving the metadata its inputs index alone.
	pub(crate) fn insert_input(&mut self, index: usize, input: NodeInput) {
		let index = index.min(self.node.inputs.len());
		self.node.inputs.insert(index, input);
		self.emit_graph_inputs();
	}

	/// Removes the input at `index`, leaving the metadata its inputs index alone.
	pub(crate) fn remove_input(&mut self, index: usize) -> Option<NodeInput> {
		if index >= self.node.inputs.len() {
			return None;
		}
		let input = self.node.inputs.remove(index);
		self.emit_graph_inputs();
		Some(input)
	}

	fn emit_graph_inputs(&mut self) {
		let delta = RuntimeDelta::SetInputs {
			network: self.deltas.network(),
			node_id: self.deltas.node_id(),
			inputs: self.node.inputs.clone(),
		};
		self.deltas.graph(delta);
	}

	fn emit_input_metadata(&mut self) {
		let input_metadata = self.input_metadata().to_vec();
		match self.interface_on_network() {
			true => self.deltas.owned_network(&self.node.implementation, NetworkMetadataChange::ImportMetadata(input_metadata)),
			false => self.deltas.input_metadata(input_metadata),
		}
	}
}

/// The persistent metadata of one network, resolved once so a run of writes walks the tree a single time.
///
/// Metadata only: the graph half of a network is its export slots, which are addressed through
/// [`NodeNetworkInterface::set_input_slot`] like any other input.
pub(crate) struct NetworkMut<'a> {
	pub(super) metadata: &'a mut NodeNetworkPersistentMetadata,
	pub(super) deltas: &'a mut Vec<EditorDelta>,
	pub(super) network: NetworkLocation<'a>,
}

impl<'a> NetworkMut<'a> {
	pub(super) fn new(metadata: &'a mut NodeNetworkPersistentMetadata, deltas: &'a mut Vec<EditorDelta>, network: NetworkLocation<'a>) -> Self {
		Self { metadata, deltas, network }
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

	pub(crate) fn insert_import_metadata(&mut self, index: usize, input_metadata: InputMetadata) {
		let index = index.min(self.metadata.import_metadata.len());
		self.metadata.import_metadata.insert(index, input_metadata);
		let change = NetworkMetadataChange::ImportMetadata(self.metadata.import_metadata.clone());
		self.emit(change);
	}

	pub(crate) fn remove_import_metadata(&mut self, index: usize) -> Option<InputMetadata> {
		if index >= self.metadata.import_metadata.len() {
			return None;
		}
		let removed = self.metadata.import_metadata.remove(index);
		let change = NetworkMetadataChange::ImportMetadata(self.metadata.import_metadata.clone());
		self.emit(change);
		Some(removed)
	}

	pub(crate) fn set_import_metadata(&mut self, import_metadata: Vec<InputMetadata>) -> bool {
		let changed = self.metadata.import_metadata != import_metadata;
		self.metadata.import_metadata = import_metadata;
		if changed {
			let change = NetworkMetadataChange::ImportMetadata(self.metadata.import_metadata.clone());
			self.emit(change);
		}
		changed
	}

	pub(crate) fn set_export_names(&mut self, export_names: Vec<String>) -> bool {
		let changed = self.metadata.export_names != export_names;
		self.metadata.export_names = export_names;
		if changed {
			let change = NetworkMetadataChange::ExportNames(self.metadata.export_names.clone());
			self.emit(change);
		}
		changed
	}

	pub(crate) fn set_export_descriptions(&mut self, export_descriptions: Vec<String>) -> bool {
		let changed = self.metadata.export_descriptions != export_descriptions;
		self.metadata.export_descriptions = export_descriptions;
		if changed {
			let change = NetworkMetadataChange::ExportDescriptions(self.metadata.export_descriptions.clone());
			self.emit(change);
		}
		changed
	}

	/// The display order of pinned nodes in the Inspector.
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
			network: self.network.to_owned_location(),
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
