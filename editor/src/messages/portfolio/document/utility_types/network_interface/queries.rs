use super::*;

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
	pub fn document_network_mut(&mut self) -> &mut NodeNetwork {
		self.network.network_mut()
	}

	/// Gets the nested network based on network_path
	pub fn nested_network(&self, network_path: &[NodeId]) -> Option<&NodeNetwork> {
		let Some(network) = self.document_network().nested_network(network_path) else {
			log::error!("Could not get nested network with path {network_path:?} in NodeNetworkInterface::network");
			return None;
		};
		Some(network)
	}

	pub fn network_hash(&self) -> u64 {
		self.network.current_hash()
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
		match self.view(network_path).and_then(query) {
			Ok(value) => Some(value),
			Err(NetworkError::NoEncapsulatingNode) => None,
			Err(error) => {
				log::error!("{error} in {caller}");
				None
			}
		}
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
	pub fn downstream_layer_for_chain_node(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<NodeId> {
		let mut id = *node_id;
		while !self.is_layer(&id, network_path) {
			id = self.outward_wires(network_path)?.get(&OutputConnector::node(id, 0))?.first()?.node_id()?;
		}
		Some(id)
	}

	/// Returns all downstream layers (inclusive) from a node. If the node is a layer, it will return itself.
	pub fn downstream_layers(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Vec<NodeId> {
		let mut stack = vec![*node_id];
		let mut layers = Vec::new();
		while let Some(current_node) = stack.pop() {
			if self.is_layer(&current_node, network_path) {
				layers.push(current_node);
			} else {
				let Some(outward_wires) = self.outward_wires(network_path).and_then(|outward_wires| outward_wires.get(&OutputConnector::node(current_node, 0))) else {
					log::error!("Could not get outward wires in downstream_layer");
					return Vec::new();
				};
				stack.extend(outward_wires.iter().filter_map(|input_connector| input_connector.node_id()));
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
		self.view(network_path).map(|view| view.number_of_imports()).unwrap_or_default()
	}

	pub fn number_of_exports(&self, network_path: &[NodeId]) -> usize {
		self.view(network_path).map(|view| view.number_of_exports()).unwrap_or_default()
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
		self.view(network_path).and_then(|view| view.has_primary_input(node_id)).unwrap_or_default()
	}

	pub fn number_of_outputs(&self, node_id: &NodeId, network_path: &[NodeId]) -> usize {
		self.query(network_path, "number_of_outputs", |view| view.number_of_outputs(node_id)).unwrap_or_default()
	}

	/// Creates a copy for each node by disconnecting nodes which are not connected to other copied nodes.
	/// Returns an iterator of all persistent metadata for a node and their ids
	pub fn copy_nodes<'a>(&'a mut self, new_ids: &'a HashMap<NodeId, NodeId>, network_path: &'a [NodeId]) -> impl Iterator<Item = (NodeId, NodeTemplate)> + 'a {
		let mut new_nodes = new_ids
			.iter()
			.filter_map(|(node_id, &new)| {
				self.create_node_template(node_id, network_path).and_then(|mut node_template| {
					let Some(outward_wires) = self.outward_wires(network_path) else {
						log::error!("Could not get outward wires in copy_nodes");
						return None;
					};
					// TODO: Get downstream connections from all outputs
					let mut downstream_connections = outward_wires.get(&OutputConnector::node(*node_id, 0)).map_or([].iter(), |outputs| outputs.iter());
					let has_selected_node_downstream = downstream_connections.any(|input_connector| input_connector.node_id().is_some_and(|upstream_id| new_ids.keys().any(|key| *key == upstream_id)));
					// If the copied node does not have a downstream connection to another copied node, then set the position to absolute
					if !has_selected_node_downstream {
						let Some(position) = self.position(node_id, network_path) else {
							log::error!("Could not get position in create_node_template");
							return None;
						};
						match &mut node_template.persistent_node_metadata.node_type_metadata {
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
						node_template.persistent_node_metadata.node_type_metadata = NodeTypePersistentMetadata::Node(NodePersistentMetadata {
							position: NodePosition::Absolute(position),
						});
					}

					// Shift all absolute nodes 2 to the right and 2 down
					// TODO: Remove 2x2 offset and replace with layout system to find space for new node
					match &mut node_template.persistent_node_metadata.node_type_metadata {
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
				for valid_upstream_chain_node in self.valid_upstream_chain_nodes(&InputConnector::node(old_id, 1), network_path) {
					if let Some(node_template) = new_nodes.iter_mut().find_map(|(_, old_id, template)| (*old_id == valid_upstream_chain_node).then_some(template)) {
						match &mut node_template.persistent_node_metadata.node_type_metadata {
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
	pub fn map_ids(&mut self, mut node_template: NodeTemplate, node_id: &NodeId, new_ids: &HashMap<NodeId, NodeId>, network_path: &[NodeId]) -> NodeTemplate {
		for (input_index, input) in node_template.document_node.inputs.iter_mut().enumerate() {
			if let &mut NodeInput::Node { node_id: id, output_index } = input {
				if let Some(&new_id) = new_ids.get(&id) {
					*input = NodeInput::Node { node_id: new_id, output_index };
				} else {
					// Disconnect node input if it is not connected to another node in new_ids
					let tagged_value = self.tagged_value_from_input(&InputConnector::node(*node_id, input_index), network_path);
					*input = NodeInput::value(tagged_value, true);
				}
			} else if let &mut NodeInput::Import { .. } = input {
				// Always disconnect network node input
				let tagged_value = self.tagged_value_from_input(&InputConnector::node(*node_id, input_index), network_path);
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

	pub fn position(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<IVec2> {
		let top_left_position = self
			.node_click_targets(node_id, network_path)
			.and_then(|click_targets| click_targets.node_click_target.bounding_box())
			.map(|mut bounding_box| {
				if !self.is_layer(node_id, network_path) {
					bounding_box[0] -= DVec2::new(0., 12.);
				}
				(bounding_box[0] / 24.).as_ivec2()
			});
		top_left_position.map(|position| {
			if self.is_layer(node_id, network_path) {
				position + IVec2::new(self.chain_width(node_id, network_path) as i32, 0)
			} else {
				position
			}
		})
	}

	pub fn collect_used_resources(&self, target: &mut HashSet<ResourceId>) {
		collect_network_resources(self.document_network(), target);
	}

	pub fn frontend_imports(&mut self, network_path: &[NodeId]) -> Vec<Option<FrontendGraphOutput>> {
		match network_path.split_last() {
			Some((node_id, encapsulating_network_path)) => {
				let Some(node) = self.document_node(node_id, encapsulating_network_path) else {
					log::error!("Could not get node {node_id} in network {encapsulating_network_path:?}");
					return Vec::new();
				};
				let mut frontend_imports = (0..node.inputs.len())
					.map(|import_index| self.frontend_output_from_connector(&OutputConnector::Import(import_index), network_path))
					.collect::<Vec<_>>();
				if frontend_imports.is_empty() {
					frontend_imports.push(None);
				}
				frontend_imports
			}
			// In the document network display no imports
			None => Vec::new(),
		}
	}

	pub fn frontend_exports(&mut self, network_path: &[NodeId]) -> Vec<Option<FrontendGraphInput>> {
		let Some(network) = self.nested_network(network_path) else { return Vec::new() };
		let mut frontend_exports = ((0..network.exports.len()).map(|export_index| self.frontend_input_from_connector(&InputConnector::Export(export_index), network_path))).collect::<Vec<_>>();
		if frontend_exports.is_empty() {
			frontend_exports.push(None);
		}
		frontend_exports
	}

	pub fn import_export_position(&mut self, network_path: &[NodeId]) -> Option<(IVec2, IVec2)> {
		let Some(all_nodes_bounding_box) = self.all_nodes_bounding_box(network_path).cloned() else {
			log::error!("Could not get all nodes bounding box in load_export_ports");
			return None;
		};
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return None;
		};

		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in load_export_ports");
			return None;
		};
		let node_graph_to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
		let target_viewport_top_left = DVec2::new(IMPORTS_TO_LEFT_EDGE_PIXEL_GAP as f64, IMPORTS_TO_TOP_EDGE_PIXEL_GAP as f64);

		let node_graph_pixel_offset_top_left = node_graph_to_viewport.inverse().transform_point2(target_viewport_top_left);

		// A 5x5 grid offset from the top left corner
		let node_graph_grid_space_offset_top_left = node_graph_to_viewport.inverse().transform_point2(DVec2::ZERO) + DVec2::new(5. * GRID_SIZE as f64, 4. * GRID_SIZE as f64);

		// The inner bound of the import is the highest/furthest left of the two offsets
		let top_left_inner_bound = DVec2::new(
			node_graph_pixel_offset_top_left.x.min(node_graph_grid_space_offset_top_left.x),
			node_graph_pixel_offset_top_left.y.min(node_graph_grid_space_offset_top_left.y),
		);

		let offset_from_top_left = if network
			.exports
			.first()
			.is_some_and(|export| export.as_node().is_some_and(|export_node| self.is_layer(&export_node, network_path)))
		{
			DVec2::new(-4. * GRID_SIZE as f64, -2. * GRID_SIZE as f64)
		} else {
			DVec2::new(-4. * GRID_SIZE as f64, 0.)
		};

		let bounding_box_top_left = DVec2::new((all_nodes_bounding_box[0].x / 24. + 0.5).floor() * 24., (all_nodes_bounding_box[0].y / 24. + 0.5).floor() * 24.) + offset_from_top_left;
		let import_top_left = DVec2::new(top_left_inner_bound.x.min(bounding_box_top_left.x), top_left_inner_bound.y.min(bounding_box_top_left.y));
		let rounded_import_top_left = DVec2::new((import_top_left.x / 24.).round() * 24., (import_top_left.y / 24.).round() * 24.);

		let viewport_width = network_metadata.persistent_metadata.navigation_metadata.node_graph_width;

		let target_viewport_top_right = DVec2::new(viewport_width - EXPORTS_TO_RIGHT_EDGE_PIXEL_GAP as f64, EXPORTS_TO_TOP_EDGE_PIXEL_GAP as f64);

		// An offset from the right edge in viewport pixels
		let node_graph_pixel_offset_top_right = node_graph_to_viewport.inverse().transform_point2(target_viewport_top_right);

		// A 5x5 grid offset from the right corner
		let node_graph_grid_space_offset_top_right = node_graph_to_viewport.inverse().transform_point2(DVec2::new(viewport_width, 0.)) + DVec2::new(-5. * GRID_SIZE as f64, 4. * GRID_SIZE as f64);

		// The inner bound of the export is the highest/furthest right of the two offsets.
		// When zoomed out this keeps it a constant grid space away from the edge, but when zoomed in it prevents the exports from getting too far in
		let top_right_inner_bound = DVec2::new(
			node_graph_pixel_offset_top_right.x.max(node_graph_grid_space_offset_top_right.x),
			node_graph_pixel_offset_top_right.y.min(node_graph_grid_space_offset_top_right.y),
		);

		let offset_from_top_right = if network
			.exports
			.first()
			.is_some_and(|export| export.as_node().is_some_and(|export_node| self.is_layer(&export_node, network_path)))
		{
			DVec2::new(2. * GRID_SIZE as f64, -2. * GRID_SIZE as f64)
		} else {
			DVec2::new(4. * GRID_SIZE as f64, 0.)
		};

		let mut bounding_box_top_right = DVec2::new((all_nodes_bounding_box[1].x / 24. + 0.5).floor() * 24., (all_nodes_bounding_box[0].y / 24. + 0.5).floor() * 24.);
		bounding_box_top_right += offset_from_top_right;
		let export_top_right = DVec2::new(top_right_inner_bound.x.max(bounding_box_top_right.x), top_right_inner_bound.y.min(bounding_box_top_right.y));
		let rounded_export_top_right = DVec2::new((export_top_right.x / 24.).round() * 24., (export_top_right.y / 24.).round() * 24.);

		Some((rounded_import_top_left.as_ivec2(), rounded_export_top_right.as_ivec2()))
	}

	/// Returns None if there is an error, it is a hidden primary export, or a hidden input
	pub fn frontend_input_from_connector(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<FrontendGraphInput> {
		// Return None if it is a hidden input
		if self.input_from_connector(input_connector, network_path).is_some_and(|input| !input.is_exposed()) {
			return None;
		}
		let input_type = self.input_type(input_connector, network_path);
		let data_type = input_type.displayed_type();
		let resolved_type = input_type.resolved_type_node_string();

		let connected_to = self
			.upstream_output_connector(input_connector, network_path)
			.map(|output_connector| match output_connector {
				OutputConnector::Node { node_id, output_index } => {
					let name = self.display_name(&node_id, network_path);
					format!("Connected to output #{output_index} of \"{name}\", ID: {node_id}.")
				}
				OutputConnector::Import(import_index) => format!("Connected to import #{import_index}."),
			})
			.unwrap_or("Connected to nothing.".to_string());

		let (name, description) = match input_connector {
			InputConnector::Node { node_id, input_index } => self.displayed_input_name_and_description(node_id, *input_index, network_path),
			InputConnector::Export(export_index) => {
				// Get export name from parent node metadata input, which must match the number of exports.
				// Empty string means to use type, or "Export + index" if type is empty determined
				let export_name = if network_path.is_empty() {
					"Canvas".to_string()
				} else {
					self.encapsulating_node_metadata(network_path)
						.and_then(|encapsulating_metadata| encapsulating_metadata.persistent_metadata.output_names.get(*export_index).cloned())
						.unwrap_or_default()
				};

				let export_name = if !export_name.is_empty() {
					export_name
				} else if let Some(export_type_name) = input_type.compiled_nested_type().map(ToString::to_string) {
					export_type_name
				} else {
					format!("Export #{}", export_index)
				};

				(export_name, String::new())
			}
		};

		let valid_types = self.potential_valid_input_types(input_connector, network_path).iter().map(ToString::to_string).collect::<Vec<_>>();
		let valid_types = {
			// Dedupe while preserving order
			let mut found = HashSet::new();
			valid_types.into_iter().filter(|s| found.insert(s.clone())).collect::<Vec<_>>()
		};

		Some(FrontendGraphInput {
			data_type,
			resolved_type,
			name,
			description,
			valid_types,
			connected_to,
		})
	}

	/// Returns None if there is an error, it is the document network, a hidden primary output or import
	pub fn frontend_output_from_connector(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Option<FrontendGraphOutput> {
		let output_type = self.output_type(output_connector, network_path);
		let (name, description) = match output_connector {
			OutputConnector::Node { node_id, output_index } => {
				// Do not display the primary output port for a node if it is a network node with a hidden primary export
				if *output_index == 0 && self.hidden_primary_output(node_id, network_path) {
					return None;
				};
				// Get the output name from the interior network export name
				let node_metadata = self.node_metadata(node_id, network_path)?;
				let output_name = node_metadata.persistent_metadata.output_names.get(*output_index).cloned().unwrap_or_default();

				let output_name = if !output_name.is_empty() { output_name } else { output_type.resolved_type_node_string() };
				(output_name, String::new())
			}
			OutputConnector::Import(import_index) => {
				// Get the import name from the encapsulating node input metadata
				let Some((encapsulating_node_id, encapsulating_path)) = network_path.split_last() else {
					// Return None if it is an import in the document network
					return None;
				};
				// Return None if the primary input is hidden and this is the primary import
				if *import_index == 0 && self.hidden_primary_import(network_path) {
					return None;
				};
				let (import_name, description) = self.displayed_input_name_and_description(encapsulating_node_id, *import_index, encapsulating_path);

				let import_name = if !import_name.is_empty() {
					import_name
				} else if let Some(import_type_name) = output_type.compiled_nested_type().map(ToString::to_string) {
					import_type_name
				} else {
					format!("Import #{}", import_index)
				};

				(import_name, description)
			}
		};
		let data_type = output_type.displayed_type();
		let resolved_type = output_type.resolved_type_node_string();
		let mut connected_to = self
			.outward_wires(network_path)
			.and_then(|outward_wires| outward_wires.get(output_connector))
			.cloned()
			.unwrap_or_default()
			.iter()
			.map(|input| match input {
				&InputConnector::Node { node_id, input_index } => {
					let name = self.display_name(&node_id, network_path);
					format!("Connected to input #{input_index} of \"{name}\", ID: {node_id}.")
				}
				InputConnector::Export(export_index) => format!("Connected to export #{export_index}."),
			})
			.collect::<Vec<_>>();

		if connected_to.is_empty() {
			connected_to.push("Connected to nothing.".to_string());
		}

		Some(FrontendGraphOutput {
			data_type,
			resolved_type,
			name,
			description,
			connected_to,
		})
	}

	pub fn height_from_click_target(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<u32> {
		let mut node_height: Option<u32> = self
			.node_click_targets(node_id, network_path)
			.and_then(|click_targets: &DocumentNodeClickTargets| click_targets.node_click_target.bounding_box())
			.map(|bounding_box| ((bounding_box[1].y - bounding_box[0].y) / 24.) as u32);
		if !self.is_layer(node_id, network_path) {
			node_height = node_height.map(|height| height + 1);
		}
		node_height
	}

	// All chain nodes and branches from the chain which are sole dependents of the layer
	pub fn upstream_nodes_below_layer(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> HashSet<NodeId> {
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
				let number_of_outputs = self.number_of_outputs(&upstream_node_from_input, network_path);

				// A node is a sole dependent if all outputs are sole dependents, and there are no dead ends
				let mut all_outputs_are_sole_dependents = true;
				let mut dead_ends = 0;

				for output_index in 0..number_of_outputs {
					let downstream_connections = {
						let Some(outward_wires) = self.outward_wires(network_path) else {
							log::error!("Could not get outward wires in upstream_nodes_below_layer");
							continue;
						};
						outward_wires.get(&OutputConnector::node(upstream_node_from_input, output_index)).cloned()
					};
					let Some(downstream_connections) = downstream_connections else {
						log::error!("Could not get outward wires in upstream_nodes_below_layer");
						continue;
					};
					let mut current_output_is_sole_dependent = true;
					let mut stack = downstream_connections;
					while let Some(current_downstream_connection) = stack.pop() {
						// Iterate downstream. If a sole dependent or chain_node_id is reached, then stop the iteration. If the exports is eventually reached, then it is not a sole dependent
						match &current_downstream_connection {
							InputConnector::Node {
								node_id: downstream_node_id,
								input_index,
							} => {
								// Stop iterating once the downstream node is the left input to the chain or a sole dependent
								if !(sole_dependents.contains(downstream_node_id) || downstream_node_id == node_id && *input_index == 1) {
									// Continue iterating downstream for the downstream node
									let number_of_outputs = self.number_of_outputs(downstream_node_id, network_path);
									let Some(outward_wires) = self.outward_wires(network_path) else {
										log::error!("Could not get outward wires in upstream_nodes_below_layer");
										continue;
									};
									let mut has_downstream_connections = false;
									for output_index in 0..number_of_outputs {
										let Some(downstream_connections) = outward_wires.get(&OutputConnector::node(*downstream_node_id, output_index)) else {
											log::error!("Could not get outward wires in upstream_nodes_below_layer");
											continue;
										};
										if !downstream_connections.is_empty() {
											has_downstream_connections = true;
										}
										stack.extend(downstream_connections.clone());
									}
									if !has_downstream_connections {
										dead_ends += 1;
									}
								}
							}
							InputConnector::Export(_) => current_output_is_sole_dependent = false,
						}
					}
					if !current_output_is_sole_dependent || dead_ends != 0 {
						all_outputs_are_sole_dependents = false;
						break;
					}
				}
				if all_outputs_are_sole_dependents && dead_ends == 0 {
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
		self.view(network_path).ok().and_then(|view| view.input_data(node_id, index, key).ok()).flatten()
	}

	pub fn persistent_input_metadata(&self, node_id: &NodeId, index: usize, network_path: &[NodeId]) -> Option<&InputPersistentMetadata> {
		self.view(network_path).ok().and_then(|view| view.persistent_input_metadata(node_id, index).ok())
	}

	pub(crate) fn transient_input_metadata(&self, node_id: &NodeId, index: usize, network_path: &[NodeId]) -> Option<&InputTransientMetadata> {
		self.view(network_path).ok().and_then(|view| view.transient_input_metadata(node_id, index).ok())
	}

	pub fn set_input_override(&mut self, node_id: &NodeId, index: usize, widget_override: Option<String>, network_path: &[NodeId]) {
		let Some(metadata) = self
			.node_metadata_mut(node_id, network_path)
			.and_then(|node_metadata| node_metadata.persistent_metadata.input_metadata.get_mut(index))
		else {
			log::error!("Could not get input metadata for {node_id} index {index} in set_input_override");
			return;
		};
		metadata.persistent_metadata.widget_override = widget_override;
	}

	/// Returns the input name to display in the properties panel. If the name is empty then the type is used.
	pub fn displayed_input_name_and_description(&mut self, node_id: &NodeId, input_index: usize, network_path: &[NodeId]) -> (String, String) {
		let Some(input_metadata) = self.persistent_input_metadata(node_id, input_index, network_path) else {
			log::warn!("input metadata not found in displayed_input_name_and_description");
			return (String::new(), String::new());
		};
		let description = input_metadata.input_description.to_string();
		let name = if input_metadata.input_name.is_empty() {
			self.input_type(&InputConnector::node(*node_id, input_index), network_path).resolved_type_node_string()
		} else {
			input_metadata.input_name.to_string()
		};
		(name, description)
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
		self.view(network_path).map(|view| view.ordered_pinned_nodes()).unwrap_or_default()
	}

	pub fn is_visible(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_visible", |view| view.is_visible(node_id)).unwrap_or_default()
	}

	pub fn is_layer(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_layer", |view| view.is_layer(node_id)).unwrap_or_default()
	}

	pub fn primary_output_connected_to_layer(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(outward_wires) = self.outward_wires(network_path) else {
			log::error!("Could not get outward_wires in primary_output_connected_to_layer");
			return false;
		};
		let Some(downstream_connectors) = outward_wires.get(&OutputConnector::node(*node_id, 0)) else {
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
		self.view(network_path).map(|view| view.hidden_primary_export()).unwrap_or_default()
	}

	pub fn hidden_primary_output(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.view(network_path).and_then(|view| view.hidden_primary_output(node_id)).unwrap_or_default()
	}

	pub fn hidden_primary_import(&self, network_path: &[NodeId]) -> bool {
		self.view(network_path).map(|view| view.hidden_primary_import()).unwrap_or_default()
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
		self.view(network_path).map(|view| view.is_artboard(node_id)).unwrap_or_default()
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
		self.document_metadata
			.all_layers()
			.filter(|layer| include_artboards || !self.is_artboard(&layer.to_node(), &[]))
			.filter_map(|layer| {
				if !self.is_artboard(&layer.to_node(), &[])
					&& let Some(artboard_node_identifier) = layer
						.ancestors(self.document_metadata())
						.find(|ancestor| *ancestor != LayerNodeIdentifier::ROOT_PARENT && self.is_artboard(&ancestor.to_node(), &[]))
				{
					let artboard = self.document_node(&artboard_node_identifier.to_node(), &[]);
					let clip_input = artboard.unwrap().inputs.get(5).unwrap();
					if let NodeInput::Value { tagged_value, .. } = clip_input
						&& tagged_value.clone().deref() == &TaggedValue::Bool(true)
					{
						return Some(Quad::clip(
							self.document_metadata.bounding_box_document(layer).unwrap_or_default(),
							self.document_metadata.bounding_box_document(artboard_node_identifier).unwrap_or_default(),
						));
					}
				}
				self.document_metadata.bounding_box_document(layer)
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
		self.document_metadata
			.all_layers()
			.filter(|layer| include_artboards || !self.is_artboard(&layer.to_node(), &[]))
			.filter_map(|layer| {
				if !self.is_artboard(&layer.to_node(), &[])
					&& let Some(artboard_node_identifier) = layer
						.ancestors(self.document_metadata())
						.find(|ancestor| *ancestor != LayerNodeIdentifier::ROOT_PARENT && self.is_artboard(&ancestor.to_node(), &[]))
					&& let Some(artboard) = self.document_node(&artboard_node_identifier.to_node(), &[])
					&& let Some(clip_input) = artboard.inputs.get(5)
					&& let NodeInput::Value { tagged_value, .. } = clip_input
					&& tagged_value.clone().deref() == &TaggedValue::Bool(true)
				{
					return Some(Quad::clip(
						self.document_metadata.bounding_box_document_with_stroke(layer).unwrap_or_default(),
						self.document_metadata.bounding_box_document(artboard_node_identifier).unwrap_or_default(),
					));
				}
				self.document_metadata.bounding_box_document_with_stroke(layer)
			})
			// Skip any layer bounds containing NaN to avoid poisoning the combined result
			.filter(|[min, max]| min.is_finite() && max.is_finite())
			.reduce(Quad::combine_bounds)
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
						owned_nodes: TransientMetadata::Unloaded,
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
		Self {
			network: MemoNetwork::new(node_network),
			network_metadata,
			document_metadata: DocumentMetadata::default(),
			resolved_types: ResolvedDocumentNodeTypes::default(),
			transaction_status: TransactionStatus::Finished,
		}
	}
}

// Private mutable getters for use within the network interface
impl NodeNetworkInterface {
	pub(crate) fn network_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetwork> {
		self.document_network_mut().nested_network_mut(network_path)
	}

	pub(crate) fn network_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkMetadata> {
		self.network_metadata.nested_metadata_mut(network_path)
	}

	pub(crate) fn node_metadata_mut(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&mut DocumentNodeMetadata> {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata");
			return None;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get nested node_metadata for node {node_id} in network {network_path:?}");
			return None;
		};
		Some(node_metadata)
	}

	/// Mutably get the network which the encapsulating node of the currently viewed network is part of. Will always be None in the document network.
	pub(crate) fn encapsulating_network_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		encapsulating_path.pop()?;
		let Some(parent_metadata) = self.network_metadata_mut(&encapsulating_path) else {
			log::error!("Could not get parent network in encapsulating_node_metadata");
			return None;
		};
		Some(parent_metadata)
	}

	// /// Mutably get the node which encapsulates the currently viewed network. Will always be None in the document network.
	// fn encapsulating_node_mut(&mut self, network_path: &[NodeId]) -> Option<&mut DocumentNode> {
	// 	let mut encapsulating_path = network_path.to_vec();
	// 	let encapsulating_node_id = encapsulating_path.pop()?;
	// 	let Some(parent_network) = self.network_mut(&encapsulating_path) else {
	// 		log::error!("Could not get parent network in encapsulating_node_mut");
	// 		return None;
	// 	};
	// 	let Some(encapsulating_node) = parent_network.nodes.mut(&encapsulating_node_id) else {
	// 		log::error!("Could not get encapsulating node in encapsulating_node_mut");
	// 		return None;
	// 	};
	// 	Some(encapsulating_node)
	// }

	/// Get the node metadata for the node which encapsulates the currently viewed network. Will always be None in the document network.
	pub(crate) fn encapsulating_node_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut DocumentNodeMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		let encapsulating_node_id = encapsulating_path.pop()?;
		let Some(parent_metadata) = self.network_metadata_mut(&encapsulating_path) else {
			log::error!("Could not get parent network in encapsulating_node_metadata");
			return None;
		};
		let Some(encapsulating_node_metadata) = parent_metadata.persistent_metadata.node_metadata.get_mut(&encapsulating_node_id) else {
			log::error!("Could not get encapsulating node metadata in encapsulating_node_metadata");
			return None;
		};
		Some(encapsulating_node_metadata)
	}
}
