use super::*;
use graph_craft::runtime_delta::RuntimeDelta;

impl NodeNetworkInterface {
	/// Mirrors a delta into the interface without recording it.
	///
	/// A delta reaching here came from somewhere else, so recording it would offer a change this peer
	/// never made back to whoever sent it. Writing through the store and then dropping what the store
	/// recorded keeps the two parallel trees written by one place, which is the property the store
	/// exists for, without the echo.
	#[cfg_attr(not(test), expect(dead_code, reason = "no caller until deltas from another peer are replayed; the replay tests exercise it"))]
	pub(crate) fn apply(&mut self, delta: &EditorDelta) {
		let recorded = self.deltas.len();
		self.mirror(delta);
		self.deltas.truncate(recorded);
	}

	fn mirror(&mut self, delta: &EditorDelta) {
		match delta {
			// The metadata arrives separately, so a node lands with default metadata that the paired
			// snapshot then fills in
			EditorDelta::Graph(RuntimeDelta::AddNode { network, node_id, node }) | EditorDelta::Graph(RuntimeDelta::ReplaceNode { network, node_id, node }) => {
				let template = NodeTemplate::from_parts((**node).clone(), DocumentNodePersistentMetadata::default());
				self.insert_node_entry_at(network.as_location(), *node_id, template);
			}

			EditorDelta::Graph(RuntimeDelta::RemoveNode { network, node_id }) => {
				self.remove_node_entry_at(network.as_location(), *node_id);
			}

			EditorDelta::Graph(RuntimeDelta::SetInput { network, node_id, input_index, input }) => {
				self.set_input_slot_at(&InputConnector::node_at_index(*node_id, *input_index), network.as_location(), input.clone());
			}

			EditorDelta::Graph(RuntimeDelta::SetInputs { network, node_id, inputs }) => {
				let Some(mut node) = self.node_mut_at(network.as_location(), *node_id) else { return };

				// The slots are what changed; their metadata follows in the paired delta. Resizing keeps the
				// two arrays the same length in the meantime, which the rest of the interface assumes.
				let mut input_metadata = node.input_metadata().to_vec();
				input_metadata.resize(inputs.len(), InputMetadata::default());
				node.replace_inputs(inputs.clone(), input_metadata);
			}

			EditorDelta::Graph(RuntimeDelta::SetExport { network, export_index, input }) => {
				let location = network.as_location();
				let number_of_exports = self.document_network().located_network(location).map_or(0, |network| network.exports.len());
				match input {
					None => {
						self.remove_export_slot_at(location, *export_index);
					}
					// A peer can name a slot past the end of this network's exports, which is an insert here
					// rather than a write to a slot that does not exist yet.
					Some(input) if *export_index >= number_of_exports => {
						self.insert_export_slot_at(location, *export_index, input.clone(), String::new(), String::new());
					}
					Some(input) => {
						self.set_input_slot_at(&InputConnector::Export(*export_index), location, input.clone());
					}
				}
			}

			EditorDelta::Graph(RuntimeDelta::SetVisibility { network, node_id, visible }) => {
				if let Some(mut node) = self.node_mut_at(network.as_location(), *node_id) {
					node.set_visible(*visible);
				}
			}

			EditorDelta::Graph(RuntimeDelta::SetCallArgument { network, node_id, call_argument }) => {
				if let Some(mut node) = self.node_mut_at(network.as_location(), *node_id) {
					node.set_call_argument(call_argument.clone());
				}
			}

			EditorDelta::NodeMetadataSnapshot { network, node_id, metadata } => {
				if let Some(mut node) = self.node_mut_at(network.as_location(), *node_id) {
					node.replace_metadata((**metadata).clone());
				}
			}

			EditorDelta::NodeInputMetadata { network, node_id, input_metadata } => {
				if let Some(mut node) = self.node_mut_at(network.as_location(), *node_id) {
					node.set_input_metadata(input_metadata.clone());
				}
			}

			EditorDelta::NodeMetadata { network, node_id, change } => {
				let Some(mut node) = self.node_mut_at(network.as_location(), *node_id) else { return };
				match change {
					NodeMetadataChange::NodeType(node_type) => node.set_node_type(node_type.clone()),
					NodeMetadataChange::DisplayName(display_name) => node.set_display_name(display_name.clone()),
					NodeMetadataChange::Locked(locked) => node.set_locked(*locked),
					NodeMetadataChange::Pinned(pinned) => node.set_pinned(*pinned),
					NodeMetadataChange::OutputNames(output_names) => node.set_output_names(output_names.clone()),
					NodeMetadataChange::OutputDescriptions(output_descriptions) => node.set_output_descriptions(output_descriptions.clone()),
					NodeMetadataChange::InputName { index, name } => node.set_input_name(*index, name.clone()),
					NodeMetadataChange::WidgetOverride { index, widget_override } => node.set_widget_override(*index, widget_override.clone()),
				};
			}

			EditorDelta::NetworkMetadata { network, change } => {
				let Some(mut network) = self.network_mut_at(network.as_location()) else { return };
				match change {
					NetworkMetadataChange::Reference(reference) => network.set_reference(reference.clone()),
					NetworkMetadataChange::PinnedOrder(order) => network.set_pinned_order(order.clone()),
					NetworkMetadataChange::ImportMetadata(import_metadata) => network.set_import_metadata(import_metadata.clone()),
					NetworkMetadataChange::ExportNames(export_names) => network.set_export_names(export_names.clone()),
					NetworkMetadataChange::ExportDescriptions(export_descriptions) => network.set_export_descriptions(export_descriptions.clone()),
				};
			}

			EditorDelta::DefinitionsChanged => log::warn!("Replayed a definitions table change, which apply cannot mirror yet, so this peer's definitions table may be stale"),
		}
	}
}
