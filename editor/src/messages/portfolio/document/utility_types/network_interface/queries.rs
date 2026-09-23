use super::*;
use std::hash::{Hash, Hasher};

/// Index of the Artboard definition's Clip input, which must match the input order authored in document_node_definitions.rs.
pub(crate) const ARTBOARD_CLIP_INPUT_INDEX: usize = 5;

impl NodeNetworkInterface {
	/// Runs a query against a resolved [`NetworkView`], logging any error at this message-boundary wrapper and mapping it to None.
	pub(crate) fn query<'a, 'p, T>(&'a self, network_path: &'p [NodeId], caller: &str, query: impl FnOnce(NetworkView<'a, 'p>) -> Result<T, NetworkError>) -> Option<T> {
		match self.view(network_path).and_then(query) {
			Ok(value) => Some(value),
			Err(error) => {
				log::error!("{error} in {caller}");
				None
			}
		}
	}
}

// Public immutable getters for the network interface
impl NodeNetworkInterface {
	/// Gets the network of the root document
	pub fn document_network(&self) -> &NodeNetwork {
		self.network.network()
	}

	/// The document network as it should be evaluated, which is the document itself with the previewed
	/// node substituted for the export.
	///
	/// Previewing is this peer's alone, so it is applied to the copy being compiled rather than written
	/// into the document. Every path that compiles the graph goes through here, so none of them can
	/// render the export while the user is previewing something else.
	pub fn network_to_evaluate(&self) -> NodeNetwork {
		let mut network = self.document_network().clone();

		// A preview is set on whichever network the user is looking at, which is nested whenever they are
		// working inside a node, so every network's preview is applied rather than only the document's.
		for (network_path, previewed) in self.previewed_nodes() {
			let Some(nested) = network.nested_network_mut(&network_path) else { continue };
			let Some(export) = nested.exports.first_mut() else { continue };
			*export = NodeInput::node(previewed.node_id, previewed.output_index);
		}

		network
	}

	/// The node each network is previewing, paired with that network's path, ordered by path so the
	/// result is stable across runs rather than following the metadata map's iteration order.
	pub fn previewed_nodes(&self) -> Vec<(Vec<NodeId>, RootNode)> {
		let mut previewed = Vec::new();
		let mut pending = vec![(Vec::new(), &*self.network_metadata)];

		while let Some((network_path, network_metadata)) = pending.pop() {
			if let Previewing::Yes { previewed: root_node } = network_metadata.persistent_metadata.previewing {
				previewed.push((network_path.clone(), root_node));
			}

			for (node_id, node_metadata) in &network_metadata.persistent_metadata.node_metadata {
				let Some(nested) = node_metadata.persistent_metadata.network_metadata.as_ref() else { continue };
				pending.push(([network_path.as_slice(), &[*node_id]].concat(), nested));
			}
		}

		previewed.sort_by(|(left, _), (right, _)| left.cmp(right));
		previewed
	}

	/// Every network whose preview was written by a version that rewired the export, paired with what
	/// that version would have restored.
	pub(crate) fn legacy_rewired_previews(&self) -> Vec<(Vec<NodeId>, Option<RootNode>)> {
		let mut legacy = Vec::new();
		let mut pending = vec![(Vec::new(), &*self.network_metadata)];

		while let Some((network_path, network_metadata)) = pending.pop() {
			if let Previewing::LegacyRewired { root_node_to_restore } = network_metadata.persistent_metadata.previewing {
				legacy.push((network_path.clone(), root_node_to_restore));
			}

			for (node_id, node_metadata) in &network_metadata.persistent_metadata.node_metadata {
				let Some(nested) = node_metadata.persistent_metadata.network_metadata.as_ref() else { continue };
				pending.push(([network_path.as_slice(), &[*node_id]].concat(), nested));
			}
		}

		legacy
	}
	/// Gets the nested network based on network_path
	pub fn nested_network(&self, network_path: &[NodeId]) -> Option<&NodeNetwork> {
		let Some(network) = self.document_network().nested_network(network_path) else {
			log::error!("Could not get nested network with path {network_path:?} in NodeNetworkInterface::network");
			return None;
		};
		Some(network)
	}

	/// Identifies the graph that would be evaluated, so a caller can skip recompiling when nothing the
	/// compiler sees has changed.
	///
	/// Previewing is mixed in because it redirects an export on the way to the compiler without touching
	/// the graph itself, so the graph's own hash does not move when a preview is toggled.
	pub fn network_hash(&self) -> u64 {
		let mut hasher = std::hash::DefaultHasher::new();
		self.network.current_hash().hash(&mut hasher);
		self.previewed_nodes().hash(&mut hasher);
		hasher.finish()
	}

