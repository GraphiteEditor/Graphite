use super::*;

// Public mutable methods
impl NodeNetworkInterface {
	pub fn copy_all_navigation_metadata(&mut self, other_interface: &NodeNetworkInterface) {
		self.for_each_network_view_state_mut(|path, view| {
			if let Some(other_network_metadata) = other_interface.network_metadata(path) {
				*view.navigation = other_network_metadata.persistent_metadata.navigation_metadata.clone();
			}
		});
	}

	/// Copy all transient view and selection state from `other_interface` onto `self` at every nesting level:
	/// navigation metadata (pan/zoom), selection undo/redo history, and the document-to-viewport camera. Lets
	/// an interface rebuilt from storage keep the user's current view rather than reset it.
	pub fn copy_all_transient_view_state(&mut self, other_interface: &NodeNetworkInterface) {
		self.for_each_network_view_state_mut(|path, view| {
			if let Some(other_network_metadata) = other_interface.network_metadata(path) {
				let other_persistent = &other_network_metadata.persistent_metadata;
				*view.navigation = other_persistent.navigation_metadata.clone();

				// The preview is this peer's and survives a rebuild, unless it names a node the rebuilt
				// network no longer contains (e.g. after a storage undo), in which case there is nothing
				// left to look at.
				*view.previewing = match other_persistent.previewing {
					Previewing::Yes { previewed } if !view.nodes.contains_key(&previewed.node_id) => Previewing::No,
					previewing => previewing,
				};

				*view.selection.undo = other_persistent.selection_undo_history.clone();
				*view.selection.redo = other_persistent.selection_redo_history.clone();
			}
		});

		self.document_metadata.document_to_viewport = other_interface.document_metadata.document_to_viewport;
	}

	pub fn set_transform(&mut self, transform: DAffine2, network_path: &[NodeId]) {
		let Some(mut network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_transform");
			return;
		};
		network.set_navigation_transform(transform);

		self.invalidate_import_export(network_path);
	}

	// This should be run whenever the pan ends, a zoom occurs, or the network is opened
	pub fn set_node_graph_width(&mut self, node_graph_width: f64, network_path: &[NodeId]) {
		let Some(mut network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_node_graph_width");
			return;
		};
		network.set_navigation_width(node_graph_width);

		self.invalidate_import_export(network_path);
	}

	pub fn vector_modify(&mut self, node_id: &NodeId, modification_type: VectorModificationType) {
		let mut modified = false;
		self.edit_input_value(&InputConnector::node_at_index(*node_id, 1), &[], |value| {
			let TaggedValue::VectorModification(modification) = value else {
				log::error!("Path node {node_id} does not have a modification input");
				return;
			};
			modification.modify(&modification_type);
			modified = true;
		});

		if modified {
			self.transaction_modified();
		}
	}

	/// Inserts a new export at insert index. If the insert index is -1 it is inserted at the end. The output_name is used by the encapsulating node.
	pub fn add_export(&mut self, default_value: TaggedValue, insert_index: isize, output_name: &str, network_path: &[NodeId]) {
		let inserted_index = if insert_index == -1 { self.number_of_exports(network_path) } else { insert_index as usize };
		if !self.insert_export_slot(network_path, inserted_index, NodeInput::value(default_value, true), output_name.to_string()) {
			return;
		}
		self.clear_encapsulating_reference(network_path);

		self.transaction_modified();

		let mut encapsulating_path = network_path.to_vec();
		// Set the parent node (if it exists) to be a non layer if it is no longer eligible to be a layer
		if let Some(parent_id) = encapsulating_path.pop()
			&& !self.is_eligible_to_be_layer(&parent_id, &encapsulating_path)
			&& self.is_layer(&parent_id, &encapsulating_path)
		{
			self.set_to_node_or_layer(&parent_id, &encapsulating_path, false);
		};

		// Update the export ports and outward wires for the current network
		self.invalidate_import_export(network_path);
		self.unload_outward_wires(network_path);

		// Update the outward wires and bounding box for all nodes in the encapsulating network
		if let Some(encapsulating_transient) = self.encapsulating_network_transient_mut(network_path) {
			encapsulating_transient.outward_wires.unload();
			encapsulating_transient.all_nodes_bounding_box.unload();
		}

		// Update the click targets for the encapsulating node, if it exists. There is no encapsulating node if the network is the document network
		let mut path = network_path.to_vec();
		if let Some(encapsulating_node) = path.pop() {
			self.unload_node_click_targets(&encapsulating_node, &path);
		}

		// If the export is inserted as the first input or second input, and the parent network is the document_network, then it may have affected the document metadata structure
		if network_path.len() == 1 && inserted_index <= 1 {
			self.load_structure();
		}
	}

	/// Inserts a new input at insert index. If the insert index is -1 it is inserted at the end. The input_name is used by the encapsulating node.
	pub fn add_import(&mut self, default_value: TaggedValue, exposed: bool, insert_index: isize, input_name: &str, input_description: &str, network_path: &[NodeId]) {
		let mut encapsulating_network_path = network_path.to_vec();
		let Some(node_id) = encapsulating_network_path.pop() else {
			log::error!("Cannot add import for document network");
			return;
		};

		let locator = NodeLocator::new(node_id, &encapsulating_network_path);
		let inserted_index = if insert_index == -1 {
			self.number_of_inputs(&node_id, &encapsulating_network_path)
		} else {
			insert_index as usize
		};
		if !self.insert_input_slot(locator, inserted_index, NodeInput::value(default_value, exposed), (input_name, input_description).into()) {
			return;
		}
		self.clear_encapsulating_reference(network_path);

		self.transaction_modified();

		// Set the node to be a non layer if it is no longer eligible to be a layer
		if !self.is_eligible_to_be_layer(&node_id, &encapsulating_network_path) && self.is_layer(&node_id, &encapsulating_network_path) {
			self.set_to_node_or_layer(&node_id, &encapsulating_network_path, false);
		}

		// Update the metadata for the encapsulating node
		self.unload_node_click_targets(&node_id, &encapsulating_network_path);
		self.unload_all_nodes_bounding_box(&encapsulating_network_path);
		if encapsulating_network_path.is_empty() && inserted_index <= 1 {
			self.load_structure();
		}

		// Unload the metadata for the nested network
		self.unload_outward_wires(network_path);
		self.invalidate_import_export(network_path);
	}

