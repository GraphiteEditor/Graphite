use super::*;

// Public mutable methods
impl NodeNetworkInterface {
	/// Walk every network at every nesting level, invoking `visit` with each network's path and a mutable
	/// reference to its metadata. Only nodes that contain a nested network are recursed into.
	fn for_each_network_metadata_mut(&mut self, mut visit: impl FnMut(&[NodeId], &mut NodeNetworkMetadata)) {
		let mut stack = vec![vec![]];
		while let Some(path) = stack.pop() {
			let Some(self_network_metadata) = self.network_metadata_mut(&path) else {
				continue;
			};

			visit(&path, self_network_metadata);

			// Only nodes that contain a nested network have metadata to recurse into, so skip leaf nodes.
			stack.extend(
				self_network_metadata
					.persistent_metadata
					.node_metadata
					.iter()
					.filter(|(_, node_metadata)| node_metadata.persistent_metadata.network_metadata.is_some())
					.map(|(node_id, _)| {
						let mut current_path: Vec<NodeId> = path.clone();
						current_path.push(*node_id);
						current_path
					}),
			);
		}
	}

	pub fn copy_all_navigation_metadata(&mut self, other_interface: &NodeNetworkInterface) {
		self.for_each_network_metadata_mut(|path, self_network_metadata| {
			if let Some(other_network_metadata) = other_interface.network_metadata(path) {
				self_network_metadata.persistent_metadata.navigation_metadata = other_network_metadata.persistent_metadata.navigation_metadata.clone();
			}
		});
	}

	/// Copy all transient view and selection state from `other_interface` onto `self` at every nesting level:
	/// navigation metadata (pan/zoom), selection undo/redo history, and the document-to-viewport camera. Lets
	/// an interface rebuilt from storage keep the user's current view rather than reset it. `resolved_types`
	/// is a separate runtime cache the caller handles.
	pub fn copy_all_transient_view_state(&mut self, other_interface: &NodeNetworkInterface) {
		self.for_each_network_metadata_mut(|path, self_network_metadata| {
			if let Some(other_network_metadata) = other_interface.network_metadata(path) {
				let self_persistent = &mut self_network_metadata.persistent_metadata;
				let other_persistent = &other_network_metadata.persistent_metadata;
				self_persistent.navigation_metadata = other_persistent.navigation_metadata.clone();

				// The live preview root may name a node that the rebuilt-from-storage network no longer contains
				// (e.g. after a storage undo), so only carry it over when the root still exists; otherwise drop it.
				self_persistent.previewing = match other_persistent.previewing {
					Previewing::Yes {
						root_node_to_restore: Some(root_node),
					} if !self_persistent.node_metadata.contains_key(&root_node.node_id) => Previewing::No,
					previewing => previewing,
				};

				self_persistent.selection_undo_history = other_persistent.selection_undo_history.clone();
				self_persistent.selection_redo_history = other_persistent.selection_redo_history.clone();
			}
		});

		self.document_metadata.document_to_viewport = other_interface.document_metadata.document_to_viewport;
	}

