use super::*;
use graph_craft::runtime_delta::RuntimeDelta;

// Replay: putting a delta back into the interface, the inverse of the store recording one.
//
// Mechanism only. A mutator decides policy, validates, and lays nodes out, then records what it
// settled on; replaying reproduces that outcome rather than deciding it again. A peer running
// different layout rules must still end up with the state the delta describes.
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
			// snapshot then fills in. Replacing an entry and adding one are the same write here; they
			// differ only in what the storage side has to remove first.
			EditorDelta::Graph(RuntimeDelta::AddNode { network_path, node_id, node }) | EditorDelta::Graph(RuntimeDelta::ReplaceNode { network_path, node_id, node }) => {
				let template = NodeTemplate::from_parts((**node).clone(), DocumentNodePersistentMetadata::default());
				self.insert_node_entry(NodeLocator::new(*node_id, network_path), template);
			}

			EditorDelta::Graph(RuntimeDelta::RemoveNode { network_path, node_id }) => {
				self.remove_node_entry(NodeLocator::new(*node_id, network_path));
			}

			EditorDelta::Graph(RuntimeDelta::SetInput {
				network_path,
				node_id,
				input_index,
				input,
			}) => {
				self.set_input_slot(&InputConnector::node_at_index(*node_id, *input_index), network_path, input.clone());
			}

			EditorDelta::Graph(RuntimeDelta::SetInputs { network_path, node_id, inputs }) => {
				let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else { return };

				// The slots are what changed; their metadata follows in the paired delta. Resizing keeps the
				// two arrays the same length in the meantime, which the rest of the interface assumes.
				let mut input_metadata = node.input_metadata().to_vec();
				input_metadata.resize(inputs.len(), InputMetadata::default());
				node.replace_inputs(inputs.clone(), input_metadata);
			}

			EditorDelta::Graph(RuntimeDelta::SetExport { network_path, export_index, input }) => match input {
				None => {
					self.remove_export_slot(network_path, *export_index);
				}
				// A peer can name a slot past the end of this network's exports, which is an insert here
				// rather than a write to a slot that does not exist yet.
				Some(input) if *export_index >= self.number_of_exports(network_path) => {
					self.insert_export_slot(network_path, *export_index, input.clone(), String::new());
				}
				Some(input) => {
					self.set_input_slot(&InputConnector::Export(*export_index), network_path, input.clone());
				}
			},

			EditorDelta::Graph(RuntimeDelta::SetVisibility { network_path, node_id, visible }) => {
				if let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) {
					node.set_visible(*visible);
				}
			}

			EditorDelta::Graph(RuntimeDelta::SetCallArgument { network_path, node_id, call_argument }) => {
				if let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) {
					node.set_call_argument(call_argument.clone());
				}
			}

			EditorDelta::Graph(RuntimeDelta::SetContextFeatures {
				network_path,
				node_id,
				context_features,
			}) => {
				if let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) {
					node.set_context_features(*context_features);
				}
			}

			EditorDelta::NodeMetadataSnapshot { network_path, node_id, metadata } => {
				if let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) {
					node.replace_metadata((**metadata).clone());
				}
			}

			EditorDelta::NodeInputMetadata {
				network_path,
				node_id,
				input_metadata,
			} => {
				if let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) {
					node.set_input_metadata(input_metadata.clone());
				}
			}

			EditorDelta::NodeMetadata { network_path, node_id, change } => {
				let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else { return };
				match change {
					NodeMetadataChange::NodeType(node_type) => node.set_node_type(node_type.clone()),
					NodeMetadataChange::DisplayName(display_name) => node.set_display_name(display_name.clone()),
					NodeMetadataChange::Locked(locked) => node.set_locked(*locked),
					NodeMetadataChange::Pinned(pinned) => node.set_pinned(*pinned),
					NodeMetadataChange::OutputNames(output_names) => node.set_output_names(output_names.clone()),
					NodeMetadataChange::InputName { index, name } => node.set_input_name(*index, name.clone()),
					NodeMetadataChange::WidgetOverride { index, widget_override } => node.set_widget_override(*index, widget_override.clone()),
				};
			}

			EditorDelta::NetworkMetadata { network_path, change } => {
				let Some(mut network) = self.network_mut(network_path) else { return };
				match change {
					NetworkMetadataChange::Reference(reference) => network.set_reference(reference.clone()),
					NetworkMetadataChange::Previewing(previewing) => network.set_previewing(*previewing),
					NetworkMetadataChange::PinnedOrder(order) => network.set_pinned_order(order.clone()),
				};
			}
		}
	}
}
