use super::*;

// Public mutable getters for data that involves transient network metadata
// Mutable methods never recalculate the transient metadata, they only unload it. Loading metadata should only be done by the getter.
impl NodeNetworkInterface {
	pub fn start_transaction(&mut self) {
		self.transaction_status = TransactionStatus::Started;
	}

	pub fn transaction_modified(&mut self) {
		if self.transaction_status == TransactionStatus::Started {
			self.transaction_status = TransactionStatus::Modified;
		}
	}

	pub fn finish_transaction(&mut self) {
		self.transaction_status = TransactionStatus::Finished;
	}

	/// Mutably get the selected nodes for the network at the network_path. Every time they are mutated, the transient metadata for the top of the stack gets unloaded.
	pub fn selected_nodes_mut(&mut self, network_path: &[NodeId]) -> Option<&mut SelectedNodes> {
		let (last_selection_state, prev_state, is_selection_empty) = {
			let network_metadata = self.network_metadata(network_path)?;
			let history = &network_metadata.persistent_metadata.selection_undo_history;
			let current = history.back().cloned().unwrap_or_default();
			let previous = history.iter().rev().nth(1).cloned();
			let empty = current.selected_layers_except_artboards(self).next().is_none();
			(current, previous, empty)
		};
		self.unload_stack_dependents(network_path);

		let Some(history) = self.selection_history_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes_mut");
			return None;
		};

		if history.undo.is_empty() {
			history.undo.push_back(SelectedNodes::default());
		}

