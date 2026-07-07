use super::*;

/// A failure to resolve part of the network or its metadata, returned by [`NetworkView`] accessors.
#[derive(Debug, Clone, PartialEq)]
pub enum NetworkError {
	NetworkNotFound { path: Vec<NodeId> },
	NodeNotFound { node_id: NodeId },
	NodeMetadataNotFound { node_id: NodeId },
	NestedMetadataNotFound { node_id: NodeId },
	InputNotFound { connector: InputConnector },
	NoEncapsulatingNode,
}

impl std::fmt::Display for NetworkError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			NetworkError::NetworkNotFound { path } => write!(f, "Network not found at path {path:?}"),
			NetworkError::NodeNotFound { node_id } => write!(f, "Node {node_id} not found"),
			NetworkError::NodeMetadataNotFound { node_id } => write!(f, "Node metadata for {node_id} not found"),
			NetworkError::NestedMetadataNotFound { node_id } => write!(f, "Nested network metadata for node {node_id} not found"),
			NetworkError::InputNotFound { connector } => write!(f, "Input {connector:?} not found"),
			NetworkError::NoEncapsulatingNode => write!(f, "The document network has no encapsulating node"),
		}
	}
}

/// A read cursor over one network nesting level, resolving the network and its parallel metadata once instead of per query.
/// Obtained from [`NodeNetworkInterface::view`]; accessors return typed errors that callers at the message boundary may log.
#[derive(Clone, Copy)]
pub struct NetworkView<'a, 'p> {
	pub(crate) interface: &'a NodeNetworkInterface,
	pub(crate) path: &'p [NodeId],
	pub(crate) network: &'a NodeNetwork,
	pub(crate) metadata: &'a NodeNetworkMetadata,
}

impl NodeNetworkInterface {
	/// Resolves a read cursor for the network at `network_path`, walking the nested network and metadata trees once.
	pub fn view<'a, 'p>(&'a self, network_path: &'p [NodeId]) -> Result<NetworkView<'a, 'p>, NetworkError> {
		let network = self
			.document_network()
			.nested_network(network_path)
			.ok_or_else(|| NetworkError::NetworkNotFound { path: network_path.to_vec() })?;
		let metadata = self
			.document_network_metadata()
			.nested_metadata(network_path)
			.ok_or_else(|| NetworkError::NetworkNotFound { path: network_path.to_vec() })?;

		Ok(NetworkView {
			interface: self,
			path: network_path,
			network,
			metadata,
		})
	}
}

