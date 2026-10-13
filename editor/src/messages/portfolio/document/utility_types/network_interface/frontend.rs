//! The shapes the node graph frontend is sent: one entry per visible port, carrying its type, its
//! name and whether it's connected. Read-only projections of the graph, rather than a source of truth.

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

		let connected = self.upstream_output_connector(input_connector, network_path).is_some();

		let (name, description) = match input_connector {
			InputConnector::Node { node_id, input_index } => self.displayed_input_name_and_description(node_id, *input_index, network_path),
			InputConnector::Export(export_index) => {
				let export_description = self
					.encapsulating_node_metadata(network_path)
					.and_then(|metadata| metadata.persistent_metadata.output_descriptions.get(*export_index).cloned())
					.unwrap_or_default();

				(self.displayed_export_name(*export_index, network_path), export_description)
			}
		};

		let valid_types = match input_connector {
			InputConnector::Node { .. } => self.potential_valid_input_types(input_connector, network_path),
			// An export takes any type itself, so it's limited only by what its subgraph node's output feeds, or at the document's root, what can be rendered
			InputConnector::Export(_) => self
				.complete_valid_input_types(input_connector, network_path)
				.into_iter()
				.filter(|valid_type| !valid_type.nested_type().is_generic())
				.collect(),
		};
		let valid_types = distinct_type_names(&valid_types);

		Some(FrontendGraphInput {
			data_type,
			resolved_type,
			name,
			description,
			valid_types,
			connected,
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
				(output_name, self.output_description(node_id, *output_index, network_path))
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
				let description = self
					.persistent_input_metadata(encapsulating_node_id, *import_index, encapsulating_path)
					.map(|metadata| metadata.input_description.clone())
					.unwrap_or_default();

				(self.displayed_import_name(*import_index, network_path), description)
			}
		};
		let data_type = output_type.displayed_type();
		let resolved_type = output_type.resolved_type_node_string();
		let possible_types = distinct_type_names(&self.possible_output_types(output_connector, network_path));
		let connected = self
			.with_outward_wires(network_path, |outward_wires| outward_wires.get(output_connector).is_some_and(|inputs| !inputs.is_empty()))
			.unwrap_or_default();

		Some(FrontendGraphOutput {
			data_type,
			resolved_type,
			name,
			description,
			possible_types,
			connected,
		})
	}

	/// Returns the input name to display in the Inspector. If the name is empty then the type is used.
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

	/// The name a subgraph's import is shown with: its encapsulating node's input name, or else its type, or else its number.
	pub fn displayed_import_name(&self, import_index: usize, network_path: &[NodeId]) -> String {
		let Some((encapsulating_node_id, encapsulating_path)) = network_path.split_last() else {
			return String::new();
		};

		let (import_name, _) = self.displayed_input_name_and_description(encapsulating_node_id, import_index, encapsulating_path);
		if !import_name.is_empty() {
			return import_name;
		}

		match self.output_type(&OutputConnector::Import(import_index), network_path).compiled_nested_type() {
			Some(import_type) => import_type.to_string(),
			None => format!("Import #{import_index}"),
		}
	}

	/// The name a network's export is shown with: its encapsulating node's output name, or else its type, or else its number.
	/// The document network has one export, the canvas.
	pub fn displayed_export_name(&self, export_index: usize, network_path: &[NodeId]) -> String {
		if network_path.is_empty() {
			return "Canvas".to_string();
		}

		let export_name = self
			.encapsulating_node_metadata(network_path)
			.and_then(|metadata| metadata.persistent_metadata.output_names.get(export_index).cloned())
			.unwrap_or_default();
		if !export_name.is_empty() {
			return export_name;
		}

		match self.input_type(&InputConnector::Export(export_index), network_path).compiled_nested_type() {
			Some(export_type) => export_type.to_string(),
			None => format!("Export #{export_index}"),
		}
	}
}

/// The displayed names of `types` in order, leaving out repeats where distinct types share a name.
fn distinct_type_names(types: &[Type]) -> Vec<String> {
	let mut found = HashSet::new();
	types.iter().map(ToString::to_string).filter(|name| found.insert(name.clone())).collect()
}
