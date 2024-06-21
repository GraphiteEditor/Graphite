use std::collections::HashMap;

use bezier_rs::Subpath;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeNetwork};
use graphene_std::{
	renderer::{ClickTarget, Quad},
	uuid::ManipulatorGroupId,
};

use super::{misc::PTZ, nodes::SelectedNodes};

#[derive(Debug, Clone, Default)]
#[serde(default)]
pub struct NodeNetworkInterface {
	/// The node graph that generates this document's artwork. It recursively stores its sub-graphs, so this root graph is the whole snapshot of the document content.
	/// A mutable reference should never be created. It should only be mutated through custom setters which perform the necessary side effects to keep network_metadata in sync
	network: NodeNetwork,
	// Path to the current nested network
	network_path: Vec<NodeId>,
	/// Stores all editor information for a NodeNetwork. For the network this includes viewport transforms, outward links, and bounding boxes. For nodes this includes click target, position, and alias
	/// network_metadata will initialize it if it does not exist, so it cannot be public. If NetworkMetadata exists, then it must be correct. If it is possible for NetworkMetadata to become stale, it should be removed.
	#[serde(skip)]
	network_metadata: HashMap<Vec<NodeId>, NetworkMetadata>,
	// These fields have no side effects are are not related to the network state, although they are stored for every network. Maybe this field should be moved to DocumentMessageHandler?
	#[serde(skip)]
	navigation_metadata: HashMap<Vec<NodeId>, NavigationMetadata>,
}

// Getter methods
impl NodeNetworkInterface {
	pub fn document_network(&self) -> &NodeNetwork {
		&self.network
	}

	pub fn nested_network(&self) -> Option<&NodeNetwork> {
		self.network.nested_network(&network_path)
	}

	/// Get the network the selected nodes are part of, which is either self or the nested network from nested_path. Used to get nodes in the document network when a sub network is open
	pub fn nested_network_for_selected_nodes<'a>(&self, nested_path: &Vec<NodeId>, selected_nodes: impl Iterator<Item = &'a NodeId>) -> Option<&NodeNetwork> {
		if selected_nodes.any(|node_id| self.network.nodes.contains_key(node_id) || self.network.exports_metadata.0 == *node_id || self.network.imports_metadata.0 == *node_id) {
			Some(&self.network)
		} else {
			self.network.nested_network(nested_path)
		}
	}

	pub fn network_path(&self) -> &Vec<NodeId> {
		&self.network_path
	}

	/// Returns network_metadata for the current or document network, and creates a default if it does not exist
	pub fn network_metadata(&self, use_document_network: bool) -> &NetworkMetadata {
		&self
			.network_metadata
			.entry(if use_document_network { Vec::new() } else { self.network_path.clone() })
			.or_insert_with(|| NetworkMetadata::new(&self.network, &self.network_path))
	}

	pub fn navigation_metadata(&self) -> &NavigationMetadata {
		&self.navigation_metadata.entry(self.network_path.clone()).or_insert_with(|| NavigationMetadata::default())
	}

	// Returns a mutable reference, so it should only be used to get data independent from the network with no side effects (such as NavigationMetadata)
	pub fn navigation_metadata_mut(&mut self) -> &mut NavigationMetadata {
		&mut self.navigation_metadata.entry(self.network_path.clone()).or_insert_with(|| NavigationMetadata::default())
	}

	/// Get the combined bounding box of the click targets of the selected nodes in the node graph in viewport space
	pub fn selected_nodes_bounding_box_viewport(&self, selected_nodes: &SelectedNodes) -> Option<[DVec2; 2]> {
		let Some(network) = self.nested_network() else {
			log::error!("Could not get nested network in selected_nodes_bounding_box_viewport");
			return None;
		};

		selected_nodes
			.selected_nodes(network)
			.filter_map(|node| {
				let Some(node_metadata) = self.network_metadata(false).node_metadata.get(&node) else {
					log::debug!("Could not get click target for node {node}");
					return None;
				};
				node_metadata.node_click_target.subpath.bounding_box_with_transform(*self.navigation_metadata().node_graph_to_viewport)
			})
			.reduce(graphene_core::renderer::Quad::combine_bounds)
	}

	/// Gets the bounding box in viewport coordinates for each node in the node graph
	pub fn graph_bounds_viewport_space(&self) -> Option<[DVec2; 2]> {
		self.network_metadata(false)
			.bounding_box_subpath
			.as_ref()
			.and_then(|bounding_box| bounding_box.bounding_box_with_transform(self.navigation_metadata().node_graph_to_viewport))
	}

	pub fn downstream_layer(&self, node_id: &NodeId) -> Option<NodeId> {
		let mut id = id;
		while !self.network.nodes.get(&node_id)?.is_layer {
			id = self.network_metadata(true).outward_wires.get(&id)?.first().copied()?;
		}
		Some(id)
	}
}