impl<'a, 'p> NetworkView<'a, 'p> {
	pub fn network(&self) -> &'a NodeNetwork {
		self.network
	}

	pub fn network_metadata(&self) -> &'a NodeNetworkMetadata {
		self.metadata
	}

	pub fn node(&self, node_id: &NodeId) -> Result<&'a DocumentNode, NetworkError> {
		self.network.nodes.get(node_id).ok_or(NetworkError::NodeNotFound { node_id: *node_id })
	}

	pub fn node_metadata(&self, node_id: &NodeId) -> Result<&'a DocumentNodeMetadata, NetworkError> {
		self.metadata
			.persistent_metadata
			.node_metadata
			.get(node_id)
			.ok_or(NetworkError::NodeMetadataNotFound { node_id: *node_id })
	}

	pub fn input(&self, input_connector: &InputConnector) -> Result<&'a NodeInput, NetworkError> {
		let input = match input_connector {
			InputConnector::Node { node_id, input_index } => self.node(node_id)?.inputs.get(*input_index),
			InputConnector::Export(export_index) => self.network.exports.get(*export_index),
		};
		input.ok_or(NetworkError::InputNotFound { connector: *input_connector })
	}

	/// The view over the network which contains this network's encapsulating node, or an error in the document network.
	pub fn encapsulating(&self) -> Result<NetworkView<'a, 'p>, NetworkError> {
		let (_, encapsulating_path) = self.path.split_last().ok_or(NetworkError::NoEncapsulatingNode)?;
		self.interface.view(encapsulating_path)
	}

	/// The node which encapsulates this network, or an error in the document network.
	pub fn encapsulating_node(&self) -> Result<&'a DocumentNode, NetworkError> {
		let (node_id, _) = self.path.split_last().ok_or(NetworkError::NoEncapsulatingNode)?;
		self.encapsulating()?.node(node_id)
	}

	/// The metadata of the node which encapsulates this network, or an error in the document network.
	pub fn encapsulating_node_metadata(&self) -> Result<&'a DocumentNodeMetadata, NetworkError> {
		let (node_id, _) = self.path.split_last().ok_or(NetworkError::NoEncapsulatingNode)?;
		self.encapsulating()?.node_metadata(node_id)
	}

	pub fn implementation(&self, node_id: &NodeId) -> Result<&'a DocumentNodeImplementation, NetworkError> {
		Ok(&self.node(node_id)?.implementation)
	}

	pub fn reference(&self, node_id: &NodeId) -> Result<Option<DefinitionIdentifier>, NetworkError> {
		match self.implementation(node_id)? {
			DocumentNodeImplementation::Network(_) => {
				let node_metadata = self.node_metadata(node_id)?;
				let network_metadata = node_metadata
					.persistent_metadata
					.network_metadata
					.as_ref()
					.ok_or(NetworkError::NestedMetadataNotFound { node_id: *node_id })?;
				Ok(network_metadata.persistent_metadata.reference.clone().map(DefinitionIdentifier::Network))
			}
			DocumentNodeImplementation::ProtoNode(protonode_id) => Ok(Some(DefinitionIdentifier::ProtoNode(protonode_id.clone()))),
			_ => Ok(None),
		}
	}

	pub fn is_layer(&self, node_id: &NodeId) -> Result<bool, NetworkError> {
		Ok(self.node_metadata(node_id)?.persistent_metadata.is_layer())
	}

	pub fn is_locked(&self, node_id: &NodeId) -> Result<bool, NetworkError> {
		Ok(self.node_metadata(node_id)?.persistent_metadata.locked)
	}

	pub fn is_pinned(&self, node_id: &NodeId) -> Result<bool, NetworkError> {
		Ok(self.node_metadata(node_id)?.persistent_metadata.pinned)
	}

	pub fn is_visible(&self, node_id: &NodeId) -> Result<bool, NetworkError> {
		Ok(self.node(node_id)?.visible)
	}

	pub fn is_absolute(&self, node_id: &NodeId) -> Result<bool, NetworkError> {
		Ok(match &self.node_metadata(node_id)?.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => matches!(layer_metadata.position, LayerPosition::Absolute(_)),
			NodeTypePersistentMetadata::Node(node_metadata) => matches!(node_metadata.position, NodePosition::Absolute(_)),
		})
	}

	pub fn is_chain(&self, node_id: &NodeId) -> Result<bool, NetworkError> {
		Ok(match &self.node_metadata(node_id)?.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Node(node_metadata) => matches!(node_metadata.position, NodePosition::Chain),
			_ => false,
		})
	}

	pub fn is_stack(&self, node_id: &NodeId) -> Result<bool, NetworkError> {
		Ok(match &self.node_metadata(node_id)?.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => matches!(layer_metadata.position, LayerPosition::Stack(_)),
			_ => false,
		})
	}

	pub fn number_of_inputs(&self, node_id: &NodeId) -> Result<usize, NetworkError> {
		Ok(self.node(node_id)?.inputs.len())
	}

	pub fn number_of_displayed_inputs(&self, node_id: &NodeId) -> Result<usize, NetworkError> {
		Ok(self.node(node_id)?.inputs.iter().filter(|input| input.is_exposed()).count())
	}

	pub fn number_of_outputs(&self, node_id: &NodeId) -> Result<usize, NetworkError> {
		Ok(match self.implementation(node_id)? {
			DocumentNodeImplementation::Network(nested_network) => nested_network.exports.len(),
			_ => 1,
		})
	}

	/// The number of imports as defined by the encapsulating node's input count, or zero for the document network.
	pub fn number_of_imports(&self) -> usize {
		self.encapsulating_node().map_or(0, |node| node.inputs.len())
	}

	pub fn number_of_exports(&self) -> usize {
		self.network.exports.len()
	}

	pub fn has_primary_input(&self, node_id: &NodeId) -> Result<bool, NetworkError> {
		Ok(self.input(&InputConnector::node(*node_id, 0)).is_ok_and(|input| input.is_exposed()))
	}

	pub fn hidden_primary_output(&self, node_id: &NodeId) -> Result<bool, NetworkError> {
		Ok(match self.implementation(node_id)? {
			DocumentNodeImplementation::Network(network) => network.exports.first().is_none_or(|input| !input.is_exposed()),
			_ => false,
		})
	}

	pub fn hidden_primary_export(&self) -> bool {
		let Some((node_id, _)) = self.path.split_last() else { return false };
		self.encapsulating().and_then(|parent| parent.hidden_primary_output(node_id)).unwrap_or(false)
	}

	pub fn hidden_primary_import(&self) -> bool {
		self.encapsulating_node().is_ok_and(|node| node.inputs.first().is_some_and(|input| !input.is_exposed()))
	}

	pub fn previewing(&self) -> Previewing {
		self.metadata.persistent_metadata.previewing
	}

	/// The root node (the node that the solid line is connected to), or None if no nodes are connected to the output.
	pub fn root_node(&self) -> Option<RootNode> {
		match &self.metadata.persistent_metadata.previewing {
			Previewing::Yes { root_node_to_restore } => *root_node_to_restore,
			Previewing::No => self.network.exports.first().and_then(|export| {
				if let NodeInput::Node { node_id, output_index, .. } = export {
					Some(RootNode {
						node_id: *node_id,
						output_index: *output_index,
					})
				} else {
					None
				}
			}),
		}
	}

	pub fn input_data(&self, node_id: &NodeId, index: usize, key: &str) -> Result<Option<&'a Value>, NetworkError> {
		Ok(self.persistent_input_metadata(node_id, index)?.input_data.get(key))
	}

	pub fn persistent_input_metadata(&self, node_id: &NodeId, index: usize) -> Result<&'a InputPersistentMetadata, NetworkError> {
		let metadata = self.node_metadata(node_id)?;
		let input_metadata = metadata.persistent_metadata.input_metadata.get(index).ok_or(NetworkError::InputNotFound {
			connector: InputConnector::node(*node_id, index),
		})?;
		Ok(&input_metadata.persistent_metadata)
	}

	pub(crate) fn transient_input_metadata(&self, node_id: &NodeId, index: usize) -> Result<&'a InputTransientMetadata, NetworkError> {
		let metadata = self.node_metadata(node_id)?;
		let input_metadata = metadata.persistent_metadata.input_metadata.get(index).ok_or(NetworkError::InputNotFound {
			connector: InputConnector::node(*node_id, index),
		})?;
		Ok(&input_metadata.transient_metadata)
	}

	pub fn upstream_output_connector(&self, input_connector: &InputConnector) -> Result<Option<OutputConnector>, NetworkError> {
		Ok(match self.input(input_connector)? {
			NodeInput::Node { node_id, output_index, .. } => Some(OutputConnector::node(*node_id, *output_index)),
			NodeInput::Import { import_index, .. } => Some(OutputConnector::Import(*import_index)),
			_ => None,
		})
	}

	/// Whether the node reaches the exports by following wires downstream.
	// O(nodes + wires) reachability walk
	pub fn connected_to_output(&self, target_node_id: &NodeId) -> bool {
		if self
			.network
			.exports
			.iter()
			.any(|export| if let NodeInput::Node { node_id, .. } = export { node_id == target_node_id } else { false })
		{
			return true;
		}

		let mut stack = self
			.network
			.exports
			.iter()
			.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { self.network.nodes.get(node_id) } else { None })
			.collect::<Vec<_>>();
		let mut already_visited = HashSet::new();
		already_visited.extend(
			self.network
				.exports
				.iter()
				.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { Some(node_id) } else { None }),
		);

		while let Some(node) = stack.pop() {
			for input in &node.inputs {
				if let &NodeInput::Node { node_id: ref_id, .. } = input {
					if already_visited.contains(&ref_id) {
						continue;
					}
					if ref_id == *target_node_id {
						return true;
					}
					let Some(ref_node) = self.network.nodes.get(&ref_id) else { continue };
					already_visited.insert(ref_id);
					stack.push(ref_node);
				}
			}
		}

		false
	}

	/// An iterator of all nodes connected to the given nodes by the flow type, traversing backwards upstream from the given nodes' inputs.
	pub(crate) fn upstream_flow(&self, mut node_ids: Vec<NodeId>, mut flow_type: FlowType) -> FlowIter<'a> {
		if matches!(flow_type, FlowType::LayerChildrenUpstreamFlow) {
			node_ids = node_ids
				.iter()
				.filter_map(|node_id| {
					if self.is_layer(node_id).unwrap_or_default() {
						self.network.nodes.get(node_id).and_then(|node| node.inputs.get(1)).and_then(|input| input.as_node())
					} else {
						Some(*node_id)
					}
				})
				.collect::<Vec<_>>();
			flow_type = FlowType::UpstreamFlow;
		};
		FlowIter {
			stack: node_ids,
			network: self.network,
			network_metadata: self.metadata,
			flow_type,
		}
	}

	/// The distance in grid cells from the layer to the end of its chain, or zero for a layer with no chain.
	pub fn chain_width(&self, node_id: &NodeId) -> u32 {
		if self.number_of_displayed_inputs(node_id).unwrap_or_default() > 1 {
			let mut last_chain_node_distance = 0_u32;
			for (index, node_id) in self.upstream_flow(vec![*node_id], FlowType::HorizontalPrimaryOutputFlow).skip(1).enumerate() {
				if self.is_chain(&node_id).unwrap_or_default() {
					last_chain_node_distance = (index as u32) + 1;
				} else {
					return last_chain_node_distance * NODE_CHAIN_WIDTH as u32 + 1;
				}
			}

			last_chain_node_distance * NODE_CHAIN_WIDTH as u32 + 1
		} else {
			// A layer with no secondary input has no chain
			0
		}
	}

	/// The given network's pinned nodes in display order: pinning appends, dragging rearranges, and any not yet recorded go last.
	pub fn ordered_pinned_nodes(&self) -> Vec<NodeId> {
		let order = &self.metadata.persistent_metadata.pinned_node_order;
		let is_pinned = |node_id: &NodeId| self.is_pinned(node_id).unwrap_or_default();

		// Follow the saved order, keeping only nodes that still exist and are still pinned
		let mut pinned_nodes = order
			.iter()
			.copied()
			.filter(|node_id| self.network.nodes.contains_key(node_id) && is_pinned(node_id))
			.collect::<Vec<_>>();

		// Append any pinned nodes missing from the saved order at the end
		let mut unordered = self.network.nodes.keys().copied().filter(|node_id| is_pinned(node_id) && !order.contains(node_id)).collect::<Vec<_>>();
		unordered.sort();
		pinned_nodes.extend(unordered);

		pinned_nodes
	}

	/// Create a node template from an existing node.
	pub fn create_node_template(&self, node_id: &NodeId) -> Result<NodeTemplate, NetworkError> {
		let node = self.node(node_id)?;
		let node_metadata = self.node_metadata(node_id)?;

		Ok(NodeTemplate {
			persistent_node_metadata: node_metadata.persistent_metadata.clone(),
			document_node: node.clone(),
		})
	}
}