	/// Disconnects every wire fed by the given import within the network. Returns false without mutating if the import's wires cannot be resolved.
	pub(crate) fn disconnect_import_wires(&mut self, import_index: usize, network_path: &[NodeId]) -> bool {
		self.disconnect_output_wires(&OutputConnector::Import(import_index), network_path)
	}

	/// Disconnects every wire fed by the given output within the network. Returns false without mutating if the output's wires cannot be resolved.
	pub(crate) fn disconnect_output_wires(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> bool {
		let Some(downstream_connections) = self.with_outward_wires(network_path, |outward_wires| outward_wires.get(output_connector).cloned()) else {
			log::error!("Could not get outward wires in disconnect_output_wires");
			return false;
		};
		let Some(downstream_connections) = downstream_connections else {
			log::error!("Could not get downstream connections for {output_connector:?} in disconnect_output_wires");
			return false;
		};
		for downstream_connection in downstream_connections {
			self.disconnect_input(&downstream_connection, network_path);
		}

		true
	}

	/// Refreshes the metadata invalidated when the encapsulating node's signature changes, demoting it from a layer if it is no longer eligible.
	fn finish_signature_edit(&mut self, parent_id: NodeId, encapsulating_network_path: &[NodeId], network_path: &[NodeId]) {
		// Update the metadata for the encapsulating node
		self.unload_outward_wires(encapsulating_network_path);
		self.unload_node_click_targets(&parent_id, encapsulating_network_path);
		self.unload_all_nodes_bounding_box(encapsulating_network_path);
		if !self.is_eligible_to_be_layer(&parent_id, encapsulating_network_path) && self.is_layer(&parent_id, encapsulating_network_path) {
			self.set_to_node_or_layer(&parent_id, encapsulating_network_path, false);
		}
		if encapsulating_network_path.is_empty() {
			self.load_structure();
		}

		// Unload the metadata for the nested network
		self.unload_outward_wires(network_path);
		self.invalidate_import_export(network_path);
	}

	// First disconnects the export, then removes it
	pub fn remove_export(&mut self, export_index: usize, network_path: &[NodeId]) {
		let mut encapsulating_network_path = network_path.to_vec();
		let Some(parent_id) = encapsulating_network_path.pop() else {
			log::error!("Cannot remove export for document network");
			return;
		};

		// Disconnect the removed export, and handle connections to the node which had its output removed
		self.disconnect_input(&InputConnector::Export(export_index), network_path);
		let number_of_outputs = self.number_of_outputs(&parent_id, &encapsulating_network_path);
		for shifted_export in export_index..number_of_outputs {
			let Some(encapsulating_outward_wires) = self.outward_wires(&encapsulating_network_path) else {
				log::error!("Could not get outward wires in remove_export");
				return;
			};
			let Some(downstream_connections_for_shifted_export) = encapsulating_outward_wires.get(&OutputConnector::node(parent_id, shifted_export)).cloned() else {
				log::error!("Could not get downstream connections for shifted export in remove_export");
				return;
			};
			for downstream_connection in downstream_connections_for_shifted_export {
				self.disconnect_input(&downstream_connection, &encapsulating_network_path);
				if shifted_export != export_index {
					self.create_wire(&OutputConnector::node(parent_id, shifted_export - 1), &downstream_connection, &encapsulating_network_path);
				}
			}
		}

		if self.remove_export_slot(network_path, export_index).is_none() {
			return;
		}
		self.clear_encapsulating_reference(network_path);

		self.transaction_modified();

		self.finish_signature_edit(parent_id, &encapsulating_network_path, network_path);
	}

	// First disconnects the import, then removes it
	pub fn remove_import(&mut self, import_index: usize, network_path: &[NodeId]) {
		let Some((parent_id, encapsulating_network_path)) = network_path.split_last() else {
			log::error!("Cannot remove export for document network");
			return;
		};

		let number_of_inputs = self.number_of_inputs(parent_id, encapsulating_network_path);
		let Some(outward_wires) = self.outward_wires(network_path) else {
			log::error!("Could not get outward wires in remove_import");
			return;
		};
		let mut new_import_mapping = Vec::new();
		for i in (import_index + 1)..number_of_inputs {
			let Some(outward_wires_for_import) = outward_wires.get(&OutputConnector::Import(i)).cloned() else {
				log::error!("Could not get outward wires for import in remove_import");
				return;
			};
			for upstream_input_wire in outward_wires_for_import {
				new_import_mapping.push((OutputConnector::Import(i - 1), upstream_input_wire));
			}
		}

		// Disconnect all upstream connections, aborting before any mutation if the import's wires cannot be resolved
		if !self.disconnect_import_wires(import_index, network_path) {
			return;
		}
		// Shift inputs connected to imports at a higher index down one
		for (output_connector, input_wire) in new_import_mapping {
			self.create_wire(&output_connector, &input_wire, network_path);
		}

		let parent_id = *parent_id;
		let encapsulating_network_path = encapsulating_network_path.to_vec();
		if self.remove_input_slot(NodeLocator::new(parent_id, &encapsulating_network_path), import_index).is_none() {
			return;
		}
		self.clear_encapsulating_reference(network_path);

		self.transaction_modified();

		self.finish_signature_edit(parent_id, &encapsulating_network_path, network_path);
	}

	/// The end index is before the export is removed, so moving to the end is the length of the current exports
	pub fn reorder_export(&mut self, start_index: usize, mut end_index: usize, network_path: &[NodeId]) {
		let mut encapsulating_network_path = network_path.to_vec();
		let Some(parent_id) = encapsulating_network_path.pop() else {
			log::error!("Could not reorder export for document network");
			return;
		};

		if end_index > start_index {
			end_index -= 1;
		}
		if !self.move_export_slot(network_path, start_index, end_index) {
			return;
		}
		self.clear_encapsulating_reference(network_path);

		self.transaction_modified();

		// An export is an output of the encapsulating node, so its wires live in the encapsulating network
		self.unload_outward_wires(&encapsulating_network_path);
		self.unload_stack_dependents(&encapsulating_network_path);

		let last_output_index = self.number_of_outputs(&parent_id, &encapsulating_network_path) - 1;
		self.reindex_downstream_wires(&encapsulating_network_path, |index| OutputConnector::node(parent_id, index), last_output_index, start_index, end_index);

		self.unload_outward_wires(network_path);
		self.invalidate_import_export(network_path);
		self.unload_stack_dependents(network_path);
	}

	/// The end index is before the import is removed, so moving to the end is the length of the current imports
	pub fn reorder_import(&mut self, start_index: usize, mut end_index: usize, network_path: &[NodeId]) {
		let mut encapsulating_network_path = network_path.to_vec();
		let Some(parent_id) = encapsulating_network_path.pop() else {
			log::error!("Could not reorder import for document network");
			return;
		};

		if end_index > start_index {
			end_index -= 1;
		}
		if !self.move_input_slot(NodeLocator::new(parent_id, &encapsulating_network_path), start_index, end_index) {
			return;
		}
		self.clear_encapsulating_reference(network_path);

		self.transaction_modified();

		// An import is an input of the encapsulating node, so moving it moves that node's ports
		self.unload_outward_wires(&encapsulating_network_path);
		self.unload_stack_dependents(&encapsulating_network_path);

		// The wires an import feeds live in the network the import belongs to
		let last_import_index = self.number_of_imports(network_path) - 1;
		self.reindex_downstream_wires(network_path, OutputConnector::Import, last_import_index, start_index, end_index);

		self.unload_outward_wires(network_path);
		self.invalidate_import_export(network_path);
		self.unload_stack_dependents(network_path);
	}

	/// Rewires every downstream connection so it follows the slot it was pointing at, after the slot at
	/// `start_index` moved to `end_index` within `0..=last_index`. `output_connector` names one slot's
	/// output within `network_path`, which is the network holding the wires.
	fn reindex_downstream_wires(&mut self, network_path: &[NodeId], output_connector: impl Fn(usize) -> OutputConnector, last_index: usize, start_index: usize, end_index: usize) {
		let Some(moved_connections) = self.downstream_connections(&output_connector(start_index), network_path) else {
			return;
		};

		// Slots above the one that moved close the gap it left behind
		for shifted_index in (start_index + 1)..=last_index {
			let Some(connections) = self.downstream_connections(&output_connector(shifted_index), network_path) else {
				return;
			};
			self.rewire_downstream(connections, &output_connector(shifted_index - 1), network_path);
		}

		// Slots at or above the destination open a gap for it
		for shifted_index in (end_index..last_index).rev() {
			let Some(connections) = self.downstream_connections(&output_connector(shifted_index), network_path) else {
				return;
			};
			self.rewire_downstream(connections, &output_connector(shifted_index + 1), network_path);
		}

		// Last, so neither shift above can land on the moved slot's own connections
		self.rewire_downstream(moved_connections, &output_connector(end_index), network_path);
	}

	/// The inputs fed by `output_connector`, or `None` if the network's outward wires do not name it.
	fn downstream_connections(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Option<Vec<InputConnector>> {
		let connections = self.outward_wires(network_path).and_then(|outward_wires| outward_wires.get(output_connector)).cloned();
		if connections.is_none() {
			log::error!("Could not get outward wires for {output_connector:?} in network {network_path:?}");
		}
		connections
	}

	/// Moves each given downstream connection onto `output_connector`.
	pub(super) fn rewire_downstream(&mut self, downstream_connections: Vec<InputConnector>, output_connector: &OutputConnector, network_path: &[NodeId]) {
		for downstream_connection in downstream_connections {
			self.disconnect_input(&downstream_connection, network_path);
			self.create_wire(output_connector, &downstream_connection, network_path);
		}
	}

	/// Replaces the implementation and corresponding metadata.
	pub fn replace_implementation(&mut self, node_id: &NodeId, network_path: &[NodeId], new_template: &mut NodeTemplate) {
		let (new_implementation, new_network_metadata) = std::mem::take(&mut new_template.implementation).into_parts();

		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in replace_implementation");
			return;
		};
		node.replace_implementation(new_implementation, new_network_metadata);
	}

	/// Replaces the inputs and corresponding metadata.
	pub fn replace_inputs(&mut self, node_id: &NodeId, network_path: &[NodeId], new_template: &mut NodeTemplate) -> Option<Vec<NodeInput>> {
		let new_inputs = std::mem::take(&mut new_template.inputs);
		let new_input_metadata = std::mem::take(&mut new_template.input_metadata);

		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in replace_inputs");
			return None;
		};
		Some(node.replace_inputs(new_inputs, new_input_metadata))
	}

