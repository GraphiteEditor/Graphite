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

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes");
			return None;
		};

		// Initialize default value if selection_undo_history is empty
		if network_metadata.persistent_metadata.selection_undo_history.is_empty() {
			network_metadata.persistent_metadata.selection_undo_history.push_back(SelectedNodes::default());
		}

		// Update history only if selection is non-empty/does not contain only artboards
		if !is_selection_empty && prev_state.as_ref() != Some(&last_selection_state) {
			network_metadata.persistent_metadata.selection_undo_history.push_back(last_selection_state);
			network_metadata.persistent_metadata.selection_redo_history.clear();

			if network_metadata.persistent_metadata.selection_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
				network_metadata.persistent_metadata.selection_undo_history.pop_front();
			}
		}

		network_metadata.persistent_metadata.selection_undo_history.back_mut()
	}

	pub fn selection_step_back(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selection_step_back");
			return;
		};

		if let Some(selection_state) = network_metadata.persistent_metadata.selection_undo_history.pop_back() {
			network_metadata.persistent_metadata.selection_redo_history.push_front(selection_state);
		}
	}

	pub fn selection_step_forward(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selection_step_forward");
			return;
		};

		if let Some(selection_state) = network_metadata.persistent_metadata.selection_redo_history.pop_front() {
			network_metadata.persistent_metadata.selection_undo_history.push_back(selection_state);
		}
	}

	pub(crate) fn stack_dependents(&mut self, network_path: &[NodeId]) -> Option<&HashMap<NodeId, LayerOwner>> {
		self.try_load_stack_dependents(network_path);
		self.try_get_stack_dependents(network_path)
	}

	pub(crate) fn try_load_stack_dependents(&self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in stack_dependents");
			return;
		};

		if !network_metadata.transient_metadata.stack_dependents.is_loaded() {
			self.load_stack_dependents(network_path);
		}
	}

	/// Reads the stack dependents through &self if they are already loaded.
	pub(crate) fn with_stack_dependents<R>(&self, network_path: &[NodeId], read: impl FnOnce(&HashMap<NodeId, LayerOwner>) -> R) -> Option<R> {
		self.network_metadata(network_path)?.transient_metadata.stack_dependents.with_loaded(read)
	}

	pub(crate) fn try_get_stack_dependents(&mut self, network_path: &[NodeId]) -> Option<&HashMap<NodeId, LayerOwner>> {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in try_get_stack_dependents");
			return None;
		};
		let Some(stack_dependents) = network_metadata.transient_metadata.stack_dependents.get_loaded_mut() else {
			log::error!("could not load stack_dependents");
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
						.get(&OutputConnector::node(current_node, 0))
						.map(|layer_outward_wires| layer_outward_wires.first().copied())
				}) else {
					log::error!("Cannot load outward wires in load_stack_dependents");
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
						log::error!("Could not get layer node in load_stack_dependents");
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
			log::error!("Could not get current network in load_stack_dependents");
			return;
		};

		network_metadata.transient_metadata.stack_dependents.store(stack_dependents);
	}

	pub fn unload_stack_dependents(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_stack_dependents");
			return;
		};
		network_metadata.transient_metadata.stack_dependents.unload();
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
		self.try_load_import_export_ports(network_path);

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in export_ports");
			return None;
		};
		let Some(ports) = network_metadata.transient_metadata.import_export_ports.get_loaded_mut() else {
			log::error!("could not load import ports");
			return None;
		};
		Some(ports)
	}

	/// Reads the import/export ports through &self, loading them first if needed.
	pub(crate) fn with_import_export_ports<R>(&self, network_path: &[NodeId], read: impl FnOnce(&Ports) -> R) -> Option<R> {
		self.try_load_import_export_ports(network_path);
		self.network_metadata(network_path)?.transient_metadata.import_export_ports.with_loaded(read)
	}

	fn try_load_import_export_ports(&self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in export_ports");
			return;
		};
		if !network_metadata.transient_metadata.import_export_ports.is_loaded() {
			self.load_import_export_ports(network_path);
		}
	}

	pub fn load_import_export_ports(&self, network_path: &[NodeId]) {
		let Some(import_export_position) = self.import_export_position(network_path) else {
			log::error!("Could not get import_export_position");
			return;
		};
		let Some(network) = self.nested_network(network_path) else { return };
		let mut import_export_ports = Ports::new();

		if !network_path.is_empty() {
			let import_start_index = if self.hidden_primary_import(network_path) { 1 } else { 0 };
			for import_index in import_start_index..self.number_of_imports(network_path) {
				import_export_ports.insert_output_port_at_center(import_index, import_export_position.0.as_dvec2() + DVec2::new(0., import_index as f64 * 24.));
			}
		}

		let export_start_index = if self.hidden_primary_export(network_path) { 1 } else { 0 };
		for export_index in export_start_index..network.exports.len() {
			import_export_ports.insert_input_port_at_center(export_index, import_export_position.1.as_dvec2() + DVec2::new(0., export_index as f64 * 24.));
		}

		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return;
		};

		network_metadata.transient_metadata.import_export_ports.store(import_export_ports);
	}

	pub(crate) fn unload_import_export_ports(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_export_ports");
			return;
		};
		network_metadata.transient_metadata.import_export_ports.unload();

		// Always unload all wires connected to them as well
		let number_of_imports = self.number_of_imports(network_path);
		let Some(outward_wires) = self.outward_wires(network_path) else {
			log::error!("Could not get outward wires in remove_import");
			return;
		};
		let mut input_connectors = Vec::new();
		for import_index in 0..number_of_imports {
			let Some(outward_wires_for_import) = outward_wires.get(&OutputConnector::Import(import_index)).cloned() else {
				log::error!("Could not get outward wires for import in remove_import");
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
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in modify_import_export");
			return None;
		};
		if !network_metadata.transient_metadata.modify_import_export.is_loaded() {
			self.load_modify_import_export(network_path);
		}
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in modify_import_export");
			return None;
		};
		let Some(click_targets) = network_metadata.transient_metadata.modify_import_export.get_loaded_mut() else {
			log::error!("could not load modify import export ports");
			return None;
		};
		Some(click_targets)
	}

	pub fn load_modify_import_export(&self, network_path: &[NodeId]) {
		let mut reorder_imports_exports = Ports::new();
		let mut remove_imports_exports = Ports::new();

		if !network_path.is_empty() {
			let ports_built = self.with_import_export_ports(network_path, |import_exports| {
				for (import_index, import_click_target) in import_exports.output_ports() {
					let Some(import_bounding_box) = import_click_target.bounding_box() else {
						log::error!("Could not get export bounding box in load_modify_import_export");
						continue;
					};
					let reorder_import_center = (import_bounding_box[0] + import_bounding_box[1]) / 2. + DVec2::new(-12., 0.);

					if *import_index == 0 {
						let remove_import_center = reorder_import_center + DVec2::new(-4., 0.);
						let remove_import = ClickTarget::new_with_subpath(Subpath::new_rectangle(remove_import_center - DVec2::new(8., 8.), remove_import_center + DVec2::new(8., 8.)), 0.);
						remove_imports_exports.insert_custom_output_port(*import_index, remove_import);
					} else {
						let remove_import_center = reorder_import_center + DVec2::new(-12., 0.);
						let reorder_import = ClickTarget::new_with_subpath(Subpath::new_rectangle(reorder_import_center - DVec2::new(3., 4.), reorder_import_center + DVec2::new(3., 4.)), 0.);
						let remove_import = ClickTarget::new_with_subpath(Subpath::new_rectangle(remove_import_center - DVec2::new(8., 8.), remove_import_center + DVec2::new(8., 8.)), 0.);
						reorder_imports_exports.insert_custom_output_port(*import_index, reorder_import);
						remove_imports_exports.insert_custom_output_port(*import_index, remove_import);
					}
				}

				for (export_index, export_click_target) in import_exports.input_ports() {
					let Some(export_bounding_box) = export_click_target.bounding_box() else {
						log::error!("Could not get export bounding box in load_modify_import_export");
						continue;
					};
					let reorder_export_center = (export_bounding_box[0] + export_bounding_box[1]) / 2. + DVec2::new(12., 0.);

					if *export_index == 0 {
						let remove_export_center = reorder_export_center + DVec2::new(4., 0.);
						let remove_export = ClickTarget::new_with_subpath(Subpath::new_rectangle(remove_export_center - DVec2::new(8., 8.), remove_export_center + DVec2::new(8., 8.)), 0.);
						remove_imports_exports.insert_custom_input_port(*export_index, remove_export);
					} else {
						let remove_export_center = reorder_export_center + DVec2::new(12., 0.);
						let reorder_export = ClickTarget::new_with_subpath(Subpath::new_rectangle(reorder_export_center - DVec2::new(3., 4.), reorder_export_center + DVec2::new(3., 4.)), 0.);
						let remove_export = ClickTarget::new_with_subpath(Subpath::new_rectangle(remove_export_center - DVec2::new(8., 8.), remove_export_center + DVec2::new(8., 8.)), 0.);
						reorder_imports_exports.insert_custom_input_port(*export_index, reorder_export);
						remove_imports_exports.insert_custom_input_port(*export_index, remove_export);
					}
				}
			});
			if ports_built.is_none() {
				log::error!("Could not get import_export_ports in load_modify_import_export");
				return;
			}
		}

		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get current network in load_modify_import_export");
			return;
		};

		network_metadata.transient_metadata.modify_import_export.store(ModifyImportExportClickTarget {
			remove_imports_exports,
			reorder_imports_exports,
		});
	}

	pub(crate) fn unload_modify_import_export(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_export_ports");
			return;
		};
		network_metadata.transient_metadata.modify_import_export.unload();
	}

	/// Reads the owned nodes of a layer through &self if they are loaded.
	pub(crate) fn with_owned_nodes<R>(&self, node_id: &NodeId, network_path: &[NodeId], read: impl FnOnce(&HashSet<NodeId>) -> R) -> Option<R> {
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

		if !network_metadata.transient_metadata.all_nodes_bounding_box.is_loaded() {
			self.load_all_nodes_bounding_box(network_path);
		}

		let bounding_box = self.network_metadata(network_path)?.transient_metadata.all_nodes_bounding_box.with_loaded(|bounds| *bounds);
		if bounding_box.is_none() {
			log::error!("could not load all nodes bounding box");
		}
		bounding_box
	}

	pub fn load_all_nodes_bounding_box(&self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in load_all_nodes_bounding_box");
			return;
		};
		let nodes = network_metadata.persistent_metadata.node_metadata.keys().copied().collect::<Vec<_>>();

		let all_nodes_bounding_box = nodes
			.iter()
			.filter_map(|node_id| self.node_bounding_box(node_id, network_path))
			.reduce(Quad::combine_bounds)
			.unwrap_or([DVec2::new(0., 0.), DVec2::new(0., 0.)]);

		let Some(network_metadata) = self.network_metadata(network_path) else { return };
		network_metadata.transient_metadata.all_nodes_bounding_box.store(all_nodes_bounding_box);
	}

	pub fn unload_all_nodes_bounding_box(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_all_nodes_bounding_box");
			return;
		};
		network_metadata.transient_metadata.all_nodes_bounding_box.unload();
		self.unload_import_export_ports(network_path);
	}

	pub fn outward_wires(&mut self, network_path: &[NodeId]) -> Option<&HashMap<OutputConnector, Vec<InputConnector>>> {
		self.try_load_outward_wires(network_path);

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in outward_wires");
			return None;
		};
		let Some(outward_wires) = network_metadata.transient_metadata.outward_wires.get_loaded_mut() else {
			log::error!("could not load outward wires");
			return None;
		};

		Some(outward_wires)
	}

	/// Reads the outward wires through &self, loading them first if needed.
	pub(crate) fn with_outward_wires<R>(&self, network_path: &[NodeId], read: impl FnOnce(&HashMap<OutputConnector, Vec<InputConnector>>) -> R) -> Option<R> {
		self.try_load_outward_wires(network_path);
		self.network_metadata(network_path)?.transient_metadata.outward_wires.with_loaded(read)
	}

	fn try_load_outward_wires(&self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in outward_wires");
			return;
		};
		if !network_metadata.transient_metadata.outward_wires.is_loaded() {
			self.load_outward_wires(network_path);
		}
	}

	fn load_outward_wires(&self, network_path: &[NodeId]) {
		let mut outward_wires = HashMap::new();
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in load_outward_wires");
			return;
		};
		// Initialize all output connectors for nodes
		for (node_id, _) in network.nodes.iter() {
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
				log::error!("Output connector {output_connector:?} should be initialized in load_outward_wires");
				Vec::new()
			});
			outward_wires_entry.push(input_connector);
		};
		for (current_node_id, node) in network.nodes.iter() {
			for (input_index, input) in node.inputs.iter().enumerate() {
				if let NodeInput::Node { node_id, output_index, .. } = input {
					push_outward_wire(&mut outward_wires, OutputConnector::node(*node_id, *output_index), InputConnector::node(*current_node_id, input_index));
				} else if let NodeInput::Import { import_index, .. } = input {
					push_outward_wire(&mut outward_wires, OutputConnector::Import(*import_index), InputConnector::node(*current_node_id, input_index));
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

		let Some(network_metadata) = self.network_metadata(network_path) else { return };

		network_metadata.transient_metadata.outward_wires.store(outward_wires);
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
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			return;
		};
		let Some(outward_wires) = network_metadata.transient_metadata.outward_wires.get_loaded_mut() else {
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

		if !node_metadata.transient_metadata.layer_width.is_loaded() {
			self.load_layer_width(node_id, network_path);
		}

		let node_metadata = self.node_metadata(node_id, network_path)?;
		node_metadata.transient_metadata.layer_width.with_loaded(|layer_width| *layer_width)
	}

	pub fn load_layer_width(&self, node_id: &NodeId, network_path: &[NodeId]) {
		const GAP_WIDTH: f64 = 8.;
		const FONT_SIZE: f64 = 14.;
		let left_thumbnail_padding = GRID_SIZE as f64 / 2.;
		let thumbnail_width = 3. * GRID_SIZE as f64;
		let layer_text = self.display_name(node_id, network_path);

		let text_width = text_width(&layer_text, FONT_SIZE);

		let grip_padding = 4.;
		let grip_width = 8.;
		let lock_icon_width = if self.is_locked(node_id, network_path) { GRID_SIZE as f64 } else { 0. };
		let icon_overhang_width = GRID_SIZE as f64 / 2.;

		let layer_width_pixels = left_thumbnail_padding + thumbnail_width + GAP_WIDTH + text_width + grip_padding + grip_width + lock_icon_width + icon_overhang_width;
		let layer_width = ((layer_width_pixels / 24.).ceil() as u32).max(8);

		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in load_layer_width");
			return;
		};

		// Ensure layer width is not loaded for a non layer node
		if node_metadata.persistent_metadata.is_layer() {
			node_metadata.transient_metadata.layer_width.store(layer_width);
		} else {
			log::warn!("Tried loading layer width for non layer node");
		}
	}

	/// Unloads layer width if the node is a layer
	pub fn try_unload_layer_width(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let is_layer = self.is_layer(node_id, network_path);

		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			return;
		};

		// If the node is a layer, then the width and click targets need to be recalculated
		if is_layer {
			node_metadata.transient_metadata.layer_width.unload();
		}
	}

	pub fn get_input_center(&self, input: &InputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		fn port_center(ports: &Ports, index: usize) -> Option<DVec2> {
			ports
				.input_ports
				.iter()
				.find_map(|(input_index, click_target)| if index == *input_index { click_target.bounding_box_center() } else { None })
		}

		match input {
			InputConnector::Node { node_id, input_index } => {
				self.try_load_node_click_targets(node_id, network_path);
				self.with_node_click_targets(node_id, network_path, |click_targets| port_center(&click_targets.port_click_targets, *input_index))
					.flatten()
			}
			InputConnector::Export(export_index) => self.with_import_export_ports(network_path, |ports| port_center(ports, *export_index)).flatten(),
		}
	}

	pub fn get_output_center(&self, output: &OutputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		fn port_center(ports: &Ports, index: usize) -> Option<DVec2> {
			ports
				.output_ports
				.iter()
				.find_map(|(output_index, click_target)| if index == *output_index { click_target.bounding_box_center() } else { None })
		}

		match output {
			OutputConnector::Node { node_id, output_index } => {
				self.try_load_node_click_targets(node_id, network_path);
				self.with_node_click_targets(node_id, network_path, |click_targets| port_center(&click_targets.port_click_targets, *output_index))
					.flatten()
			}
			OutputConnector::Import(import_index) => self.with_import_export_ports(network_path, |ports| port_center(ports, *import_index)).flatten(),
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
		let dashed = match self.previewing(network_path) {
			Previewing::Yes { .. } => match input {
				InputConnector::Node { .. } => false,
				InputConnector::Export(export_index) => *export_index == 0,
			},
			Previewing::No => false,
		};
		let Some(wire) = self.wire_path_from_input(input, graph_wire_style, dashed, network_path) else {
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
				input_connectors.push(InputConnector::node(*node_id, input_index));
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
			log::error!("Could not get outward wires in reorder_export");
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
			input_connectors.push(InputConnector::node(*node_id, input_index));
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

	/// When previewing, there may be a second path to the root node.
	pub fn wire_to_root(&self, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<WirePathUpdate> {
		let input = InputConnector::Export(0);
		let current_export = self.upstream_output_connector(&input, network_path)?;

		let root_node = match self.previewing(network_path) {
			Previewing::Yes { root_node_to_restore } => root_node_to_restore,
			Previewing::No => None,
		}?;

		if Some(root_node.node_id) == current_export.node_id() {
			return None;
		}
		let Some(input_position) = self.get_input_center(&input, network_path) else {
			log::error!("Could not get input position for wire end in root node: {input:?}");
			return None;
		};
		let upstream_output = OutputConnector::node(root_node.node_id, root_node.output_index);
		let Some(output_position) = self.get_output_center(&upstream_output, network_path) else {
			log::error!("Could not get output position for wire start in root node: {upstream_output:?}");
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
			dashed: false,
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
			log::error!("Could not get output port for wire start: {:?}", upstream_output);
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
		self.try_load_node_click_targets(node_id, network_path);

		let node_metadata = self.node_metadata_mut(node_id, network_path)?;
		let Some(click_targets) = node_metadata.transient_metadata.click_targets.get_loaded_mut() else {
			log::error!("Could not load node type metadata when getting click targets");
			return None;
		};
		Some(click_targets)
	}

	pub(crate) fn try_load_node_click_targets(&self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in node_click_targets");
			return;
		};
		if !node_metadata.transient_metadata.click_targets.is_loaded() {
			self.load_node_click_targets(node_id, network_path)
		};
	}

	/// Loads the node click targets if needed, then reads them through &self.
	pub(crate) fn with_loaded_node_click_targets<R>(&self, node_id: &NodeId, network_path: &[NodeId], read: impl FnOnce(&DocumentNodeClickTargets) -> R) -> Option<R> {
		self.try_load_node_click_targets(node_id, network_path);
		self.with_node_click_targets(node_id, network_path, read)
	}

	/// Reads the modify import/export click targets through &self, loading them first if needed.
	pub(crate) fn with_modify_import_export<R>(&self, network_path: &[NodeId], read: impl FnOnce(&ModifyImportExportClickTarget) -> R) -> Option<R> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in modify_import_export");
			return None;
		};
		if !network_metadata.transient_metadata.modify_import_export.is_loaded() {
			self.load_modify_import_export(network_path);
		}
		self.network_metadata(network_path)?.transient_metadata.modify_import_export.with_loaded(read)
	}

	/// Reads the node click targets through &self if they are already loaded.
	pub(crate) fn with_node_click_targets<R>(&self, node_id: &NodeId, network_path: &[NodeId], read: impl FnOnce(&DocumentNodeClickTargets) -> R) -> Option<R> {
		let node_metadata = self.node_metadata(node_id, network_path)?;
		let result = node_metadata.transient_metadata.click_targets.with_loaded(read);
		if result.is_none() {
			log::error!("Could not load node type metadata when getting click targets");
		}
		result
	}

	pub fn load_node_click_targets(&self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(node_position) = self.position_from_downstream_node(node_id, network_path) else {
			log::error!("Could not get node position in load_node_click_targets for node {node_id}");
			return;
		};
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in load_node_click_targets");
			return;
		};
		let Some(document_node) = self.document_node(node_id, network_path) else {
			log::error!("Could not get document node in load_node_click_targets");
			return;
		};

		let node_top_left = node_position.as_dvec2() * GRID_SIZE as f64;
		let mut port_click_targets = Ports::new();
		let document_node_click_targets = if !node_metadata.persistent_metadata.is_layer() {
			// Create input/output click targets
			let mut input_row_count = 0;
			for (input_index, input) in document_node.inputs.iter().enumerate() {
				if input.is_exposed() {
					port_click_targets.insert_node_input(input_index, input_row_count, node_top_left);
				}
				// Primary input row is always displayed, even if the input is not exposed
				if input_index == 0 || input.is_exposed() {
					input_row_count += 1;
				}
			}

			let number_of_outputs = match &document_node.implementation {
				DocumentNodeImplementation::Network(network) => network.exports.len(),
				_ => 1,
			};
			// If the node has a hidden primary output, do not display the first output
			let start_index = if self.hidden_primary_output(node_id, network_path) { 1 } else { 0 };
			for output_index in start_index..number_of_outputs {
				port_click_targets.insert_node_output(output_index, node_top_left);
			}

			let height = input_row_count.max(number_of_outputs).max(1) as u32 * GRID_SIZE;
			let width = 5 * GRID_SIZE;
			// Offset down by half a grid so the click target sits below the top connector strip.
			let node_click_target_top_left = node_top_left + DVec2::new(0., HALF_GRID_SIZE as f64);
			let node_click_target_bottom_right = node_click_target_top_left + DVec2::new(width as f64, height as f64);

			let radius = 3.;
			let subpath = Subpath::new_rounded_rectangle(node_click_target_top_left, node_click_target_bottom_right, [radius; 4]);
			let node_click_target = ClickTarget::new_with_subpath(subpath, 0.);

			DocumentNodeClickTargets {
				node_click_target,
				port_click_targets,
				node_type_metadata: NodeTypeClickTargets::Node,
			}
		} else {
			// Layer inputs
			port_click_targets.insert_layer_input(0, node_top_left);
			if document_node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
				port_click_targets.insert_layer_input(1, node_top_left);
			}
			port_click_targets.insert_layer_output(node_top_left);

			let layer_width_cells = self.layer_width(node_id, network_path).unwrap_or_else(|| {
				log::error!("Could not get layer width in load_node_click_targets");
				0
			});
			let width = layer_width_cells * GRID_SIZE;
			let height = 2 * GRID_SIZE;
			let locked = self.is_locked(node_id, network_path);

			// The layer is `2 * GRID_SIZE` tall, so its vertical center sits one grid unit below `node_top_left.y`.
			// Visibility/lock buttons fill a 1-grid-cell square (so half-extents of HALF_GRID_SIZE each side of center).
			const LAYER_VERTICAL_CENTER: f64 = GRID_SIZE as f64;
			const ICON_HALF_EXTENT: f64 = HALF_GRID_SIZE as f64;

			// Update visibility button click target
			let visibility_offset = node_top_left + DVec2::new(width as f64, LAYER_VERTICAL_CENTER);
			let subpath = Subpath::new_rounded_rectangle(
				DVec2::new(-ICON_HALF_EXTENT, -ICON_HALF_EXTENT) + visibility_offset,
				DVec2::new(ICON_HALF_EXTENT, ICON_HALF_EXTENT) + visibility_offset,
				[3.; 4],
			);
			let visibility_click_target = ClickTarget::new_with_subpath(subpath, 0.);

			// Update lock button click target, positioned one grid unit to the left of the visibility button (only when locked)
			let lock_click_target = if locked {
				let lock_offset = node_top_left + DVec2::new(width as f64 - GRID_SIZE as f64, LAYER_VERTICAL_CENTER);
				let subpath = Subpath::new_rounded_rectangle(
					DVec2::new(-ICON_HALF_EXTENT, -ICON_HALF_EXTENT) + lock_offset,
					DVec2::new(ICON_HALF_EXTENT, ICON_HALF_EXTENT) + lock_offset,
					[3.; 4],
				);
				Some(ClickTarget::new_with_subpath(subpath, 0.))
			} else {
				None
			};

			// Update grip button click target, which is positioned to the left of the leftmost icon.
			// The grip is 8px wide but spans the full layer-vertical-center band.
			const GRIP_WIDTH: f64 = 8.;
			let icons_width = if locked { GRID_SIZE as f64 } else { 0. };
			let grip_offset_right_edge = node_top_left + DVec2::new(width as f64 - ICON_HALF_EXTENT - icons_width, LAYER_VERTICAL_CENTER);
			let subpath = Subpath::new_rounded_rectangle(
				DVec2::new(-GRIP_WIDTH, -ICON_HALF_EXTENT) + grip_offset_right_edge,
				DVec2::new(0., ICON_HALF_EXTENT) + grip_offset_right_edge,
				[0.; 4],
			);
			let grip_click_target = ClickTarget::new_with_subpath(subpath, 0.);

			// Update display-name text click target, used to detect double-click rename. Sized to the text bounds
			// (not the surrounding `.details` area) so the rest of the layer still drills into the subgraph on double-click.

			/// `.layer` margin-left (= 12), for chain layers the negative margin-left and positive padding-left cancel out, keeping content at this same offset
			const LAYER_LEFT_MARGIN: f64 = HALF_GRID_SIZE as f64;
			/// `.thumbnail` (70px) + its 1px side margins (= 72)
			const THUMBNAIL_BLOCK_WIDTH: f64 = 3. * GRID_SIZE as f64;
			/// `.details` margin-left
			const DETAILS_LEFT_MARGIN: f64 = 8.;
			const NAME_LEFT_OFFSET: f64 = LAYER_LEFT_MARGIN + THUMBNAIL_BLOCK_WIDTH + DETAILS_LEFT_MARGIN;
			/// Distance from layer's right edge to visibility's left edge (= 12)
			const VISIBILITY_INSET_FROM_LAYER_RIGHT: f64 = HALF_GRID_SIZE as f64;
			const FONT_SIZE: f64 = 14.;

			let display_name = self.display_name(node_id, network_path);
			let name_click_target = if display_name.is_empty() {
				None
			} else {
				let name_left = node_top_left.x + NAME_LEFT_OFFSET;
				let icons_reserve = VISIBILITY_INSET_FROM_LAYER_RIGHT + icons_width + GRIP_WIDTH;
				let name_right_max = node_top_left.x + width as f64 - icons_reserve;
				let text_w = text_width(&display_name, FONT_SIZE);
				let name_right = (name_left + text_w).min(name_right_max);
				if name_right > name_left {
					// The 1-grid-tall name strip is centered vertically in the 2-grid-tall layer.
					let name_top = node_top_left.y + HALF_GRID_SIZE as f64;
					let name_bottom = node_top_left.y + GRID_SIZE as f64 + HALF_GRID_SIZE as f64;
					let subpath = Subpath::new_rounded_rectangle(DVec2::new(name_left, name_top), DVec2::new(name_right, name_bottom), [3.; 4]);
					Some(ClickTarget::new_with_subpath(subpath, 0.))
				} else {
					None
				}
			};

			// Create layer click target, which is contains the layer and the chain background
			let chain_width_grid_spaces = self.chain_width(node_id, network_path);

			let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
			let chain_top_left = node_top_left - DVec2::new((chain_width_grid_spaces * GRID_SIZE) as f64, 0.);
			const CORNER_RADIUS: f64 = 10.;
			let subpath = Subpath::new_rounded_rectangle(chain_top_left, node_bottom_right, [CORNER_RADIUS; 4]);
			let node_click_target = ClickTarget::new_with_subpath(subpath, 0.);

			DocumentNodeClickTargets {
				node_click_target,
				port_click_targets,
				node_type_metadata: NodeTypeClickTargets::Layer(LayerClickTargets {
					visibility_click_target,
					lock_click_target,
					grip_click_target,
					name_click_target,
				}),
			}
		};

		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in load_node_click_targets");
			return;
		};
		node_metadata.transient_metadata.click_targets.store(document_node_click_targets);
	}

	pub fn node_bounding_box(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		self.try_load_node_click_targets(node_id, network_path);
		self.try_get_node_bounding_box(node_id, network_path)
	}

	pub fn try_get_node_bounding_box(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		self.with_node_click_targets(node_id, network_path, |click_targets| click_targets.node_click_target.bounding_box())
			.flatten()
	}

	pub fn try_load_all_node_click_targets(&self, network_path: &[NodeId]) {
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get network in load_all_node_click_targets");
			return;
		};
		for node_id in network.nodes.keys().cloned().collect::<Vec<_>>() {
			self.try_load_node_click_targets(&node_id, network_path);
		}
	}

	/// Get the top left position in node graph coordinates for a node by recursively iterating downstream through cached positions, which means the iteration can be broken once a known position is reached.
	pub fn position_from_downstream_node(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<IVec2> {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in position_from_downstream_node");
			return None;
		};
		match &node_metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => {
				match layer_metadata.position {
					LayerPosition::Absolute(position) => Some(position),
					LayerPosition::Stack(y_offset) => {
						let Some(downstream_node_connectors) = self
							.with_outward_wires(network_path, |outward_wires| outward_wires.get(&OutputConnector::node(*node_id, 0)).cloned())
							.flatten()
						else {
							log::error!("Could not get downstream node in position_from_downstream_node");
							return None;
						};
						let downstream_connector = downstream_node_connectors
							.iter()
							.find_map(|input_connector| input_connector.node_id().map(|node_id| (node_id, input_connector.input_index())));

						let Some((downstream_node_id, _)) = downstream_connector else {
							log::error!("Could not get downstream node input connector for node {node_id}");
							return None;
						};
						// Get the height of the node to ensure nodes do not overlap
						let Some(downstream_node_height) = self.height_from_click_target(&downstream_node_id, network_path) else {
							log::error!("Could not get click target height in position_from_downstream_node");
							return None;
						};
						self.position(&downstream_node_id, network_path)
							.map(|position| position + IVec2::new(0, 1 + downstream_node_height as i32 + y_offset as i32))
					}
				}
			}
			NodeTypePersistentMetadata::Node(node_metadata) => {
				match node_metadata.position {
					NodePosition::Absolute(position) => Some(position),
					NodePosition::Chain => {
						// Iterate through primary flow to find the first Layer
						let mut current_node_id = *node_id;
						let mut node_distance_from_layer = 1;
						loop {
							// TODO: Use root node to restore if previewing
							let Some(downstream_node_connectors) = self
								.with_outward_wires(network_path, |outward_wires| outward_wires.get(&OutputConnector::node(current_node_id, 0)).cloned())
								.flatten()
							else {
								log::error!("Could not get downstream node for node {node_id} with Position::Chain");
								return None;
							};
							let Some(downstream_node_id) = downstream_node_connectors.iter().find_map(|input_connector| {
								if let InputConnector::Node { node_id, input_index } = input_connector {
									let downstream_input_index = if self.is_layer(node_id, network_path) { 1 } else { 0 };
									if *input_index == downstream_input_index { Some(node_id) } else { None }
								} else {
									None
								}
							}) else {
								log::error!("Could not get downstream node input connector with input index 1 for node with Position::Chain");
								return None;
							};
							let Some(downstream_node_metadata) = self.network_metadata(network_path)?.persistent_metadata.node_metadata.get(downstream_node_id) else {
								log::error!("Downstream node metadata not found in node_metadata for node with Position::Chain");
								return None;
							};
							if downstream_node_metadata.persistent_metadata.is_layer() {
								// Get the position of the layer
								let layer_position = self.position(downstream_node_id, network_path)?;
								return Some(layer_position + IVec2::new(-node_distance_from_layer * NODE_CHAIN_WIDTH, 0));
							}
							node_distance_from_layer += 1;
							current_node_id = *downstream_node_id;
						}
					}
				}
			}
		}
	}

	pub fn unload_node_click_targets(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in unload_node_click_target");
			return;
		};
		node_metadata.transient_metadata.click_targets.unload();
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