	/// Get the specified document node in the nested network based on node_id and network_path
	pub fn document_node(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&DocumentNode> {
		self.query(network_path, "document_node", |view| view.node(node_id))
	}

	pub fn node_metadata(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&DocumentNodeMetadata> {
		self.query(network_path, "node_metadata", |view| view.node_metadata(node_id))
	}

	pub fn document_network_metadata(&self) -> &NodeNetworkMetadata {
		&self.network_metadata
	}

	/// The network metadata should always exist for the current network
	pub fn network_metadata(&self, network_path: &[NodeId]) -> Option<&NodeNetworkMetadata> {
		let Some(network_metadata) = self.network_metadata.nested_metadata(network_path) else {
			log::error!("Could not get nested network_metadata with path {network_path:?}");
			return None;
		};
		Some(network_metadata)
	}

	pub fn document_metadata(&self) -> &DocumentMetadata {
		&self.document_metadata
	}

	pub fn transaction_status(&self) -> TransactionStatus {
		self.transaction_status
	}

	pub fn selected_nodes(&self) -> SelectedNodes {
		self.selected_nodes_in_nested_network(&[]).unwrap_or_default()
	}

	/// Get the selected nodes for the network at the network_path
	pub fn selected_nodes_in_nested_network(&self, network_path: &[NodeId]) -> Option<SelectedNodes> {
		self.query(network_path, "selected_nodes_in_nested_network", |view| Ok(view.selected_nodes()))
	}

	/// Runs an encapsulating-node query, staying silent for the document network which has no encapsulating node.
	fn query_encapsulating<'a, 'p, T>(&'a self, network_path: &'p [NodeId], caller: &str, query: impl FnOnce(NetworkView<'a, 'p>) -> Result<T, NetworkError>) -> Option<T> {
		self.query(network_path, caller, |view| match query(view) {
			Err(NetworkError::NoEncapsulatingNode) => Ok(None),
			result => result.map(Some),
		})
		.flatten()
	}

	/// Get the network which the encapsulating node of the currently viewed network is part of. Will always be None in the document network.
	pub fn encapsulating_network_metadata(&self, network_path: &[NodeId]) -> Option<&NodeNetworkMetadata> {
		self.query_encapsulating(network_path, "encapsulating_network_metadata", |view| view.encapsulating().map(|parent| parent.network_metadata()))
	}

	/// Get the node which encapsulates the currently viewed network. Will always be None in the document network.
	pub fn encapsulating_node(&self, network_path: &[NodeId]) -> Option<&DocumentNode> {
		self.query_encapsulating(network_path, "encapsulating_node", |view| view.encapsulating_node())
	}

	/// Get the node metadata for the node which encapsulates the currently viewed network. Will always be None in the document network.
	pub fn encapsulating_node_metadata(&self, network_path: &[NodeId]) -> Option<&DocumentNodeMetadata> {
		self.query_encapsulating(network_path, "encapsulating_node_metadata", |view| view.encapsulating_node_metadata())
	}

	/// Returns the first downstream layer(inclusive) from a node. If the node is a layer, it will return itself.
	pub fn downstream_layer_for_chain_node(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<NodeId> {
		let mut id = *node_id;
		while !self.is_layer(&id, network_path) {
			id = self.with_outward_wires(network_path, |outward_wires| {
				outward_wires
					.get(&OutputConnector::primary_output(id))
					.and_then(|connections| connections.first())
					.and_then(|connector| connector.node_id())
			})??;
		}
		Some(id)
	}

	/// Returns all downstream layers (inclusive) from a node. If the node is a layer, it will return itself.
	pub fn downstream_layers(&self, node_id: &NodeId, network_path: &[NodeId]) -> Vec<NodeId> {
		let mut stack = vec![*node_id];
		let mut layers = Vec::new();
		while let Some(current_node) = stack.pop() {
			if self.is_layer(&current_node, network_path) {
				layers.push(current_node);
			} else {
				let downstream_found = self.with_outward_wires(network_path, |outward_wires| {
					let Some(connections) = outward_wires.get(&OutputConnector::primary_output(current_node)) else {
						return false;
					};
					stack.extend(connections.iter().filter_map(|input_connector| input_connector.node_id()));
					true
				});
				if downstream_found != Some(true) {
					log::error!("Could not get outward wires in downstream_layer");
					return Vec::new();
				}
			}
		}
		layers
	}

	pub fn chain_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> u32 {
		self.query(network_path, "chain_width", |view| Ok(view.chain_width(node_id))).unwrap_or_default()
	}

	/// Check if the specified node id is connected to the output
	pub fn connected_to_output(&self, target_node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "connected_to_output", |view| Ok(view.connected_to_output(target_node_id))).unwrap_or_default()
	}

	pub fn number_of_imports(&self, network_path: &[NodeId]) -> usize {
		self.query(network_path, "number_of_imports", |view| Ok(view.number_of_imports())).unwrap_or_default()
	}

	pub fn number_of_exports(&self, network_path: &[NodeId]) -> usize {
		self.query(network_path, "number_of_exports", |view| Ok(view.number_of_exports())).unwrap_or_default()
	}

	pub(crate) fn number_of_displayed_inputs(&self, node_id: &NodeId, network_path: &[NodeId]) -> usize {
		self.query(network_path, "number_of_displayed_inputs", |view| view.number_of_displayed_inputs(node_id))
			.unwrap_or_default()
	}

	pub fn number_of_inputs(&self, node_id: &NodeId, network_path: &[NodeId]) -> usize {
		self.query(network_path, "number_of_inputs", |view| view.number_of_inputs(node_id)).unwrap_or_default()
	}

	/// Whether the node has an exposed input at index 0 to accept the horizontal flow from upstream.
	/// A node without one (e.g. a generator) can only be the most-upstream node in a chain.
	pub fn has_primary_input(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "has_primary_input", |view| view.has_primary_input(node_id)).unwrap_or_default()
	}

	/// Grid rows the node body spans, which is the greater of its input rows and its output count.
	pub fn displayed_row_count(&self, node_id: &NodeId, network_path: &[NodeId]) -> usize {
		self.query(network_path, "displayed_row_count", |view| view.displayed_row_count(node_id)).unwrap_or(1)
	}

	pub fn number_of_outputs(&self, node_id: &NodeId, network_path: &[NodeId]) -> usize {
		self.query(network_path, "number_of_outputs", |view| view.number_of_outputs(node_id)).unwrap_or_default()
	}

	/// Creates a copy for each node by disconnecting nodes which are not connected to other copied nodes.
	/// Returns an iterator of all persistent metadata for a node and their ids
	pub fn copy_nodes<'a>(&'a self, new_ids: &'a HashMap<NodeId, NodeId>, network_path: &'a [NodeId]) -> impl Iterator<Item = (NodeId, NodeTemplate)> + 'a {
		let mut new_nodes = new_ids
			.iter()
			.filter_map(|(node_id, &new)| {
				self.create_node_template(node_id, network_path).and_then(|mut node_template| {
					// TODO: Get downstream connections from all outputs
					let Some(has_selected_node_downstream) = self.with_outward_wires(network_path, |outward_wires| {
						outward_wires.get(&OutputConnector::primary_output(*node_id)).is_some_and(|outputs| {
							outputs
								.iter()
								.any(|input_connector| input_connector.node_id().is_some_and(|upstream_id| new_ids.keys().any(|key| *key == upstream_id)))
						})
					}) else {
						log::error!("Could not get outward wires in copy_nodes");
						return None;
					};
					// If the copied node does not have a downstream connection to another copied node, then set the position to absolute
					if !has_selected_node_downstream {
						let Some(position) = self.position(node_id, network_path) else {
							log::error!("Could not get position in create_node_template");
							return None;
						};
						match &mut node_template.node_type_metadata {
							NodeTypePersistentMetadata::Layer(layer_metadata) => layer_metadata.position = LayerPosition::Absolute(position),
							NodeTypePersistentMetadata::Node(node_metadata) => node_metadata.position = NodePosition::Absolute(position),
						};
					}

					// If a chain node does not have a selected downstream layer, then set the position to absolute
					let downstream_layer = self.downstream_layer_for_chain_node(node_id, network_path);
					if downstream_layer.is_none_or(|downstream_layer| new_ids.keys().all(|key| *key != downstream_layer)) {
						let Some(position) = self.position(node_id, network_path) else {
							log::error!("Could not get position in create_node_template");
							return None;
						};
						node_template.node_type_metadata = NodeTypePersistentMetadata::Node(NodePersistentMetadata {
							position: NodePosition::Absolute(position),
						});
					}

					// Shift all absolute nodes 2 to the right and 2 down
					// TODO: Remove 2x2 offset and replace with layout system to find space for new node
					match &mut node_template.node_type_metadata {
						NodeTypePersistentMetadata::Layer(layer_metadata) => {
							if let LayerPosition::Absolute(position) = &mut layer_metadata.position {
								*position += IVec2::new(2, 2)
							}
						}
						NodeTypePersistentMetadata::Node(node_metadata) => {
							if let NodePosition::Absolute(position) = &mut node_metadata.position {
								*position += IVec2::new(2, 2)
							}
						}
					}

					Some((new, *node_id, node_template))
				})
			})
			.collect::<Vec<_>>();