	/// Used when opening an old document to add the persistent metadata for each input if it doesn't exist, which is where the name/description are saved.
	pub fn validate_input_metadata(&mut self, node_id: &NodeId, node: &DocumentNode, network_path: &[NodeId]) {
		let number_of_inputs = node.inputs.len();
		let definition = self.reference(node_id, network_path).as_ref().and_then(resolve_document_node_type);

		let Some(mut node_entry) = self.node_mut(NodeLocator::new(*node_id, network_path)) else { return };
		node_entry.pad_input_metadata(number_of_inputs, |input_index| {
			definition.and_then(|definition| definition.node_template.input_metadata.get(input_index).cloned())
		});
	}

	// When opening an old document to ensure the output names match the number of exports
	pub fn validate_output_names(&mut self, node_id: &NodeId, node: &DocumentNode, network_path: &[NodeId]) {
		let DocumentNodeImplementation::Network(network) = &node.implementation else { return };
		let number_of_exports = network.exports.len();

		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in validate_output_names");
			return;
		};
		node.resize_output_names(number_of_exports);
	}

	/// Keep metadata in sync with the new implementation if this is used by anything other than the upgrade scripts.
	/// Only works with network nodes. Proto nodes use their ID as the reference.
	pub fn set_reference(&mut self, node_id: &NodeId, network_path: &[NodeId], reference_name: Option<String>) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in set_reference");
			return;
		};
		node.set_reference(reference_name);
	}

	/// Keep metadata in sync with the new implementation if this is used by anything other than the upgrade scripts
	pub fn set_call_argument(&mut self, node_id: &NodeId, network_path: &[NodeId], call_argument: Type) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in set_call_argument");
			return;
		};
		node.set_call_argument(call_argument);
	}

	pub fn set_context_features(&mut self, node_id: &NodeId, network_path: &[NodeId], context_features: ContextDependencies) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in set_context_features");
			return;
		};
		node.set_context_features(context_features);
	}

	/// Lightweight version of `set_input` for bulk import operations.
	/// Directly sets the input without `is_acyclic` checks, `load_structure`, position conversions,
	/// or per-node cache invalidation. Call `load_structure`, `unload_all_nodes_click_targets`, and
	/// `unload_all_nodes_bounding_box` once after all import wiring is complete.
	pub fn set_input_for_import(&mut self, input_connector: &InputConnector, new_input: NodeInput, network_path: &[NodeId]) {
		if matches!(input_connector, InputConnector::Export(_)) && matches!(new_input, NodeInput::Import { .. }) {
			log::error!("Cannot connect a network to an export, see https://github.com/GraphiteEditor/Graphite/issues/1762");
			return;
		}

		let Some(old_input) = self.set_input_slot(input_connector, network_path, new_input.clone()) else {
			return;
		};

		self.transaction_modified();
		self.update_outward_wires(network_path, input_connector, &old_input, &new_input);
	}

	pub fn set_input(&mut self, input_connector: &InputConnector, new_input: NodeInput, network_path: &[NodeId]) {
		if matches!(input_connector, InputConnector::Export(_)) && matches!(new_input, NodeInput::Import { .. }) {
			// TODO: Add support for flattening NodeInput::Import exports in flatten_with_fns https://github.com/GraphiteEditor/Graphite/issues/1762
			log::error!("Cannot connect a network to an export, see https://github.com/GraphiteEditor/Graphite/issues/1762");
			return;
		}
		let Some(previous_input) = self.input_from_connector(input_connector, network_path).cloned() else {
			log::error!("Could not get previous input in set_input");
			return;
		};

		// Only a node connection can close a loop, and the check has to run before any side effect does
		if matches!(new_input, NodeInput::Node { .. }) && self.would_create_cycle(input_connector, &new_input, network_path) {
			return;
		}

		// Rewiring one node connection to another runs the disconnect side effects first
		if matches!(previous_input, NodeInput::Node { .. }) && matches!(new_input, NodeInput::Node { .. }) {
			self.disconnect_input(input_connector, network_path);
			self.set_input(input_connector, new_input, network_path);
			return;
		}

		// A chain positions its nodes relative to the layer they feed, so either end of the rewire
		// leaving that layer breaks the chain it belonged to
		for chain_input in [&previous_input, &new_input] {
			if let NodeInput::Node { node_id: chain_node_id, .. } = chain_input
				&& self.is_chain(chain_node_id, network_path)
			{
				self.set_upstream_chain_to_absolute(chain_node_id, network_path);
			}
		}

		let Some(old_input) = self.set_input_slot(input_connector, network_path, new_input.clone()) else {
			return;
		};
		if old_input == new_input {
			return;
		}

		// Read after the write so a node the write disconnected reports where it comes to rest.
		// `position` can crash on a cyclic graph (#3227), which the check above has ruled out.
		let disconnected_upstream = match &previous_input {
			NodeInput::Node { node_id, .. } => self.position(node_id, network_path).map(|position| (*node_id, position)),
			_ => None,
		};

		self.transaction_modified();

		// A node that no longer satisfies the layer shape is shown as a node instead
		let layer_node_path = match input_connector {
			InputConnector::Node { node_id, .. } => Some((node_id, network_path)),
			InputConnector::Export(_) => network_path.split_last(),
		};
		if let Some((layer_id, layer_path)) = layer_node_path
			&& !self.is_eligible_to_be_layer(layer_id, layer_path)
			&& self.is_layer(layer_id, layer_path)
		{
			self.set_to_node_or_layer(layer_id, layer_path, false);
		}

		// A no-op unless one of the two inputs is a wire
		self.update_outward_wires(network_path, input_connector, &old_input, &new_input);

		match (&old_input, &new_input) {
			(NodeInput::Value { exposed: old_exposed, .. }, NodeInput::Value { exposed: new_exposed, .. }) => match input_connector {
				// Exposing or hiding a value input changes the node's port count
				InputConnector::Node { node_id, .. } if old_exposed != new_exposed => {
					self.unload_upstream_node_click_targets(vec![*node_id], network_path);
					self.unload_all_nodes_bounding_box(network_path);

					// A nested network draws its interior ports from the encapsulating node's inputs
					if matches!(self.implementation(node_id, network_path), Some(DocumentNodeImplementation::Network(_))) {
						let nested_path = [network_path, &[*node_id]].concat();
						self.invalidate_import_export(&nested_path);
					}
				}
				InputConnector::Node { .. } => {}
				InputConnector::Export(_) => self.invalidate_import_export(network_path),
			},

			(_, NodeInput::Node { node_id: upstream_node_id, .. }) => {
				// `Node` inputs are always exposed, so connecting to a hidden input adds a port
				if !old_input.is_exposed()
					&& let InputConnector::Node { node_id, .. } = input_connector
				{
					self.unload_node_click_targets(node_id, network_path);
				}

				self.reload_structure_if_affected(input_connector, network_path);

				if !self.reposition_connected_upstream(upstream_node_id, input_connector, network_path) {
					return;
				}

				// Altering an export may move the connectors, so the ports have to be refreshed
				if matches!(input_connector, InputConnector::Export(_)) {
					self.unload_import_export_ports(network_path);
				}
				self.unload_upstream_node_click_targets(vec![*upstream_node_id], network_path);
				self.unload_stack_dependents(network_path);
				self.try_set_upstream_to_chain(input_connector, network_path);
			}

			// A wire to or from the imports appeared or vanished
			(NodeInput::Value { .. } | NodeInput::Scope { .. } | NodeInput::Inline { .. }, NodeInput::Import { .. })
			| (NodeInput::Import { .. }, NodeInput::Value { .. } | NodeInput::Scope { .. } | NodeInput::Inline { .. }) => {
				self.unload_wire(input_connector, network_path);
			}

			// A node was disconnected
			(NodeInput::Node { .. }, NodeInput::Value { .. } | NodeInput::Scope { .. } | NodeInput::Inline { .. }) => {
				self.unload_wire(input_connector, network_path);

				if let Some((old_upstream_node_id, previous_position)) = disconnected_upstream
					&& !self.reposition_disconnected_upstream(&old_upstream_node_id, previous_position, network_path)
				{
					return;
				}

				self.reload_structure_if_affected(input_connector, network_path);
				self.unload_stack_dependents(network_path);
			}

			_ => {}
		}
	}

	/// Whether writing `new_input` at `input_connector` would leave the network cyclic.
	///
	/// Asked of the network as it stands, with the proposed input substituted only while the question is
	/// answered, so nothing is written. An input that is not there reports `true`, so the caller abandons
	/// a write that would fail anyway.
	///
	/// Writing an export can never close a cycle, since nothing takes an export as its input.
	fn would_create_cycle(&self, input_connector: &InputConnector, new_input: &NodeInput, network_path: &[NodeId]) -> bool {
		let InputConnector::Node { node_id, input_index } = input_connector else { return false };

		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in would_create_cycle");
			return true;
		};
		if self.input_from_connector(input_connector, network_path).is_none() {
			log::error!("Could not get input in would_create_cycle");
			return true;
		}

		!network.is_acyclic_with(Some((*node_id, *input_index, new_input)))
	}

	/// Rebuilds the layer tree when a change to the document network could have moved a layer within it:
	/// its first export, or the first or second input of a node that reaches that export.
	fn reload_structure_if_affected(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) {
		if !network_path.is_empty() {
			return;
		}

		let affects_structure = match input_connector {
			InputConnector::Export(export_index) => *export_index == 0,
			InputConnector::Node { node_id, input_index } => *input_index <= 1 && self.connected_to_output(node_id, network_path),
		};
		if affects_structure {
			self.load_structure();
		}
	}

	/// Ensure network metadata, positions, and other metadata is kept in sync
	pub fn disconnect_input(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) {
		let Some(current_input) = self.input_from_connector(input_connector, network_path).cloned() else {
			log::error!("Could not get current input in disconnect_input");
			return;
		};
		// Only disconnect inputs that are actual wire connections (Node or Import)
		if !matches!(current_input, NodeInput::Node { .. } | NodeInput::Import { .. }) {
			return;
		}

		if let NodeInput::Node {
			node_id: upstream_node_id,
			output_index,
			..
		} = &current_input
		{
			// If the node upstream from the disconnected input is a chain, then break the chain by setting it to absolute positioning
			if self.is_chain(upstream_node_id, network_path) {
				self.set_upstream_chain_to_absolute(upstream_node_id, network_path);
			}
			// If the node upstream from the disconnected input has an outward wire to the bottom of a layer, set it back to stack positioning
			if self.is_layer(upstream_node_id, network_path) {
				let Some(outward_wires) = self
					.outward_wires(network_path)
					.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(*upstream_node_id, *output_index)))
				else {
					log::error!("Could not get outward wires in disconnect_input");
					return;
				};
				let mut other_outward_wires = outward_wires.iter().filter(|outward_wire| *outward_wire != input_connector);
				if let Some(other_outward_wire) = other_outward_wires.next().cloned()
					&& other_outward_wires.next().is_none()
					&& let InputConnector::Node {
						node_id: downstream_node_id,
						input_index,
					} = other_outward_wire
					&& self.is_layer(&downstream_node_id, network_path)
					&& input_index == 0
				{
					self.set_stack_position_calculated_offset(upstream_node_id, &downstream_node_id, network_path);
				}
			}
		}

		let tagged_value = self.tagged_value_from_input(input_connector, network_path);

		let value_input = NodeInput::value(tagged_value, true);

		self.set_input(input_connector, value_input, network_path);
	}

	pub fn create_wire(&mut self, output_connector: &OutputConnector, input_connector: &InputConnector, network_path: &[NodeId]) {
		let input = match output_connector {
			OutputConnector::Node { node_id, output_index } => NodeInput::node(*node_id, *output_index),
			OutputConnector::Import(import_index) => NodeInput::Import {
				import_type: graph_craft::generic!(T),
				import_index: *import_index,
			},
		};

		self.set_input(input_connector, input, network_path);
	}

	/// Used to insert a group of nodes into the network
	pub fn insert_node_group(&mut self, nodes: Vec<(NodeId, NodeTemplate)>, new_ids: HashMap<NodeId, NodeId>, network_path: &[NodeId]) {
		for (old_node_id, mut node_template) in nodes {
			node_template = self.map_ids(node_template, &old_node_id, &new_ids, network_path);
			let node_id = *new_ids.get(&old_node_id).unwrap();
			self.insert_node_entry(NodeLocator::new(node_id, network_path), node_template);

			self.transaction_modified();
		}
		for new_node_id in new_ids.values() {
			self.unload_node_click_targets(new_node_id, network_path);
		}
		self.unload_all_nodes_bounding_box(network_path);
		self.unload_outward_wires(network_path);
	}

	/// Used to insert a node template with no node/network inputs into the network and returns the a NodeTemplate with information from the previous node, if it existed.
	pub fn insert_node(&mut self, node_id: NodeId, node_template: NodeTemplate, network_path: &[NodeId]) -> Option<NodeTemplate> {
		let has_node_or_network_input = node_template
			.inputs
			.iter()
			.all(|input| !(matches!(input, NodeInput::Node { .. }) || matches!(input, NodeInput::Import { .. })));
		assert!(has_node_or_network_input, "Cannot insert node with node or network inputs. Use insert_node_group instead");

		let previous_entry = self.insert_node_entry(NodeLocator::new(node_id, network_path), node_template);

		self.transaction_modified();
		self.unload_all_nodes_bounding_box(network_path);
		self.unload_node_click_targets(&node_id, network_path);

		previous_entry
	}

	/// Deletes all nodes in `node_ids` and any sole dependents in the horizontal chain if the node to delete is a layer node.
	pub fn delete_nodes(&mut self, nodes_to_delete: Vec<NodeId>, delete_children: bool, network_path: &[NodeId]) {
		if self.outward_wires(network_path).is_none() {
			log::error!("Could not get outward wires in delete_nodes");
			return;
		}

		// Layer membership is fixed during the expansion phase, so gather it once for the sole-dependent closure
		let layer_nodes = if delete_children {
			self.nested_network(network_path)
				.map(|network| network.nodes.keys().copied().collect::<Vec<_>>())
				.unwrap_or_default()
				.into_iter()
				.filter(|candidate| self.is_layer(candidate, network_path))
				.collect::<HashSet<_>>()
		} else {
			HashSet::new()
		};

		let mut delete_nodes = HashSet::new();
		for node_id in &nodes_to_delete {
			delete_nodes.insert(*node_id);

			if !delete_children {
				continue;
			};

			// Perform an upstream traversal to try delete children for secondary inputs
			let mut upstream_nodes = (1..self.number_of_inputs(node_id, network_path))
				.filter_map(|input_index| {
					self.upstream_output_connector(&InputConnector::node_at_index(*node_id, input_index), network_path)
						.and_then(|oc| oc.node_id())
				})
				.collect::<Vec<_>>();
			while let Some(upstream_node) = upstream_nodes.pop() {
				// Add the upstream nodes to the traversal
				for input_connector in (0..self.number_of_inputs(&upstream_node, network_path)).map(|input_index| InputConnector::node_at_index(upstream_node, input_index)) {
					if let Some(upstream_node) = self.upstream_output_connector(&input_connector, network_path).and_then(|oc| oc.node_id()) {
						upstream_nodes.push(upstream_node);
					}
				}

				// A path terminates when absorbed by another node marked for deletion, except through a layer's bottom input, which is stack flow to walk through.
				// Reaching the primary input of the node being deleted means this is the stack continuation rather than a child, so it must survive.
				let can_delete = self.is_sole_dependent(upstream_node, network_path, |downstream_node, input_index| {
					if downstream_node == *node_id && input_index == 0 {
						SoleDependentStep::Escape
					} else if delete_nodes.contains(&downstream_node) && !(input_index == 0 && layer_nodes.contains(&downstream_node)) {
						SoleDependentStep::Terminate
					} else {
						SoleDependentStep::Continue
					}
				});

				if can_delete {
					delete_nodes.insert(upstream_node);
				}
			}
		}

		for delete_node_id in &delete_nodes {
			let upstream_chain_nodes = self
				.upstream_flow_back_from_nodes(vec![*delete_node_id], network_path, FlowType::PrimaryFlow)
				.skip(1)
				.take_while(|upstream_node| self.is_chain(upstream_node, network_path))
				.collect::<Vec<_>>();

			if !self.remove_references_from_network(delete_node_id, network_path) {
				log::error!("could not remove references from network");
				continue;
			}

			// Disconnect every input by position, since hidden inputs make the displayed count undershoot the index of a later exposed wire
			for input_index in 0..self.number_of_inputs(delete_node_id, network_path) {
				self.disconnect_input(&InputConnector::node_at_index(*delete_node_id, input_index), network_path);
			}

			self.remove_node_entry(NodeLocator::new(*delete_node_id, network_path));

			self.transaction_modified();
			for previous_chain_node in upstream_chain_nodes {
				self.set_chain_position(&previous_chain_node, network_path);
			}
		}

		// Prune this network's pinned display order down to the nodes that still exist
		let surviving_nodes = self.nested_network(network_path).map(|network| network.nodes.keys().copied().collect::<HashSet<_>>());
		if let Some(surviving_nodes) = surviving_nodes
			&& let Some(mut network) = self.network_mut(network_path)
		{
			network.retain_pinned(|node_id| surviving_nodes.contains(node_id));
		}

		// Purge the deleted nodes' cached wire paths, which the per-node unload can no longer reach
		if let Some(network_metadata) = self.network_metadata(network_path) {
			network_metadata
				.transient_metadata
				.wires
				.borrow_mut()
				.retain(|connector, _| connector.node_id().is_none_or(|node_id| !delete_nodes.contains(&node_id)));
		}

		self.unload_all_nodes_bounding_box(network_path);
		// The per-node unload cannot reach nodes that no longer exist
		self.unload_all_nodes_click_targets(network_path);
		let Some(selected_nodes) = self.selected_nodes_mut(network_path) else {
			log::error!("Could not get selected nodes in NodeGraphMessage::DeleteNodes");
			return;
		};
		selected_nodes.retain_selected_nodes(|node_id| !delete_nodes.contains(node_id));
	}

	/// Removes all references to the node with the given id from the network, and reconnects the input to the node below.
	pub fn remove_references_from_network(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		// A preview of the node being removed cannot outlive it. A preview of any other node is untouched:
		// previewing does not rewire anything, so the reconnection below cannot invalidate it.
		if matches!(self.previewing(network_path), Previewing::Yes { previewed } if previewed.node_id == *node_id)
			&& let Some(mut network) = self.network_mut(network_path)
		{
			network.set_previewing(Previewing::No);
		}

		// The wire the downstream inputs reconnect to, which is the first exposed input of the node being removed
		let reconnect_to_input = self.document_node(node_id, network_path).and_then(|node| {
			node.inputs
				.iter()
				.find(|input| input.is_exposed())
				.filter(|input| matches!(input, NodeInput::Node { .. } | NodeInput::Import { .. }))
				.cloned()
		});
		let number_of_outputs = self.number_of_outputs(node_id, network_path);
		let Some(all_outward_wires) = self.outward_wires(network_path) else {
			log::error!("Could not get outward wires in remove_references_from_network");
			return false;
		};
		let mut downstream_inputs_to_disconnect = Vec::new();
		for output_index in 0..number_of_outputs {
			if let Some(outward_wires) = all_outward_wires.get(&OutputConnector::node(*node_id, output_index)) {
				downstream_inputs_to_disconnect.extend(outward_wires.clone());
			}
		}

		let mut reconnect_node = None;

		for downstream_input in &downstream_inputs_to_disconnect {
			self.disconnect_input(downstream_input, network_path);
			// Prevent reconnecting export to import until https://github.com/GraphiteEditor/Graphite/issues/1762 is solved
			if !(matches!(reconnect_to_input, Some(NodeInput::Import { .. })) && matches!(downstream_input, InputConnector::Export(_)))
				&& let Some(reconnect_input) = &reconnect_to_input
			{
				reconnect_node = reconnect_input.as_node().filter(|&node_id| self.is_stack(&node_id, network_path));
				self.disconnect_input(&InputConnector::primary_input(*node_id), network_path);
				self.set_input(downstream_input, reconnect_input.clone(), network_path);
			}
		}

		// Shift the reconnected node up to collapse space
		if let Some(reconnect_node) = &reconnect_node {
			let Some(reconnected_node_position) = self.position(reconnect_node, network_path) else {
				log::error!("Could not get reconnected node position in remove_references_from_network");
				return false;
			};
			let Some(disconnected_node_position) = self.position(node_id, network_path) else {
				log::error!("Could not get disconnected node position in remove_references_from_network");
				return false;
			};
			let max_shift_distance = reconnected_node_position.y - disconnected_node_position.y;

			let upstream_nodes = self.upstream_flow_back_from_nodes(vec![*reconnect_node], network_path, FlowType::PrimaryFlow).collect::<HashSet<_>>();

			// Build the stack dependents from the reconnected flow rather than the selection so the shifting works correctly
			self.unload_stack_dependents(network_path);
			self.load_stack_dependents_for_nodes(upstream_nodes.iter().copied().collect(), network_path);

			// Shift up until there is either a collision or the disconnected node position is reached
			let mut current_shift_distance = 0;
			while self.check_collision_with_stack_dependents(reconnect_node, -1, network_path).is_empty() && max_shift_distance > current_shift_distance {
				self.shift_nodes(upstream_nodes.clone(), Direction::Up, false, network_path);
				current_shift_distance += 1;
			}

			self.unload_stack_dependents(network_path);
		}

		true
	}

	pub fn set_display_name(&mut self, node_id: &NodeId, display_name: String, network_path: &[NodeId]) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in set_display_name");
			return;
		};

		if !node.set_display_name(display_name) {
			return;
		}

		self.transaction_modified();
		self.invalidate_node_appearance(node_id, network_path);
	}

	pub fn set_import_export_name(&mut self, name: String, index: ImportOrExport, network_path: &[NodeId]) {
		let Some((encapsulating_node_id, encapsulating_network_path)) = network_path.split_last() else {
			log::error!("Could not get encapsulating network in set_import_export_name");
			return;
		};

		let Some(mut node) = self.node_mut(NodeLocator::new(*encapsulating_node_id, encapsulating_network_path)) else {
			log::error!("Could not get encapsulating node in set_import_export_name");
			return;
		};

		let name_changed = match index {
			ImportOrExport::Import(import_index) => node.set_input_name(import_index, name),
			ImportOrExport::Export(export_index) => node.set_output_name(export_index, name),
		};

		if name_changed {
			self.transaction_modified();
		}
	}

	pub fn set_pinned(&mut self, node_id: &NodeId, network_path: &[NodeId], pinned: bool) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in set_pinned");
			return;
		};
		node.set_pinned(pinned);

		if let Some(mut network) = self.network_mut(network_path) {
			network.record_pinned(*node_id, pinned);
		}

		self.transaction_modified();
	}

	/// Reorders a pinned node within its network's Properties panel display order so it ends up at `insert_index` among the
	/// pinned nodes (0 being the topmost). Rebuilds the order from the list as currently shown, which also drops stale entries.
	pub fn reorder_pinned_node(&mut self, node_id: NodeId, insert_index: usize, network_path: &[NodeId]) {
		let shown = self.ordered_pinned_nodes(network_path);

		let Some(from) = shown.iter().position(|id| *id == node_id) else { return };
		let to = (if insert_index > from { insert_index - 1 } else { insert_index }).min(shown.len().saturating_sub(1));
		if to == from {
			return;
		}

		let mut new_order = shown;
		let moved = new_order.remove(from);
		new_order.insert(to, moved);

		let Some(mut network) = self.network_mut(network_path) else {
			log::error!("Could not get network_metadata in reorder_pinned_node");
			return;
		};
		network.set_pinned_order(new_order);

		self.transaction_modified();
	}

	pub fn set_visibility(&mut self, node_id: &NodeId, network_path: &[NodeId], is_visible: bool) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		if node.set_visible(is_visible) {
			self.transaction_modified();
		}
	}

	pub fn set_locked(&mut self, node_id: &NodeId, network_path: &[NodeId], locked: bool) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node {node_id} in set_locked");
			return;
		};

		if !node.set_locked(locked) {
			return;
		}

		self.transaction_modified();
		self.invalidate_node_appearance(node_id, network_path);
	}

	pub fn set_to_node_or_layer(&mut self, node_id: &NodeId, network_path: &[NodeId], is_layer: bool) {
		// If a layer is set to a node, set upstream nodes to absolute position, and upstream siblings to absolute position
		let child_id = { self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).nth(1) };
		let upstream_sibling_id = { self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::PrimaryFlow).nth(1) };
		match (self.is_layer(node_id, network_path), is_layer) {
			(true, false) => {
				if let Some(child_id) = child_id {
					self.set_upstream_chain_to_absolute(&child_id, network_path);
				}
				if let Some(upstream_sibling_id) = upstream_sibling_id {
					let Some(upstream_sibling_position) = self.position(&upstream_sibling_id, network_path) else {
						log::error!("Could not get upstream sibling position in set_to_node_or_layer");
						return;
					};
					self.set_absolute_position(&upstream_sibling_id, upstream_sibling_position, network_path);
				}
			}
			(false, true) => {
				// If a node is set to a layer
				if let Some(upstream_sibling_id) = upstream_sibling_id {
					// If the upstream sibling layer has a single output, then set it to stack position
					if self.is_layer(&upstream_sibling_id, network_path)
						&& self
							.outward_wires(network_path)
							.and_then(|outward_wires| outward_wires.get(&OutputConnector::primary_output(upstream_sibling_id)))
							.is_some_and(|outward_wires| outward_wires.len() == 1)
					{
						self.set_stack_position_calculated_offset(&upstream_sibling_id, node_id, network_path);
					} else {
						self.set_upstream_chain_to_absolute(&upstream_sibling_id, network_path);
					}
				}
			}
			_ => return,
		};

		let Some(position) = self.position(node_id, network_path) else {
			log::error!("Could not get position in set_to_node_or_layer");
			return;
		};

		let single_downstream_layer_position = self
			.outward_wires(network_path)
			.and_then(|outward_wires| {
				outward_wires
					.get(&OutputConnector::primary_output(*node_id))
					.and_then(|outward_wires| (outward_wires.len() == 1).then(|| outward_wires[0]))
					.and_then(|downstream_connector| if downstream_connector.input_index() == 0 { downstream_connector.node_id() } else { None })
			})
			.filter(|downstream_node_id| self.is_layer(downstream_node_id, network_path))
			.and_then(|downstream_layer| self.position(&downstream_layer, network_path));

		// First set the position to absolute
		let absolute = match is_layer {
			true => NodeTypePersistentMetadata::layer(position),
			false => NodeTypePersistentMetadata::node(position),
		};
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		node.set_node_type(absolute);

		// Try build the chain
		if is_layer {
			self.try_set_upstream_to_chain(&InputConnector::layer_secondary_input(*node_id), network_path);
		} else {
			self.try_set_node_to_chain(node_id, network_path);
		}

		// Set the position to stack if necessary
		if let Some(downstream_position) = is_layer.then_some(single_downstream_layer_position).flatten() {
			let offset = (position.y - downstream_position.y - STACK_VERTICAL_GAP).max(0) as u32;
			let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
				log::error!("Could not get node_metadata for node {node_id}");
				return;
			};
			node.set_stack_position(offset);
		}

		let Some(transient) = self.node_transient_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		transient.layer_width.unload();
		transient.owned_nodes.unload();

		self.transaction_modified();
		self.unload_stack_dependents(network_path);
		self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		self.unload_all_nodes_bounding_box(network_path);
		self.invalidate_import_export(network_path);
		self.load_structure();
	}

	/// Renders `toggle_id` instead of the network's export, or stops doing so if it already is.
	///
	/// Per-peer and not a change to the document: the export keeps whatever it is wired to, and the
	/// compile path substitutes the previewed node into the graph it evaluates, so no other peer sees it.
	pub fn toggle_preview(&mut self, toggle_id: NodeId, network_path: &[NodeId]) {
		let previewing = match self.previewing(network_path) {
			Previewing::Yes { previewed } if previewed.node_id == toggle_id => Previewing::No,
			_ => Previewing::Yes {
				previewed: RootNode { node_id: toggle_id, output_index: 0 },
			},
		};

		let Some(mut network) = self.network_mut(network_path) else { return };
		network.set_previewing(previewing);
	}
}
