use std::{collections::HashMap, hash::DefaultHasher};

use bezier_rs::Subpath;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::{concrete, document::{value::TaggedValue, DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork, Previewing}, Type};
use graphene_std::{
	renderer::{ClickTarget, Quad},
	uuid::ManipulatorGroupId,
};
use interpreted_executor::{dynamic_executor::ResolvedDocumentNodeTypes, node_registry::NODE_REGISTRY};

use crate::messages::prelude::{BroadcastEvent, GraphOperationMessage, NodeGraphMessage, NodeGraphMessageHandler};

use super::{document_metadata::LayerNodeIdentifier, misc::PTZ, nodes::SelectedNodes};

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
	#[serde(skip)]
	pub resolved_types: ResolvedDocumentNodeTypes,
}

// Getter methods
impl NodeNetworkInterface {
	pub fn document_network(&self) -> &NodeNetwork {
		&self.network
	}

	pub fn nested_network(&self) -> Option<&NodeNetwork> {
		self.network.nested_network(&network_path)
	}

	pub fn document_or_nested_network(&self, use_document_network: bool) -> Option<&NodeNetwork> {
		if use_document_network {
			Some(&self.network)
		} else {
			self.nested_network()
		}
	}

	// Do not make this public
	fn nested_network_mut(&self, use_document_network: bool) -> Option<&mut NodeNetwork> {
		&mut self.network.nested_network_mut(if use_document_network { &Vec::new() } else { &self.network_path })
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

	/// Returns network_metadata for the selected nodes, and creates a default if it does not exist
	pub fn network_metadata_for_selected_nodes(&self, selected_nodes: impl Iterator<Item = &'a NodeId>) -> &NetworkMetadata {
		if selected_nodes.any(|node_id| self.network.nodes.contains_key(node_id) || self.network.exports_metadata.0 == *node_id || self.network.imports_metadata.0 == *node_id) {
			self.network_metadata(true)
		} else {
			self.network_metadata(false)
		}
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

	/// Returns the first downstream layer from a node, inclusive. If the node is a layer, it will return itself
	pub fn downstream_layer(&self, node_id: &NodeId) -> Option<LayerNodeIdentifier> {
		let mut id = *node_id;
		while !self.network.nodes.get(&node_id)?.is_layer {
			id = self.network_metadata(true).outward_wires.get(&id)?.first().copied()?;
		}
		Some(LayerNodeIdentifier::new(id, self.document_network()))
	}

	/// Get the [`Type`] for any `node_id` and `input_index`. The `network_path` is the path to the encapsulating node (including the encapsulating node). The `node_id` is the selected node.
	pub fn get_input_type(&self, node_id: NodeId, input_index: usize, use_document_network: bool) -> Type {
		let Some(network) = self.document_or_nested_network(use_document_network) else {
			log::error!("Could not get network in get_tagged_value");
			return concrete!(());
		};

		// TODO: Store types for all document nodes, not just the compiled proto nodes, which currently skips isolated nodes
		let node_id_path = &[&self.network_path[..], &[node_id]].concat();
		let input_type = self.resolved_types.inputs.get(&graph_craft::document::Source {
			node: node_id_path.clone(),
			index: input_index,
		});

		if let Some(input_type) = input_type {
			input_type.clone()
		} else if node_id == network.exports_metadata.0 {
			if let Some(parent_node_id) = network_path.last() {
				let mut parent_path = network_path.clone();
				parent_path.pop();

				let parent_node = self.document_network()
					.nested_network(&parent_path)
					.expect("Parent path should always exist")
					.nodes
					.get(&parent_node_id)
					.expect("Last path node should always exist in parent network");

				let output_types = NodeGraphMessageHandler::get_output_types(parent_node, &self.resolved_types, &self.network_path);
				output_types.iter().nth(input_index).map_or_else(
					|| {
						warn!("Could not find output type for export node {node_id}");
						concrete!(())
					},
					|output_type| output_type.clone().map_or(concrete!(()), |output| output),
				)
			} else {
				concrete!(graphene_core::ArtboardGroup)
			}
		} else {
			// TODO: Once there is type inference (#1621), replace this workaround approach when disconnecting node inputs with NodeInput::Node(ToDefaultNode),
			// TODO: which would be a new node that implements the Default trait (i.e. `Default::default()`)

			// Resolve types from proto nodes in node_registry
			let Some(node) = network.nodes.get(&node_id) else {
				return concrete!(());
			};

			fn get_type_from_node(node: &DocumentNode, input_index: usize) -> Type {
				match &node.implementation {
					DocumentNodeImplementation::ProtoNode(protonode) => {
						let Some(node_io_hashmap) = NODE_REGISTRY.get(&protonode) else {
							log::error!("Could not get hashmap for proto node: {protonode:?}");
							return concrete!(());
						};

						let mut all_node_io_types = node_io_hashmap.keys().collect::<Vec<_>>();
						all_node_io_types.sort_by_key(|node_io_types| {
							let mut hasher = DefaultHasher::new();
							node_io_types.hash(&mut hasher);
							hasher.finish()
						});
						let Some(node_types) = all_node_io_types.first() else {
							log::error!("Could not get node_types from hashmap");
							return concrete!(());
						};

						let skip_footprint = if node.manual_composition.is_some() { 1 } else { 0 };

						let Some(input_type) = std::iter::once(node_types.input.clone())
							.chain(node_types.parameters.clone().into_iter())
							.nth(input_index + skip_footprint)
						else {
							log::error!("Could not get type");
							return concrete!(());
						};

						input_type
					}
					DocumentNodeImplementation::Network(network) => {
						for node in &network.nodes {
							for (network_node_input_index, input) in node.1.inputs.iter().enumerate() {
								if let NodeInput::Network { import_index, .. } = input {
									if *import_index == input_index {
										return get_type_from_node(&node.1, network_node_input_index);
									}
								}
							}
						}
						// Input is disconnected
						concrete!(())
					}
					_ => concrete!(()),
				}
			}

			get_type_from_node(node, input_index)
		}
	}
}

// Setter methods for layer related changes in the document network not directly to position
// TODO: assert!(!self.network_interface.document_network().nodes.contains_key(&id), "Creating already existing node");
impl NodeNetworkInterface {
	/// Replaces the current network with another, and returns the old network. Since changes can be made to various sub networks, all network_metadata is reset.
	pub fn replace(&mut self, new_network: NodeNetwork) -> NodeNetwork {
		let old_network = std::mem::replace(&mut self.network, network);
		self.network_metadata.clear();
	}

	//TODO: Remove and replace with passing NodeConnector
	pub fn set_export(&mut self, node_id: NodeId, output_index: usize, use_document_network: bool) {
		self.network.exports[0] = NodeInput::node(id, 0);
		// Ensure correct stack positioning
	}

	pub fn set_input(&mut self, input_connector: InputConnector, input: NodeInput, skip_rerender: bool, use_document_network: bool) {}

	/// Deletes all nodes in `node_ids` and any sole dependents in the horizontal chain if the node to delete is a layer node.
	/// The various side effects to external data (network metadata, selected nodes, rendering document) are added through responses
	pub fn delete_nodes(&mut self, nodes_to_delete: Vec<NodeId>, reconnect: bool, selected_nodes: &SelectedNodes, responses: &mut VecDeque<Message>) {
		//TODO: Pass as parameter
		let use_document_network = selected_nodes
			.selected_nodes_ref()
			.iter()
			.any(|node_id| self.document_network().nodes.contains_key(node_id) || self.document_network().exports_metadata.0 == *node_id || self.document_network().imports_metadata.0 == *node_id);

		let Some(network) = self.document_or_nested_network(use_document_network) else {
			return;
		};

		let outward_wires = self.network_metadata(use_document_network).outward_wires;

		let mut delete_nodes = HashSet::new();
		for node_id in &node_ids {
			delete_nodes.insert(*node_id);

			if !reconnect {
				continue;
			};
			let Some(node) = network.nodes.get(&node_id) else {
				continue;
			};
			let child_id = node.inputs.get(1).and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id) } else { None });
			let Some(child_id) = child_id else {
				continue;
			};

			for (_, upstream_id) in network.upstream_flow_back_from_nodes(vec![*child_id], graph_craft::document::FlowType::UpstreamFlow) {
				// This does a downstream traversal starting from the current node, and ending at either a node in the `delete_nodes` set or the output.
				// If the traversal find as child node of a node in the `delete_nodes` set, then it is a sole dependent. If the output node is eventually reached, then it is not a sole dependent.
				let mut stack = vec![upstream_id];
				let mut can_delete = true;

				while let Some(current_node) = stack.pop() {
					let Some(downstream_nodes) = outward_wires.get(&current_node) else { continue };
					for downstream_node in downstream_nodes {
						// If the traversal reaches the root node, and the root node should not be deleted, then the current node is not a sole dependent
						if network
							.get_root_node()
							.is_some_and(|root_node| root_node.id == *downstream_node && !delete_nodes.contains(&root_node.id))
						{
							can_delete = false;
						} else if !delete_nodes.contains(downstream_node) {
							stack.push(*downstream_node);
						}
						// Continue traversing over the downstream sibling, which happens if the current node is a sibling to a node in node_ids
						else {
							for deleted_node_id in &node_ids {
								let Some(output_node) = network.nodes.get(&deleted_node_id) else { continue };
								let Some(input) = output_node.inputs.get(0) else { continue };

								if let NodeInput::Node { node_id, .. } = input {
									if *node_id == current_node {
										stack.push(*deleted_node_id);
									}
								}
							}
						}
					}
				}
				if can_delete {
					delete_nodes.insert(upstream_id);
				}
			}
		}