		for old_id in new_nodes.iter().map(|(_, old_id, _)| *old_id).collect::<Vec<_>>() {
			// Try set all selected nodes upstream of a layer to be chain nodes
			if self.is_layer(&old_id, network_path) {
				for valid_upstream_chain_node in self.valid_upstream_chain_nodes(&InputConnector::layer_secondary_input(old_id), network_path) {
					if let Some(node_template) = new_nodes.iter_mut().find_map(|(_, old_id, template)| (*old_id == valid_upstream_chain_node).then_some(template)) {
						match &mut node_template.node_type_metadata {
							NodeTypePersistentMetadata::Node(node_metadata) => node_metadata.position = NodePosition::Chain,
							NodeTypePersistentMetadata::Layer(_) => log::error!("Node cannot be a layer"),
						};
					}
				}
			}
		}
		new_nodes.into_iter().map(move |(new, node_id, node)| (new, self.map_ids(node, &node_id, new_ids, network_path)))
	}

	/// Create a node template from an existing node.
	pub fn create_node_template(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<NodeTemplate> {
		self.query(network_path, "create_node_template", |view| view.create_node_template(node_id))
	}

	/// Converts all node id inputs to a new id based on a HashMap.
	///
	/// If the node is not in the hashmap then a default input is found based on the compiled network, using the node_id passed as a parameter
	pub fn map_ids(&self, mut node_template: NodeTemplate, node_id: &NodeId, new_ids: &HashMap<NodeId, NodeId>, network_path: &[NodeId]) -> NodeTemplate {
		for (input_index, input) in node_template.inputs.iter_mut().enumerate() {
			if let &mut NodeInput::Node { node_id: id, output_index } = input {
				if let Some(&new_id) = new_ids.get(&id) {
					*input = NodeInput::Node { node_id: new_id, output_index };
				} else {
					// Disconnect node input if it is not connected to another node in new_ids
					let tagged_value = self.tagged_value_from_input(&InputConnector::node_at_index(*node_id, input_index), network_path);
					*input = NodeInput::value(tagged_value, true);
				}
			} else if let &mut NodeInput::Import { .. } = input {
				// Always disconnect network node input
				let tagged_value = self.tagged_value_from_input(&InputConnector::node_at_index(*node_id, input_index), network_path);
				*input = NodeInput::value(tagged_value, true);
			}
		}
		node_template
	}

	pub fn input_from_connector(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<&NodeInput> {
		match self.view(network_path).and_then(|view| view.input(input_connector)) {
			Ok(input) => Some(input),
			// An out-of-range input index is an expected lookup miss, not an error worth logging
			Err(NetworkError::InputNotFound { .. }) => None,
			Err(error) => {
				log::error!("{error} in input_from_connector");
				None
			}
		}
	}

	pub fn collect_used_resources(&self, target: &mut HashSet<ResourceId>) {
		visit_network_resources(self.document_network(), &mut |id| {
			target.insert(id);
		});
	}

	pub fn collect_resources_use_counts(&self) -> HashMap<ResourceId, usize> {
		let mut counts = HashMap::new();
		visit_network_resources(self.document_network(), &mut |id| *counts.entry(id).or_insert(0) += 1);
		counts
	}

	/// Returns whether every downstream path from the node's outputs stays within the dependent set defined by `classify`, meaning nothing else in the graph depends on this node.
	/// Reaching an export or a dead end (a walked node with no outward wires) always escapes. O(nodes + wires) per call.
	pub(crate) fn is_sole_dependent(&self, node_id: NodeId, network_path: &[NodeId], classify: impl Fn(NodeId, usize) -> SoleDependentStep) -> bool {
		let mut visited = HashSet::new();
		let mut stack = vec![node_id];

		while let Some(current_node) = stack.pop() {
			if !visited.insert(current_node) {
				continue;
			}

			// Classify every downstream connection of this node, collecting the ones to keep walking through
			let number_of_outputs = self.number_of_outputs(&current_node, network_path);
			let keeps_within_set = self.with_outward_wires(network_path, |outward_wires| {
				let mut has_downstream_connections = false;
				let mut nodes_to_walk_through = Vec::new();
				for output_index in 0..number_of_outputs {
					let Some(downstream_connections) = outward_wires.get(&OutputConnector::node(current_node, output_index)) else {
						continue;
					};
					for downstream_connection in downstream_connections {
						has_downstream_connections = true;
						let InputConnector::Node {
							node_id: downstream_node,
							input_index,
						} = downstream_connection
						else {
							return false;
						};
						match classify(*downstream_node, *input_index) {
							SoleDependentStep::Terminate => {}
							SoleDependentStep::Continue => nodes_to_walk_through.push(*downstream_node),
							SoleDependentStep::Escape => return false,
						}
					}
				}

				if !has_downstream_connections {
					return false;
				}
				stack.extend(nodes_to_walk_through);
				true
			});

			match keeps_within_set {
				Some(true) => {}
				Some(false) => return false,
				None => {
					log::error!("Could not get outward wires in is_sole_dependent");
					return false;
				}
			}
		}

		true
	}

	/// The input connector into which a layer should be inserted for the given parent and stack index.
	pub(crate) fn post_node_with_index(&self, parent: LayerNodeIdentifier, insert_index: usize, network_path: &[NodeId]) -> InputConnector {
		let mut post_node_input_connector = if parent == LayerNodeIdentifier::ROOT_PARENT {
			InputConnector::Export(0)
		} else {
			InputConnector::layer_secondary_input(parent.to_node())
		};
		// Skip layers based on skip_layer_nodes, which inserts the new layer at a certain index of the layer stack.
		let mut current_index = 0;

		// Set the post node to the layer node at insert_index
		loop {
			if current_index == insert_index {
				break;
			}
			let next_node_in_stack_id = self
				.input_from_connector(&post_node_input_connector, network_path)
				.and_then(|input_from_connector| if let NodeInput::Node { node_id, .. } = input_from_connector { Some(node_id) } else { None });

			if let Some(next_node_in_stack_id) = next_node_in_stack_id {
				// Only increment index for layer nodes
				if self.is_layer(next_node_in_stack_id, network_path) {
					current_index += 1;
				}
				// Input as a sibling to the Layer node above
				post_node_input_connector = InputConnector::primary_input(*next_node_in_stack_id);
			} else {
				log::error!("Error getting post node: insert_index out of bounds");
				break;
			};
		}

		let layer_input_connector = post_node_input_connector;

		// Sink post_node down to the end of the non layer chain that feeds into post_node, such that pre_node is the layer node at insert_index + 1, or None if insert_index is the last layer
		loop {
			let pre_node_output_connector = self.upstream_output_connector(&post_node_input_connector, network_path);

			match pre_node_output_connector {
				Some(OutputConnector::Node { node_id: pre_node_id, .. }) if !self.is_layer(&pre_node_id, network_path) => {
					// Update post_node_input_connector for the next iteration
					post_node_input_connector = InputConnector::primary_input(pre_node_id);
					// Insert directly under layer if moving to the end of a layer stack that ends with a non layer node that does not have an exposed primary input
					let primary_is_exposed = self.input_from_connector(&post_node_input_connector, network_path).is_some_and(|input| input.is_exposed());
					if !primary_is_exposed {
						return layer_input_connector;
					}
				}
				_ => break, // Break if pre_node_output_connector is None or if pre_node_id is a layer
			}
		}

		post_node_input_connector
	}

	// All chain nodes and branches from the chain which are sole dependents of the layer
	pub fn upstream_nodes_below_layer(&self, node_id: &NodeId, network_path: &[NodeId]) -> HashSet<NodeId> {
		// Every upstream node below layer must be a sole dependent
		let mut upstream_nodes_below_layer = HashSet::new();

		let mut potential_upstream_nodes = HashSet::new();
		for chain_node in self
			.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow)
			.skip(1)
			.take_while(|node_id| self.is_chain(node_id, network_path))
			.collect::<Vec<_>>()
		{
			upstream_nodes_below_layer.insert(chain_node);
			let Some(chain_node) = self.document_node(&chain_node, network_path) else {
				log::error!("Could not get node {node_id} in upstream_nodes_below_layer");
				continue;
			};
			potential_upstream_nodes.extend(chain_node.inputs.iter().filter(|input| input.is_exposed()).skip(1).filter_map(|node_input| node_input.as_node()))
		}

		// Get the node feeding into the left input of the chain
		let mut current_node_id = *node_id;
		loop {
			let Some(current_node) = self.document_node(&current_node_id, network_path) else {
				log::error!("Could not get node {node_id} in upstream_nodes_below_layer");
				break;
			};
			if let Some(primary_node_id) = current_node
				.inputs
				.iter()
				.filter(|input| input.is_exposed())
				.nth(if self.is_layer(&current_node_id, network_path) { 1 } else { 0 })
				.and_then(|left_input| left_input.as_node())
			{
				if self.is_chain(&primary_node_id, network_path) {
					current_node_id = primary_node_id;
				} else {
					potential_upstream_nodes.insert(primary_node_id);
					break;
				}
			} else {
				break;
			}
		}

		for potential_upstream_node in potential_upstream_nodes {
			// The upstream chain cannot be added if there is some node upstream from an input that is not a sole dependent
			let mut upstream_chain_can_be_added = true;
			// Collect a vec of nodes that are sole dependents while iterating
			let mut sole_dependents = HashSet::new();

			for upstream_node_from_input in self
				.upstream_flow_back_from_nodes(vec![potential_upstream_node], network_path, FlowType::UpstreamFlow)
				.collect::<Vec<_>>()
			{
				// A path terminates at an already-verified sole dependent or at the left input to the chain
				let is_sole_dependent = self.is_sole_dependent(upstream_node_from_input, network_path, |downstream_node, input_index| {
					if sole_dependents.contains(&downstream_node) || downstream_node == *node_id && input_index == 1 {
						SoleDependentStep::Terminate
					} else {
						SoleDependentStep::Continue
					}
				});

				if is_sole_dependent {
					sole_dependents.insert(upstream_node_from_input);
				} else {
					upstream_chain_can_be_added = false;
					break;
				}
			}

			if upstream_chain_can_be_added {
				upstream_nodes_below_layer.extend(sole_dependents)
			}
		}
		upstream_nodes_below_layer
	}

	pub fn previewing(&self, network_path: &[NodeId]) -> Previewing {
		self.query(network_path, "previewing", |view| Ok(view.previewing())).unwrap_or(Previewing::No)
	}

	/// Returns the root node (the node that the solid line is connect to), or None if no nodes are connected to the output
	pub fn root_node(&self, network_path: &[NodeId]) -> Option<RootNode> {
		self.query(network_path, "root_node", |view| Ok(view.root_node())).flatten()
	}

	pub fn reference(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<DefinitionIdentifier> {
		self.query(network_path, "reference", |view| view.reference(node_id)).flatten()
	}

	pub fn implementation(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&DocumentNodeImplementation> {
		self.query(network_path, "implementation", |view| view.implementation(node_id))
	}

	pub fn input_data(&self, node_id: &NodeId, index: usize, key: &str, network_path: &[NodeId]) -> Option<&Value> {
		match self.view(network_path).and_then(|view| view.input_data(node_id, index, key)) {
			Ok(value) => value,
			// A missing input slot is the caller probing optional data, but any other failure is a corrupt network worth logging
			Err(NetworkError::InputNotFound { .. }) => None,
			Err(error) => {
				log::error!("{error} in input_data");
				None
			}
		}
	}

	pub fn persistent_input_metadata(&self, node_id: &NodeId, index: usize, network_path: &[NodeId]) -> Option<&InputPersistentMetadata> {
		self.view(network_path).ok().and_then(|view| view.persistent_input_metadata(node_id, index).ok())
	}

	pub fn set_input_override(&mut self, node_id: &NodeId, index: usize, widget_override: Option<String>, network_path: &[NodeId]) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in set_input_override");
			return;
		};
		node.set_widget_override(index, widget_override);
	}

	/// Returns the display name of the node. If the display name is empty, it will return "Untitled Node" or "Untitled Layer" depending on the node type.
	pub fn display_name(&self, node_id: &NodeId, network_path: &[NodeId]) -> String {
		self.query(network_path, "display_name", |view| Ok(view.display_name(node_id)))
			.unwrap_or_else(|| "Custom Node".to_string())
	}

	/// The uneditable name in the Properties panel which represents the function name of the node implementation.
	pub fn implementation_name(&self, node_id: &NodeId, network_path: &[NodeId]) -> String {
		self.query(network_path, "implementation_name", |view| Ok(view.implementation_name(node_id)))
			.unwrap_or_else(|| "Custom Node".to_string())
	}

	pub fn is_locked(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_locked", |view| view.is_locked(node_id)).unwrap_or_default()
	}

	pub fn is_pinned(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_pinned", |view| view.is_pinned(node_id)).unwrap_or_default()
	}

	/// The given network's pinned nodes in display order: pinning appends, dragging rearranges, and any not yet recorded go last.
	pub fn ordered_pinned_nodes(&self, network_path: &[NodeId]) -> Vec<NodeId> {
		self.query(network_path, "ordered_pinned_nodes", |view| Ok(view.ordered_pinned_nodes())).unwrap_or_default()
	}

	pub fn is_visible(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_visible", |view| view.is_visible(node_id)).unwrap_or_default()
	}

	/// Whether a layer in the document network is visible, which also requires every ancestor to be visible.
	pub fn is_layer_visible(&self, layer: LayerNodeIdentifier) -> bool {
		layer
			.ancestors(self.document_metadata())
			.all(|ancestor| ancestor == LayerNodeIdentifier::ROOT_PARENT || self.is_visible(&ancestor.to_node(), &[]))
	}

	pub fn is_layer(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_layer", |view| view.is_layer(node_id)).unwrap_or_default()
	}

	pub fn primary_output_connected_to_layer(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(downstream_connectors) = self.with_outward_wires(network_path, |outward_wires| outward_wires.get(&OutputConnector::primary_output(*node_id)).cloned()) else {
			log::error!("Could not get outward_wires in primary_output_connected_to_layer");
			return false;
		};
		let Some(downstream_connectors) = downstream_connectors else {
			log::error!("Could not get downstream_connectors in primary_output_connected_to_layer");
			return false;
		};

		let downstream_nodes = downstream_connectors
			.iter()
			.filter_map(|connector| connector.node_id().filter(|_| connector.input_index() == 0))
			.collect::<Vec<_>>();
		downstream_nodes.iter().any(|node_id| self.is_layer(node_id, network_path))
	}

	pub fn primary_input_connected_to_layer(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "primary_input_connected_to_layer", |view| Ok(view.primary_input_connected_to_layer(node_id)))
			.unwrap_or_default()
	}

	pub fn hidden_primary_export(&self, network_path: &[NodeId]) -> bool {
		self.query(network_path, "hidden_primary_export", |view| Ok(view.hidden_primary_export())).unwrap_or_default()
	}

	pub fn hidden_primary_output(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "hidden_primary_output", |view| view.hidden_primary_output(node_id)).unwrap_or_default()
	}

	pub fn hidden_primary_import(&self, network_path: &[NodeId]) -> bool {
		self.query(network_path, "hidden_primary_import", |view| Ok(view.hidden_primary_import())).unwrap_or_default()
	}

	pub fn is_absolute(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_absolute", |view| view.is_absolute(node_id)).unwrap_or_default()
	}

	pub fn is_chain(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_chain", |view| view.is_chain(node_id)).unwrap_or_default()
	}

	pub fn is_stack(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_stack", |view| view.is_stack(node_id)).unwrap_or_default()
	}

	/// Whether the node is an Artboard node by identity, regardless of whether it currently participates in the scene.
	/// Callers that care about scene membership should source their layers from the document structure or check connectivity separately.
	pub fn is_artboard(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_artboard", |view| Ok(view.is_artboard(node_id))).unwrap_or_default()
	}

	/// Whether the node is a Merge node by identity, meaning it is the generic layer wrapper rather than a specialized node displayed as a layer.
	pub fn is_merge(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_merge", |view| Ok(view.is_merge(node_id))).unwrap_or_default()
	}

	/// All artboard layers that participate in the scene, excluding disconnected Artboard nodes.
	pub fn all_artboards(&self) -> HashSet<LayerNodeIdentifier> {
		// O(n * (nodes + wires)) since connected_to_output performs a graph walk per artboard candidate
		self.document_network_metadata()
			.persistent_metadata
			.node_metadata
			.iter()
			.filter_map(|(node_id, node_metadata)| {
				if node_metadata.persistent_metadata.network_metadata.as_ref().is_some_and(|network_metadata| {
					network_metadata
						.persistent_metadata
						.reference
						.as_ref()
						.is_some_and(|reference| reference == "Artboard" && self.connected_to_output(node_id, &[]) && self.is_layer(node_id, &[]))
				}) {
					Some(LayerNodeIdentifier::new(*node_id, self))
				} else {
					None
				}
			})
			.collect()
	}

	/// Folders sorted from most nested to least nested
	pub fn folders_sorted_by_most_nested(&self, network_path: &[NodeId]) -> Vec<LayerNodeIdentifier> {
		if !network_path.is_empty() {
			log::error!("Currently can only get deepest common ancestor in the document network");
			return Vec::new();
		}
		let Some(selected_nodes) = self.selected_nodes_in_nested_network(network_path) else {
			log::error!("Could not get selected nodes in deepest_common_ancestor");
			return Vec::new();
		};
		let mut folders: Vec<_> = selected_nodes
			.selected_layers(self.document_metadata())
			.filter(|layer| layer.has_children(self.document_metadata()))
			.collect();
		folders.sort_by_cached_key(|a| std::cmp::Reverse(a.ancestors(self.document_metadata()).count()));
		folders
	}

	/// Calculates the document bounds in document space
	pub fn document_bounds_document_space(&self, include_artboards: bool) -> Option<[DVec2; 2]> {
		self.combined_document_bounds(include_artboards, |metadata, layer| metadata.bounding_box_document(layer))
	}

	fn combined_document_bounds(&self, include_artboards: bool, layer_bounds: impl Fn(&DocumentMetadata, LayerNodeIdentifier) -> Option<[DVec2; 2]>) -> Option<[DVec2; 2]> {
		self.document_metadata
			.all_layers()
			.filter(|layer| include_artboards || !self.is_artboard(&layer.to_node(), &[]))
			.filter_map(|layer| {
				// A layer clipped by its artboard contributes only the intersection of the two bounds
				if !self.is_artboard(&layer.to_node(), &[])
					&& let Some(artboard_node_identifier) = layer
						.ancestors(self.document_metadata())
						.find(|ancestor| *ancestor != LayerNodeIdentifier::ROOT_PARENT && self.is_artboard(&ancestor.to_node(), &[]))
					&& let Some(artboard) = self.document_node(&artboard_node_identifier.to_node(), &[])
					&& let Some(clip_input) = artboard.inputs.get(ARTBOARD_CLIP_INPUT_INDEX)
					&& let NodeInput::Value { tagged_value, .. } = clip_input
					&& tagged_value.clone().deref() == &TaggedValue::Bool(true)
				{
					return Some(Quad::clip(
						layer_bounds(&self.document_metadata, layer).unwrap_or_default(),
						self.document_metadata.bounding_box_document(artboard_node_identifier).unwrap_or_default(),
					));
				}
				layer_bounds(&self.document_metadata, layer)
			})
			// Skip any layer bounds containing NaN to avoid poisoning the combined result
			.filter(|[min, max]| min.is_finite() && max.is_finite())
			.reduce(Quad::combine_bounds)
	}

	pub fn document_bounds_viewport_space(&self, include_artboards: bool) -> Option<[DVec2; 2]> {
		let [min, max] = self.document_bounds_document_space(include_artboards)?;
		let quad = Quad::from_box([min, max]);
		let transformed = self.document_metadata.document_to_viewport * quad;
		Some(transformed.bounding_box())
	}

	/// Calculates the document bounds in document space, expanding vector layer bounds to include the rendered
	/// stroke width. Used for export so the output canvas captures strokes that overflow the path geometry.
	pub fn document_bounds_document_space_with_stroke(&self, include_artboards: bool) -> Option<[DVec2; 2]> {
		self.combined_document_bounds(include_artboards, |metadata, layer| metadata.bounding_box_document_with_stroke(layer))
	}

	/// Calculates the selected layer bounds in document space
	pub fn selected_bounds_document_space(&self, include_artboards: bool, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let Some(selected_nodes) = self.selected_nodes_in_nested_network(network_path) else {
			log::error!("Could not get selected nodes in shallowest_unique_layers");
			return None;
		};
		selected_nodes
			.selected_layers(&self.document_metadata)
			.filter(|&layer| include_artboards || !self.is_artboard(&layer.to_node(), &[]))
			.filter_map(|layer| self.document_metadata.bounding_box_document(layer))
			.reduce(Quad::combine_bounds)
	}

	/// Calculates the selected layer bounds in document space, expanding vector layer bounds to include the
	/// rendered stroke width. Used for export so the output canvas captures strokes that overflow the path geometry.
	pub fn selected_bounds_document_space_with_stroke(&self, include_artboards: bool, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let Some(selected_nodes) = self.selected_nodes_in_nested_network(network_path) else {
			log::error!("Could not get selected nodes in selected_bounds_document_space_with_stroke");
			return None;
		};
		selected_nodes
			.selected_layers(&self.document_metadata)
			.filter(|&layer| include_artboards || !self.is_artboard(&layer.to_node(), &[]))
			.filter_map(|layer| self.document_metadata.bounding_box_document_with_stroke(layer))
			.reduce(Quad::combine_bounds)
	}

	/// Layers excluding ones that are children of other layers in the list.
	// TODO: Cache this
	pub fn shallowest_unique_layers(&self, network_path: &[NodeId]) -> impl Iterator<Item = LayerNodeIdentifier> + use<> {
		let mut sorted_layers = if let Some(selected_nodes) = self.selected_nodes_in_nested_network(network_path) {
			selected_nodes
				.selected_layers(self.document_metadata())
				.map(|layer| {
					let mut layer_path = layer.ancestors(&self.document_metadata).collect::<Vec<_>>();
					layer_path.reverse();
					layer_path
				})
				.collect::<Vec<_>>()
		} else {
			log::error!("Could not get selected nodes in shallowest_unique_layers");
			Vec::new()
		};

		// Sorting here creates groups of similar UUID paths
		sorted_layers.sort();
		sorted_layers.dedup_by(|a, b| a.starts_with(b));
		sorted_layers.into_iter().map(|mut path| {
			let layer = path.pop().expect("Path should not be empty");
			assert!(
				layer != LayerNodeIdentifier::ROOT_PARENT,
				"The root parent cannot be selected, so it cannot be a shallowest selected layer"
			);
			layer
		})
	}

	pub fn shallowest_unique_layers_sorted(&self, network_path: &[NodeId]) -> Vec<LayerNodeIdentifier> {
		let all_layers_to_group = self.shallowest_unique_layers(network_path).collect::<Vec<_>>();
		// Ensure nodes are grouped in the correct order
		let mut all_layers_to_group_sorted = Vec::new();
		for descendant in LayerNodeIdentifier::ROOT_PARENT.descendants(self.document_metadata()) {
			if all_layers_to_group.contains(&descendant) {
				all_layers_to_group_sorted.push(descendant);
			};
		}
		all_layers_to_group_sorted
	}

	/// Ancestor that is shared by all layers and that is deepest (more nested). Default may be the root. Skips selected non-folder, non-artboard layers
	pub fn deepest_common_ancestor(&self, selected_nodes: &SelectedNodes, network_path: &[NodeId], include_self: bool) -> Option<LayerNodeIdentifier> {
		if !network_path.is_empty() {
			log::error!("Currently can only get deepest common ancestor in the document network");
			return None;
		}
		selected_nodes
			.selected_layers(&self.document_metadata)
			.map(|layer| {
				let mut layer_path = layer.ancestors(&self.document_metadata).collect::<Vec<_>>();
				layer_path.reverse();
				if !include_self || !self.is_artboard(&layer.to_node(), network_path) {
					layer_path.pop();
				}

				layer_path
			})
			.reduce(|mut a, b| {
				a.truncate(a.iter().zip(b.iter()).position(|(&a, &b)| a != b).unwrap_or_else(|| a.len().min(b.len())));
				a
			})
			.and_then(|layer| layer.last().copied())
	}

	/// Gives an iterator to all nodes connected to the given nodes by all inputs (primary or primary + secondary depending on `only_follow_primary` choice), traversing backwards upstream starting from the given node's inputs.
	pub fn upstream_flow_back_from_nodes<'a>(&'a self, node_ids: Vec<NodeId>, network_path: &'a [NodeId], flow_type: FlowType) -> impl Iterator<Item = NodeId> + 'a {
		match self.view(network_path) {
			Ok(view) => view.upstream_flow(node_ids, flow_type),
			Err(error) => {
				log::error!("{error} in upstream_flow_back_from_nodes");
				FlowIter {
					stack: Vec::new(),
					network: self.document_network(),
					network_metadata: &self.network_metadata,
					flow_type: FlowType::UpstreamFlow,
				}
			}
		}
	}

	pub fn upstream_output_connector(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<OutputConnector> {
		let input = self.input_from_connector(input_connector, network_path);
		input.and_then(|input| match input {
			NodeInput::Node { node_id, output_index, .. } => Some(OutputConnector::node(*node_id, *output_index)),
			NodeInput::Import { import_index, .. } => Some(OutputConnector::Import(*import_index)),
			_ => None,
		})
	}

	/// In the network `X -> Y -> Z`, `is_node_upstream_of_another_by_primary_flow(Z, X)` returns true.
	pub fn is_node_upstream_of_another_by_horizontal_flow(&self, node: NodeId, network_path: &[NodeId], potentially_upstream_node: NodeId) -> bool {
		self.upstream_flow_back_from_nodes(vec![node], network_path, FlowType::HorizontalFlow)
			.any(|id| id == potentially_upstream_node)
	}

	pub fn from_old_network(old_network: OldNodeNetwork) -> Self {
		let mut node_network = NodeNetwork::default();
		let mut network_metadata = NodeNetworkMetadata::default();
		let mut stack = vec![(Vec::new(), old_network)];
		while let Some((network_path, old_network)) = stack.pop() {
			let Some(nested_network) = node_network.nested_network_mut(&network_path) else {
				log::error!("Could not get nested network in from_old_network");
				continue;
			};
			nested_network.exports = old_network.exports;
			nested_network.scope_injections = old_network.scope_injections.into_iter().collect();
			let Some(nested_network_metadata) = network_metadata.nested_metadata_mut(&network_path) else {
				log::error!("Could not get nested network in from_old_network");
				continue;
			};
			nested_network_metadata.persistent_metadata.previewing = Previewing::No;
			for (node_id, old_node) in old_network.nodes {
				let mut node = DocumentNode::default();
				let mut node_metadata = DocumentNodeMetadata::default();

				node.inputs = old_node.inputs;
				node.call_argument = old_node.manual_composition.unwrap_or_default();
				node.visible = old_node.visible;
				node.skip_deduplication = old_node.skip_deduplication;
				node.original_location = old_node.original_location;
				node_metadata.persistent_metadata.display_name = old_node.alias;
				node_metadata.persistent_metadata.locked = old_node.locked;
				node_metadata.persistent_metadata.node_type_metadata = if old_node.is_layer {
					NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
						position: LayerPosition::Absolute(old_node.metadata.position),
					})
				} else {
					NodeTypePersistentMetadata::Node(NodePersistentMetadata {
						position: NodePosition::Absolute(old_node.metadata.position),
					})
				};

				match old_node.implementation {
					OldDocumentNodeImplementation::ProtoNode(protonode) => {
						node.implementation = DocumentNodeImplementation::ProtoNode(protonode);
					}
					OldDocumentNodeImplementation::Network(old_network) => {
						node.implementation = DocumentNodeImplementation::Network(NodeNetwork::default());
						node_metadata.persistent_metadata.network_metadata = Some(NodeNetworkMetadata::default());
						let mut nested_path = network_path.clone();
						nested_path.push(node_id);
						stack.push((nested_path, old_network));
					}
					OldDocumentNodeImplementation::Extract => {
						node.implementation = DocumentNodeImplementation::Extract;
					}
				}

				nested_network.nodes.insert(node_id, node);
				nested_network_metadata.persistent_metadata.node_metadata.insert(node_id, node_metadata);
			}
		}
		Self::from_trees(node_network, network_metadata)
	}
}