// General Setter methods for any changes not directly to position
impl NodeNetworkInterface {
	/// Replaces the current network with another, and returns the old network. Since changes can be made to various sub networks, all network_metadata is reset.
	pub fn replace(&mut self, new_network: NodeNetwork) -> NodeNetwork {
		let old_network = std::mem::replace(&mut self.network, network);
		self.network_metadata.clear();
	}

	pub fn set_export(&mut self, node_id: NodeId, output_index: usize) {
		self.network.exports[0] = NodeInput::node(id, 0);
		// Ensure correct stack positioning
	}
	// Inserts a node at the end of the horizontal node chain from a layer node. The position will be `Position::Chain`
	pub fn add_node_to_chain(&mut self, new_id: NodeId, node_id: NodeId, mut document_node: DocumentNode) -> Option<NodeId> {
		assert!(self.document_network().nodes.contains_key(&node_id), "add_node_to_chain only works in the document network");
		// TODO: node layout system and implementation
	}

	// Inserts a node at the end of vertical layer stack from a parent layer node. The position will be `Position::Stack(calculated y position)`
	pub fn add_layer_to_stack(&mut self, new_id: NodeId, node_id: NodeId, insert_index: usize, mut document_node: DocumentNode) -> Option<NodeId> {
		assert!(self.document_network().nodes.contains_key(&node_id), "add_node_to_stack only works in the document network");
		// TODO: node layout system and implementation
	}
}

// Layout setter methods for handling position and bounding boxes
impl NodeNetworkInterface {
	/// Shifts all nodes upstream from a certain node by a certain offset, and rearranges the graph if necessary
	pub fn shift_upstream(&mut self, node_id: NodeId, shift: IVec2, shift_self: bool) {
		// TODO: node layout system and implementation
		assert!(self.document_network().nodes.contains_key(&node_id), "shift_upstream only works in the document network");

		// let Some(network) = document_network.nested_network(network_path) else {
		// 	log::error!("Could not get nested network for shift_upstream");
		// 	return;
		// };

		// let mut shift_nodes = HashSet::new();
		// if shift_self {
		// 	shift_nodes.insert(node_id);
		// }

		// let mut stack = vec![node_id];
		// while let Some(node_id) = stack.pop() {
		// 	let Some(node) = network.nodes.get(&node_id) else { continue };
		// 	for input in &node.inputs {
		// 		let NodeInput::Node { node_id, .. } = input else { continue };
		// 		if shift_nodes.insert(*node_id) {
		// 			stack.push(*node_id);
		// 		}
		// 	}
		// }

		// for node_id in shift_nodes {
		// 	if let Some(node) = document_network.nodes.get_mut(&node_id) {
		// 		node.metadata.position += shift;
		// 		node_graph.update_click_target(node_id, document_network, network_path.clone());
		// 	}
		// }
	}

	/// Moves a node to the same position as another node, and shifts all upstream nodes
	pub fn move_node_to(&mut self, node_id: NodeId, target_id: NodeId) {}
	/// Inserts a node in the network at the same position as the target node
	pub fn insert_node_at_node(&mut self, new_id: NodeId, mut new_node: DocumentNode, target_id: NodeId) {}

	// Disconnects, moves a node and all upstream children to a stack index, and reconnects
	pub fn move_node_to_stack(&mut self, node_id: NodeId, parent: NodeId) {}

	// Moves a node and all upstream children to the end of a layer chain
	pub fn move_node_to_chain(&mut self, node_id: NodeId, parent: NodeId) {}