	pub fn set_transform(&mut self, transform: DAffine2, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network in set_transform");
			return;
		};
		network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport = transform;
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
	}

	// This should be run whenever the pan ends, a zoom occurs, or the network is opened
	pub fn set_node_graph_width(&mut self, node_graph_width: f64, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network in set_transform");
			return;
		};
		network_metadata.persistent_metadata.navigation_metadata.node_graph_width = node_graph_width;
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
	}

	pub fn vector_modify(&mut self, node_id: &NodeId, modification_type: VectorModificationType) {
		let Some(node) = self.network_mut(&[]).and_then(|network| network.nodes.get_mut(node_id)) else {
			log::error!("Could not get node in vector_modification");
			return;
		};
		{
			let mut value = node.inputs.get_mut(1).and_then(|input| input.as_value_mut());
			let Some(TaggedValue::VectorModification(modification)) = value.as_deref_mut() else {
				log::error!("Path node {node_id} does not have a modification input");
				return;
			};

			modification.modify(&modification_type);
		}
		self.transaction_modified();
	}

	/// Inserts a new export at insert index. If the insert index is -1 it is inserted at the end. The output_name is used by the encapsulating node.
	pub fn add_export(&mut self, default_value: TaggedValue, insert_index: isize, output_name: &str, network_path: &[NodeId]) {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in add_export");
			return;
		};

		let input = NodeInput::value(default_value, true);
		let inserted_index = if insert_index == -1 { network.exports.len() } else { insert_index as usize };
		if insert_index == -1 {
			network.exports.push(input);
		} else {
			network.exports.insert(insert_index as usize, input);
		}

		self.transaction_modified();

		let mut encapsulating_path = network_path.to_vec();
		// Set the parent node (if it exists) to be a non layer if it is no longer eligible to be a layer
		if let Some(parent_id) = encapsulating_path.pop()
			&& !self.is_eligible_to_be_layer(&parent_id, &encapsulating_path)
			&& self.is_layer(&parent_id, &encapsulating_path)
		{
			self.set_to_node_or_layer(&parent_id, &encapsulating_path, false);
		};

		// There will not be an encapsulating node if the network is the document network
		if let Some(encapsulating_node_metadata) = self.encapsulating_node_metadata_mut(network_path) {
			if insert_index == -1 {
				encapsulating_node_metadata.persistent_metadata.output_names.push(output_name.to_string());
			} else {
				encapsulating_node_metadata.persistent_metadata.output_names.insert(insert_index as usize, output_name.to_string());
			}
			// Clear the reference to the nodes definition
			if let Some(network_metadata) = encapsulating_node_metadata.persistent_metadata.network_metadata.as_mut() {
				network_metadata.persistent_metadata.reference = None
			}
		};

		// Update the export ports and outward wires for the current network
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
		self.unload_outward_wires(network_path);

		// Update the outward wires and bounding box for all nodes in the encapsulating network
		if let Some(encapsulating_network_metadata) = self.encapsulating_network_metadata_mut(network_path) {
			encapsulating_network_metadata.transient_metadata.outward_wires.unload();
			encapsulating_network_metadata.transient_metadata.all_nodes_bounding_box.unload();
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

	/// Inserts a new input at insert index. If the insert index is -1 it is inserted at the end. The output_name is used by the encapsulating node.
	pub fn add_import(&mut self, default_value: TaggedValue, exposed: bool, insert_index: isize, input_name: &str, input_description: &str, network_path: &[NodeId]) {
		let mut encapsulating_network_path = network_path.to_vec();
		let Some(node_id) = encapsulating_network_path.pop() else {
			log::error!("Cannot add import for document network");
			return;
		};

		let Some(network) = self.network_mut(&encapsulating_network_path) else {
			log::error!("Could not get nested network in insert_input");
			return;
		};
		let Some(node) = network.nodes.get_mut(&node_id) else {
			log::error!("Could not get node in insert_input");
			return;
		};

		let input = NodeInput::value(default_value, exposed);
		let inserted_index = if insert_index == -1 { node.inputs.len() } else { insert_index as usize };
		if insert_index == -1 {
			node.inputs.push(input);
		} else {
			node.inputs.insert(insert_index as usize, input);
		}

		self.transaction_modified();

		// Set the node to be a non layer if it is no longer eligible to be a layer
		if !self.is_eligible_to_be_layer(&node_id, &encapsulating_network_path) && self.is_layer(&node_id, &encapsulating_network_path) {
			self.set_to_node_or_layer(&node_id, &encapsulating_network_path, false);
		}

		let Some(node_metadata) = self.node_metadata_mut(&node_id, &encapsulating_network_path) else {
			log::error!("Could not get node_metadata in insert_input");
			return;
		};
		let new_input = (input_name, input_description).into();
		if insert_index == -1 {
			node_metadata.persistent_metadata.input_metadata.push(new_input);
		} else {
			node_metadata.persistent_metadata.input_metadata.insert(insert_index as usize, new_input);
		}

		// Clear the reference to the nodes definition
		if let Some(network_metadata) = node_metadata.persistent_metadata.network_metadata.as_mut() {
			network_metadata.persistent_metadata.reference = None
		}

		// Update the metadata for the encapsulating node
		self.unload_node_click_targets(&node_id, &encapsulating_network_path);
		self.unload_all_nodes_bounding_box(&encapsulating_network_path);
		if encapsulating_network_path.is_empty() && inserted_index <= 1 {
			self.load_structure();
		}

		// Unload the metadata for the nested network
		self.unload_outward_wires(network_path);
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
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
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
	}

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

		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in add_export");
			return;
		};
		network.exports.remove(export_index);

		self.transaction_modified();

		let Some(encapsulating_node_metadata) = self.node_metadata_mut(&parent_id, &encapsulating_network_path) else {
			log::error!("Could not get encapsulating node metadata in remove_export");
			return;
		};
		encapsulating_node_metadata.persistent_metadata.output_names.remove(export_index);
		if let Some(network_metadata) = encapsulating_node_metadata.persistent_metadata.network_metadata.as_mut() {
			network_metadata.persistent_metadata.reference = None;
		}

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

		let Some(network) = self.network_mut(encapsulating_network_path) else {
			log::error!("Could not get parent node in remove_import");
			return;
		};
		let Some(node) = network.nodes.get_mut(parent_id) else {
			log::error!("Could not get node in remove_import");
			return;
		};

		node.inputs.remove(import_index);

		self.transaction_modified();

		// There will not be an encapsulating node if the network is the document network
		let Some(encapsulating_node_metadata) = self.node_metadata_mut(parent_id, encapsulating_network_path) else {
			log::error!("Could not get encapsulating node metadata in remove_export");
			return;
		};
		encapsulating_node_metadata.persistent_metadata.input_metadata.remove(import_index);
		if let Some(network_metadata) = encapsulating_node_metadata.persistent_metadata.network_metadata.as_mut() {
			network_metadata.persistent_metadata.reference = None;
		}

		self.finish_signature_edit(*parent_id, encapsulating_network_path, network_path);
	}

	/// The end index is before the export is removed, so moving to the end is the length of the current exports
	pub fn reorder_export(&mut self, start_index: usize, mut end_index: usize, network_path: &[NodeId]) {
		let mut encapsulating_network_path = network_path.to_vec();
		let Some(parent_id) = encapsulating_network_path.pop() else {
			log::error!("Could not reorder export for document network");
			return;
		};

		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in reorder_export");
			return;
		};
		if end_index > start_index {
			end_index -= 1;
		}
		let export = network.exports.remove(start_index);
		network.exports.insert(end_index, export);

		self.transaction_modified();

		let Some(encapsulating_node_metadata) = self.node_metadata_mut(&parent_id, &encapsulating_network_path) else {
			log::error!("Could not get encapsulating network_metadata in reorder_export");
			return;
		};

		let name = encapsulating_node_metadata.persistent_metadata.output_names.remove(start_index);
		encapsulating_node_metadata.persistent_metadata.output_names.insert(end_index, name);
		if let Some(network_metadata) = encapsulating_node_metadata.persistent_metadata.network_metadata.as_mut() {
			network_metadata.persistent_metadata.reference = None;
		}

		// Update the metadata for the encapsulating network
		self.unload_outward_wires(&encapsulating_network_path);
		self.unload_stack_dependents(&encapsulating_network_path);

		// Node input at the start index is now at the end index
		let Some(move_to_end_index) = self
			.outward_wires(&encapsulating_network_path)
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(parent_id, start_index)))
			.cloned()
		else {
			log::error!("Could not get outward wires in reorder_export");
			return;
		};
		// Node inputs above the start index should be shifted down one
		let last_output_index = self.number_of_outputs(&parent_id, &encapsulating_network_path) - 1;
		for shift_output_down in (start_index + 1)..=last_output_index {
			let Some(outward_wires) = self
				.outward_wires(&encapsulating_network_path)
				.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(parent_id, shift_output_down)))
				.cloned()
			else {
				log::error!("Could not get outward wires in reorder_export");
				return;
			};
			for downstream_connection in &outward_wires {
				self.disconnect_input(downstream_connection, &encapsulating_network_path);
				self.create_wire(&OutputConnector::node(parent_id, shift_output_down - 1), downstream_connection, &encapsulating_network_path);
			}
		}
		// Node inputs at or above the end index should be shifted up one
		for shift_output_up in (end_index..last_output_index).rev() {
			let Some(outward_wires) = self
				.outward_wires(&encapsulating_network_path)
				.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(parent_id, shift_output_up)))
				.cloned()
			else {
				log::error!("Could not get outward wires in reorder_export");
				return;
			};
			for downstream_connection in &outward_wires {
				self.disconnect_input(downstream_connection, &encapsulating_network_path);
				self.create_wire(&OutputConnector::node(parent_id, shift_output_up + 1), downstream_connection, &encapsulating_network_path);
			}
		}

		// Move the connections to the moved export after all other ones have been shifted
		for downstream_connection in &move_to_end_index {
			self.disconnect_input(downstream_connection, &encapsulating_network_path);
			self.create_wire(&OutputConnector::node(parent_id, end_index), downstream_connection, &encapsulating_network_path);
		}

		// Update the metadata for the current network
		self.unload_outward_wires(network_path);
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
		self.unload_stack_dependents(network_path);
	}

	/// The end index is before the import is removed, so moving to the end is the length of the current imports
	pub fn reorder_import(&mut self, start_index: usize, mut end_index: usize, network_path: &[NodeId]) {
		let mut encapsulating_network_path = network_path.to_vec();
		let Some(parent_id) = encapsulating_network_path.pop() else {
			log::error!("Could not reorder import for document network");
			return;
		};

		let Some(encapsulating_network) = self.network_mut(&encapsulating_network_path) else {
			log::error!("Could not get nested network in reorder_import");
			return;
		};
		let Some(encapsulating_node) = encapsulating_network.nodes.get_mut(&parent_id) else {
			log::error!("Could not get encapsulating node in reorder_import");
			return;
		};

		if end_index > start_index {
			end_index -= 1;
		}
		let import = encapsulating_node.inputs.remove(start_index);
		encapsulating_node.inputs.insert(end_index, import);

		self.transaction_modified();

		let Some(encapsulating_node_metadata) = self.node_metadata_mut(&parent_id, &encapsulating_network_path) else {
			log::error!("Could not get encapsulating network_metadata in reorder_import");
			return;
		};

		let properties_row = encapsulating_node_metadata.persistent_metadata.input_metadata.remove(start_index);
		encapsulating_node_metadata.persistent_metadata.input_metadata.insert(end_index, properties_row);
		if let Some(network_metadata) = encapsulating_node_metadata.persistent_metadata.network_metadata.as_mut() {
			network_metadata.persistent_metadata.reference = None;
		}

		// Update the metadata for the outer network
		self.unload_outward_wires(&encapsulating_network_path);
		self.unload_stack_dependents(&encapsulating_network_path);

		// Node input at the start index is now at the end index
		let Some(move_to_end_index) = self
			.outward_wires(network_path)
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::Import(start_index)))
			.cloned()
		else {
			log::error!("Could not get outward wires in reorder_import");
			return;
		};
		// Node inputs above the start index should be shifted down one
		let last_import_index = self.number_of_imports(network_path) - 1;
		for shift_output_down in (start_index + 1)..=last_import_index {
			let Some(outward_wires) = self
				.outward_wires(network_path)
				.and_then(|outward_wires| outward_wires.get(&OutputConnector::Import(shift_output_down)))
				.cloned()
			else {
				log::error!("Could not get outward wires in reorder_import");
				return;
			};
			for downstream_connection in &outward_wires {
				self.disconnect_input(downstream_connection, network_path);
				self.create_wire(&OutputConnector::Import(shift_output_down - 1), downstream_connection, network_path);
			}
		}
		// Node inputs at or above the end index should be shifted up one
		for shift_output_up in (end_index..last_import_index).rev() {
			let Some(outward_wires) = self
				.outward_wires(network_path)
				.and_then(|outward_wires| outward_wires.get(&OutputConnector::Import(shift_output_up)))
				.cloned()
			else {
				log::error!("Could not get outward wires in reorder_import");
				return;
			};
			for downstream_connection in &outward_wires {
				self.disconnect_input(downstream_connection, network_path);
				self.create_wire(&OutputConnector::Import(shift_output_up + 1), downstream_connection, network_path);
			}
		}

		// Move the connections to the moved export after all other ones have been shifted
		for downstream_connection in &move_to_end_index {
			self.disconnect_input(downstream_connection, network_path);
			self.create_wire(&OutputConnector::Import(end_index), downstream_connection, network_path);
		}

		// Update the metadata for the current network
		self.unload_outward_wires(network_path);
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
		self.unload_stack_dependents(network_path);
	}

	/// Replaces the implementation and corresponding metadata.
	pub fn replace_implementation(&mut self, node_id: &NodeId, network_path: &[NodeId], new_template: &mut NodeTemplate) {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_implementation");
			return;
		};
		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node in set_implementation");
			return;
		};
		let (new_implementation, new_network_metadata) = std::mem::take(&mut new_template.implementation).into_parts();
		node.implementation = new_implementation;
		let Some(metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get metadata in set_implementation");
			return;
		};
		metadata.persistent_metadata.network_metadata = new_network_metadata;
	}

	/// Replaces the inputs and corresponding metadata.
	pub fn replace_inputs(&mut self, node_id: &NodeId, network_path: &[NodeId], new_template: &mut NodeTemplate) -> Option<Vec<NodeInput>> {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_implementation");
			return None;
		};
		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node in set_implementation");
			return None;
		};
		let new_inputs = std::mem::take(&mut new_template.inputs);
		let old_inputs = std::mem::replace(&mut node.inputs, new_inputs);
		let Some(metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get metadata in set_implementation");
			return None;
		};
		let new_metadata = std::mem::take(&mut new_template.input_metadata);
		let _ = std::mem::replace(&mut metadata.persistent_metadata.input_metadata, new_metadata);
		Some(old_inputs)
	}

	/// Used when opening an old document to add the persistent metadata for each input if it doesnt exist, which is where the name/description are saved.
	pub fn validate_input_metadata(&mut self, node_id: &NodeId, node: &DocumentNode, network_path: &[NodeId]) {
		let number_of_inputs = node.inputs.len();
		let Some(metadata) = self.node_metadata_mut(node_id, network_path) else { return };
		for added_input_index in metadata.persistent_metadata.input_metadata.len()..number_of_inputs {
			let input_metadata = self
				.reference(node_id, network_path)
				.as_ref()
				.and_then(resolve_document_node_type)
				.and_then(|definition| definition.node_template.input_metadata.get(added_input_index))
				.cloned();
			let Some(metadata) = self.node_metadata_mut(node_id, network_path) else { return };
			metadata.persistent_metadata.input_metadata.push(input_metadata.unwrap_or_default());
		}
	}

	// When opening an old document to ensure the output names match the number of exports
	pub fn validate_output_names(&mut self, node_id: &NodeId, node: &DocumentNode, network_path: &[NodeId]) {
		if let DocumentNodeImplementation::Network(network) = &node.implementation {
			let number_of_exports = network.exports.len();
			let Some(metadata) = self.node_metadata_mut(node_id, network_path) else {
				log::error!("Could not get metadata for node: {node_id:?}");
				return;
			};
			metadata.persistent_metadata.output_names.resize(number_of_exports, "".to_string());
		}
	}

	/// Keep metadata in sync with the new implementation if this is used by anything other than the upgrade scripts.
	/// Only works with network nodes. Proto nodes use their ID as the reference.
	pub fn set_reference(&mut self, node_id: &NodeId, network_path: &[NodeId], reference_name: Option<String>) {
		let Some(node_network_metadata) = self
			.node_metadata_mut(node_id, network_path)
			.and_then(|node_metadata| node_metadata.persistent_metadata.network_metadata.as_mut())
		else {
			log::error!("Could not get network metadata in replace_reference_name");
			return;
		};
		node_network_metadata.persistent_metadata.reference = reference_name;
	}

	/// Keep metadata in sync with the new implementation if this is used by anything other than the upgrade scripts
	pub fn set_call_argument(&mut self, node_id: &NodeId, network_path: &[NodeId], call_argument: Type) {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_implementation");
			return;
		};
		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node in set_implementation");
			return;
		};
		node.call_argument = call_argument;
	}

	pub fn set_context_features(&mut self, node_id: &NodeId, network_path: &[NodeId], context_features: ContextDependencies) {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_context_features");
			return;
		};
		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node in set_context_features");
			return;
		};
		node.context_features = context_features;
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

		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_input_for_import");
			return;
		};

		let old_input = match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(node) = network.nodes.get_mut(node_id) else {
					log::error!("Could not get node in set_input_for_import");
					return;
				};
				let Some(input) = node.inputs.get_mut(*input_index) else {
					log::error!("Could not get input in set_input_for_import");
					return;
				};
				std::mem::replace(input, new_input.clone())
			}
			InputConnector::Export(export_index) => {
				let Some(export) = network.exports.get_mut(*export_index) else {
					log::error!("Could not get export in set_input_for_import");
					return;
				};
				std::mem::replace(export, new_input.clone())
			}
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

		// When changing a NodeInput::Node to a NodeInput::Node, the input should first be disconnected to ensure proper side effects
		if (matches!(previous_input, NodeInput::Node { .. }) && matches!(new_input, NodeInput::Node { .. })) {
			self.disconnect_input(input_connector, network_path);
			self.set_input(input_connector, new_input, network_path);
			return;
		}

		// Reject a change that would create a cycle before any side effects run (only Node connections can create cycles).
		// The new input is swapped in just for this test, then restored so the layout logic below sees the unmodified network.
		if matches!(new_input, NodeInput::Node { .. }) {
			let Some(network) = self.network_mut(network_path) else {
				log::error!("Could not get nested network in set_input");
				return;
			};
			fn get_input<'a>(network: &'a mut NodeNetwork, input_connector: &InputConnector) -> Option<&'a mut NodeInput> {
				match input_connector {
					InputConnector::Node { node_id, input_index } => network.nodes.get_mut(node_id).and_then(|node| node.inputs.get_mut(*input_index)),
					InputConnector::Export(export_index) => network.exports.get_mut(*export_index),
				}
			}

			let Some(input) = get_input(network, input_connector) else {
				log::error!("Could not get input in set_input");
				return;
			};
			let old_input = std::mem::replace(input, new_input.clone());
			let is_acyclic = network.is_acyclic();
			let Some(input) = get_input(network, input_connector) else {
				log::error!("Could not get input in set_input");
				return;
			};
			*input = old_input;

			if !is_acyclic {
				return;
			}
		}

		// If the previous input is connected to a chain node, then set all upstream chain nodes to absolute position
		if let NodeInput::Node { node_id: previous_upstream_id, .. } = &previous_input
			&& self.is_chain(previous_upstream_id, network_path)
		{
			self.set_upstream_chain_to_absolute(previous_upstream_id, network_path);
		}
		if let NodeInput::Node { node_id: new_upstream_id, .. } = &new_input {
			// If the new input is connected to a chain node, then break its chain
			if self.is_chain(new_upstream_id, network_path) {
				self.set_upstream_chain_to_absolute(new_upstream_id, network_path);
			}
		}

		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_input");
			return;
		};

		let old_input = match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(node) = network.nodes.get_mut(node_id) else {
					log::error!("Could not get node in set_input");
					return;
				};
				let Some(input) = node.inputs.get_mut(*input_index) else {
					log::error!("Could not get input in set_input");
					return;
				};
				std::mem::replace(input, new_input.clone())
			}
			InputConnector::Export(export_index) => {
				let Some(export) = network.exports.get_mut(*export_index) else {
					log::error!("Could not get export in set_input");
					return;
				};
				std::mem::replace(export, new_input.clone())
			}
		};

		if old_input == new_input {
			return;
		};

		// It is necessary to ensure the graph is acyclic before calling `self.position` as it sometimes crashes with cyclic graphs #3227
		let previous_metadata = match &previous_input {
			NodeInput::Node { node_id, .. } => self.position(node_id, network_path).map(|position| (*node_id, position)),
			_ => None,
		};

		self.transaction_modified();

		// Ensure layer is toggled to non layer if it is no longer eligible to be a layer
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

		// Side effects
		match (&old_input, &new_input) {
			// If a node input is exposed or hidden reload the click targets and update the bounding box for all nodes
			(NodeInput::Value { exposed: old_exposed, .. }, NodeInput::Value { exposed: new_exposed, .. }) => {
				if let InputConnector::Node { node_id, .. } = input_connector {
					if new_exposed != old_exposed {
						self.unload_upstream_node_click_targets(vec![*node_id], network_path);
						self.unload_all_nodes_bounding_box(network_path);

						// Unload the interior import/export ports if this node has a nested network
						if matches!(self.implementation(node_id, network_path), Some(DocumentNodeImplementation::Network(_))) {
							let nested_path = [network_path, &[*node_id]].concat();
							self.unload_import_export_ports(&nested_path);
							self.unload_modify_import_export(&nested_path);
						}
					}
				} else {
					self.unload_import_export_ports(network_path);
					self.unload_modify_import_export(network_path);
				}
			}
			(_, NodeInput::Node { node_id: upstream_node_id, .. }) => {
				// If the old input wasn't exposed but the new one is (`Node` inputs are always exposed),
				// the node's port count changed, so its click targets need to be recomputed
				if !old_input.is_exposed()
					&& let InputConnector::Node { node_id, .. } = input_connector
				{
					self.unload_node_click_targets(node_id, network_path);
				}

				// Load structure if the change is to the document network and to the first or second
				if network_path.is_empty() {
					if matches!(input_connector, InputConnector::Export(0)) {
						self.load_structure();
					} else if let InputConnector::Node { node_id, input_index } = &input_connector {
						// If the connection is made to the first or second input of a node connected to the output, then load the structure
						if self.connected_to_output(node_id, network_path) && (*input_index == 0 || *input_index == 1) {
							self.load_structure();
						}
					}
				}
				self.update_outward_wires(network_path, input_connector, &old_input, &new_input);
				// Layout system
				let Some(current_node_position) = self.position(upstream_node_id, network_path) else {
					log::error!("Could not get current node position in set_input for node {upstream_node_id}");
					return;
				};
				let Some(node_metadata) = self.node_metadata(upstream_node_id, network_path) else {
					log::error!("Could not get node_metadata in set_input");
					return;
				};
				match &node_metadata.persistent_metadata.node_type_metadata {
					NodeTypePersistentMetadata::Layer(_) => {
						match &input_connector {
							InputConnector::Export(_) => {
								// If a layer is connected to the exports, it should be set to absolute position without being moved.
								self.set_absolute_position(upstream_node_id, current_node_position, network_path)
							}
							InputConnector::Node {
								node_id: downstream_node_id,
								input_index,
							} => {
								// If a layer has a single connection to the bottom of another layer, it should be set to stack positioning
								let Some(downstream_node_metadata) = self.node_metadata(downstream_node_id, network_path) else {
									log::error!("Could not get downstream node_metadata in set_input");
									return;
								};
								match &downstream_node_metadata.persistent_metadata.node_type_metadata {
									NodeTypePersistentMetadata::Layer(_) => {
										// If the layer feeds into the bottom input of layer, and has no other outputs, set its position to stack at its previous y position
										let multiple_outward_wires = self
											.outward_wires(network_path)
											.and_then(|all_outward_wires| all_outward_wires.get(&OutputConnector::node(*upstream_node_id, 0)))
											.is_some_and(|outward_wires| outward_wires.len() > 1);
										if *input_index == 0 && !multiple_outward_wires {
											self.set_stack_position_calculated_offset(upstream_node_id, downstream_node_id, network_path);
										} else {
											self.set_absolute_position(upstream_node_id, current_node_position, network_path);
										}
									}
									NodeTypePersistentMetadata::Node(_) => {
										// If the layer feeds into a node, set its y offset to 0
										self.set_absolute_position(upstream_node_id, current_node_position, network_path);
									}
								}
							}
						}
					}
					NodeTypePersistentMetadata::Node(_) => {}
				}
				// Altering an export may move the connectors meaning the ports must be refreshed.
				if matches!(input_connector, InputConnector::Export(_)) {
					self.unload_import_export_ports(network_path);
				}
				self.unload_upstream_node_click_targets(vec![*upstream_node_id], network_path);
				self.unload_stack_dependents(network_path);
				self.try_set_upstream_to_chain(input_connector, network_path);
			}
			// If a connection is made to the imports
			(NodeInput::Value { .. } | NodeInput::Scope { .. } | NodeInput::Inline { .. }, NodeInput::Import { .. }) => {
				self.update_outward_wires(network_path, input_connector, &old_input, &new_input);
				self.unload_wire(input_connector, network_path);
			}
			// If a connection to the imports is disconnected
			(NodeInput::Import { .. }, NodeInput::Value { .. } | NodeInput::Scope { .. } | NodeInput::Inline { .. }) => {
				self.update_outward_wires(network_path, input_connector, &old_input, &new_input);
				self.unload_wire(input_connector, network_path);
			}
			// If a node is disconnected.
			(NodeInput::Node { .. }, NodeInput::Value { .. } | NodeInput::Scope { .. } | NodeInput::Inline { .. }) => {
				self.update_outward_wires(network_path, input_connector, &old_input, &new_input);
				self.unload_wire(input_connector, network_path);

				if let Some((old_upstream_node_id, previous_position)) = previous_metadata {
					let old_upstream_node_is_layer = self.is_layer(&old_upstream_node_id, network_path);
					let Some(outward_wires) = self
						.outward_wires(network_path)
						.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(old_upstream_node_id, 0)))
					else {
						log::error!("Could not get outward wires in set_input");
						return;
					};
					// If it is a layer and is connected to a single layer, set its position to stack at its previous y position
					if old_upstream_node_is_layer && outward_wires.len() == 1 && outward_wires[0].input_index() == 0 {
						if let Some(downstream_node_id) = outward_wires[0].node_id()
							&& self.is_layer(&downstream_node_id, network_path)
						{
							self.set_stack_position_calculated_offset(&old_upstream_node_id, &downstream_node_id, network_path);
							self.unload_upstream_node_click_targets(vec![old_upstream_node_id], network_path);
						}
					}
					// If it is a node and is eligible to be in a chain, then set it to chain positioning
					else if !old_upstream_node_is_layer {
						self.try_set_node_to_chain(&old_upstream_node_id, network_path);
					}
					// If a node was previously connected, and it is no longer connected to any nodes, then set its position to absolute at its previous position
					else {
						self.set_absolute_position(&old_upstream_node_id, previous_position, network_path);
					}
				}
				// Load structure if the change is to the document network and to the first or second
				if network_path.is_empty() {
					if matches!(input_connector, InputConnector::Export(0)) {
						self.load_structure();
					} else if let InputConnector::Node { node_id, input_index } = &input_connector {
						// If the connection is made to the first or second input of a node connected to the output, then load the structure
						if self.connected_to_output(node_id, network_path) && (*input_index == 0 || *input_index == 1) {
							self.load_structure();
						}
					}
				}
				self.unload_stack_dependents(network_path);
			}
			_ => {}
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
			// Get the new node template
			node_template = self.map_ids(node_template, &old_node_id, &new_ids, network_path);
			// Insert node into network
			let node_id = *new_ids.get(&old_node_id).unwrap();
			let (document_node, persistent_metadata) = node_template.into_parts();
			let Some(network) = self.network_mut(network_path) else {
				log::error!("Network not found in insert_node");
				return;
			};

			network.nodes.insert(node_id, document_node);
			self.transaction_modified();

			let Some(network_metadata) = self.network_metadata_mut(network_path) else {
				log::error!("Network not found in insert_node");
				return;
			};
			let node_metadata = DocumentNodeMetadata {
				persistent_metadata,
				transient_metadata: DocumentNodeTransientMetadata::default(),
			};
			network_metadata.persistent_metadata.node_metadata.insert(node_id, node_metadata);
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
		let (document_node, persistent_metadata) = node_template.into_parts();
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Network not found in insert_node");
			return None;
		};

		let previous_node = network.nodes.insert(node_id, document_node);
		self.transaction_modified();

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Network not found in insert_node");
			return None;
		};
		let node_metadata = DocumentNodeMetadata {
			persistent_metadata,
			transient_metadata: DocumentNodeTransientMetadata::default(),
		};
		let previous_metadata = network_metadata.persistent_metadata.node_metadata.insert(node_id, node_metadata);

		self.unload_all_nodes_bounding_box(network_path);
		self.unload_node_click_targets(&node_id, network_path);

		previous_node
			.zip(previous_metadata)
			.map(|(document_node, node_metadata)| NodeTemplate::from_parts(document_node, node_metadata.persistent_metadata))
	}

	/// Deletes all nodes in `node_ids` and any sole dependents in the horizontal chain if the node to delete is a layer node.
	pub fn delete_nodes(&mut self, nodes_to_delete: Vec<NodeId>, delete_children: bool, network_path: &[NodeId]) {
		if self.outward_wires(network_path).is_none() {
			log::error!("Could not get outward wires in delete_nodes");
			return;
		}

		// Layer membership is fixed during the expansion phase, so gather it once for the sole-dependent closure
		let layer_nodes = self
			.nested_network(network_path)
			.map(|network| network.nodes.keys().copied().collect::<Vec<_>>())
			.unwrap_or_default()
			.into_iter()
			.filter(|candidate| self.is_layer(candidate, network_path))
			.collect::<HashSet<_>>();

		let mut delete_nodes = HashSet::new();
		for node_id in &nodes_to_delete {
			delete_nodes.insert(*node_id);

			if !delete_children {
				continue;
			};

			// Perform an upstream traversal to try delete children for secondary inputs
			let mut upstream_nodes = (1..self.number_of_inputs(node_id, network_path))
				.filter_map(|input_index| self.upstream_output_connector(&InputConnector::node(*node_id, input_index), network_path).and_then(|oc| oc.node_id()))
				.collect::<Vec<_>>();
			while let Some(upstream_node) = upstream_nodes.pop() {
				// Add the upstream nodes to the traversal
				for input_connector in (0..self.number_of_inputs(&upstream_node, network_path)).map(|input_index| InputConnector::node(upstream_node, input_index)) {
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

			for input_index in 0..self.number_of_displayed_inputs(delete_node_id, network_path) {
				self.disconnect_input(&InputConnector::node(*delete_node_id, input_index), network_path);
			}

			let Some(network) = self.network_mut(network_path) else {
				log::error!("Could not get nested network in delete_nodes");
				continue;
			};

			network.nodes.remove(delete_node_id);
			self.transaction_modified();

			let Some(network_metadata) = self.network_metadata_mut(network_path) else {
				log::error!("Could not get nested network_metadata in delete_nodes");
				continue;
			};
			network_metadata.persistent_metadata.node_metadata.remove(delete_node_id);
			for previous_chain_node in upstream_chain_nodes {
				self.set_chain_position(&previous_chain_node, network_path);
			}
		}

		// Prune this network's pinned display order down to the nodes that still exist, dropping any that were actually removed
		let surviving_nodes = self.nested_network(network_path).map(|network| network.nodes.keys().copied().collect::<HashSet<_>>());
		if let Some(surviving_nodes) = surviving_nodes
			&& let Some(network_metadata) = self.network_metadata_mut(network_path)
		{
			network_metadata.persistent_metadata.pinned_node_order.retain(|node_id| surviving_nodes.contains(node_id));
		}

		// Purge the deleted nodes' cached wire paths, since the per-node unload can no longer reach them once the nodes are gone
		if let Some(network_metadata) = self.network_metadata(network_path) {
			network_metadata
				.transient_metadata
				.wires
				.borrow_mut()
				.retain(|connector, _| connector.node_id().is_none_or(|node_id| !delete_nodes.contains(&node_id)));
		}

		self.unload_all_nodes_bounding_box(network_path);
		// Instead of unloaded all node click targets, just unload the nodes upstream from the deleted nodes. unload_upstream_node_click_targets will not work since the nodes have been deleted.
		self.unload_all_nodes_click_targets(network_path);
		let Some(selected_nodes) = self.selected_nodes_mut(network_path) else {
			log::error!("Could not get selected nodes in NodeGraphMessage::DeleteNodes");
			return;
		};
		selected_nodes.retain_selected_nodes(|node_id| !delete_nodes.contains(node_id));
	}

	/// Removes all references to the node with the given id from the network, and reconnects the input to the node below.
	pub fn remove_references_from_network(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		// TODO: Add more logic to support retaining preview when removing references. Since there are so many edge cases/possible crashes, for now the preview is ended.
		self.stop_previewing(network_path);

		// Check whether the being-deleted node's first (primary) input is a node
		let reconnect_to_input = self.document_node(node_id, network_path).and_then(|node| {
			node.inputs
				.iter()
				.find(|input| input.is_exposed())
				.filter(|input| matches!(input, NodeInput::Node { .. } | NodeInput::Import { .. }))
				.cloned()
		});
		// Get all upstream references
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
				reconnect_node = reconnect_input.as_node().and_then(|node_id| if self.is_stack(&node_id, network_path) { Some(node_id) } else { None });
				self.disconnect_input(&InputConnector::node(*node_id, 0), network_path);
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

	pub fn start_previewing_without_restore(&mut self, network_path: &[NodeId]) {
		// Some logic will have to be performed to prevent the graph positions from being completely changed when the export changes to some previewed node
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in start_previewing_without_restore");
			return;
		};
		network_metadata.persistent_metadata.previewing = Previewing::Yes { root_node_to_restore: None };
	}

	fn stop_previewing(&mut self, network_path: &[NodeId]) {
		if let Previewing::Yes {
			root_node_to_restore: Some(root_node_to_restore),
		} = self.previewing(network_path)
		{
			self.set_input(
				&InputConnector::Export(0),
				NodeInput::node(root_node_to_restore.node_id, root_node_to_restore.output_index),
				network_path,
			);
		}
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in stop_previewing");
			return;
		};
		network_metadata.persistent_metadata.previewing = Previewing::No;
	}

	pub fn set_display_name(&mut self, node_id: &NodeId, display_name: String, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		if node_metadata.persistent_metadata.display_name == display_name {
			return;
		}

		node_metadata.persistent_metadata.display_name = display_name;

		self.transaction_modified();
		self.try_unload_layer_width(node_id, network_path);
		self.unload_node_click_targets(node_id, network_path);
	}

	pub fn set_import_export_name(&mut self, mut name: String, index: ImportOrExport, network_path: &[NodeId]) {
		let Some(encapsulating_node) = self.encapsulating_node_metadata_mut(network_path) else {
			log::error!("Could not get encapsulating network in set_import_export_name");
			return;
		};

		let name_changed = match index {
			ImportOrExport::Import(import_index) => {
				let Some(input_properties) = encapsulating_node.persistent_metadata.input_metadata.get_mut(import_index) else {
					log::error!("Could not get input properties in set_import_export_name");
					return;
				};
				// Only return false if the previous value is the same as the current value
				std::mem::swap(&mut input_properties.persistent_metadata.input_name, &mut name);
				input_properties.persistent_metadata.input_name != name
			}
			ImportOrExport::Export(export_index) => {
				let Some(export_name) = encapsulating_node.persistent_metadata.output_names.get_mut(export_index) else {
					log::error!("Could not get export_name in set_import_export_name");
					return;
				};
				std::mem::swap(export_name, &mut name);
				*export_name != name
			}
		};
		if name_changed {
			self.transaction_modified();
		}
	}

	pub fn set_pinned(&mut self, node_id: &NodeId, network_path: &[NodeId], pinned: bool) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node {node_id} in set_pinned");
			return;
		};

		node_metadata.persistent_metadata.pinned = pinned;

		// Track the node in this network's pinned display order: append when newly pinned, prune when unpinned
		if let Some(network_metadata) = self.network_metadata_mut(network_path) {
			let order = &mut network_metadata.persistent_metadata.pinned_node_order;
			if pinned {
				if !order.contains(node_id) {
					order.push(*node_id);
				}
			} else {
				order.retain(|id| id != node_id);
			}
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

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get network_metadata in reorder_pinned_node");
			return;
		};
		network_metadata.persistent_metadata.pinned_node_order = new_order;

		self.transaction_modified();
	}

	pub fn set_visibility(&mut self, node_id: &NodeId, network_path: &[NodeId], is_visible: bool) {
		let Some(network) = self.network_mut(network_path) else {
			return;
		};
		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		node.visible = is_visible;
		self.transaction_modified();
	}

	pub fn set_locked(&mut self, node_id: &NodeId, network_path: &[NodeId], locked: bool) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		node_metadata.persistent_metadata.locked = locked;
		self.transaction_modified();
		self.try_unload_layer_width(node_id, network_path);
		self.unload_node_click_targets(node_id, network_path);
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
							.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(upstream_sibling_id, 0)))
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
					.get(&OutputConnector::node(*node_id, 0))
					.and_then(|outward_wires| (outward_wires.len() == 1).then(|| outward_wires[0]))
					.and_then(|downstream_connector| if downstream_connector.input_index() == 0 { downstream_connector.node_id() } else { None })
			})
			.filter(|downstream_node_id| self.is_layer(downstream_node_id, network_path))
			.and_then(|downstream_layer| self.position(&downstream_layer, network_path));

		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};

		// First set the position to absolute
		node_metadata.persistent_metadata.node_type_metadata = if is_layer {
			NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::Absolute(position),
			})
		} else {
			NodeTypePersistentMetadata::Node(NodePersistentMetadata {
				position: NodePosition::Absolute(position),
			})
		};

		// Try build the chain
		if is_layer {
			self.try_set_upstream_to_chain(&InputConnector::node(*node_id, 1), network_path);
		} else {
			self.try_set_node_to_chain(node_id, network_path);
		}

		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		// Set the position to stack if necessary
		if let Some(downstream_position) = is_layer.then_some(single_downstream_layer_position).flatten() {
			node_metadata.persistent_metadata.node_type_metadata = NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::Stack((position.y - downstream_position.y - STACK_VERTICAL_GAP).max(0) as u32),
			})
		}

		node_metadata.transient_metadata.layer_width.unload();
		node_metadata.transient_metadata.owned_nodes.unload();

		self.transaction_modified();
		self.unload_stack_dependents(network_path);
		self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		self.unload_all_nodes_bounding_box(network_path);
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
		self.load_structure();
	}

	pub fn toggle_preview(&mut self, toggle_id: NodeId, network_path: &[NodeId]) {
		let Some(network) = self.nested_network(network_path) else {
			return;
		};
		// If new_export is None then disconnect
		let mut new_export = None;
		let mut new_previewing_state = Previewing::No;
		if let Some(export) = network.exports.first() {
			// If there currently an export
			if let NodeInput::Node { node_id, output_index, .. } = export {
				let previous_export_id = *node_id;
				let previous_output_index = *output_index;

				// The export is clicked
				if *node_id == toggle_id {
					// If the current export is clicked and is being previewed end the preview and set either export back to root node or disconnect
					if let Previewing::Yes { root_node_to_restore } = self.previewing(network_path) {
						new_export = root_node_to_restore.map(|root_node| root_node.to_connector());
						new_previewing_state = Previewing::No;
					}
					// The export is clicked and there is no preview
					else {
						new_previewing_state = Previewing::Yes {
							root_node_to_restore: Some(RootNode {
								node_id: previous_export_id,
								output_index: previous_output_index,
							}),
						};
					}
				}
				// The export is not clicked
				else {
					new_export = Some(OutputConnector::node(toggle_id, 0));

					// There is currently a dashed line being drawn
					if let Previewing::Yes { root_node_to_restore } = self.previewing(network_path) {
						// There is also a solid line being drawn
						if let Some(root_node_to_restore) = root_node_to_restore {
							// If the node with the solid line is clicked, then start previewing that node without restore
							if root_node_to_restore.node_id == toggle_id {
								new_export = Some(OutputConnector::node(toggle_id, 0));
								new_previewing_state = Previewing::Yes { root_node_to_restore: None };
							} else {
								// Root node to restore does not change
								new_previewing_state = Previewing::Yes {
									root_node_to_restore: Some(root_node_to_restore),
								};
							}
						}
						// There is a dashed line without a solid line.
						else {
							new_previewing_state = Previewing::Yes { root_node_to_restore: None };
						}
					}
					// Not previewing, there is no dashed line being drawn
					else {
						new_export = Some(OutputConnector::node(toggle_id, 0));
						new_previewing_state = Previewing::Yes {
							root_node_to_restore: Some(RootNode {
								node_id: previous_export_id,
								output_index: previous_output_index,
							}),
						};
					}
				}
			}
			// The primary export is disconnected, so preview the node with nothing to restore, which disconnects the export again when the preview ends
			else {
				new_export = Some(OutputConnector::node(toggle_id, 0));
				new_previewing_state = Previewing::Yes { root_node_to_restore: None };
			}
		}
		match new_export {
			Some(new_export) => {
				self.create_wire(&new_export, &InputConnector::Export(0), network_path);
			}
			None => {
				self.disconnect_input(&InputConnector::Export(0), network_path);
			}
		}
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			return;
		};
		network_metadata.persistent_metadata.previewing = new_previewing_state;
	}
}
