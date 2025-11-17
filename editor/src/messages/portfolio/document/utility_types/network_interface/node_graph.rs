use crate::consts::{EXPORTS_TO_RIGHT_EDGE_PIXEL_GAP, EXPORTS_TO_TOP_EDGE_PIXEL_GAP, GRID_SIZE, IMPORTS_TO_LEFT_EDGE_PIXEL_GAP, IMPORTS_TO_TOP_EDGE_PIXEL_GAP};
use crate::messages::portfolio::document::node_graph::utility_types::{FrontendExport, FrontendExports, FrontendImport};
use crate::messages::portfolio::document::utility_types::{
	network_interface::{FlowType, InputConnector, NodeNetworkInterface, OutputConnector, Previewing},
	wires::{GraphWireStyle, build_vector_wire},
};
use glam::{DVec2, IVec2};
use graphene_std::{
	node_graph_overlay::types::{FrontendGraphInputNew, FrontendGraphOutputNew},
	uuid::NodeId,
};
use kurbo::BezPath;

// Functions used to collect data from the network interface for use in rendering the node graph
impl NodeNetworkInterface {
	/// Returns None if there is an error, it is a hidden primary export, or a hidden input
	pub fn frontend_input_from_connector(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<FrontendGraphInputNew> {
		// Return None if it is a hidden input or doesn't exist
		if self.input_from_connector(input_connector, network_path).is_some_and(|input| !input.is_exposed()) {
			return None;
		}
		let input_type = self.input_type(input_connector, network_path);
		let data_type = input_type.displayed_type();

		let name = match input_connector {
			InputConnector::Node { node_id, input_index } => self.displayed_input_name_and_description(node_id, *input_index, network_path).0,
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

				if !export_name.is_empty() {
					export_name
				} else if let Some(export_type_name) = input_type.compiled_nested_type().map(|nested| nested.to_string()) {
					export_type_name
				} else {
					format!("Export index {}", export_index)
				}
			}
		};

		let connected_to_node = self.upstream_output_connector(input_connector, network_path).and_then(|output_connector| output_connector.node_id());

		Some(FrontendGraphInputNew { data_type, name, connected_to_node })
	}

	/// Returns None if there is an error, it is the document network, a hidden primary output or import
	pub fn frontend_output_from_connector(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Option<FrontendGraphOutputNew> {
		let output_type = self.output_type(output_connector, network_path);

		let name = match output_connector {
			OutputConnector::Node { node_id, output_index } => {
				// Do not display the primary output port for a node if it is a network node with a hidden primary export
				if *output_index == 0 && self.hidden_primary_output(node_id, network_path) {
					return None;
				};
				// Get the output name from the interior network export name
				let node_metadata = self.node_metadata(node_id, network_path)?;
				let output_name = node_metadata.persistent_metadata.output_names.get(*output_index).cloned().unwrap_or_default();

				if !output_name.is_empty() { output_name } else { output_type.resolved_type_node_string() }
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
				let import_name = self.displayed_input_name_and_description(encapsulating_node_id, *import_index, encapsulating_path).0;

				if !import_name.is_empty() {
					import_name
				} else if let Some(import_type_name) = output_type.compiled_nested_type().map(|nested| nested.to_string()) {
					import_type_name
				} else {
					format!("Import index {}", *import_index)
				}
			}
		};
		let connected = self
			.outward_wires(network_path)
			.and_then(|outward_wires| outward_wires.get(output_connector))
			.is_some_and(|downstream| downstream.len() > 0);
		let data_type = output_type.displayed_type();
		Some(FrontendGraphOutputNew { data_type, name, connected })
	}

	pub fn chain_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> u32 {
		if self.number_of_displayed_inputs(node_id, network_path) > 1 {
			let mut last_chain_node_distance = 0u32;
			// Iterate upstream from the layer, and get the number of nodes distance to the last node with Position::Chain
			for (index, node_id) in self
				.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalPrimaryOutputFlow)
				.skip(1)
				.enumerate()
				.collect::<Vec<_>>()
			{
				// Check if the node is positioned as a chain
				if self.is_chain(&node_id, network_path) {
					last_chain_node_distance = (index as u32) + 1;
				} else {
					return last_chain_node_distance * 7 + 1;
				}
			}

			last_chain_node_distance * 7 + 1
		} else {
			// Layer with no inputs has no chain
			0
		}
	}