	// Inserts a node between 2 other nodes, and shifts the inserted node and its upstream nodes down.
	// used when creating a layer. Should probably be removed/reworked
	pub fn insert_between(
		&self,
		id: NodeId,
		mut new_node: DocumentNode,
		new_node_input: NodeInput,
		new_node_input_index: usize,
		post_node_id: NodeId,
		post_node_input: NodeInput,
		post_node_input_index: usize,
		shift: IVec2,
	) -> Option<NodeId> {
		// TODO: node layout system and implementation

		// assert!(!document_network.nodes.contains_key(&id), "Creating already existing node");
		// let pre_node = document_network.nodes.get_mut(&new_node_input.as_node().expect("Input should reference a node"))?;
		// new_node.metadata.position = pre_node.metadata.position;

		// let post_node = document_network.nodes.get_mut(&post_node_id)?;
		// new_node.inputs[new_node_input_index] = new_node_input;
		// post_node.inputs[post_node_input_index] = post_node_input;

		// node_graph.insert_node(id, new_node, document_network, &Vec::new());

		// self.shift_upstream(node_graph, document_network, &Vec::new(), id, shift, false);

		// Some(id)
	}
}

#[derive(Debug, Clone)]
pub struct NetworkMetadata {
	/// Stores the callers of a node by storing all nodes that use it as an input
	pub outward_wires: HashMap<NodeId, Vec<NodeId>>,
	/// Cache for the bounding box around all nodes in node graph space.
	pub bounding_box_subpath: Option<Subpath<ManipulatorGroupId>>,
	/// Click targets for every node in the network by using the path to that node
	pub node_metadata: HashMap<NodeId, NodeMetadata>,
}

/// Network modification interface. All network modifications should be done through this API
impl NetworkMetadata {
	pub fn new(document_network: &NodeNetwork, network_path: &Vec<NodeId>) -> NetworkMetadata {
		let network = document_network.nested_network(nested_path).expect("Could not get nested network when creating NetworkMetadata");

		// Collect all outward_wires
		let outward_wires = network.collect_outward_wires();

		// Create all node metadata
		let mut node_metadata = network
			.nodes
			.iter()
			.map(|(node_id, node)| (node_id, NodeMetadata::new(node)))
			.collect::<HashMap<NodeId, NodeMetadata>>();
		if let Some(imports_node) = NodeMetadata::new_imports_node(document_network, network_path) {
			node_metadata.insert(network.imports_metadata.0, imports_node)
		}
		node_metadata.insert(network.exports_metadata.0, NodeMetadata::new_exports_node(document_network, network_path));

		// Get bounding box around all nodes
		let bounds = node_metadata
			.iter()
			.filter_map(|(_, node_metadata)| node_metadata.node_click_target.subpath.bounding_box())
			.reduce(Quad::combine_bounds);
		let bounding_box_subpath = bounds.map(|bounds| bezier_rs::Subpath::new_rect(bounds[0], bounds[1]));

		NetworkMetadata {
			outward_wires: outward_wires,
			bounding_box_subpath,
			node_metadata,
		}
	}
	/// Inserts a node into the network and updates the click target
	pub fn insert_node(&mut self, node_id: NodeId, node: DocumentNode, document_network: &mut NodeNetwork, network_path: &Vec<NodeId>) {
		let Some(network) = document_network.nested_network_mut(network_path) else {
			log::error!("Network not found in update_click_target");
			return;
		};
		assert!(
			node_id != network.imports_metadata.0 && node_id != network.exports_metadata.0,
			"Cannot insert import/export node into network.nodes"
		);
		network.nodes.insert(node_id, node);
		self.update_click_target(node_id, document_network, network_path.clone());
	}
}

/// Getter methods
impl NetworkMetadata {
	fn get_node_from_point(&self, point: DVec2) -> Option<NodeId> {
		self.node_metadata
			.iter()
			.map(|(node_id, node_metadata)| (node_id, &node_metadata.node_click_target))
			.find_map(|(node_id, click_target)| if click_target.intersect_point(point, DAffine2::IDENTITY) { Some(*node_id) } else { None })
	}

	fn get_connector_from_point<F>(&self, point: DVec2, click_target_selector: F) -> Option<(NodeId, usize)>
	where
		F: Fn(&NodeMetadata) -> &Vec<ClickTarget>,
	{
		self.node_metadata
			.iter()
			.map(|(node_id, node_metadata)| (node_id, click_target_selector(node_metadata)))
			.find_map(|(node_id, click_targets)| {
				for (index, click_target) in click_targets.iter().enumerate() {
					if click_target.intersect_point(point, DAffine2::IDENTITY) {
						return Some((node_id.clone(), index));
					}
				}
				None
			})
	}

	fn get_visibility_from_point(&self, point: DVec2) -> Option<NodeId> {
		self.node_metadata
			.iter()
			.filter_map(|(node_id, node_metadata)| node_metadata.visibility_click_target.as_ref().map(|click_target| (node_id, click_target)))
			.find_map(|(node_id, click_target)| if click_target.intersect_point(point, DAffine2::IDENTITY) { Some(*node_id) } else { None })
	}
}