		// Update history only if selection is non-empty/does not contain only artboards
		if !is_selection_empty && prev_state.as_ref() != Some(&last_selection_state) {
			history.undo.push_back(last_selection_state);
			history.redo.clear();

			if history.undo.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
				history.undo.pop_front();
			}
		}

		history.undo.back_mut()
	}

	pub fn selection_step_back(&mut self, network_path: &[NodeId]) {
		let Some(history) = self.selection_history_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selection_step_back");
			return;
		};

		if let Some(selection_state) = history.undo.pop_back() {
			history.redo.push_front(selection_state);
		}
	}

	pub fn selection_step_forward(&mut self, network_path: &[NodeId]) {
		let Some(history) = self.selection_history_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selection_step_forward");
			return;
		};

		if let Some(selection_state) = history.redo.pop_front() {
			history.undo.push_back(selection_state);
		}
	}

	pub(crate) fn stack_dependents(&mut self, network_path: &[NodeId]) -> Option<&HashMap<NodeId, LayerOwner>> {
		self.try_load_stack_dependents(network_path);
		self.try_get_stack_dependents(network_path)
	}

	pub(crate) fn try_load_stack_dependents(&self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in try_load_stack_dependents");
			return;
		};

		if !network_metadata.transient_metadata.stack_dependents.is_loaded() {
			self.load_stack_dependents(network_path);
		}
	}

	/// Reads the stack dependents through &self if they are already loaded.
	pub(crate) fn with_stack_dependents_if_loaded<R>(&self, network_path: &[NodeId], read: impl FnOnce(&HashMap<NodeId, LayerOwner>) -> R) -> Option<R> {
		self.network_metadata(network_path)?.transient_metadata.stack_dependents.with_loaded(read)
	}

	pub(crate) fn try_get_stack_dependents(&mut self, network_path: &[NodeId]) -> Option<&HashMap<NodeId, LayerOwner>> {
		let Some(transient) = self.network_transient_mut(network_path) else {
			log::error!("Could not get nested network_metadata in try_get_stack_dependents");
			return None;
		};
		let Some(stack_dependents) = transient.stack_dependents.get_loaded_mut() else {
			log::error!("Could not load stack dependents in try_get_stack_dependents");
			return None;
		};
		Some(stack_dependents)
	}

	// This function always has to be in sync with the selected nodes.
	fn load_stack_dependents(&self, network_path: &[NodeId]) {
		let Some(selected_nodes) = self.selected_nodes_in_nested_network(network_path) else {
			log::error!("Could not get selected nodes in load_stack_dependents");
			return;
		};
		self.load_stack_dependents_for_nodes(selected_nodes.selected_nodes().cloned().collect(), network_path);
	}

	/// Builds the stack dependents as if `seed_nodes` were the selection, for shifts driven by a node set other than the selection.
	pub(crate) fn load_stack_dependents_for_nodes(&self, seed_nodes: Vec<NodeId>, network_path: &[NodeId]) {
		let mut selected_layers = seed_nodes.iter().filter(|node_id| self.is_layer(node_id, network_path)).copied().collect::<HashSet<_>>();

		// Deselect all layers that are upstream of other selected layers
		let mut removed_layers = Vec::new();
		for layer in selected_layers.clone() {
			if removed_layers.contains(&layer) {
				continue;
			}
			for upstream_node in self.upstream_flow_back_from_nodes(vec![layer], network_path, FlowType::UpstreamFlow).skip(1) {
				if selected_layers.remove(&upstream_node) {
					removed_layers.push(upstream_node)
				}
			}
		}

		// Get a unique list of the top of each stack for each layer
		let mut stack_tops = HashSet::new();

		for layer in &selected_layers {
			let mut current_node = *layer;
			loop {
				if self.is_layer(&current_node, network_path) && self.is_absolute(&current_node, network_path) {
					stack_tops.insert(current_node);
					break;
				};
				let Some(first_downstream_input) = self.with_outward_wires(network_path, |outward_wires| {
					outward_wires
						.get(&OutputConnector::primary_output(current_node))
						.map(|layer_outward_wires| layer_outward_wires.first().copied())
				}) else {
					log::error!("Cannot load outward wires in load_stack_dependents_for_nodes");
					return;
				};
				let Some(first_downstream_input) = first_downstream_input else {
					log::error!("Could not get outward_wires for layer {current_node}");
					break;
				};
				match first_downstream_input {
					Some(downstream_input) => {
						let Some(downstream_node) = downstream_input.node_id() else {
							log::error!("Node connected to export should be absolute");
							break;
						};
						current_node = downstream_node
					}
					None => break,
				}
			}
		}

		let mut stack_dependents = HashMap::new();
		let mut owned_sole_dependents = HashSet::new();
		// Loop through all layers below the stack_tops, and set sole dependents upstream from that layer to be owned by that layer. Ensure LayerOwner is kept in sync.
		for stack_top in &stack_tops {
			for upstream_stack_layer in self
				.upstream_flow_back_from_nodes(vec![*stack_top], network_path, FlowType::PrimaryFlow)
				.take_while(|upstream_node| self.is_layer(upstream_node, network_path))
				.collect::<Vec<_>>()
			{
				for upstream_layer in self.upstream_flow_back_from_nodes(vec![upstream_stack_layer], network_path, FlowType::UpstreamFlow).collect::<Vec<_>>() {
					if !self.is_layer(&upstream_layer, network_path) {
						continue;
					}
					let mut new_owned_nodes = HashSet::new();
					for layer_sole_dependent in &self.upstream_nodes_below_layer(&upstream_layer, network_path) {
						stack_dependents.insert(*layer_sole_dependent, LayerOwner::Layer(upstream_layer));
						owned_sole_dependents.insert(*layer_sole_dependent);
						new_owned_nodes.insert(*layer_sole_dependent);
					}
					let Some(layer_node) = self.node_metadata(&upstream_layer, network_path) else {
						log::error!("Could not get layer node in load_stack_dependents_for_nodes");
						continue;
					};
					layer_node.transient_metadata.owned_nodes.store(new_owned_nodes);
				}
			}
		}

		// Set any sole dependents of the stack top that are not dependents of a layer in the stack to LayerOwner::None. These nodes will be pushed as blocks when a layer is shifted.
		for stack_top in &stack_tops {
			let mut sole_dependents = HashSet::new();
			let mut not_sole_dependents = HashSet::new();
			sole_dependents.insert(*stack_top);
			for upstream_node in self.upstream_flow_back_from_nodes(vec![*stack_top], network_path, FlowType::UpstreamFlow).collect::<Vec<_>>() {
				if sole_dependents.contains(&upstream_node) || not_sole_dependents.contains(&upstream_node) {
					continue;
				}

				// A path terminates at an already-verified sole dependent, and fails fast through a known non-sole node
				let is_sole_dependent = self.is_sole_dependent(upstream_node, network_path, |downstream_node, _| {
					if not_sole_dependents.contains(&downstream_node) {
						SoleDependentStep::Escape
					} else if sole_dependents.contains(&downstream_node) {
						SoleDependentStep::Terminate
					} else {
						SoleDependentStep::Continue
					}
				});

				if is_sole_dependent {
					sole_dependents.insert(upstream_node);
				} else {
					not_sole_dependents.insert(upstream_node);
				}
			}

			for sole_dependent in sole_dependents {
				if !owned_sole_dependents.contains(&sole_dependent) {
					stack_dependents.insert(sole_dependent, LayerOwner::None);
				}
			}
		}

		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get current network in load_stack_dependents_for_nodes");
			return;
		};

		network_metadata.transient_metadata.stack_dependents.store(stack_dependents);
	}

	pub fn unload_stack_dependents(&mut self, network_path: &[NodeId]) {
		let Some(transient) = self.network_transient_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_stack_dependents");
			return;
		};
		transient.stack_dependents.unload();

		// Drag offsets are only meaningful relative to the stack dependents snapshot they were accumulated against, so they must not outlive it
		transient.drag_offsets.borrow_mut().clear();
	}

	/// The vertical distance the node has been pushed from its resting position during the current drag.
	pub(crate) fn drag_offset(&self, node_id: &NodeId, network_path: &[NodeId]) -> i32 {
		self.network_metadata(network_path)
			.map_or(0, |network_metadata| network_metadata.transient_metadata.drag_offsets.borrow().get(node_id).copied().unwrap_or(0))
	}

	pub(crate) fn add_drag_offset(&self, node_id: &NodeId, delta: i32, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in add_drag_offset");
			return;
		};
		*network_metadata.transient_metadata.drag_offsets.borrow_mut().entry(*node_id).or_insert(0) += delta;
	}

	/// Discards all drag offsets when the drag ends.
	pub fn clear_drag_offsets(&self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in clear_drag_offsets");
			return;
		};
		network_metadata.transient_metadata.drag_offsets.borrow_mut().clear();
	}

	pub fn import_export_ports(&mut self, network_path: &[NodeId]) -> Option<&Ports> {
		self.load_import_export_ports(network_path);

		let Some(transient) = self.network_transient_mut(network_path) else {
			log::error!("Could not get nested network_metadata in import_export_ports");
			return None;
		};
		let Some(ports) = transient.import_export_ports.get_loaded_mut() else {
			log::error!("Could not load import export ports in import_export_ports");
			return None;
		};
		Some(ports)
	}

	/// Reads the import/export ports through &self, loading them first if needed.
	pub(crate) fn with_import_export_ports<R>(&self, network_path: &[NodeId], read: impl FnOnce(&Ports) -> R) -> Option<R> {
		self.network_metadata(network_path)?
			.transient_metadata
			.import_export_ports
			.with_loaded_or(|| self.compute_import_export_ports(network_path), read)
	}

	pub fn load_import_export_ports(&self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in load_import_export_ports");
			return;
		};
		network_metadata.transient_metadata.import_export_ports.ensure_loaded(|| self.compute_import_export_ports(network_path));
	}

	pub(crate) fn unload_import_export_ports(&mut self, network_path: &[NodeId]) {
		let Some(transient) = self.network_transient_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_import_export_ports");
			return;
		};
		transient.import_export_ports.unload();

		// Always unload all wires connected to them as well
		let number_of_imports = self.number_of_imports(network_path);
		let Some(outward_wires) = self.outward_wires(network_path) else {
			log::error!("Could not get outward wires in unload_import_export_ports");
			return;
		};
		let mut input_connectors = Vec::new();
		for import_index in 0..number_of_imports {
			let Some(outward_wires_for_import) = outward_wires.get(&OutputConnector::Import(import_index)).cloned() else {
				log::error!("Could not get outward wires for import in unload_import_export_ports");
				return;
			};
			input_connectors.extend(outward_wires_for_import);
		}
		let Some(network) = self.nested_network(network_path) else {
			return;
		};
		for export_index in 0..network.exports.len() {
			input_connectors.push(InputConnector::Export(export_index));
		}
		for input in &input_connectors {
			self.unload_wire(input, network_path);
		}
	}

	pub fn modify_import_export(&mut self, network_path: &[NodeId]) -> Option<&ModifyImportExportClickTarget> {
		self.load_modify_import_export(network_path);

		let Some(transient) = self.network_transient_mut(network_path) else {
			log::error!("Could not get nested network_metadata in modify_import_export");
			return None;
		};
		let Some(click_targets) = transient.modify_import_export.get_loaded_mut() else {
			log::error!("Could not load modify import export in modify_import_export");
			return None;
		};
		Some(click_targets)
	}

	pub fn load_modify_import_export(&self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in load_modify_import_export");
			return;
		};
		network_metadata
			.transient_metadata
			.modify_import_export
			.ensure_loaded(|| self.compute_modify_import_export(network_path));
	}

	pub(crate) fn unload_modify_import_export(&mut self, network_path: &[NodeId]) {
		let Some(transient) = self.network_transient_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_modify_import_export");
			return;
		};
		transient.modify_import_export.unload();
	}

	/// Reads the owned nodes of a layer through &self if they are loaded.
	pub(crate) fn with_owned_nodes_if_loaded<R>(&self, node_id: &NodeId, network_path: &[NodeId], read: impl FnOnce(&HashSet<NodeId>) -> R) -> Option<R> {
		let layer_node = self.node_metadata(node_id, network_path)?;
		if !layer_node.persistent_metadata.is_layer() {
			return None;
		}
		layer_node.transient_metadata.owned_nodes.with_loaded(read)
	}

	pub fn all_nodes_bounding_box(&self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in all_nodes_bounding_box");
			return None;
		};

		let bounding_box = network_metadata
			.transient_metadata
			.all_nodes_bounding_box
			.with_loaded_or(|| self.compute_all_nodes_bounding_box(network_path), |bounds| *bounds);
		if bounding_box.is_none() {
			log::error!("Could not load all nodes bounding box in all_nodes_bounding_box");
		}
		bounding_box
	}

	/// The combined bounds of every node in the network, in node graph space.
	fn compute_all_nodes_bounding_box(&self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let network_metadata = self.network_metadata(network_path)?;
		let nodes = network_metadata.persistent_metadata.node_metadata.keys().copied().collect::<Vec<_>>();

		Some(
			nodes
				.iter()
				.filter_map(|node_id| self.node_bounding_box(node_id, network_path))
				.reduce(Quad::combine_bounds)
				.unwrap_or([DVec2::new(0., 0.), DVec2::new(0., 0.)]),
		)
	}

	pub fn unload_all_nodes_bounding_box(&mut self, network_path: &[NodeId]) {
		let Some(transient) = self.network_transient_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_all_nodes_bounding_box");
			return;
		};
		transient.all_nodes_bounding_box.unload();
		self.unload_import_export_ports(network_path);
	}

	pub fn outward_wires(&mut self, network_path: &[NodeId]) -> Option<&HashMap<OutputConnector, Vec<InputConnector>>> {
		self.load_outward_wires(network_path);

		let Some(transient) = self.network_transient_mut(network_path) else {
			log::error!("Could not get nested network_metadata in outward_wires");
			return None;
		};
		let Some(outward_wires) = transient.outward_wires.get_loaded_mut() else {
			log::error!("Could not load outward wires in outward_wires");
			return None;
		};

		Some(outward_wires)
	}

	/// Reads the outward wires through &self, loading them first if needed.
	pub(crate) fn with_outward_wires<R>(&self, network_path: &[NodeId], read: impl FnOnce(&HashMap<OutputConnector, Vec<InputConnector>>) -> R) -> Option<R> {
		self.network_metadata(network_path)?
			.transient_metadata
			.outward_wires
			.with_loaded_or(|| self.compute_outward_wires(network_path), read)
	}

	fn load_outward_wires(&self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in load_outward_wires");
			return;
		};
		network_metadata.transient_metadata.outward_wires.ensure_loaded(|| self.compute_outward_wires(network_path));
	}

	/// Every input fed by each node output and each import of the network.
	fn compute_outward_wires(&self, network_path: &[NodeId]) -> Option<HashMap<OutputConnector, Vec<InputConnector>>> {
		let mut outward_wires = HashMap::new();
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in compute_outward_wires");
			return None;
		};
		// Initialize all output connectors for nodes
		for node_id in network.nodes.keys() {
			let number_of_outputs = self.number_of_outputs(node_id, network_path);
			for output_index in 0..number_of_outputs {
				outward_wires.insert(OutputConnector::node(*node_id, output_index), Vec::new());
			}
		}
		// Initialize output connectors for the import node
		for import_index in 0..self.number_of_imports(network_path) {
			outward_wires.insert(OutputConnector::Import(import_index), Vec::new());
		}
		// Collect wires between all nodes and the Imports
		// A missing entry means a wire references a node output or import that does not exist, so log it and register the connector anyway rather than crashing
		let push_outward_wire = |outward_wires: &mut HashMap<OutputConnector, Vec<InputConnector>>, output_connector: OutputConnector, input_connector: InputConnector| {
			let outward_wires_entry = outward_wires.entry(output_connector).or_insert_with(|| {
				log::error!("Output connector {output_connector:?} should be initialized in compute_outward_wires");
				Vec::new()
			});
			outward_wires_entry.push(input_connector);
		};
		for (current_node_id, node) in network.nodes.iter() {
			for (input_index, input) in node.inputs.iter().enumerate() {
				if let NodeInput::Node { node_id, output_index, .. } = input {
					push_outward_wire(
						&mut outward_wires,
						OutputConnector::node(*node_id, *output_index),
						InputConnector::node_at_index(*current_node_id, input_index),
					);
				} else if let NodeInput::Import { import_index, .. } = input {
					push_outward_wire(&mut outward_wires, OutputConnector::Import(*import_index), InputConnector::node_at_index(*current_node_id, input_index));
				}
			}
		}
		for (export_index, export) in network.exports.iter().enumerate() {
			if let NodeInput::Node { node_id, output_index, .. } = export {
				push_outward_wire(&mut outward_wires, OutputConnector::node(*node_id, *output_index), InputConnector::Export(export_index));
			} else if let NodeInput::Import { import_index, .. } = export {
				push_outward_wire(&mut outward_wires, OutputConnector::Import(*import_index), InputConnector::Export(export_index));
			}
		}

		Some(outward_wires)
	}

	pub(crate) fn unload_outward_wires(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in unload_outward_wires");
			return;
		};
		network_metadata.transient_metadata.outward_wires.unload();
	}

	/// Incrementally updates the outward_wires cache when a single input connector changes,
	/// avoiding a full rebuild. If the cache is not loaded, this is a no-op (it will be fully
	/// rebuilt on the next read via `outward_wires()`).
	pub(crate) fn update_outward_wires(&mut self, network_path: &[NodeId], input_connector: &InputConnector, old_input: &NodeInput, new_input: &NodeInput) {
		let Some(transient) = self.network_transient_mut(network_path) else {
			return;
		};
		let Some(outward_wires) = transient.outward_wires.get_loaded_mut() else {
			return;
		};

		// Remove the input_connector from the old output's downstream list
		if let Some(old_output) = OutputConnector::from_input(old_input)
			&& let Some(connections) = outward_wires.get_mut(&old_output)
		{
			connections.retain(|c| c != input_connector);
		}

		// Add the input_connector to the new output's downstream list
		if let Some(new_output) = OutputConnector::from_input(new_input) {
			outward_wires.entry(new_output).or_default().push(*input_connector);
		}
	}

	pub fn layer_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<u32> {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in layer_width");
			return None;
		};
		if !node_metadata.persistent_metadata.is_layer() {
			log::error!("Cannot get layer width for non layer node {node_id} in network {network_path:?}");
			return None;
		}

		node_metadata
			.transient_metadata
			.layer_width
			.with_loaded_or(|| Some(self.compute_layer_width(node_id, network_path)), |layer_width| *layer_width)
	}

	/// Unloads layer width if the node is a layer.
	pub fn try_unload_layer_width(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let is_layer = self.is_layer(node_id, network_path);

		let Some(transient) = self.node_transient_mut(node_id, network_path) else {
			return;
		};

		if is_layer {
			transient.layer_width.unload();
		}
	}

	pub fn get_input_center(&self, input: &InputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match input {
			InputConnector::Node { node_id, input_index } => self
				.with_node_click_targets(node_id, network_path, |click_targets| click_targets.port_click_targets.input_port_position(*input_index))
				.flatten(),
			InputConnector::Export(export_index) => self.with_import_export_ports(network_path, |ports| ports.input_port_position(*export_index)).flatten(),
		}
	}

	pub fn get_output_center(&self, output: &OutputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match output {
			OutputConnector::Node { node_id, output_index } => self
				.with_node_click_targets(node_id, network_path, |click_targets| click_targets.port_click_targets.output_port_position(*output_index))
				.flatten(),
			OutputConnector::Import(import_index) => self.with_import_export_ports(network_path, |ports| ports.output_port_position(*import_index)).flatten(),
		}
	}

	pub fn newly_loaded_input_wire(&self, input: &InputConnector, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<WirePathUpdate> {
		if !self.wire_is_loaded(input, network_path) {
			self.load_wire(input, graph_wire_style, network_path);
		} else {
			return None;
		}

		let network_metadata = self.network_metadata(network_path)?;
		let Some(wire) = network_metadata.transient_metadata.wires.borrow().get(input).cloned() else {
			log::error!("Could not load wire for input: {input:?}");
			return None;
		};
		Some(wire)
	}

	pub fn wire_is_loaded(&self, input: &InputConnector, network_path: &[NodeId]) -> bool {
		self.network_metadata(network_path)
			.is_some_and(|network_metadata| network_metadata.transient_metadata.wires.borrow().contains_key(input))
	}

	fn load_wire(&self, input: &InputConnector, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) {
		// The export keeps its own solid wire while previewing, since previewing does not rewire it
		let Some(wire) = self.wire_path_from_input(input, graph_wire_style, false, network_path) else {
			log::error!("Could not load wire path from input");
			return;
		};
		let (id, input_index) = match input {
			InputConnector::Node { node_id, input_index } => (*node_id, *input_index),
			InputConnector::Export(export_index) => (NodeId(u64::MAX), *export_index),
		};
		let wire_update = WirePathUpdate {
			id,
			input_index,
			wire_path_update: Some(wire),
		};

		let Some(network_metadata) = self.network_metadata(network_path) else { return };
		network_metadata.transient_metadata.wires.borrow_mut().insert(*input, wire_update);
	}

	pub fn all_input_connectors(&self, network_path: &[NodeId]) -> Vec<InputConnector> {
		let mut input_connectors = Vec::new();
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in all_input_connectors");
			return Vec::new();
		};
		for export_index in 0..network.exports.len() {
			input_connectors.push(InputConnector::Export(export_index));
		}
		for (node_id, node) in &network.nodes {
			for input_index in 0..node.inputs.len() {
				input_connectors.push(InputConnector::node_at_index(*node_id, input_index));
			}
		}
		input_connectors
	}

	pub fn node_graph_input_connectors(&self, network_path: &[NodeId]) -> Vec<InputConnector> {
		self.all_input_connectors(network_path)
			.into_iter()
			.filter(|input| self.input_from_connector(input, network_path).is_some_and(|input| input.is_exposed()))
			.collect()
	}

	/// Maps to the frontend representation of a wire start. Includes disconnected value wire inputs.
	pub fn node_graph_wire_inputs(&self, network_path: &[NodeId]) -> Vec<(NodeId, usize)> {
		self.node_graph_input_connectors(network_path)
			.iter()
			.map(|input| match input {
				InputConnector::Node { node_id, input_index } => (*node_id, *input_index),
				InputConnector::Export(export_index) => (NodeId(u64::MAX), *export_index),
			})
			.chain(std::iter::once((NodeId(u64::MAX), u32::MAX as usize)))
			.collect()
	}

	fn unload_wires_for_node(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let number_of_outputs = self.number_of_outputs(node_id, network_path);
		let Some(outward_wires) = self.outward_wires(network_path) else {
			log::error!("Could not get outward wires in unload_wires_for_node");
			return;
		};
		let mut input_connectors = Vec::new();
		for output_index in 0..number_of_outputs {
			let Some(inputs) = outward_wires.get(&OutputConnector::node(*node_id, output_index)) else {
				continue;
			};
			input_connectors.extend(inputs.clone())
		}
		for input_index in 0..self.number_of_inputs(node_id, network_path) {
			input_connectors.push(InputConnector::node_at_index(*node_id, input_index));
		}
		for input in input_connectors {
			self.unload_wire(&input, network_path);
		}
	}

	pub fn unload_wire(&mut self, input: &InputConnector, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			return;
		};
		network_metadata.transient_metadata.wires.borrow_mut().remove(input);
	}

	/// The dashed wire from the previewed node to the export, drawn alongside the solid wire the export
	/// is really connected to.
	///
	/// Previewing does not rewire the export, so both are shown: the solid wire says what the document
	/// renders, and this one says what this peer is looking at instead.
	pub fn wire_to_preview(&self, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<WirePathUpdate> {
		let input = InputConnector::Export(0);

		let Previewing::Yes { previewed } = self.previewing(network_path) else { return None };

		// The export already reaches it, so a second wire would land on top of the solid one. Compared by
		// the whole connector: two outputs of the same node leave from different ports, so those wires do
		// not overlap.
		if self.upstream_output_connector(&input, network_path) == Some(previewed.to_connector()) {
			return None;
		}
		let Some(input_position) = self.get_input_center(&input, network_path) else {
			log::error!("Could not get input position for wire end in preview: {input:?}");
			return None;
		};
		let upstream_output = OutputConnector::node(previewed.node_id, previewed.output_index);
		let Some(output_position) = self.get_output_center(&upstream_output, network_path) else {
			log::error!("Could not get output position for wire start in preview: {upstream_output:?}");
			return None;
		};
		let vertical_end = input.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path) && input.input_index() == 0);
		let vertical_start: bool = upstream_output.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path));
		let thick = vertical_end && vertical_start;
		let vector_wire = build_vector_wire(output_position, input_position, vertical_start, vertical_end, graph_wire_style);
		let center_line = build_thick_wire_center_line(output_position, input_position, vertical_start, vertical_end);

		let path_string = vector_wire.to_svg();
		let center_path_string = center_line.to_svg();
		let input_type = self.input_type(&input, network_path);
		let data_type = input_type.displayed_type();
		let is_list = input_type.is_list();
		let wire_path_update = Some(WirePath {
			path_string,
			data_type,
			thick,
			dashed: true,
			is_list,
			center_path_string,
		});

		Some(WirePathUpdate {
			id: NodeId(u64::MAX),
			input_index: u32::MAX as usize,
			wire_path_update,
		})
	}

	/// Returns the wire subpath, its thick center-line subpath, and whether the wire should be thick.
	pub fn vector_wire_from_input(&self, input: &InputConnector, wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<(BezPath, BezPath, bool)> {
		let Some(input_position) = self.get_input_center(input, network_path) else {
			log::error!("Could not get dom rect for wire end: {input:?}");
			return None;
		};
		// An upstream output could not be found, so the wire does not exist, but it should still be loaded as as empty vector
		let Some(upstream_output) = self.upstream_output_connector(input, network_path) else {
			return Some((BezPath::new(), BezPath::new(), false));
		};
		let Some(output_position) = self.get_output_center(&upstream_output, network_path) else {
			log::error!("Could not get output port for wire start: {upstream_output:?}");
			return None;
		};
		let vertical_end = input.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path) && input.input_index() == 0);
		let vertical_start = upstream_output.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path));
		let thick = vertical_end && vertical_start;
		let vector_wire = build_vector_wire(output_position, input_position, vertical_start, vertical_end, wire_style);
		let center_line = build_thick_wire_center_line(output_position, input_position, vertical_start, vertical_end);
		Some((vector_wire, center_line, thick))
	}

	pub fn wire_path_from_input(&self, input: &InputConnector, graph_wire_style: GraphWireStyle, dashed: bool, network_path: &[NodeId]) -> Option<WirePath> {
		let (vector_wire, center_line, thick) = self.vector_wire_from_input(input, graph_wire_style, network_path)?;
		let path_string = vector_wire.to_svg();
		let center_path_string = center_line.to_svg();
		let (data_type, is_list) = self
			.upstream_output_connector(input, network_path)
			.map(|output| {
				let output_type = self.output_type(&output, network_path);
				(output_type.displayed_type(), output_type.is_list())
			})
			.unwrap_or((FrontendGraphDataType::General, false));
		Some(WirePath {
			path_string,
			data_type,
			thick,
			dashed,
			is_list,
			center_path_string,
		})
	}

	pub fn node_click_targets(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&DocumentNodeClickTargets> {
		self.load_node_click_targets(node_id, network_path);

		let transient = self.node_transient_mut(node_id, network_path)?;
		let Some(click_targets) = transient.click_targets.get_loaded_mut() else {
			log::error!("Could not load click targets in node_click_targets");
			return None;
		};
		Some(click_targets)
	}

	pub(crate) fn load_node_click_targets(&self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in load_node_click_targets");
			return;
		};
		node_metadata.transient_metadata.click_targets.ensure_loaded(|| self.compute_node_click_targets(node_id, network_path));
	}

	/// Loads the node click targets if needed, then reads them through &self.
	pub(crate) fn with_node_click_targets<R>(&self, node_id: &NodeId, network_path: &[NodeId], read: impl FnOnce(&DocumentNodeClickTargets) -> R) -> Option<R> {
		self.node_metadata(node_id, network_path)?
			.transient_metadata
			.click_targets
			.with_loaded_or(|| self.compute_node_click_targets(node_id, network_path), read)
	}

	/// Reads the modify import/export click targets through &self, loading them first if needed.
	pub(crate) fn with_modify_import_export<R>(&self, network_path: &[NodeId], read: impl FnOnce(&ModifyImportExportClickTarget) -> R) -> Option<R> {
		self.network_metadata(network_path)?
			.transient_metadata
			.modify_import_export
			.with_loaded_or(|| self.compute_modify_import_export(network_path), read)
	}

	/// Reads the node click targets through &self if they are already loaded.
	pub(crate) fn with_node_click_targets_if_loaded<R>(&self, node_id: &NodeId, network_path: &[NodeId], read: impl FnOnce(&DocumentNodeClickTargets) -> R) -> Option<R> {
		let node_metadata = self.node_metadata(node_id, network_path)?;
		let result = node_metadata.transient_metadata.click_targets.with_loaded(read);
		if result.is_none() {
			log::error!("Could not load click targets in with_node_click_targets_if_loaded");
		}
		result
	}

	pub fn node_bounding_box(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		self.with_node_click_targets(node_id, network_path, |click_targets| click_targets.node_click_target.bounding_box())
			.flatten()
	}

	/// The bounding box only if the click targets are already loaded, for a caller that is walking many
	/// nodes and cannot afford to load each one it looks at.
	pub fn try_get_node_bounding_box(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		self.with_node_click_targets_if_loaded(node_id, network_path, |click_targets| click_targets.node_click_target.bounding_box())
			.flatten()
	}

	pub fn load_all_node_click_targets(&self, network_path: &[NodeId]) {
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get network in load_all_node_click_targets");
			return;
		};
		for node_id in network.nodes.keys().cloned().collect::<Vec<_>>() {
			self.load_node_click_targets(&node_id, network_path);
		}
	}

	pub fn unload_node_click_targets(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(transient) = self.node_transient_mut(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in unload_node_click_targets");
			return;
		};
		transient.click_targets.unload();
		self.unload_wires_for_node(node_id, network_path);
	}

	pub fn unload_upstream_node_click_targets(&mut self, node_ids: Vec<NodeId>, network_path: &[NodeId]) {
		let upstream_nodes = self.upstream_flow_back_from_nodes(node_ids, network_path, FlowType::UpstreamFlow).collect::<Vec<_>>();

		for upstream_id in &upstream_nodes {
			self.unload_node_click_targets(upstream_id, network_path);
		}
	}

	pub fn unload_all_nodes_click_targets(&mut self, network_path: &[NodeId]) {
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in unload_all_nodes_click_targets");
			return;
		};
		let upstream_nodes = network.nodes.keys().cloned().collect::<Vec<_>>();

		for upstream_id in &upstream_nodes {
			self.unload_node_click_targets(upstream_id, network_path);
		}
	}
}