	/// Checks if a layer should display a gap in its left border
	pub fn layer_has_left_border_gap(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).skip(1).any(|node_id| {
			!self.is_chain(&node_id, network_path)
				|| self
					.upstream_output_connector(&InputConnector::node(node_id, 0), network_path)
					.is_some_and(|output_connector| matches!(output_connector, OutputConnector::Import(_)))
		})
	}

	/// Returns the node which should have a dashed border drawn around it
	pub fn previewed_node(&self, network_path: &[NodeId]) -> Option<NodeId> {
		self.upstream_output_connector(&InputConnector::Export(0), network_path)
			.and_then(|output_connector| output_connector.node_id())
			.filter(|output_node| self.root_node(network_path).is_some_and(|root_node| root_node.node_id != *output_node))
	}

	/// If any downstream input are bottom layer inputs, then the thick cap should be displayed above the output port
	pub fn primary_output_connected_to_layer(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(outward_wires) = self.outward_wires(network_path) else {
			log::error!("Could not get outward_wires in primary_output_connected_to_layer");
			return false;
		};
		let Some(downstream_connectors) = outward_wires.get(&OutputConnector::node(*node_id, 0)) else {
			log::error!("Could not get downstream_connectors in primary_output_connected_to_layer");
			return false;
		};
		let downstream_nodes = downstream_connectors
			.iter()
			.filter_map(|connector| if connector.input_index() == 0 { connector.node_id() } else { None })
			.collect::<Vec<_>>();
		downstream_nodes.iter().any(|node_id| self.is_layer(node_id, network_path))
	}

	/// If any upstream nodes are layers, then the thick cap should be displayed below the primary input port
	pub fn primary_input_connected_to_layer(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.input_from_connector(&InputConnector::node(*node_id, 0), network_path)
			.and_then(|input| input.as_node())
			.is_some_and(|node_id| self.is_layer(&node_id, network_path))
	}

	/// The imports contain both the output port and the outward wires
	pub fn frontend_imports(&mut self, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) -> Vec<Option<FrontendImport>> {
		match network_path.split_last() {
			Some((node_id, encapsulating_network_path)) => {
				let Some(node) = self.document_node(node_id, encapsulating_network_path) else {
					log::error!("Could not get node {node_id} in network {encapsulating_network_path:?}");
					return Vec::new();
				};
				let mut frontend_imports = (0..node.inputs.len())
					.map(|import_index| {
						let port = self.frontend_output_from_connector(&OutputConnector::Import(import_index), network_path);
						port.and_then(|port| {
							let outward_wires = self.outward_wires(network_path)?;
							let downstream_inputs = outward_wires.get(&OutputConnector::Import(import_index)).cloned()?;
							let wires = downstream_inputs
								.iter()
								.filter_map(|input_connector| {
									let Some(wire) = self.wire_from_input_new(&input_connector, graph_wire_style, network_path) else {
										log::error!("Could not get wire path for import input: {input_connector:?}");
										return None;
									};
									Some(wire.to_svg())
								})
								.collect::<Vec<_>>();
							Some(FrontendImport { port, wires })
						})
					})
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

	/// The imports contain the export port, the outward wires, and the preview wire if it exists
	pub fn frontend_exports(&mut self, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) -> FrontendExports {
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in frontend exports");
			return FrontendExports::default();
		};
		let mut exports = (0..network.exports.len())
			.map(|export_index| {
				let export_connector = InputConnector::Export(export_index);
				let frontend_export = self.frontend_input_from_connector(&export_connector, network_path);

				frontend_export.and_then(|export| {
					let wire = self.wire_from_input_new(&export_connector, graph_wire_style, network_path).map(|path| path.to_svg());
					Some(FrontendExport { port: export, wire })
				})
			})
			.collect::<Vec<_>>();

		if exports.is_empty() {
			exports.push(None);
		}
		let preview_wire = self.wire_to_root_new(graph_wire_style, network_path).map(|wire| wire.to_svg());
		FrontendExports { exports, preview_wire }
	}

	pub fn import_export_position(&mut self, network_path: &[NodeId]) -> Option<(IVec2, IVec2)> {
		let Some(all_nodes_bounding_box) = self.all_nodes_bounding_box(network_path).cloned() else {
			log::error!("Could not get all nodes bounding box in load_export_ports");
			return None;
		};
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return None;
		};

		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in load_export_ports");
			return None;
		};
		let node_graph_to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
		let target_viewport_top_left = DVec2::new(IMPORTS_TO_LEFT_EDGE_PIXEL_GAP as f64, IMPORTS_TO_TOP_EDGE_PIXEL_GAP as f64);

		let node_graph_pixel_offset_top_left = node_graph_to_viewport.inverse().transform_point2(target_viewport_top_left);

		// A 5x5 grid offset from the top left corner
		let node_graph_grid_space_offset_top_left = node_graph_to_viewport.inverse().transform_point2(DVec2::ZERO) + DVec2::new(5. * GRID_SIZE as f64, 4. * GRID_SIZE as f64);

		// The inner bound of the import is the highest/furthest left of the two offsets
		let top_left_inner_bound = DVec2::new(
			node_graph_pixel_offset_top_left.x.min(node_graph_grid_space_offset_top_left.x),
			node_graph_pixel_offset_top_left.y.min(node_graph_grid_space_offset_top_left.y),
		);

		let offset_from_top_left = if network
			.exports
			.first()
			.is_some_and(|export| export.as_node().is_some_and(|export_node| self.is_layer(&export_node, network_path)))
		{
			DVec2::new(-4. * GRID_SIZE as f64, -2. * GRID_SIZE as f64)
		} else {
			DVec2::new(-4. * GRID_SIZE as f64, 0.)
		};

		let bounding_box_top_left = DVec2::new((all_nodes_bounding_box[0].x / 24. + 0.5).floor() * 24., (all_nodes_bounding_box[0].y / 24. + 0.5).floor() * 24.) + offset_from_top_left;
		let import_top_left = DVec2::new(top_left_inner_bound.x.min(bounding_box_top_left.x), top_left_inner_bound.y.min(bounding_box_top_left.y));
		let rounded_import_top_left = DVec2::new((import_top_left.x / 24.).round() * 24., (import_top_left.y / 24.).round() * 24.);

		let viewport_width = network_metadata.persistent_metadata.navigation_metadata.node_graph_width;

		let target_viewport_top_right = DVec2::new(viewport_width - EXPORTS_TO_RIGHT_EDGE_PIXEL_GAP as f64, EXPORTS_TO_TOP_EDGE_PIXEL_GAP as f64);

		// An offset from the right edge in viewport pixels
		let node_graph_pixel_offset_top_right = node_graph_to_viewport.inverse().transform_point2(target_viewport_top_right);

		// A 5x5 grid offset from the right corner
		let node_graph_grid_space_offset_top_right = node_graph_to_viewport.inverse().transform_point2(DVec2::new(viewport_width, 0.)) + DVec2::new(-5. * GRID_SIZE as f64, 4. * GRID_SIZE as f64);

		// The inner bound of the export is the highest/furthest right of the two offsets.
		// When zoomed out this keeps it a constant grid space away from the edge, but when zoomed in it prevents the exports from getting too far in
		let top_right_inner_bound = DVec2::new(
			node_graph_pixel_offset_top_right.x.max(node_graph_grid_space_offset_top_right.x),
			node_graph_pixel_offset_top_right.y.min(node_graph_grid_space_offset_top_right.y),
		);

		let offset_from_top_right = if network
			.exports
			.first()
			.is_some_and(|export| export.as_node().is_some_and(|export_node| self.is_layer(&export_node, network_path)))
		{
			DVec2::new(2. * GRID_SIZE as f64, -2. * GRID_SIZE as f64)
		} else {
			DVec2::new(4. * GRID_SIZE as f64, 0.)
		};

		let mut bounding_box_top_right = DVec2::new((all_nodes_bounding_box[1].x / 24. + 0.5).floor() * 24., (all_nodes_bounding_box[0].y / 24. + 0.5).floor() * 24.);
		bounding_box_top_right += offset_from_top_right;
		let export_top_right = DVec2::new(top_right_inner_bound.x.max(bounding_box_top_right.x), top_right_inner_bound.y.min(bounding_box_top_right.y));
		let rounded_export_top_right = DVec2::new((export_top_right.x / 24.).round() * 24., (export_top_right.y / 24.).round() * 24.);

		Some((rounded_import_top_left.as_ivec2(), rounded_export_top_right.as_ivec2()))
	}

	pub fn wire_is_thick(&self, input: &InputConnector, network_path: &[NodeId]) -> bool {
		let Some(upstream_output) = self.upstream_output_connector(input, network_path) else {
			return false;
		};
		let vertical_end = input.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path) && input.input_index() == 0);
		let vertical_start = upstream_output.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path));
		vertical_end && vertical_start
	}

	/// Returns the vector subpath and a boolean of whether the wire should be thick.
	pub fn wire_from_input_new(&mut self, input: &InputConnector, wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<BezPath> {
		let Some(input_position) = self.get_input_center(input, network_path) else {
			log::error!("Could not get dom rect for wire end: {input:?}");
			return None;
		};
		// An upstream output could not be found
		let Some(upstream_output) = self.upstream_output_connector(input, network_path) else {
			return None;
		};
		let Some(output_position) = self.get_output_center(&upstream_output, network_path) else {
			log::error!("Could not get output port for wire start: {:?}", upstream_output);
			return None;
		};
		let vertical_end = input.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path) && input.input_index() == 0);
		let vertical_start = upstream_output.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path));
		Some(build_vector_wire(output_position, input_position, vertical_start, vertical_end, wire_style))
	}

	/// When previewing, there may be a second path to the root node.
	pub fn wire_to_root_new(&mut self, graph_wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<BezPath> {
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
		let vertical_start = upstream_output.node_id().is_some_and(|node_id| self.is_layer(&node_id, network_path));
		let vector_wire = build_vector_wire(output_position, input_position, vertical_start, false, graph_wire_style);

		Some(vector_wire)
	}
}