#[derive(Debug, Clone)]
struct NodeMetadata {
	/// Cache for all node click targets in node graph space. Ensure update_click_target is called when modifying a node property that changes its size. Currently this is alias, inputs, is_layer, and metadata
	pub node_click_target: ClickTarget,
	/// Cache for all node inputs. Should be automatically updated when update_click_target is called
	pub input_click_targets: Vec<ClickTarget>,
	/// Cache for all node outputs. Should be automatically updated when update_click_target is called
	pub output_click_targets: Vec<ClickTarget>,
	/// Cache for all visibility buttons. Should be automatically updated when update_click_target is called
	pub visibility_click_target: Option<ClickTarget>,
	// Position
	// alias
	/// Stores the width in grid cell units for layer nodes from the left edge of the thumbnail (+12px padding since thumbnail ends between grid spaces) to the end of the node
	pub layer_width: Option<u32>,
}

impl NodeMetadata {
	const GRID_SIZE: u32 = 24;
	/// Create a new NodeMetadata from a `DocumentNode`. layer_width is cached in NodeMetadata
	pub fn new(node: &DocumentNode) -> NodeMetadata {
		let mut layer_width = None;
		let width = if node.is_layer {
			let layer_width_cells = Self::layer_width_cells(node);
			layer_width = Some(layer_width_cells);
			layer_width_cells * Self::GRID_SIZE
		} else {
			5 * Self::GRID_SIZE
		};

		let height = if node.is_layer {
			2 * Self::GRID_SIZE
		} else {
			let inputs_count = node.inputs.iter().filter(|input| input.is_exposed()).count();
			let outputs_count = if let DocumentNodeImplementation::Network(network) = &node.implementation {
				network.exports.len()
			} else {
				1
			};
			std::cmp::max(inputs_count, outputs_count) as u32 * Self::GRID_SIZE
		};
		let mut corner1 = DVec2::new(
			(node.metadata.position.x * Self::GRID_SIZE as i32) as f64,
			(node.metadata.position.y * Self::GRID_SIZE as i32 + if !node.is_layer { (Self::GRID_SIZE / 2) } else { 0 }) as f64,
		);
		let radius = if !node.is_layer { 3. } else { 10. };

		let corner2 = corner1 + DVec2::new(width as f64, height as f64);
		let mut click_target_corner_1 = corner1;
		if node.is_layer && node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
			click_target_corner_1 -= DVec2::new(24., 0.)
		}

		let subpath = bezier_rs::Subpath::new_rounded_rect(click_target_corner_1, corner2, [radius; 4]);
		let stroke_width = 1.;
		let node_click_target = ClickTarget { subpath, stroke_width };

		// Create input/output click targets
		let mut input_click_targets = Vec::new();
		let mut output_click_targets = Vec::new();
		let mut visibility_click_target = None;

		if !node.is_layer {
			let mut node_top_right = corner1 + DVec2::new(5. * 24., 0.);

			let number_of_inputs = node.inputs.iter().filter(|input| input.is_exposed()).count();
			let number_of_outputs = if let DocumentNodeImplementation::Network(network) = &node.implementation {
				network.exports.len()
			} else {
				1
			};

			if !node.has_primary_output {
				node_top_right.y += 24.;
			}

			let input_top_left = DVec2::new(-8., 4.);
			let input_bottom_right = DVec2::new(8., 20.);

			for node_row_index in 0..number_of_inputs {
				let stroke_width = 1.;
				let subpath = Subpath::new_ellipse(
					input_top_left + corner1 + DVec2::new(0., node_row_index as f64 * 24.),
					input_bottom_right + corner1 + DVec2::new(0., node_row_index as f64 * 24.),
				);
				let input_click_target = ClickTarget { subpath, stroke_width };
				input_click_targets.push(input_click_target);
			}

			for node_row_index in 0..number_of_outputs {
				let stroke_width = 1.;
				let subpath = Subpath::new_ellipse(
					input_top_left + node_top_right + DVec2::new(0., node_row_index as f64 * 24.),
					input_bottom_right + node_top_right + DVec2::new(0., node_row_index as f64 * 24.),
				);
				let output_click_target = ClickTarget { subpath, stroke_width };
				output_click_targets.push(output_click_target);
			}
		} else {
			let input_top_left = DVec2::new(-8., -8.);
			let input_bottom_right = DVec2::new(8., 8.);
			let layer_input_offset = corner1 + DVec2::new(2. * 24., 2. * 24. + 8.);

			let stroke_width = 1.;
			let subpath = Subpath::new_ellipse(input_top_left + layer_input_offset, input_bottom_right + layer_input_offset);
			let layer_input_click_target = ClickTarget { subpath, stroke_width };
			input_click_targets.push(layer_input_click_target);

			if node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
				let layer_input_offset = corner1 + DVec2::new(0., 24.);
				let stroke_width = 1.;
				let subpath = Subpath::new_ellipse(input_top_left + layer_input_offset, input_bottom_right + layer_input_offset);
				let input_click_target = ClickTarget { subpath, stroke_width };
				input_click_targets.push(input_click_target);
			}

