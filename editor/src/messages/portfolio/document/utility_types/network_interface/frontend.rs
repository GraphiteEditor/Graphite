//! The shapes the node graph frontend is sent: one entry per visible port, carrying its type, its
//! name and what it connects to. Read-only projections of the graph, never a source of truth.

use super::*;

impl NodeNetworkInterface {
	pub fn frontend_imports(&self, network_path: &[NodeId]) -> Vec<Option<FrontendGraphOutput>> {
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

	pub fn frontend_exports(&self, network_path: &[NodeId]) -> Vec<Option<FrontendGraphInput>> {
		let Some(network) = self.nested_network(network_path) else { return Vec::new() };
		let mut frontend_exports = ((0..network.exports.len()).map(|export_index| self.frontend_input_from_connector(&InputConnector::Export(export_index), network_path))).collect::<Vec<_>>();
		if frontend_exports.is_empty() {
			frontend_exports.push(None);
		}
		frontend_exports
	}

	/// Returns None if there is an error, it is a hidden primary export, or a hidden input
	pub fn frontend_input_from_connector(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<FrontendGraphInput> {
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
	pub fn frontend_output_from_connector(&self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Option<FrontendGraphOutput> {
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
			.with_outward_wires(network_path, |outward_wires| outward_wires.get(output_connector).cloned())
			.flatten()
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

	/// Returns the input name to display in the properties panel. If the name is empty then the type is used.
	pub fn displayed_input_name_and_description(&self, node_id: &NodeId, input_index: usize, network_path: &[NodeId]) -> (String, String) {
		let Some(input_metadata) = self.persistent_input_metadata(node_id, input_index, network_path) else {
			log::warn!("input metadata not found in displayed_input_name_and_description");
			return (String::new(), String::new());
		};
		let description = input_metadata.input_description.to_string();
		let name = if input_metadata.input_name.is_empty() {
			self.input_type(&InputConnector::node_at_index(*node_id, input_index), network_path).resolved_type_node_string()
		} else {
			input_metadata.input_name.to_string()
		};
		(name, description)
	}
}
