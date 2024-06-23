use std::{
	collections::{HashMap, HashSet},
	hash::DefaultHasher,
};

use bezier_rs::Subpath;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::{
	concrete,
	document::{value::TaggedValue, DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork, Position, Previewing},
	Type,
};
use graphene_std::{
	renderer::{ClickTarget, Quad},
	uuid::ManipulatorGroupId,
};
use interpreted_executor::{dynamic_executor::ResolvedDocumentNodeTypes, node_registry::NODE_REGISTRY};
use usvg::filter::Input;

use crate::messages::{
	portfolio::document::node_graph,
	prelude::{BroadcastEvent, GraphOperationMessage, NodeGraphMessage, NodeGraphMessageHandler},
};

use super::{document_metadata::LayerNodeIdentifier, misc::PTZ, nodes::SelectedNodes};

/// Network modification interface. All network modifications should be done through this API
#[derive(Debug, Clone, Default)]
#[serde(default)]
pub struct NodeNetworkInterface {
	/// The node graph that generates this document's artwork. It recursively stores its sub-graphs, so this root graph is the whole snapshot of the document content.
	/// A mutable reference should never be created. It should only be mutated through custom setters which perform the necessary side effects to keep network_metadata in sync
	network: NodeNetwork,
	// TODO: Create a EditorNetwork struct that mirrors the NodeNetwork and is used to store DocumentNodeMetadata
	// editor_network_data: EditorNetworkData,
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

