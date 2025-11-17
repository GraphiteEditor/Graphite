use crate::messages::portfolio::document::node_graph::utility_types::{FrontendGraphInputOld, FrontendGraphOutputOld};
use crate::messages::portfolio::document::utility_types::network_interface::FlowType;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::portfolio::document::utility_types::network_interface::Previewing;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector, TransientMetadata};
use crate::messages::portfolio::document::utility_types::wires::{GraphWireStyle, WirePathOld, WirePathUpdateOld, build_vector_wire};
use graphene_std::node_graph_overlay::types::FrontendGraphDataType;
use graphene_std::uuid::NodeId;
use kurbo::BezPath;
use std::collections::HashMap;

// All these functions will be deleted once the svelte node graph rendering is removed
impl NodeNetworkInterface {
	/// Returns None if there is an error, it is a hidden primary export, or a hidden input
	pub fn frontend_input_from_connector_old(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<FrontendGraphInputOld> {
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
					let mut name = self.display_name(&node_id, network_path);
					if cfg!(debug_assertions) {
						name.push_str(&format!(" (id: {node_id})"));
					}
					format!("{name} output {output_index}")
				}
				OutputConnector::Import(import_index) => format!("Import index {import_index}"),
			})
			.unwrap_or("nothing".to_string());

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
				} else if let Some(export_type_name) = input_type.compiled_nested_type().map(|nested| nested.to_string()) {
					export_type_name
				} else {
					format!("Export index {}", export_index)
				};

				(export_name, String::new())
			}
		};

		Some(FrontendGraphInputOld {
			data_type,
			resolved_type,
			name,
			description,
			valid_types: self.potential_valid_input_types(input_connector, network_path).iter().map(|ty| ty.to_string()).collect(),
			connected_to,
		})
	}

	/// Returns None if there is an error, it is the document network, a hidden primary output or import
	pub fn frontend_output_from_connector_old(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Option<FrontendGraphOutputOld> {
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
				} else if let Some(import_type_name) = output_type.compiled_nested_type().map(|nested| nested.to_string()) {
					import_type_name
				} else {
					format!("Import index {}", import_index)
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
				InputConnector::Node { node_id, input_index } => {
					let mut name = self.display_name(node_id, network_path);
					if cfg!(debug_assertions) {
						name.push_str(&format!(" (id: {node_id})"));
					}
					format!("{name} input {input_index}")
				}
				InputConnector::Export(export_index) => format!("Export index {export_index}"),
			})
			.collect::<Vec<_>>();

		if connected_to.is_empty() {
			connected_to.push("nothing".to_string());
		}

		Some(FrontendGraphOutputOld {
			data_type,
			resolved_type,
			name,
			description,
			connected_to,
		})
	}

	pub fn unload_import_export_wires_old(&mut self, network_path: &[NodeId]) {
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
			self.unload_wire_old(input, network_path);
		}
	}

	pub fn newly_loaded_input_wire(&mut self, input: &InputConnector, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<WirePathUpdateOld> {
		if !self.wire_is_loaded_old(input, network_path) {
			self.load_wire_old(input, graph_wire_style, network_path);
		} else {
			return None;
		}

		let wire = match input {
			InputConnector::Node { node_id, input_index } => {
				let input_metadata = self.transient_input_metadata_old(node_id, *input_index, network_path)?;
				let TransientMetadata::Loaded(wire) = &input_metadata.wire else {
					log::error!("Could not load wire for input: {input:?}");
					return None;
				};
				wire.clone()
			}
			InputConnector::Export(export_index) => {
				let network_metadata = self.network_metadata(network_path)?;
				let Some(TransientMetadata::Loaded(wire)) = network_metadata.transient_metadata.wires.get(*export_index) else {
					log::error!("Could not load wire for input: {input:?}");
					return None;
				};
				wire.clone()
			}
		};
		Some(wire)
	}

	pub fn wire_is_loaded_old(&mut self, input: &InputConnector, network_path: &[NodeId]) -> bool {
		match input {
			InputConnector::Node { node_id, input_index } => {
				let Some(input_metadata) = self.transient_input_metadata_old(node_id, *input_index, network_path) else {
					log::error!("Input metadata should always exist for input");
					return false;
				};
				input_metadata.wire.is_loaded()
			}
			InputConnector::Export(export_index) => {
				let Some(network_metadata) = self.network_metadata(network_path) else {
					return false;
				};
				match network_metadata.transient_metadata.wires.get(*export_index) {
					Some(wire) => wire.is_loaded(),
					None => false,
				}
			}
		}
	}

	fn load_wire_old(&mut self, input: &InputConnector, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) {
		let dashed = match self.previewing(network_path) {
			Previewing::Yes { .. } => match input {
				InputConnector::Node { .. } => false,
				InputConnector::Export(export_index) => *export_index == 0,
			},
			Previewing::No => false,
		};
		let Some(wire) = self.wire_path_from_input_old(input, graph_wire_style, dashed, network_path) else {
			log::error!("Could not load wire path from input");
			return;
		};
		match input {
			InputConnector::Node { node_id, input_index } => {
				let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else { return };
				let Some(input_metadata) = node_metadata.persistent_metadata.input_metadata.get_mut(*input_index) else {
					// log::warn!("Node metadata must exist on node: {input:?}");
					return;
				};
				let wire_update = WirePathUpdateOld {
					id: *node_id,
					input_index: *input_index,
					wire_path_update: Some(wire),
				};
				input_metadata.transient_metadata.wire = TransientMetadata::Loaded(wire_update);
			}
			InputConnector::Export(export_index) => {
				let Some(network_metadata) = self.network_metadata_mut(network_path) else { return };
				if *export_index >= network_metadata.transient_metadata.wires.len() {
					network_metadata.transient_metadata.wires.resize(export_index + 1, TransientMetadata::Unloaded);
				}
				let Some(input_metadata) = network_metadata.transient_metadata.wires.get_mut(*export_index) else {
					return;
				};
				let wire_update = WirePathUpdateOld {
					id: NodeId(u64::MAX),
					input_index: *export_index,
					wire_path_update: Some(wire),
				};
				*input_metadata = TransientMetadata::Loaded(wire_update);
			}
		}
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

	pub fn unload_wires_for_node_old(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
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
			self.unload_wire_old(&input, network_path);
		}
	}

	pub fn unload_wire_old(&mut self, input: &InputConnector, network_path: &[NodeId]) {
		match input {
			InputConnector::Node { node_id, input_index } => {
				let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
					return;
				};
				let Some(input_metadata) = node_metadata.persistent_metadata.input_metadata.get_mut(*input_index) else {
					// log::warn!("Node metadata must exist on node: {input:?}");
					return;
				};
				input_metadata.transient_metadata.wire = TransientMetadata::Unloaded;
			}
			InputConnector::Export(export_index) => {
				let Some(network_metadata) = self.network_metadata_mut(network_path) else {
					return;
				};
				if *export_index >= network_metadata.transient_metadata.wires.len() {
					network_metadata.transient_metadata.wires.resize(export_index + 1, TransientMetadata::Unloaded);
				}
				let Some(input_metadata) = network_metadata.transient_metadata.wires.get_mut(*export_index) else {
					return;
				};
				*input_metadata = TransientMetadata::Unloaded;
			}
		}
	}

	/// When previewing, there may be a second path to the root node.
	pub fn wire_to_root_old(&mut self, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<WirePathUpdateOld> {
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

		let path_string = vector_wire.to_svg();
		let data_type = self.input_type(&input, network_path).displayed_type();
		let wire_path_update = Some(WirePathOld {
			path_string,
			data_type,
			thick,
			dashed: false,
		});

		Some(WirePathUpdateOld {
			id: NodeId(u64::MAX),
			input_index: u32::MAX as usize,
			wire_path_update,
		})
	}

	/// Returns the vector subpath and a boolean of whether the wire should be thick.
	pub fn vector_wire_from_input_old(&mut self, input: &InputConnector, wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<(BezPath, bool)> {
		let Some(input_position) = self.get_input_center(input, network_path) else {
			log::error!("Could not get dom rect for wire end: {input:?}");
			return None;
		};
		// An upstream output could not be found, so the wire does not exist, but it should still be loaded as as empty vector
		let Some(upstream_output) = self.upstream_output_connector(input, network_path) else {
			return Some((BezPath::new(), false));
		};
		let Some(output_position) = self.get_output_center(&upstream_output, network_path) else {
			log::error!("Could not get output port for wire start: {:?}", upstream_output);
			return None;
		};
		let vertical_end = input.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path) && input.input_index() == 0);
		let vertical_start = upstream_output.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path));
		let thick = vertical_end && vertical_start;
		Some((build_vector_wire(output_position, input_position, vertical_start, vertical_end, wire_style), thick))
	}

	pub fn wire_path_from_input_old(&mut self, input: &InputConnector, graph_wire_style: GraphWireStyle, dashed: bool, network_path: &[NodeId]) -> Option<WirePathOld> {
		let (vector_wire, thick) = self.vector_wire_from_input_old(input, graph_wire_style, network_path)?;
		let path_string = vector_wire.to_svg();
		let data_type = self
			.upstream_output_connector(input, network_path)
			.as_ref()
			.map(|output| self.output_type(output, network_path).displayed_type())
			.unwrap_or(FrontendGraphDataType::General);
		Some(WirePathOld {
			path_string,
			data_type,
			thick,
			dashed,
		})
	}

	pub fn collect_layer_widths_old(&mut self, network_path: &[NodeId]) -> (HashMap<NodeId, u32>, HashMap<NodeId, u32>, HashMap<NodeId, bool>) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in collect_layer_widths");
			return (HashMap::new(), HashMap::new(), HashMap::new());
		};
		let nodes = network_metadata
			.persistent_metadata
			.node_metadata
			.iter()
			.filter_map(|(node_id, _)| if self.is_layer(node_id, network_path) { Some(*node_id) } else { None })
			.collect::<Vec<_>>();
		let layer_widths = nodes
			.iter()
			.filter_map(|node_id| self.layer_width(node_id, network_path).map(|layer_width| (*node_id, layer_width)))
			.collect::<HashMap<NodeId, u32>>();
		let chain_widths = nodes.iter().map(|node_id| (*node_id, self.chain_width(node_id, network_path))).collect::<HashMap<NodeId, u32>>();
		let has_left_input_wire = nodes
			.iter()
			.map(|node_id| {
				(
					*node_id,
					!self
						.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow)
						.skip(1)
						.all(|node_id| self.is_chain(&node_id, network_path)),
				)
			})
			.collect::<HashMap<NodeId, bool>>();

		(layer_widths, chain_widths, has_left_input_wire)
	}
}