			// Output
			let layer_output_offset = corner1 + DVec2::new(2. * 24., -8.);
			let stroke_width = 1.;
			let subpath = Subpath::new_ellipse(input_top_left + layer_output_offset, input_bottom_right + layer_output_offset);
			let layer_output_click_target = ClickTarget { subpath, stroke_width };
			output_click_targets.push(layer_output_click_target);

			// Update visibility button click target
			let visibility_offset = corner1 + DVec2::new(width as f64, 24.);
			let subpath = Subpath::new_rounded_rect(DVec2::new(-12., -12.) + visibility_offset, DVec2::new(12., 12.) + visibility_offset, [3.; 4]);
			let stroke_width = 1.;
			let layer_visibility_click_target = ClickTarget { subpath, stroke_width };
			visibility_click_target = Some(layer_visibility_click_target);
		}
		NodeMetadata {
			node_click_target,
			input_click_targets,
			output_click_targets,
			visibility_click_target,
			layer_width,
		}
	}

	/// Returns none if network_path is empty, since the document network does not have an Imports node.
	pub fn new_imports_node(document_network: &NodeNetwork, network_path: &Vec<NodeId>) -> Option<NodeMetadata> {
		let network = document_network.nested_network(nested_path).expect("Could not get nested network when creating NetworkMetadata");

		let mut encapsulating_path = network_path.clone();
		// Import count is based on the number of inputs to the encapsulating node. If the current network is the document network, there is no import node
		encapsulating_path.pop().map(|encapsulating_node| {
			let parent_node = document_network
				.nested_network(&encapsulating_path)
				.expect("Encapsulating path should always exist")
				.nodes
				.get(&encapsulating_node)
				.expect("Last path node should always exist in encapsulating network");
			let import_count = parent_node.inputs.len();

			let width = 5 * Self::GRID_SIZE;
			// 1 is added since the first row is reserved for the "Exports" name
			let height = (import_count + 1) as u32 * Self::GRID_SIZE;

			let corner1 = IVec2::new(
				network.imports_metadata.1.x * Self::GRID_SIZE as i32,
				network.imports_metadata.1.y * Self::GRID_SIZE as i32 + Self::GRID_SIZE as i32 / 2,
			);
			let corner2 = corner1 + IVec2::new(width as i32, height as i32);
			let radius = 3.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(corner1.into(), corner2.into(), [radius; 4]);
			let stroke_width = 1.;
			let node_click_target = ClickTarget { subpath, stroke_width };

			let node_top_right = network.imports_metadata.1 * Self::GRID_SIZE as i32;
			let mut node_top_right = DVec2::new(node_top_right.x as f64 + width as f64, node_top_right.y as f64);
			// Offset 12px due to nodes being centered, and another 24px since the first import is on the second line
			node_top_right.y += 36.;
			let input_top_left = DVec2::new(-8., 4.);
			let input_bottom_right = DVec2::new(8., 20.);

			// Create input/output click targets
			let input_click_targets = Vec::new();
			let mut output_click_targets = Vec::new();
			let visibility_click_target = None;
			for _ in 0..import_count {
				let stroke_width = 1.;
				let subpath = Subpath::new_ellipse(input_top_left + node_top_right, input_bottom_right + node_top_right);
				let top_left_input = ClickTarget { subpath, stroke_width };
				output_click_targets.push(top_left_input);

				node_top_right.y += 24.;
			}
			NodeMetadata {
				node_click_target,
				input_click_targets,
				output_click_targets,
				visibility_click_target,
				layer_width: None,
			}
		})
	}
	pub fn new_exports_node(document_network: &NodeNetwork, network_path: &Vec<NodeId>) -> NodeMetadata {
		let network = document_network.nested_network(nested_path).expect("Could not get nested network when creating NetworkMetadata");

		let width = 5 * Self::GRID_SIZE;
		// 1 is added since the first row is reserved for the "Exports" name
		let height = (network.exports.len() as u32 + 1) * Self::GRID_SIZE;

		let corner1 = IVec2::new(
			network.exports_metadata.1.x * Self::GRID_SIZE as i32,
			network.exports_metadata.1.y * Self::GRID_SIZE as i32 + Self::GRID_SIZE as i32 / 2,
		);
		let corner2 = corner1 + IVec2::new(width as i32, height as i32);
		let radius = 3.;
		let subpath = bezier_rs::Subpath::new_rounded_rect(corner1.into(), corner2.into(), [radius; 4]);
		let stroke_width = 1.;
		let node_click_target = ClickTarget { subpath, stroke_width };

		let node_top_left = network.exports_metadata.1 * Self::GRID_SIZE as i32;
		let mut node_top_left = DVec2::new(node_top_left.x as f64, node_top_left.y as f64);
		// Offset 12px due to nodes being centered, and another 24px since the first export is on the second line
		node_top_left.y += 36.;
		let input_top_left = DVec2::new(-8., 4.);
		let input_bottom_right = DVec2::new(8., 20.);

		// Create input/output click targets
		let mut input_click_targets = Vec::new();
		let output_click_targets = Vec::new();
		let visibility_click_target = None;

		for _ in 0..network.exports.len() {
			let stroke_width = 1.;
			let subpath = Subpath::new_ellipse(input_top_left + node_top_left, input_bottom_right + node_top_left);
			let top_left_input = ClickTarget { subpath, stroke_width };
			input_click_targets.push(top_left_input);

			node_top_left += 24.;
		}

		NodeMetadata {
			node_click_target,
			input_click_targets,
			output_click_targets,
			visibility_click_target,
			layer_width: None,
		}
	}
	fn get_text_width(node: &DocumentNode) -> Option<f64> {
		let document = window().unwrap().document().unwrap();
		let div = match document.create_element("div") {
			Ok(div) => div,
			Err(err) => {
				log::error!("Error creating div: {:?}", err);
				return None;
			}
		};

		// Set the div's style to make it offscreen and single line
		match div.set_attribute("style", "position: absolute; top: -9999px; left: -9999px; white-space: nowrap;") {
			Err(err) => {
				log::error!("Error setting attribute: {:?}", err);
				return None;
			}
			_ => {}
		};

		// From NodeGraphMessageHandler::untitled_layer_label(node)
		let name = (node.alias != "")
			.then_some(node.alias.to_string())
			.unwrap_or(if node.is_layer && node.name == "Merge" { "Untitled Layer".to_string() } else { node.name.clone() });

		div.set_text_content(Some(&name));

		// Append the div to the document body
		match document.body().unwrap().append_child(&div) {
			Err(err) => {
				log::error!("Error setting adding child to document {:?}", err);
				return None;
			}
			_ => {}
		};

		// Measure the width
		let text_width = div.get_bounding_client_rect().width();

		// Remove the div from the document
		match document.body().unwrap().remove_child(&div) {
			Err(_) => log::error!("Could not remove child when rendering text"),
			_ => {}
		};

		Some(text_width)
	}
	pub fn layer_width_cells(node: &DocumentNode) -> u32 {
		let half_grid_cell_offset = 24. / 2.;
		let thumbnail_width = 3. * 24.;
		let gap_width = 8.;
		let text_width = Self::get_text_width(node).unwrap_or_default();
		let icon_width = 24.;
		let icon_overhang_width = icon_width / 2.;

		let text_right = half_grid_cell_offset + thumbnail_width + gap_width + text_width;
		let layer_width_pixels = text_right + gap_width + icon_width - icon_overhang_width;
		((layer_width_pixels / 24.) as u32).max(8)
	}
}

pub struct NavigationMetadata {
	/// The current pan, and zoom state of the viewport's view of the node graph.
	pub node_graph_ptz: PTZ,
	/// Transform from node graph space to viewport space.
	pub node_graph_to_viewport: DAffine2,
}

pub impl Default for NavigationMetadata {
	fn default() -> NavigationMetadata {
		//Default PTZ and transform
		NavigationMetadata {
			node_graph_ptz: PTZ::default(),
			node_graph_to_viewport: DAffine2::IDENTITY,
		}
	}
}