	/// Get the network the selected nodes are part of, which is either self or the nested network from nested_path. Used to get nodes in the document network when a sub network is open
	pub fn nested_network_for_selected_nodes<'a>(&self, nested_path: &Vec<NodeId>, selected_nodes: impl Iterator<Item = &'a NodeId>) -> Option<&NodeNetwork> {
		if selected_nodes.any(|node_id| self.network.nodes.contains_key(node_id) || self.network.exports_metadata.0 == *node_id || self.network.imports_metadata.0 == *node_id) {
			Some(&self.network)
		} else {
			self.network.nested_network(nested_path)
		}
	}

	// Do not make this public
	fn nested_network_mut(&self, use_document_network: bool) -> Option<&mut NodeNetwork> {
		&mut self.network.nested_network_mut(if use_document_network { &Vec::new() } else { &self.network_path })
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

				let parent_node = self
					.document_network()
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

	pub fn set_input(&mut self, input_connector: InputConnector, input: NodeInput, use_document_network: bool) {}

	pub fn create_wire(&mut self, output_connector: OutputConnector, input_connector: InputConnector, use_document_network: bool) {

		//let input_index = NodeGraphMessageHandler::get_input_index(network, input_node, input_node_connector_index);

		// match (output_node_id, input_node_id) {
		// 	// Connecting 2 document nodes
		// 	(Some(output_node_id), Some(input_node_id)) => {
		// 		let input = NodeInput::node(output_node_id, output_node_connector_index);
		// 		responses.add(NodeGraphMessage::SetNodeInput {
		// 			node_id: input_node_id,
		// 			input_index,
		// 			input,
		// 		});
		// 		if network.connected_to_output(input_node_id) {
		// 			responses.add(NodeGraphMessage::RunDocumentGraph);
		// 		}
		// 	}
		// 	// Connecting a document node output to the Export node input
		// 	(Some(output_node_id), None) => {
		// 		let input = NodeInput::node(output_node_id, output_node_connector_index);
		// 		responses.add(NodeGraphMessage::SetNodeInput {
		// 			node_id: network.exports_metadata.0,
		// 			input_index,
		// 			input,
		// 		});
		// 		responses.add(NodeGraphMessage::RunDocumentGraph);
		// 	}
		// 	// Connecting a document node input to the Import node output
		// 	(None, Some(input_node_id)) => {
		// 		let input = NodeInput::network(generic!(T), output_node_connector_index);
		// 		responses.add(NodeGraphMessage::SetNodeInput {
		// 			node_id: input_node_id,
		// 			input_index,
		// 			input,
		// 		});

		// 	}
		// 	// Connecting a Export node input to the Import node output
		// 	(None, None) => {
		// 		// TODO: Add support for flattening NodeInput::Network exports in flatten_with_fns https://github.com/GraphiteEditor/Graphite/issues/1762
		// 		responses.add(DialogMessage::RequestComingSoonDialog { issue: Some(1762) })
		// 		// let input = NodeInput::network(generic!(T), output_node_connector_index);
		// 		// responses.add(NodeGraphMessage::SetNodeInput {
		// 		// 	node_id: network.exports_metadata.0,
		// 		// 	input_index,
		// 		// 	input,
		// 		// });
		// 		// responses.add(NodeGraphMessage::RunDocumentGraph);
		// 	}
		// }
	}

	/// Inserts a node into the document network and updates the click target
	pub fn insert_node(&mut self, node_id: NodeId, node: DocumentNode, use_document_network: bool) {
		// Ensure there is space for the new node
		// let Some(network) = document_network.nested_network_mut(network_path) else {
		// 	log::error!("Network not found in update_click_target");
		// 	return;
		// };
		// assert!(
		// 	node_id != network.imports_metadata.0 && node_id != network.exports_metadata.0,
		// 	"Cannot insert import/export node into network.nodes"
		// );
		// network.nodes.insert(node_id, node);
		// self.update_click_target(node_id, document_network, network_path.clone());
	}

	/// Deletes all nodes in `node_ids` and any sole dependents in the horizontal chain if the node to delete is a layer node.
	/// The various side effects to external data (network metadata, selected nodes, rendering document) are added through responses
	pub fn delete_nodes(&mut self, nodes_to_delete: Vec<NodeId>, reconnect: bool, selected_nodes: &mut SelectedNodes, responses: &mut VecDeque<Message>) {
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
		selected_nodes.retain_selected_nodes(|node_id| !delete_nodes.contains(node_id));
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

		if let Some(Previewing::Yes { root_node_to_restore }) = network.previewing {
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

/// Represents a connector with index based on the [`DocumentNode::inputs`] index, not the visible input index
#[derive(Debug, Clone)]
pub enum Connector {
	Node {node_id: NodeId, port: Port},
	Import(Port),
	Export(Port),
}

/// The index stored by the port is the actual input/output index, not the visible index
pub enum Port {
	Input(usize),
	Output(usize),
}
/// Represents the mapping of Ports to Click Targets for a node
#[derive(Debug, Clone)]
pub struct Ports(Vec<(Port, ClickTarget)>);

impl Ports {
	pub fn new() -> Ports {
		Ports(Vec::new())
	}

	fn insert_port_at_center(&mut self, port: Port, center: DVec2) {
		let subpath = Subpath::new_ellipse(center - DVec2::new(8., 8.), center + DVec2::new(8., 8.));
		self.0.push((port, ClickTarget { subpath, stroke_width: 1. }));
	}

	fn insert_node_input(&mut self, input_index: usize, row_index: usize, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(0, 24. + 24. * row_index as f64);
		self.insert_port_at_center(Port::Input(input_index), center);
	}
	fn insert_node_output(&mut self, output_index: usize, row_index: usize, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(5.*24., 24. + 24. * row_index as f64);
		self.insert_port_at_center(Port::Output(output_index), center);
	}

	fn insert_layer_input(&mut self, input_index: usize, node_top_left: DVec2) {
		let center = if input_index == 0 {
			node_top_left + DVec2::new(2. * 24., 24. * 2. + 8.);
		} else {
			node_top_left + DVec2::new(0., 24. * 1);
		};
		let subpath = Subpath::new_ellipse(center - DVec2::new(8., 8.), center + DVec2::new(8., 8.));
		self.insert_port_at_center(Port::Input(input_index), center);
	}

	fn insert_layer_output(&mut self, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(2.*24., -8);
		self.insert_port_at_center(Port::Output(0), center);
	}

	pub fn iter(&self) -> impl Iterator<Item = &(Port, ClickTarget)> {
		self.0.iter()
	}

	pub fn clicked_port_from_point(&self, point: DVec2) -> Option<Port> {
		self.0.iter().find_map(|(port, click_target)| click_target.intersect_point(point, DAffine2::IDENTITY).then(|| port))
	}
}

/// All fields in NetworkMetadata should automatically be updated by using the network interface API
#[derive(Debug, Clone)]
pub struct NetworkMetadata {
	/// Stores the callers of a node by storing all nodes that use it as an input
	outward_wires: HashMap<NodeId, Vec<NodeId>>,
	/// Cache for the bounding box around all nodes in node graph space.
	bounding_box_subpath: Option<Subpath<ManipulatorGroupId>>,
	/// Click targets and layer widths for every layer in the network
	layer_metadata: HashMap<NodeId, LayerMetadata>,
	/// Click targets for every non layer node in the network 
	node_metadata: HashMap<NodeId, NodeMetadata>,
	/// Import node click targets, which may not exist, such as in the document network
	/// TODO: Delete this and replace with inputs placed on edges of the graph UI
	import_node_click_target: Option<(NodeId, ClickTarget)>,
	/// Import node click targets
	/// TODO: Delete this and replace with outputs placed on edges of the graph UI
	export_node_click_target: (NodeId, ClickTarget),
	/// All import connector click targets
	import_ports: Ports,
	/// All export connector click targets
	export_ports: Ports,
}

impl NetworkMetadata {
	pub const GRID_SIZE: u32 = 24;

	// Create NetworkMetadata from a NodeNetwork
	pub fn new(document_network: &NodeNetwork, network_path: &Vec<NodeId>) -> NetworkMetadata {
		let network = document_network.nested_network(nested_path).expect("Could not get nested network when creating NetworkMetadata");

		// Collect all outward_wires
		let outward_wires = network.collect_outward_wires();

		// Create all node metadata
		// TODO: Instead of iterating over all nodes randomly which then have to iterate downstream in order to get each nodes position, iterate a single time from the exports node upstream to get the position of all the nodes with Chain and Stack position
		let mut node_metadata = HashMap::new();
		let mut layer_metadata = HashMap::new();
		network
			.nodes
			.iter()
			.map(|(node_id, node)| 
			if node.is_layer{layer_metadata.insert(*node_id, LayerMetadata::new(network, &outward_wires, node_id, node))} else {node_metadata.insert(*node_id, NodeMetadata::new(network, &outward_wires, node_id, node))}
		);

		/// Eventually the import/export nodes will be removed here, so calculate node and input click targets seperately
		let import_node_click_target = NodeMetadata::import_node_click_target(document_network, network_path).map(|click_target|(network.imports_metadata.0, click_target));
		let export_node_click_target = (network.exports_metadata.0, NodeMetadata::export_node_click_target(document_network, network_path));

		let import_top_left = network.imports_metadata.1.as_dvec2() * 24.;
		let mut import_ports = Ports::new();
		for output_index in 0..network.imports.len() {
			// Skip first row since the first row is reserved for the "Exports" name
			import_ports.insert_node_output(output_index, output_index+1, import_top_left);
		}

		let export_top_left = network.exports_metadata.1.as_dvec2() * 24.;
		let mut export_ports = Ports::new();
		for output_index in 0..network.exports.len() {
			// Skip first row since the first row is reserved for the "Exports" name
			export_ports.insert_node_input(output_index, output_index+1, export_top_left);
		}

		// Get bounding box around all nodes
		let bounds = node_metadata
			.iter()
			.filter_map(|(_, node_metadata)| node_metadata.node_click_target.subpath.bounding_box())
			.reduce(Quad::combine_bounds);
		let bounding_box_subpath = bounds.map(|bounds| bezier_rs::Subpath::new_rect(bounds[0], bounds[1]));

		NetworkMetadata {
			outward_wires,
			bounding_box_subpath,
			node_metadata,
			layer_metadata,
			import_node_click_target,
			export_node_click_target,
			import_ports,
			export_ports,
		}
	}

	/// Click target getter methods
	fn get_node_from_point(&self, point: DVec2) -> Option<NodeId> {
		self.node_metadata.iter().find_map(|(node_id, node_metadata)| node_metadata.node_click_target.intersect_point(point, DAffine2::IDENTITY).then(|| *node_id))
		.or_else(|| self.import_node_click_target.and_then(|(node_id, click_target)| click_target.intersect_point(point, DAffine2::IDENTITY).then(|| node_id)))
		.or_else(|| self.export_node_click_target.1.intersect_point(point, DAffine2::IDENTITY).then(|| node_id))
		.or_else(|| self.layer_metadata.iter().find_map(|(node_id, layer_metadata)| layer_metadata.layer_click_target.intersect_point(point, DAffine2::IDENTITY).then(|| *node_id)))
	}

	fn get_visibility_from_point(&self, point: DVec2) -> Option<NodeId> {
		self.layer_metadata.iter().find_map(|(node_id, layer_metadata)| layer_metadata.visibility_click_target.intersect_point(point, DAffine2::IDENTITY).then(|| *node_id))
	}

	pub fn get_connector_from_point(&self, point: DVec2) -> Option<Connector> {
		self.node_metadata.iter().find_map(|(node_id, node_metadata)| node_metadata.port_click_targets.clicked_port_from_point(point).map(|port| Connector::Node {node_id, port}))
		.or_else(|| self.import_ports.clicked_port_from_point(point).map(|port| Connector::Import(port)))
		.or_else(|| self.export_ports.clicked_port_from_point(point).map(|port| Connector::Export(port)))
		.or_else(|| self.layer_metadata.iter().find_map(|(node_id, layer_metadata)| layer_metadata.port_click_targets.clicked_port_from_point(point).map(|port| Connector::Node {node_id, port})))
	}
}

/// All fields in LayerMetadata should automatically be updated by using the network interface API
/// If performance is a concern then also cache the absolute position for each node
#[derive(Debug, Clone)]
pub struct LayerMetadata {
	/// Ensure layer_click_target is kept in sync when modifying a node property that changes its size. Currently this is the alias or any changes to the upstream chain
	layer_click_target: ClickTarget,
	/// Stores all port click targets in node graph space in order of the port index
	port_click_targets: Ports,
	/// Cache for all visibility buttons. Should be automatically updated when update_click_target is called
	visibility_click_target: ClickTarget,
	// TODO: Store click target for the preview button, which will appear when the node is a selected/(hovered?) layer node
	// preview_click_target: Option<ClickTarget>,
	
	/// Stores the width in grid cell units for layer nodes from the left edge of the thumbnail (+12px padding since thumbnail ends between grid spaces) to the end of the node
	/// This is necessary since calculating the layer width through web_sys is very slow
	layer_width: u32,

	/// Stores the width in grid cell units for layer nodes from the left edge of the thumbnail to the end of the chain
	/// Should not be a performance concern to calculate when needed with get_chain_width.
	// chain_width: u32,
}

impl LayerMetadata {
	/// Create a new LayerMetadata from a `DocumentNode
	pub fn new(network: &NodeNetwork, outward_links: HashMap<NodeId, Vec<NodeId>>, node_id: &NodeId, node: &DocumentNode) -> LayerMetadata {
		let node_top_left = NodeMetadata::get_position(network, outward_links, node_id).as_dvec2() * 24.;

		// Create input/output click targets
		let mut port_click_targets = Ports::new();
		// Layer inputs
		port_click_targets.insert_layer_input(0, node_top_left);
		if node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
			port_click_targets.insert_layer_input(1, node_top_left);
		}
		port_click_targets.insert_layer_output(node_top_left);

		let layer_width_grid_spaces = Self::layer_width_grid_spaces(node);
		let width = layer_width_grid_spaces * NetworkMetadata::GRID_SIZE;
		let height = 2 * NetworkMetadata::GRID_SIZE;

		// Update visibility button click target
		let visibility_offset = node_top_left + DVec2::new(width as f64, 24.);
		let subpath = Subpath::new_rounded_rect(DVec2::new(-12., -12.) + visibility_offset, DVec2::new(12., 12.) + visibility_offset, [3.; 4]);
		let stroke_width = 1.;
		let visibility_click_target = ClickTarget { subpath, stroke_width };

		// Create layer click target, which is contains the layer and the chain background
		let chain_width_grid_spaces = get_chain_width(network, node_id);

		let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
		let chain_top_left = node_top_left - DVec2::new(chain_width_grid_spaces * NetworkMetadata::GRID_SIZE, 0);
		let radius = 10.;
		let subpath = bezier_rs::Subpath::new_rounded_rect(chain_top_left, node_bottom_right, [radius; 4]);
		let layer_click_target = ClickTarget { subpath, stroke_width: 1. };

		LayerMetadata {
			layer_click_target,
			port_click_targets,
			visibility_click_target,
			layer_width: layer_width_grid_spaces,
		}
	}

	fn get_chain_width(network: &NodeNetwork, node_id: &NodeId) -> u32 {
		let node = network.nodes.get(node_id).expect("Node not found in get_chain_width");
		assert!(node.is_layer, "Node is not a layer node in get_chain_width");
		if node.inputs.len() > 1 {
			let mut last_chain_node_distance = 0u32;
			// Iterate upstream from the layer, and get the number of nodes distance to the last node with Position::Chain
			for (index, (node, _)) in network.upstream_flow_back_from_nodes(vec![node_id], graph_craft::document::FlowType::HorizontalFlow).enumerate() {
				if Position::Chain = node.metadata.position {
					last_chain_node_distance = index;
				}
			}
			last_chain_node_distance
		} else {
			// Layer with no inputs has no chain
			0
		}
	}
}

/// All fields in NodeMetadata should automatically be updated by using the network interface API
/// If performance is a concern then also cache the absolute position for each node
#[derive(Debug, Clone)]
pub struct NodeMetadata {
	/// Ensure node_click_target is kept in sync when modifying a node property that changes its size. Currently this is alias, inputs, is_layer, and metadata
	node_click_target: ClickTarget,
	/// Stores all port click targets in node graph space.
	port_click_targets: Ports,
}

impl NodeMetadata {
	/// Create a new NodeMetadata from a `DocumentNode`
	pub fn new(network: &NodeNetwork, outward_links: HashMap<NodeId, Vec<NodeId>>, node_id: &NodeId, node: &DocumentNode) -> NodeMetadata {
		let node_top_left = NodeMetadata::get_position(network, outward_links, node_id).as_dvec2() * 24.;

		// Create input/output click targets
		let mut port_click_targets = Ports::new();

		let input_row_count = 0;
		for (input_index, input) in node.inputs.iter().enumerate() {
			if input.is_exposed() {
				port_click_targets.insert_node_input(input_index, input_row_count, node_top_left);
			}
			// Primary input row is always displayed, even if the input is not exposed
			if input_index == 0 || input.is_exposed() {
				input_row_count += 1;
			}
		}

		let number_of_outputs = if let DocumentNodeImplementation::Network(network) = &node.implementation {
			network.exports.len()
		} else {
			1
		};
		// If the node does not have a primary output, shift all ports down a row
		let mut output_row_count = if !node.has_primary_output { 1 } else { 0 };
		for output_index in 0..number_of_outputs {
			port_click_targets.insert_node_output(output_index, output_row_count, node_top_left);
			output_row_count += 1;
		}

		let height = std::cmp::max(input_row_count, output_row_count) as u32 * NetworkMetadata::GRID_SIZE;
		let width = 5 * NetworkMetadata::GRID_SIZE;
		let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);

		let radius = 3.;
		let subpath = bezier_rs::Subpath::new_rounded_rect(node_top_left, node_bottom_right, [radius; 4]);
		let node_click_target = ClickTarget { subpath, stroke_width: 1. };

		NodeMetadata { node_click_target, port_click_targets }
	}

	/// Returns none if network_path is empty, since the document network does not have an Imports node.
	pub fn import_node_click_target(document_network: &NodeNetwork, network_path: &Vec<NodeId>) -> Option<ClickTarget> {
		let network = document_network.nested_network(network_path).expect("Could not get nested network when creating NetworkMetadata");

		let mut encapsulating_path = network_path.clone();
		// Import count is based on the number of inputs to the encapsulating node. If the current network is the document network, there is no import node
		// TODO: Use length of import_types in NodeNetwork
		encapsulating_path.pop().map(|encapsulating_node| {
			let parent_node = document_network
				.nested_network(&encapsulating_path)
				.expect("Encapsulating path should always exist")
				.nodes
				.get(&encapsulating_node)
				.expect("Last path node should always exist in encapsulating network");
			let import_count = parent_node.inputs.len();

			let node_top_left = network.imports_metadata.1.as_dvec2() * 24.;

			// Skip first row since the first row is reserved for the "Exports" name
			let mut output_row_count = import_count + 1;
			let width = 5 * NetworkMetadata::GRID_SIZE;
			let height = output_row_count as u32 * NetworkMetadata::GRID_SIZE;
			let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
			let radius = 3.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(node_top_left, node_bottom_right, [radius; 4]);
			ClickTarget { subpath, stroke_width: 1. }
		})
	}

	pub fn export_node_click_target(document_network: &NodeNetwork, network_path: &Vec<NodeId>) -> ClickTarget {
		let network = document_network.nested_network(network_path).expect("Could not get nested network when creating NetworkMetadata");

		let node_top_left = network.exports_metadata.1.as_dvec2() * 24.;
		let input_row_count = network.exports.len() + 1;
		let width = 5 * NetworkMetadata::GRID_SIZE;
		let height = input_row_count as u32 * NetworkMetadata::GRID_SIZE;
		let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
		let radius = 3.;
		let subpath = bezier_rs::Subpath::new_rounded_rect(node_top_left, node_bottom_right, [radius; 4]);
		ClickTarget { subpath, stroke_width: 1. }
	}

	/// Get the top left position and width for any node in the network by recursively iterating downstream
	pub fn get_position(network: &NodeNetwork, outward_links: HashMap<NodeId, Vec<NodeId>>, node_id: &NodeId) -> IVec2 {
		let node = network.nodes.get(node_id).expect("Node not found in get_position");
		match node.metadata.position {
			Position::Absolute(position) => (position),
			Position::Chain => {
				// Iterate through primary flow to find the first Layer
				let mut current_node_id = node_id;
				let mut node_distance_from_layer = 1;
				while let downstream_node_id = outward_links
					.get(current_node_id)
					.and_then(|nodes| nodes.get(0))
					.expect("Downstream layer not found for node with Position::Chain")
				{
					let downstream_node = network.nodes.get(downstream_node_id).expect("Downstream node not found for node with Position::Chain");
					if downstream_node.is_layer {
						// Get the position of the layer
						let layer_position = NodeMetadata::get_position(network, outward_links, downstream_node_id);
						return layer_position + IVec2::new(0, node_distance_from_layer * 8);
					}
					node_distance_from_layer += 1;
					current_node_id = downstream_node_id;
				}
			}
			Position::Stack(y_position) => {
				// Iterate through primary flow to find the first non layer node layer node where the stack feeds into input index 1, or the exports node
				let mut current_node_id = node_id;
				while let Some(downstream_node_id) = outward_links.get(current_node_id).and_then(|nodes| nodes.get(0)) {
					let downstream_node = network.nodes.get(downstream_node_id).expect("Downstream node not found for node with Position::Chain");
					// The stack feeds into a non layer node
					if !downstream_node.is_layer {
						let downstream_node_position = NodeMetadata::get_position(network, outward_links, downstream_node_id);
						// The stack output should be 1 coordinate left of the node
						return downstream_node_position + IVec2::new(-3, y_position);
					}
					// The stack feeds into the side input of a layer node
					else if let Some(NodeInput::Node { node_id, .. }) = downstream_node.inputs.get(1) {
						if node_id == current_node_id {
							let downstream_node_position = NodeMetadata::get_position(network, outward_links, downstream_node_id);
							// The stack output should be 2 coordinates left of the layer node since there is 1 space of padding
							return downstream_node_position + IVec2::new(-4, y_position);
						}
					}
					current_node_id = downstream_node_id;
				}
				// The stack feeds into the exports node
				network.exports_metadata.1 + IVec2::new(-5, y_position)
			}
		}
	}

	fn clicked_node(&self, point: DVec2) -> bool {
		self.node_click_target.intersect_point(point, DAffine2::IDENTITY)
	}

	pub fn clicked_visibility(&self, point: DVec2) -> bool {
		self.layer_metadata
			.is_some_and(|layer_metadata| layer_metadata.visibility_click_target.intersect_point(point, DAffine2::IDENTITY))
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
