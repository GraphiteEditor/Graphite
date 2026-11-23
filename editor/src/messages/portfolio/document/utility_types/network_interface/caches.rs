use super::*;

// Public mutable getters for data that involves transient network metadata
// Mutable methods never recalculate the transient metadata, they only unload it. Loading metadata should only be done by the getter.
impl NodeNetworkInterface {
	pub fn start_transaction(&mut self) {
		self.transaction_status = TransactionStatus::Started;
		self.clear_transaction_edited_nodes();
		self.transaction_edited_nodes_scanned = self.deltas.len();
	}

	pub fn transaction_modified(&mut self) {
		if self.transaction_status == TransactionStatus::Started {
			self.transaction_status = TransactionStatus::Modified;
		}
	}

	pub fn finish_transaction(&mut self) {
		self.transaction_status = TransactionStatus::Finished;
		self.clear_transaction_edited_nodes();
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

	pub fn glide_offsets(&self, network_path: &[NodeId]) -> GlideOffsets {
		self.network_metadata(network_path)
			.map(|network_metadata| network_metadata.transient_metadata.glide_offsets.borrow().clone())
			.unwrap_or_default()
	}

	pub fn set_glide_offsets(&self, glide_offsets: GlideOffsets, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in set_glide_offsets");
			return;
		};
		*network_metadata.transient_metadata.glide_offsets.borrow_mut() = glide_offsets;
	}

	/// Reads the import/export ports through &self, building them again first if the layout they were placed by has changed.
	pub(crate) fn with_import_export_ports<R>(&self, network_path: &[NodeId], read: impl FnOnce(&Ports) -> R) -> Option<R> {
		let generation = self.with_layout(network_path, NetworkLayout::generation)?;
		let cache = &self.network_metadata(network_path)?.transient_metadata.import_export_ports;
		if cache.with_loaded(|(placed_by, _)| *placed_by != generation) == Some(true) {
			cache.unload();
		}

		cache.with_loaded_or(|| Some((generation, self.compute_import_export_ports(network_path)?)), |(_, ports)| read(ports))
	}