		for delete_node_id in delete_nodes {
			if !self.remove_references_from_network(delete_node_id, reconnect, use_document_network) {
				log::error!("could not remove references from network");
				continue;
			}
			//TODO: Create function/side effects for updating all the network state/position
			let Some(network) = self.nested_network_mut(use_document_network) else {
				log::error!("Could not get nested network for delete_nodes");
				continue;
			};
			network.nodes.remove(&node_id);
			//node_graph.update_click_target(node_id, document_network, network_path.clone());
		}
		// Updates the selected nodes, and rerender the document
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		responses.add(GraphOperationMessage::LoadStructure);
	}

	pub fn remove_references_from_network(&mut self, deleting_node_id: NodeId, reconnect: bool, use_document_network: bool) -> bool {
		let Some(network) = self.document_or_nested_network(use_document_network) else {
			log::error!("Could not get nested network in remove_references_from_network");
			return;
		};
		let mut reconnect_to_input: Option<NodeInput> = None;

		if reconnect {
			// Check whether the being-deleted node's first (primary) input is a node
			if let Some(node) = network.nodes.get(&deleting_node_id) {
				// Reconnect to the node below when deleting a layer node.
				if matches!(&node.inputs.get(0), Some(NodeInput::Node { .. })) || matches!(&node.inputs.get(0), Some(NodeInput::Network { .. })) {
					reconnect_to_input = Some(node.inputs[0].clone());
				}
			}
		}

		let mut nodes_to_set_input = Vec::new();

		// Boolean flag if the downstream input can be reconnected to the upstream node
		let mut can_reconnect = true;

		for (node_id, input_index, input) in network
			.nodes
			.iter()
			.filter_map(|(node_id, node)| {
				if *node_id == deleting_node_id {
					None
				} else {
					Some(node.inputs.iter().enumerate().map(|(index, input)| (*node_id, index, input)))
				}
			})
			.flatten()
			.chain(network.exports.iter().enumerate().map(|(index, input)| (network.exports_metadata.0, index, input)))
		{
			let NodeInput::Node { node_id: upstream_node_id, .. } = input else { continue };
			if *upstream_node_id != deleting_node_id {
				continue;
			}

			// Do not reconnect export to import until (#1762) is solved
			if node_id == network.exports_metadata.0 && reconnect_to_input.as_ref().is_some_and(|reconnect| matches!(reconnect, NodeInput::Network { .. })) {
				can_reconnect = false;
			}

			// Do not reconnect to EditorApi network input in the document network.
			if use_document_network && reconnect_to_input.as_ref().is_some_and(|reconnect| matches!(reconnect, NodeInput::Network { .. })) {
				can_reconnect = false;
			}

			// Only reconnect if the output index for the node to be deleted is 0
			if can_reconnect && reconnect_to_input.is_some() {
				// None means to use reconnect_to_input, which can be safely unwrapped
				nodes_to_set_input.push((node_id, input_index, None));

				// Only one node can be reconnected
				can_reconnect = false;
			} else {
				// Disconnect input
				let tagged_value = TaggedValue::from_type(&self.get_input_type(node_id, input_index, use_document_network));
				let value_input = NodeInput::value(tagged_value, true);
				nodes_to_set_input.push((node_id, input_index, Some(value_input)));
			}
		}

		let Some(network) = self.document_or_nested_network(use_document_network) else { return false };

		if let Some(Previewing::Yes { root_node_to_restore }) = network.previewing
		{
			if let Some(root_node_to_restore) = root_node_to_restore {
				if root_node_to_restore.id == deleting_node_id {
					self.nested_network_mut(use_document_network).unwrap().start_previewing_without_restore();
				}
			}
		}

		//TODO: Rework using interface API
		// for (node_id, input_index, value_input) in nodes_to_set_input {
		// 	if let Some(value_input) = value_input {
		// 		// Disconnect input to root node only if not previewing
		// 		if self.network
		// 			.nested_network(&network_path)
		// 			.is_some_and(|network| node_id != network.exports_metadata.0 || matches!(&network.previewing, Previewing::No))
		// 		{
		// 			self.set_input( network_path, node_id, input_index, value_input, is_document_network);
		// 		} else if let Some(Previewing::Yes { root_node_to_restore }) = document_network.nested_network(network_path).map(|network| &network.previewing) {
		// 			if let Some(root_node) = root_node_to_restore {
		// 				if node_id == root_node.id {
		// 					self.network.nested_network_mut(&network_path).unwrap().start_previewing_without_restore();
		// 				} else {
		// 					ModifyInputsContext::set_input(
		// 						node_graph,
		// 						document_network,
		// 						network_path,
		// 						node_id,
		// 						input_index,
		// 						NodeInput::node(root_node.id, root_node.output_index),

		// 					);
		// 				}
		// 			} else {
		// 				ModifyInputsContext::set_input(node_graph, document_network, network_path, node_id, input_index, value_input, is_document_network);
		// 			}
		// 		}
		// 	}
		// 	// Reconnect to node upstream of the deleted node
		// 	else if document_network
		// 		.nested_network(network_path)
		// 		.is_some_and(|network| node_id != network.exports_metadata.0 || matches!(network.previewing, Previewing::No))
		// 	{
		// 		if let Some(reconnect_to_input) = reconnect_to_input.clone() {
		// 			ModifyInputsContext::set_input(node_graph, document_network, network_path, node_id, input_index, reconnect_to_input, is_document_network);
		// 		}
		// 	}
		// 	// Reconnect previous root node to the export, or disconnect export
		// 	else if let Some(Previewing::Yes { root_node_to_restore }) = document_network.nested_network(network_path).map(|network| &network.previewing) {
		// 		if let Some(root_node) = root_node_to_restore {
		// 			ModifyInputsContext::set_input(
		// 				node_graph,
		// 				document_network,
		// 				network_path,
		// 				node_id,
		// 				input_index,
		// 				NodeInput::node(root_node.id, root_node.output_index),
		// 			);
		// 		} else if let Some(reconnect_to_input) = reconnect_to_input.clone() {
		// 			ModifyInputsContext::set_input(node_graph, document_network, network_path, node_id, input_index, reconnect_to_input, is_document_network);
		// 			document_network.nested_network_mut(network_path).unwrap().start_previewing_without_restore();
		// 		}
		// 	}
		// }
		true
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

	// Disconnects, moves a node and all upstream children to a stack index, and reconnects
	pub fn move_node_to_stack(&mut self, node_id: NodeId, parent: NodeId) {}

	// Moves a node and all upstream children to the end of a layer chain
	pub fn move_node_to_chain(&mut self, node_id: NodeId, parent: NodeId) {}
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