	pub(crate) fn unload_import_export_ports(&mut self, network_path: &[NodeId]) {
		let Some(transient) = self.network_transient_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_import_export_ports");
			return;
		};
		transient.import_export_ports.unload();
	}

	/// The combined bounds of every node in the network, in node graph space.
	pub fn all_nodes_bounding_box(&self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		self.with_layout(network_path, NetworkLayout::bounds)
	}

	pub(crate) fn import_export_clearance(&self, network_path: &[NodeId]) -> Option<ImportExportClearance> {
		self.with_layout(network_path, NetworkLayout::clearance)
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
	pub(super) fn compute_outward_wires(&self, network_path: &[NodeId]) -> Option<HashMap<OutputConnector, Vec<InputConnector>>> {
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
		let drained = {
			let Some(transient) = self.network_transient_mut(network_path) else { return };
			let Some(outward_wires) = transient.outward_wires.get_loaded_mut() else { return };

			unhook_outward_wire(outward_wires, input_connector, old_input);
			hook_outward_wire(outward_wires, input_connector, new_input);

			drained_outward_wire(outward_wires, old_input)
		};

		if let Some(output) = drained {
			self.delist_vacated_output(network_path, output);
		}
	}

	/// The same update for a node whose whole input list was swapped.
	pub(crate) fn update_outward_wires_for_inputs(&mut self, network_path: &[NodeId], node_id: &NodeId, old_inputs: &[NodeInput], new_inputs: &[NodeInput]) {
		let drained = {
			let Some(transient) = self.network_transient_mut(network_path) else { return };
			let Some(outward_wires) = transient.outward_wires.get_loaded_mut() else { return };

			for (input_index, old_input) in old_inputs.iter().enumerate() {
				unhook_outward_wire(outward_wires, &InputConnector::node_at_index(*node_id, input_index), old_input);
			}
			for (input_index, new_input) in new_inputs.iter().enumerate() {
				hook_outward_wire(outward_wires, &InputConnector::node_at_index(*node_id, input_index), new_input);
			}

			old_inputs.iter().filter_map(|old_input| drained_outward_wire(outward_wires, old_input)).collect::<Vec<_>>()
		};

		for output in drained {
			self.delist_vacated_output(network_path, output);
		}
	}

	/// Drops an emptied entry for an output the network no longer has, which a fresh computation keys only while a wire still reaches it.
	fn delist_vacated_output(&mut self, network_path: &[NodeId], output: OutputConnector) {
		let still_exists = match output {
			OutputConnector::Node { node_id, output_index } => output_index < self.number_of_outputs(&node_id, network_path),
			OutputConnector::Import(import_index) => import_index < self.number_of_imports(network_path),
		};
		if still_exists {
			return;
		}

		let Some(transient) = self.network_transient_mut(network_path) else { return };
		let Some(outward_wires) = transient.outward_wires.get_loaded_mut() else { return };

		outward_wires.remove(&output);
	}

	/// Lists or delists a node's outputs after their count changed, since every output is keyed even with no consumers.
	pub(crate) fn update_outward_wires_for_outputs(&mut self, network_path: &[NodeId], node_id: &NodeId, old_output_count: usize, new_output_count: usize) {
		let Some(transient) = self.network_transient_mut(network_path) else { return };
		let Some(outward_wires) = transient.outward_wires.get_loaded_mut() else { return };

		// A consumer still wired to a vanished output keeps the entry, as a fresh computation lists it too
		for output_index in new_output_count..old_output_count {
			let output = OutputConnector::node(*node_id, output_index);
			if outward_wires.get(&output).is_some_and(Vec::is_empty) {
				outward_wires.remove(&output);
			}
		}
		for output_index in old_output_count..new_output_count {
			outward_wires.entry(OutputConnector::node(*node_id, output_index)).or_default();
		}
	}

	pub fn layer_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<u32> {
		self.layer_width_and_name_width(node_id, network_path).map(|(layer_width, _)| layer_width)
	}

	pub(super) fn layer_width_and_name_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<(u32, f64)> {
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
			.with_loaded_or(|| Some(self.compute_layer_width(node_id, network_path)), |widths| *widths)
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
		self.input_position(input, network_path)
	}

	pub fn get_output_center(&self, output: &OutputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		self.output_position(output, network_path)
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

	/// The dashed wire from the previewed node to the export, drawn alongside the export's own wire.
	pub fn vector_wire_to_preview(&self, graph_wire_style: GraphWireStyle, glide_offsets: &GlideOffsets, network_path: &[NodeId]) -> Option<(OutputConnector, BezPath, BezPath, bool)> {
		let input = InputConnector::Export(0);

		let Previewing::Yes { previewed } = self.previewing(network_path) else { return None };

		if self.upstream_output_connector(&input, network_path) == Some(previewed.to_connector()) {
			return None;
		}
		let Some(input_position) = self.get_input_center(&input, network_path) else {
			log::error!("Could not get input position for wire end in preview: {input:?}");
			return None;
		};
		let input_position = input_position + glide_offsets.input(&input);
		let upstream_output = OutputConnector::node(previewed.node_id, previewed.output_index);
		let Some(output_position) = self.get_output_center(&upstream_output, network_path) else {
			log::error!("Could not get output position for wire start in preview: {upstream_output:?}");
			return None;
		};
		let output_position = output_position + glide_offsets.output(&upstream_output);
		let vertical_end = input.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path) && input.input_index() == 0);
		let vertical_start: bool = upstream_output.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path));
		let thick = vertical_end && vertical_start;
		let vector_wire = build_vector_wire(output_position, input_position, vertical_start, vertical_end, graph_wire_style);
		let center_line = build_thick_wire_center_line(output_position, input_position, vertical_start, vertical_end);
		Some((upstream_output, vector_wire, center_line, thick))
	}

	/// Returns the wire subpath, its thick center-line subpath, and whether the wire should be thick, or None if nothing feeds the input.
	pub fn vector_wire_from_input(&self, input: &InputConnector, wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<(BezPath, BezPath, bool)> {
		self.gliding_vector_wire_from_input(input, wire_style, &GlideOffsets::default(), network_path)
	}

	/// Like [`Self::vector_wire_from_input`], but with its ends following anything drawn gliding into place.
	pub(crate) fn gliding_vector_wire_from_input(&self, input: &InputConnector, wire_style: GraphWireStyle, glide_offsets: &GlideOffsets, network_path: &[NodeId]) -> Option<(BezPath, BezPath, bool)> {
		let Some(input_position) = self.get_input_center(input, network_path) else {
			log::error!("Could not get dom rect for wire end: {input:?}");
			return None;
		};
		let input_position = input_position + glide_offsets.input(input);
		let upstream_output = self.upstream_output_connector(input, network_path)?;
		let Some(output_position) = self.get_output_center(&upstream_output, network_path) else {
			log::error!("Could not get output port for wire start: {upstream_output:?}");
			return None;
		};
		let output_position = output_position + glide_offsets.output(&upstream_output);
		let vertical_end = input.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path) && input.input_index() == 0);
		let vertical_start = upstream_output.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path));
		let thick = vertical_end && vertical_start;
		let vector_wire = build_vector_wire(output_position, input_position, vertical_start, vertical_end, wire_style);
		let center_line = build_thick_wire_center_line(output_position, input_position, vertical_start, vertical_end);
		Some((vector_wire, center_line, thick))
	}

	/// The node's body, ports, and handles, built where it is drawn now.
	/// The box around the node's body where it sits in its grid cell, in node graph space.
	pub fn node_bounding_box(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let body = self.with_layout(network_path, |layout| layout.body(node_id)).flatten()?;
		Some([body.min, body.max])
	}
}

/// Drops `input_connector` from the consumers of the output that `input` was wired to.
fn unhook_outward_wire(outward_wires: &mut HashMap<OutputConnector, Vec<InputConnector>>, input_connector: &InputConnector, input: &NodeInput) {
	if let Some(output) = OutputConnector::from_input(input)
		&& let Some(connections) = outward_wires.get_mut(&output)
	{
		connections.retain(|connection| connection != input_connector);
	}
}

/// Adds `input_connector` to the consumers of the output that `input` is wired to.
fn hook_outward_wire(outward_wires: &mut HashMap<OutputConnector, Vec<InputConnector>>, input_connector: &InputConnector, input: &NodeInput) {
	if let Some(output) = OutputConnector::from_input(input) {
		outward_wires.entry(output).or_default().push(*input_connector);
	}
}

/// The output `input` was wired to, once it has lost its last consumer.
fn drained_outward_wire(outward_wires: &HashMap<OutputConnector, Vec<InputConnector>>, input: &NodeInput) -> Option<OutputConnector> {
	OutputConnector::from_input(input).filter(|output| outward_wires.get(output).is_some_and(Vec::is_empty))
}
