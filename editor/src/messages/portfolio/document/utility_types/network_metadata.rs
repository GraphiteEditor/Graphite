use std::{
	collections::{HashMap, HashSet},
	hash::DefaultHasher,
};

use bezier_rs::Subpath;
use glam::{DAffine2, DVec2, IVec2, IVec2};
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
	portfolio::document::node_graph::{self, document_node_types::DocumentNodeDefinition},
	prelude::{BroadcastEvent, GraphOperationMessage, NodeGraphMessage, NodeGraphMessageHandler},
};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, FlowType, NodeId, NodeInput, NodeNetwork, Previewing, Source};

use super::{document_metadata::LayerNodeIdentifier, misc::PTZ, nodes::SelectedNodes};

/// All network modifications should be done through this API, so the fields cannot be public. However, all fields within this struct can be public since it it not possible to have a public mutable reference.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkInterface {
	/// The node graph that generates this document's artwork. It recursively stores its sub-graphs, so this root graph is the whole snapshot of the document content.
	/// A mutable reference should never be created. It should only be mutated through custom setters which perform the necessary side effects to keep network_metadata in sync
	network: NodeNetwork,
	/// Stores all editor information for a NodeNetwork. For the network this includes viewport transforms, outward links, and bounding boxes. For nodes this includes click target, position, and alias
	network_metadata: NodeNetworkMetadata,
	// Path to the current nested network. Used by the editor to keep track of what network is currently open.
	network_path: Vec<NodeId>,
	/// All input/output types based on the compiled network.
	#[serde(skip)]
	pub resolved_types: ResolvedDocumentNodeTypes,
}

// Public immutable getters for the network interface
impl NodeNetworkInterface {
	pub fn document_network(&self) -> &NodeNetwork {
		&self.network
	}

	/// Gets the nested network based on network_path if use_document_network is false. If it is true then the document network is returned.
	pub fn network(&self, use_document_network: bool) -> Option<&NodeNetwork> {
		if use_document_network {
			Some(self.document_network())
		} else {
			self.network.nested_network(&self.network_path)
		}
	}

	pub fn document_network_metadata(&self) -> &NodeNetworkMetadata {
		&self.network_metadata
	}

	/// The network metadata should always exist for the current network
	pub fn network_metadata(&self, use_document_network: bool) -> Option<&NodeNetworkMetadata> {
		if use_document_network {
			Some(&self.document_network_metadata())
		} else {
			self.network_metadata.nested_metadata(&self.network_path)
		}
	}

	pub fn selected_nodes_in_document_network(&self, selected_nodes: impl Iterator<Item = &'a NodeId>) -> bool {
		selected_nodes.any(|node_id| self.network.nodes.contains_key(node_id) || self.network.exports_metadata.0 == *node_id || self.network.imports_metadata.0 == *node_id)
	}

	/// Get the network the selected nodes are part of, which is either the document network metadata or the metadata from the network_path.
	pub fn network_for_selected_nodes<'a>(&self, selected_nodes: impl Iterator<Item = &'a NodeId>) -> Option<&NodeNetwork> {
		self.network(self.selected_nodes_in_document_network(selected_nodes))
	}

	/// Get the metadata for the network the selected nodes are part of, which is either the document network metadata or the metadata from the network_path.
	pub fn network_metadata_for_selected_nodes<'a>(&self, selected_nodes: impl Iterator<Item = &'a NodeId>) -> Option<&NodeNetworkMetadata> {
		self.network_metadata(self.selected_nodes_in_document_network(selected_nodes))
	}

	pub fn network_path(&self) -> &Vec<NodeId> {
		&self.network_path
	}

	pub fn is_document_network(&self) -> bool {
		self.network_path.is_empty()
	}

	/// Returns the first downstream layer from a node, inclusive. If the node is a layer, it will return itself
	pub fn downstream_layer(&self, node_id: &NodeId, use_document_network: bool) -> Option<LayerNodeIdentifier> {
		let Some(network_metadata) = self.network_metadata(use_document_network) else {
			log::error!("Could not get nested network in downstream_layer");
			return None;
		};
		let outward_wires = self.collect_outward_wires(use_document_network);
		let mut id = *node_id;
		while !network_metadata.persistent_metadata.node_metadata.get(&node_id)?.persistent_metadata.is_layer() {
			id = outward_wires.get(&id)?.first().copied()?;
		}
		Some(LayerNodeIdentifier::new(id, self.document_network()))
	}

	pub fn get_chain_width(&self, node_id: &NodeId, use_document_network: bool) -> u32 {
		let Some(network) = self.network(use_document_network) else {
			log::error!("Could not get nested network in get_chain_width");
			return 0;
		};
		let Some(network_metadata) = self.network_metadata(use_document_network) else {
			log::error!("Could not get nested network_metadata in get_chain_width");
			return 0;
		};
		assert!(network_metadata.persistent_metadata.node_metadata.get(node_id).is_some_and(|node_metadata| node_metadata.persistent_metadata.is_layer()), "Node is not a layer node in get_chain_width");
		let node = network.nodes.get(node_id).expect("Node not found in get_chain_width");
		if node.inputs.len() > 1 {
			let mut last_chain_node_distance = 0u32;
			// Iterate upstream from the layer, and get the number of nodes distance to the last node with Position::Chain
			for (index, (_, node_id)) in network.upstream_flow_back_from_nodes(vec![node_id], graph_craft::document::FlowType::HorizontalFlow).enumerate() {
				if let Some(NodeTypePersistentMetadata::Node(node_persistent_metadata)) = network_metadata.persistent_metadata.node_metadata.get(&node_id).map(|node_metadata| node_metadata.persistent_metadata.node_type_metadata) {
					if Position::Chain = node_persistent_metadata.position {
						last_chain_node_distance = index;
					}
				}
			}
			last_chain_node_distance
		} else {
			// Layer with no inputs has no chain
			0
		}
	}

		/// Check if the specified node id is connected to the output
	pub fn connected_to_output(&self, target_node_id: NodeId) -> bool {
		let Some(network) = self.network_for_selected_nodes(std::iter::once(&target_node_id)) else {
			log::error!("Could not get network in connected_to_output");
			return false;
		};
		// If the node is the output then return true
		if network
			.exports
			.iter()
			.any(|export| if let NodeInput::Node { node_id, .. } = export { *node_id == target_node_id } else { false })
		{
			return true;
		}

		if network.exports_metadata.0 == target_node_id {
			return true;
		}
		// Get the outputs
		let mut stack = network
			.exports
			.iter()
			.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { network.nodes.get(node_id) } else { None })
			.collect::<Vec<_>>();
		let mut already_visited = HashSet::new();
		already_visited.extend(network.exports.iter().filter_map(|output| if let NodeInput::Node { node_id, .. } = output { Some(node_id) } else { None }));

		while let Some(node) = stack.pop() {
			for input in &node.inputs {
				if let &NodeInput::Node { node_id: ref_id, .. } = input {
					// Skip if already viewed
					if already_visited.contains(&ref_id) {
						continue;
					}
					// If the target node is used as input then return true
					if ref_id == target_node_id {
						return true;
					}
					// Add the referenced node to the stack
					let Some(ref_node) = network.nodes.get(&ref_id) else {
						continue;
					};
					already_visited.insert(ref_id);
					stack.push(ref_node);
				}
			}
		}

		false
	}

	/// Collect a hashmap of all downstream inputs from an output.
	pub fn collect_outward_wires(&self, use_document_network: bool) -> HashMap<OutputConnector, Vec<InputConnector>> {
		let mut outward_wires = HashMap::new();
		let Some(network) = self.network(use_document_network) else {
			log::error!("Could not get nested network in collect_outward_wires");
			return HashMap::new();
		};
		// Initialize all output connectors for nodes
		for (node_id, node) in network.nodes.iter() {
			let number_of_outputs = match &node.implementation {
				DocumentNodeImplementation::ProtoNode(_) => 1,
				DocumentNodeImplementation::Network(nested_network) => nested_network.exports.len(),
				DocumentNodeImplementation::Extract => 1,
			};
			for _ in 0..number_of_outputs {
				outward_wires.insert(OutputConnector::node(*node_id, output_index), Vec::new());
			}
		};
		// Initialize output connectors for the import node
		for import_index in 0..network.import_types.len(){
			outward_wires.insert(OutputConnector::Import(import_index), Vec::new());
		};
		// Collect wires between all nodes and the Imports
		for (current_node_id, node) in network.nodes.iter() {
			// Collect wires between the nodes as well as exports
			for (input_index, input) in node.inputs.iter().chain(network.exports.iter()).enumerate() {
				if let NodeInput::Node { node_id, output_index, .. } = input {
					let outward_wires_entry = outward_wires.get_mut(&OutputConnector::node(*node_id, *output_port)).expect("All output connectors should be initialized");
					outward_wires_entry.push(InputConnector::node(*current_node_id, input_index));
				} else if let NodeInput::Network { import_index, .. } = input {
					let outward_wires_entry = outward_wires.get_mut(&OutputConnector::Import(*import_index)).expect("All output connectors should be initialized");
					outward_wires_entry.push(InputConnector::node(*current_node_id, input_index));
				}
			}
		}
		outward_wires
	}

	/// Creates a copy for each node by disconnecting nodes which are not connected to other copied nodes.
	/// Returns an iterator of all persistent metadata for a node and their ids
	pub fn copy_nodes<'a>(&self, new_ids: &'a HashMap<NodeId, NodeId>, use_document_network: bool) -> impl Iterator<Item = (NodeId, NodeTemplate)> + 'a {
		let Some(network) = self.network(use_document_network) else {
			log::error!("Could not get network in copy_nodes");
			return Vec::new().into_iter();
		};

		new_ids
			.iter()
			.filter_map(|(&node_id, &new)| 
				network.nodes.get(&node_id).and_then(|node| 
					self.create_node_template(node_id, use_document_network).map(|node_template| (new, node_id, node_template))
				)
			)
			.map(move |(new, node_id, node)| (new, self.map_ids(node, new_ids, use_document_network)))
	}

	pub fn create_node_template(&self, node_id: NodeId, use_document_network: bool) -> Option<NodeTemplate> {
		let Some(network) = self.network(use_document_network) else {
			log::error!("Could not get network in create_node_template");
			return None;
		};
		let Some(node) = network.nodes.get(&node_id) else {
			log::error!("Could not get node in create_node_template");
			return None;
		};
		let Some(node_metadata) = self.network_metadata(use_document_network).and_then(|network_metadata| network_metadata.persistent_metadata.node_metadata.get(&node_id)) else {
			log::error!("Could not get node_metadata in create_node_template");
			return None;
		};
		Some(
			NodeTemplate {
				persistent_node_metadata: node_metadata.persistent_metadata.clone(),
				document_node: node.clone(),
			}
		)
	}

	/// Converts all node id inputs to a new id based on a HashMap.
	///
	/// If the node is not in the hashmap then a default input is found based on the compiled network
	pub fn map_ids(&self, mut node_template: NodeTemplate, new_ids: &HashMap<NodeId, NodeId>, use_document_network: bool) -> NodeTemplate {
		for (input_index, input) in node_template.document_node.inputs.iter_mut().enumerate() {
			if let &mut NodeInput::Node { node_id: id, output_index, lambda } = input {
				if let Some(&new_id) = new_ids.get(&id) {
					*input = NodeInput::Node {
						node_id: new_id,
						output_index,
						lambda,
					};
				} else {
					// Disconnect node input if it is not connected to another node in new_ids
					let tagged_value = TaggedValue::from_type(&self.get_input_type(node_id, input_index, use_document_network));
					*input = NodeInput::Value { tagged_value, exposed: true };
				}
			} else if let &mut NodeInput::Network { .. } = input {
				// Always disconnect network node input 
				let tagged_value = TaggedValue::from_type(&self.get_input_type(node_id, input_index, use_document_network));
				*input = NodeInput::Value { tagged_value, exposed: true };
			}
		}
		node_template
	}

	/// Get the [`Type`] for any `node_id` and `input_index`. The `network_path` is the path to the encapsulating node (including the encapsulating node). The `node_id` is the selected node.
	pub fn get_input_type(&self, node_id: NodeId, input_index: usize, use_document_network: bool) -> Type {
		let Some(network) = self.network(use_document_network) else {
			log::error!("Could not get network in get_tagged_value");
			return concrete!(());
		};

		// TODO: Store types for all document nodes, not just the compiled proto nodes, which currently skips isolated nodes
		let node_id_path: &Vec<NodeId> = &[&self.network_path[..], &[node_id]].concat();
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

	/// Get the top left position in node graph coordinates for a node by recursively iterating downstream
	pub fn get_position(&self, node_id: NodeId, outward_wires: HashMap<OutputConnector, Vec<InputConnector>>, use_document_network: bool) -> Option<IVec2> {
		let Some(network_metadata) = self.network_metadata(use_document_network) else {
			log::error!("Could not get nested network_metadata in get_position");
			return None;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(&node_id) else {
			log::error!("Could not get nested node_metadata in get_position");
			return None;
		};
		match node_metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => {
				match layer_metadata.position {
					LayerPosition::Absolute(position) => return Some(position),
					LayerPosition::Stack(y_offset) => {
						// TODO: Use root node to restore if previewing
						let Some(downstream_node_connectors) = outward_wires.get(&OutputConnector::node(node_id, 0)) else {
							log::error!("Could not get downstream node in get_position");
							return None;
						};
						let Some(downstream_node_id) = downstream_node_connectors.iter().find_map(|input_connector| if let InputConnector::Node {node_id, input_index} = input_connector { if *input_index == 0 {Some(node_id)} else {None}} else {None}) else {
							log::error!("Could not get downstream node input connector with input index 0");
							return None;
						};
						return self.get_position(downstream_node_id, outward_wires, use_document_network).map(|position| position + DVec2::Y(y_offset));
					}
				}
			},
			NodeTypePersistentMetadata::Node(node_metadata) => {
				match node_metadata.position {
					NodePosition::Absolute(_) => todo!(),
					NodePosition::Chain => {
						// Iterate through primary flow to find the first Layer
						let mut current_node_id = node_id;
						let mut node_distance_from_layer = 1;
						loop {
							// TODO: Use root node to restore if previewing
							let Some(downstream_node_connectors) = outward_wires.get(&OutputConnector::node(node_id, 0)) else {
								log::error!("Could not get downstream node for node with Position::Chain");
								return None;
							};
							let Some(downstream_node_id) = downstream_node_connectors.iter().find_map(|input_connector| if let InputConnector::Node {node_id, input_index} = input_connector { if *input_index == 0 {Some(node_id)} else {None}} else {None}) else {
								log::error!("Could not get downstream node input connector with input index 0 for node with Position::Chain");
								return None;
							};
							let Some(downstream_node_metadata) = network_metadata.persistent_metadata.node_metadata.get(downstream_node_id) else {
								log::error!("Downstream node metadata not found in node_metadata for node with Position::Chain");
								return None
							};
							if downstream_node_metadata.persistent_metadata.is_layer() {
								// Get the position of the layer
								let layer_position = self.get_position(downstream_node_id, outward_wires, use_document_network);
								return layer_position + IVec2::new(0, node_distance_from_layer * 8);
							}
							node_distance_from_layer += 1;
							current_node_id = downstream_node_id;
						}
					},
				}
			},
		}
	}

	pub fn get_output_connector_from_input_connector(&self, input_connector: InputConnector) -> Option<OutputConnector> {
		let Some(network) = self.network(false) else {
			log::error!("Could not get network in get_upstream_node_from_input");
			return None;
		};
		let input = match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(node) = network.nodes.get(&node_id) else {
					log::error!("Could not get node in get_upstream_node_from_input");
					return None;
				};
				node.inputs.get(input_index)
			},
			InputConnector::Export(export_index) => {
				network.exports.get(export_index)
			}
		};
		input.map(|input| {
			match input {
				NodeInput::Node { node_id, output_index, .. } => OutputConnector::node(node_id, output_index),
				NodeInput::Network { import_index, .. } => OutputConnector::Import(import_index),
				_ => None,
			}
		})
	}
	
	pub fn previewing(&self, use_document_network: bool) -> Previewing {
		let Some(network_metadata) = self.network_metadata(use_document_network) else {
			log::error!("Could not get nested network_metadata in previewing");
			return false;
		};
		network_metadata.persistent_metadata.previewing
	}

	/// Returns the root node (the node that the solid line is connect to), or None if no nodes are connected to the output
	pub fn get_root_node(&self) -> Option<OutputConnector> {
		match self.previewing {
			Previewing::Yes { root_node_to_restore } => root_node_to_restore,
			Previewing::No => self.exports.first().and_then(|export| {
				if let NodeInput::Node { node_id, output_index, .. } = export {
					Some(RootNode {
						id: *node_id,
						output_index: *output_index,
					})
				} else {
					None
				}
			}),
		}
	}

	pub fn persistent_node_metadata(&self, node_id: &NodeId, use_document_network: bool) -> Option<&DocumentNodePersistentMetadata> {
		let Some(network_metadata) = self.network_metadata(use_document_network) else {
			log::error!("Could not get nested network_metadata");
			return None;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata for node {node_id}");
			return None;
		};
		Some(&node_metadata.persistent_metadata)
	}

	pub fn get_reference(&self, node_id: &NodeId, use_document_network: bool ) -> Option<String> {
		self.persistent_node_metadata(&node_id, false).and_then(|node_metadata| node_metadata.reference.map(|reference| reference.to_string()))
	}

	pub fn get_alias(&self, node_id: &NodeId, use_document_network: bool) -> Option<String> {
		self.persistent_node_metadata(&node_id, false).map(|node_metadata| node_metadata.alias.clone())
	}

	pub fn untitled_layer_label(&self, node_id: &NodeId, use_document_network: bool) -> String {
		self.get_alias(node_id, use_document_network)
			.unwrap_or(if self.persistent_node_metadata(node_id, use_document_network).expect("Could not get persistent node metadata in untitled_layer_label").is_layer() && self.get_reference(node_id, use_document_network).is_some_and(|reference| reference == "Merge")  { "Untitled Layer".to_string() } else { self.get_reference(node_id, use_document_network).unwrap_or_default() })
	}

	pub fn get_locked(&self, node_id: &NodeId, use_document_network: bool) -> bool {
		let Some(persistent_node_metadata) = self.persistent_node_metadata(&node_id, false) else {
			log::error!("Could not get persistent node metadata in get_locked for node {node_id}");
			return false;
		};
		if let NodeTypePersistentMetadata::Layer(layer_metadata) = persistent_node_metadata.node_type_metadata {
			layer_metadata.locked
		} else {
			false
		}
	}

	pub fn is_layer(&self, node_id: &NodeId) -> bool {
		let Some(network_metadata) = self.network_metadata_for_selected_nodes(std::iter::once(node_id)) else {
			log::error!("Could not get nested network_metadata in is_layer");
			return false;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata in is_layer");
			return false;
		};
		node_metadata.persistent_metadata.is_layer()
	}

	pub fn is_eligible_to_be_layer(&self, node_id: &NodeId) -> bool {
		let use_document_network = self.selected_nodes_in_document_network(std::iter::once(&node_id));
		let Some(network) = self.network(use_document_network) else {
			log::error!("Could not get network in is_eligible_to_be_layer");
			return false;
		};
		if network.imports_metadata.0 == node_id || network.exports_metadata.0 == node_id {
			return false;
		}

		let Some(node) = network.nodes.get(&node_id) else { return false };

		let input_count = node.inputs.iter().filter(|input| input.is_exposed_to_frontend(use_document_network)).count();
		let output_count = if let graph_craft::document::DocumentNodeImplementation::Network(nested_network) = &node.implementation {
			nested_network.exports.len()
		} else {
			// Node is a Protonode, so it must have 1 output
			1
		};

		// TODO: Eventually allow nodes at the bottom of a stack to be layers, where `input_count` is 0
		self.persistent_node_metadata(node_id, use_document_network).is_some_and(|node_metadata| node_metadata.has_primary_output) && output_count == 1 && (input_count == 1 || input_count == 2)
	}
}

// Private mutable getters for use within the network interface
impl NodeNetworkInterface {
	fn document_network_mut(&self) -> &mut NodeNetwork {
		&mut self.network
	}

	fn network_mut(&mut self, use_document_network: bool) -> Option<&mut NodeNetwork> {
		if use_document_network {
			Some(&mut self.network)
		} else {
			self.network.nested_network_mut(&self.network_path)
		}
	}

	pub fn document_network_metadata_mut(&mut self) -> &mut NodeNetworkMetadata {
		&mut self.network_metadata
	}

	fn network_metadata_mut(&mut self, use_document_network: bool) -> Option<&mut NodeNetworkMetadata> {
		if use_document_network {
			Some(self.document_network_metadata_mut())
		} else {
			self.network_metadata.nested_metadata_mut(&self.network_path)
		}
	}

	/// Get the mutable network the selected nodes are part of, which is either the document network metadata or the metadata from the network_path.
	fn network_for_selected_nodes_mut<'a>(&mut self, selected_nodes: impl Iterator<Item = &'a NodeId>) -> Option<&NodeNetwork> {
		self.network(self.selected_nodes_in_document_network(selected_nodes))
	}

	/// Get the metadata for the network the selected nodes are part of, which is either the document network metadata or the metadata from the network_path.
	fn network_metadata_for_selected_nodes_mut<'a>(&mut self, selected_nodes: impl Iterator<Item = &'a NodeId>) -> Option<&NodeNetworkMetadata> {
		self.network_metadata_mut(self.selected_nodes_in_document_network(selected_nodes))
	}

	/// This method is implemented in the interface since creating a node requires information from both the NodeNetwork and network metadata
	pub fn get_transient_node_metadata(&mut self, node_id: &NodeId, use_document_network: bool) -> Option<&mut DocumentNodeTransientMetadata> {
		let Some(network_metadata) = self.network_metadata_mut(use_document_network) else {
			log::error!("Could not get nested network_metadata in get_transient_node_metadata");
			return None
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get nested node_metadata in get_transient_node_metadata");
			return None
		};
		node_metadata.transient_metadata.get_transient_node_metadata(self, node_id, use_document_network)
	}

	pub fn get_transient_network_metadata(&mut self, node_id: &NodeId, use_document_network: bool) -> Option<&mut NodeNetworkTransientMetadata> {
		let Some(network_metadata) = self.network_metadata_mut(use_document_network) else {
			log::error!("Could not get nested network_metadata in get_transient_node_metadata");
			return None
		};
		network_metadata.transient_metadata.get_transient_network_metadata(self, use_document_network)
	}
}

// Public mutable getters for data that involves transient metadata, which may need to be created if they are unloaded
impl NodeNetworkInterface {
	/// Click target getter methods
	pub fn get_node_from_click(&mut self, click: DVec2) -> Option<NodeId> {
		let Some(network_metadata) = self.network_metadata(false) else {
			log::error!("Could not get nested network_metadata in get_node_from_click");
			return None;
		};
		let Some(network) = self.network(false) else {
			log::error!("Could not get nested network in get_node_from_point");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);

		let mut clicked_nodes = network.nodes.iter().filter(|(node_id, _)| self.get_transient_node_metadata(node_id, false).is_some_and(|transient_node_metadata| transient_node_metadata.node_click_target.intersect_point(point, DAffine2::IDENTITY))).filter_map(|(node_id, _)| {
			let Some(node_metadata) = self.network_metadata.persistent_metadata.node_metadata.get(node_id) else {
				log::debug!("Could not get node_metadata for node {node_id}");
				return None;
			};
			Some((node_id, node_metadata.persistent_metadata.is_layer()))
		});
		// Since nodes are placed on top of layer chains, find the first non layer node that was clicked, and if there way no non layer nodes clicked, then find the first layer node that was clicked
		clicked_nodes.find(|(_, is_layer)| !is_layer).or_else(|| clicked_nodes.next()).map(|(node_id, _)| node_id)
	}

	pub fn get_visibility_from_click(&mut self, click: DVec2) -> Option<NodeId> {
		let Some(network_metadata) = self.network_metadata(false) else {
			log::error!("Could not get nested network_metadata in get_node_from_click");
			return None;
		};
		let Some(network) = self.network(false) else {
			log::error!("Could not get nested network in get_node_from_point");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);

		network.nodes.iter().filter_map(|(node_id, _)|{
			self.get_transient_node_metadata(node_id, false).and_then(|transient_node_metadata|{
				if let NodeTypeTransientMetadata::Layer(layer) = transient_node_metadata.node_type_metadata {
					layer.visibility_click_target.intersect_point(point, DAffine2::IDENTITY).then(|| *node_id)
				} else {
					None
				}
			})
		}).next()
	}

	pub fn get_input_connector_from_click(&mut self, click: DVec2) -> Option<InputConnector> {
		let Some(network_metadata) = self.network_metadata(false) else {
			log::error!("Could not get nested network_metadata in get_node_from_click");
			return None;
		};
		let Some(network) = self.network(false) else {
			log::error!("Could not get nested network in get_node_from_point");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);

		network.nodes.iter().filter_map(|(node_id, _)|{
			self.get_transient_node_metadata(node_id, false).and_then(|transient_node_metadata|{ 
				transient_node_metadata.port_click_targets.clicked_input_port_from_point(point).map(|port| InputConnector::node(node_id, port))
				.or_else(|| self.get_transient_network_metadata(network_interface, false).and_then(|transient_network_metadata|{
					transient_network_metadata.export_ports.clicked_input_port_from_point(point).map(|port| InputConnector::Export(port))
				}
				))
			})
		}).next()
	}

	pub fn get_output_connector_from_click(&mut self, click: DVec2) -> Option<OutputConnector> {
		let Some(network_metadata) = self.network_metadata(false) else {
			log::error!("Could not get nested network_metadata in get_node_from_click");
			return None;
		};
		let Some(network) = self.network(false) else {
			log::error!("Could not get nested network in get_node_from_point");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);

		network.nodes.iter().filter_map(|(node_id, _)|{
			self.get_transient_node_metadata(node_id, false).and_then(|transient_node_metadata|{ 
				transient_node_metadata.port_click_targets.clicked_output_port_from_point(point).map(|output_index| OutputConnector::node(node_id, output_index))
				.or_else(|| self.get_transient_network_metadata(network_interface, false).and_then(|transient_network_metadata|{
					transient_network_metadata.export_ports.clicked_output_port_from_point(point).map(|output_index| OutputConnector::Import(output_index))
				}
				))
			})
		}).next()
	}

	pub fn node_bounding_box(&mut self, node_id: NodeId) -> Option<[DVec2; 2]> {
		self.get_transient_node_metadata(&node_id, false).and_then(|transient_node_metadata| transient_node_metadata.node_click_target.subpath.bounding_box())
	}

	pub fn get_input_position(&mut self, input_connector: InputConnector) -> Option<DVec2> {
		match input_connector {
			InputConnector::Node(node_id, output_index) => self.get_transient_node_metadata(&node_id, false).and_then(|transient_node_metadata| transient_node_metadata.port_click_targets.get_input_port_position(output_index)),
			InputConnector::Export(import_index) => None, // TODO: Implement getting position for the new import connection UI
		}
	}
	pub fn get_output_position(&mut self, output_connector: OutputConnector) -> Option<DVec2> {
		match output_connector {
			OutputConnector::Node(node_id, output_index) => self.get_transient_node_metadata(&node_id, false).and_then(|transient_node_metadata| transient_node_metadata.port_click_targets.get_output_port_position(output_index)),
			OutputConnector::Import(import_index) => None, // TODO: Implement getting position for the new import connection UI
		}
	}

	/// Get the combined bounding box of the click targets of the selected nodes in the node graph in viewport space
	pub fn selected_nodes_bounding_box_viewport(&mut self, selected_nodes: &SelectedNodes) -> Option<[DVec2; 2]> {
		// Always get the bounding box for nodes in the currently viewed network
		let use_document_network = false;
		let Some(network) = self.network(use_document_network) else {
			log::error!("Could not get nested network in selected_nodes_bounding_box_viewport");
			return None;
		};
		let Some(network_metadata) = self.network_metadata(use_document_network) else {
			log::error!("Could not get nested network_metadata in selected_nodes_bounding_box_viewport");
			return None;
		};

		selected_nodes
			.selected_nodes()
			.filter_map(|node_id| {
				let Some(node_metadata) = self.network_metadata.persistent_metadata.node_metadata.get(&node_id) else {
					log::debug!("Could not get click target for node {node_id}");
					return None;
				};
				self.get_transient_node_metadata(node_id, use_document_network).and_then(|transient_node_metadata| transient_node_metadata.node_click_target.subpath.bounding_box_with_transform(network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport))
			})
			.reduce(graphene_core::renderer::Quad::combine_bounds)
	}

	/// Gets the bounding box in viewport coordinates for each node in the node graph
	pub fn graph_bounds_viewport_space(&mut self) -> Option<[DVec2; 2]> {
		let Some(network_metadata) = self.network_metadata(false) else {
			log::error!("Could not get nested network_metadata in selected_nodes_bounding_box_viewport");
			return None;
		};
		// Get bounding box around all nodes. Cache this data in transient network metadata if it is too slow to calculate on every frame.
		let bounds = network_metadata.persistent_metadata.node_metadata
			.iter()
			.filter_map(|(node_id, node_metadata)| self.get_transient_node_metadata(&self, node_id).and_then(|transient_node_metadata| transient_node_metadata.node_click_target.subpath.bounding_box()))
			.reduce(Quad::combine_bounds);
		let bounding_box_subpath = bounds.map(|bounds| bezier_rs::Subpath::new_rect(bounds[0], bounds[1]));
		bounding_box_subpath.as_ref()
			.and_then(|bounding_box| bounding_box.bounding_box_with_transform(self.network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport))
	}
}

// Public mutable methods
impl NodeNetworkInterface {
	/// Replaces the current network with another, and returns the old network. Since changes can be made to various sub networks, all network_metadata is reset.
	pub fn replace(&mut self, new_network: NodeNetwork) -> NodeNetwork {
		let old_network = std::mem::replace(&mut self.network, network);
		/// Clear all transient metadata from all network metadata
		let stack = vec![&mut self.network_metadata];
		while let Some(network_metadata) = stack.pop() {
			network_metadata.transient_metadata.unload();
			network_metadata.persistent_metadata.node_metadata.values_mut().for_each(|node_metadata| node_metadata.transient_metadata.unload());
			stack.extend(network_metadata.persistent_metadata.node_metadata.values_mut().filter_map(|node_metadata| node_metadata.persistent_metadata.network_metadata.as_mut()));
		}
		old_network
	}

	/// Ensure network metadata, positions, and other metadata is kept in sync
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

	/// Used to insert a node from a node definition into the network.
	pub fn insert_node(&mut self, node_id: NodeId, node_template: NodeTemplate, use_document_network: bool) {
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
	/// TODO: Store network metadata, selected nodes, mutable reference to responses as fields in the interface?
	pub fn delete_nodes(&mut self, mut nodes_to_delete: Vec<NodeId>, reconnect: bool, selected_nodes: &mut SelectedNodes, responses: &mut VecDeque<Message>, use_document_network: bool) {
		let Some(network) = self.network(use_document_network) else {
			return;
		};

		// Prevent deleting import/export nodes
		nodes_to_delete.retain(|node_id| node_id != network.imports_metadata.0 && node_id != network.exports_metadata.0);

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
			self.network.nodes.remove(&node_id);
			//node_graph.update_click_target(node_id, document_network, network_path.clone());
		}
		// Updates the selected nodes, and rerender the document
		selected_nodes.retain_selected_nodes(|node_id| !delete_nodes.contains(node_id));
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		if use_document_network {
			responses.add(GraphOperationMessage::LoadStructure);
		}
	}

	pub fn remove_references_from_network(&mut self, deleting_node_id: NodeId, reconnect: bool, use_document_network: bool) -> bool {
		let Some(network) = self.network(use_document_network) else {
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

		let Some(network) = self.network(use_document_network) else { return false };

		if let Some(Previewing::Yes { root_node_to_restore }) = network.previewing {
			if let Some(root_node_to_restore) = root_node_to_restore {
				if root_node_to_restore.id == deleting_node_id {
					self.start_previewing_without_restore();
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

	pub fn enter_nested_network(&mut self, node_id: NodeId) {
		self.network_path.push(node_id);
	}
	
	pub fn exit_nested_network(&mut self) {
		self.network_path.pop();
	}
	
	pub fn exit_all_nested_networks(&mut self) {
		self.network_path.clear();
	}
	
	/// Start previewing with a restore node
	pub fn start_previewing(&mut self, previous_node_id: NodeId, output_index: usize) {
		self.previewing = Previewing::Yes {
			root_node_to_restore: Some(RootNode { id: previous_node_id, output_index }),
		};
	}

	pub fn start_previewing_without_restore(&mut self) {
		// Some logic will have to be performed to prevent the graph positions from being completely changed when the export changes to some previewed node
		// self.network.start_previewing_without_restore();
	}

	/// Sets the root node only if a node is being previewed
	pub fn update_root_node(&mut self, node_id: NodeId, output_index: usize) {
		if let Previewing::Yes { root_node_to_restore } = self.previewing {
			// Only continue previewing if the new root node is not the same as the primary export. If it is the same, end the preview
			if let Some(root_node_to_restore) = root_node_to_restore {
				if root_node_to_restore.id != node_id {
					self.start_previewing(node_id, output_index);
				} else {
					self.stop_preview();
				}
			} else {
				self.stop_preview();
			}
		}
	}

	/// Stops preview, does not reset export
	pub fn stop_preview(&mut self) {
		self.previewing = Previewing::No;
	}

	pub fn set_alias(&mut self, node_id: NodeId, alias:String){
		let Some(network_metadata) = self.network_metadata_for_selected_nodes_mut(std::iter::once(&node_id)) else {
			return;
		};

		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(&node_id) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			layer_metadata.locked = locked;
		} else {
			log::error!("Cannot set non layer node to locked");
		}
		//TODO: Recalculate transient metadata
		node_metadata.transient_metadata.unload();
	}

	pub fn set_visibility(&mut self, node_id: NodeId, is_visible:bool) {
		let Some(network) = self.network_for_selected_nodes_mut(std::iter::once(&node_id)) else {
			return;
		};

		let Some(node) = network.nodes.get_mut(&node_id) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		node.visible = is_visible;
	}

	pub fn set_locked(&mut self, node_id: NodeId, locked:bool) {
		let Some(network_metadata) = self.network_metadata_for_selected_nodes_mut(std::iter::once(&node_id)) else {
			return;
		};

		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(&node_id) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			layer_metadata.locked = locked;
		} else {
			log::error!("Cannot set non layer node to locked");
		}
	}

	pub fn set_to_node_or_layer(&mut self, node_id: NodeId, is_layer: bool) {
		let use_document_network = self.selected_nodes_in_document_network(std::iter::once(&node_id));
		let Some(network_metadata) = self.network_metadata_mut(use_document_network) else {
			log::error!("Could not get nested network_metadata in set_to_node_or_layer");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(&node_id) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		// TODO: Ensure transient metadata, persistent metadata, and document metadata are correctly updated when switching between node and layer
		// node_metadata.persistent_metadata.node_type_metadata = if is_layer {
		// 	NodeTypePersistentMetadata::Layer
		// } else {
		// 	NodeTypePersistentMetadata::Node
		// };
		node_metadata.transient_metadata.unload();
	}

	pub fn toggle_preview(&mut self, toggle_id: NodeId) {
		let use_document_network = self.selected_nodes_in_document_network(std::iter::once(&toggle_id));
		let Some(network) = self.network(use_document_network) else {
			return;
		};
		// If new_export is None then disconnect
		let mut new_export = None;
		let mut new_previewing_state =  Previewing::No;
		if let Some(export) = network.exports.get(0) {
			// If there currently an export
			if let NodeInput::Node { node_id, output_index, .. } = export {
				let previous_export_id = *node_id;
				let previous_output_index = *output_index;

				// The export is clicked
				if *node_id == toggle_id {
					// If the current export is clicked and is being previewed end the preview and set either export back to root node or disconnect
					if let Previewing::Yes { root_node_to_restore } = self.previewing(use_document_network) {
						new_export = root_node_to_restore;
						new_previewing_state = Previewing::No;
					}
					// The export is clicked and there is no preview
					else {
						new_previewing_state = Previewing::Yes { root_node_to_restore: Some(OutputConnector::node(previous_export_id, previous_output_index))};
					}
				}
				// The export is not clicked
				else {
					new_export = Some(OutputConnector::node(toggle_id, 0));

					// There is currently a dashed line being drawn
					if let Previewing::Yes { root_node_to_restore } = self.previewing(use_document_network) {
						// There is also a solid line being drawn
						if let Some(root_node_to_restore) = root_node_to_restore {
							// If the node with the solid line is clicked, then start previewing that node without restore
							if root_node_to_restore.id == toggle_id {
								new_export = Some(OutputConnector::node(toggle_id, 0));
								new_previewing_state = Previewing::Yes { root_node_to_restore: None };
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
						new_previewing_state = Previewing::Yes { root_node_to_restore: Some(OutputConnector::node(previous_export_id, previous_output_index)) };
					}
				}
			}
			// The primary export is disconnected
			else {
				// Set node as export and cancel any preview
				new_export = Some(OutputConnector::node(toggle_id, 0));
				network.start_previewing_without_restore();
			}
		}
		match new_export {
			Some(new_export) => {
				self.create_wire(new_export, InputConnector::Export(0), use_document_network);
			}
			None => {
				self.disconnect_input(InputConnector::Export(0), use_document_network);
			}
		}
		let Some(network_metadata) = self.network_metadata_mut(use_document_network) else {
			return;
		};
		network_metadata.persistent_metadata.previewing = new_previewing_state;
	}

	/// Shifts a node by a certain offset without the auto layout system. If the node is a layer in a stack, the y_offset is shifted. If the node is a node in a chain, its position gets set to absolute.
	pub fn shift_node(&mut self, node_id: NodeId, shift: IVec2){
		let use_document_network = self.selected_nodes_in_document_network(std::iter::once(&node_id));
		let Some(network_metadata) = self.network_metadata_mut(use_document_network) else {
			log::error!("Could not get nested network_metadata in shift_node");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(&node_id) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if let NodeTypePersistentMetadata::Layer(mut layer_metadata) = node_metadata.persistent_metadata.node_type_metadata {
			if let LayerPosition::Absolute(mut layer_position) = layer_metadata.position {
				layer_position += shift;
			} else if let LayerPosition::Stack(mut y_offset) = layer_metadata.position {
				y_offset += shift.y;
			}
		} else if let NodeTypePersistentMetadata::Node(mut node_metadata) = node_metadata.persistent_metadata.node_type_metadata {
			if let NodePosition::Absolute(mut node_metadata) = node_metadata.position {
				node_metadata += shift;
			} else if let NodePosition::Stack = node_metadata.position {
				// TODO: Dont't break the chain when shifting a node left or right. Instead, shift the entire chain (?).
				let position = IVec2::new(0, self.get_position(node_id, self.collect_outward_wires(use_document_network), use_document_network) + shift.y);
				node_metadata.position = NodePosition::Absolute(position);
			}
		}
		//TODO: Update transient metadata based on the movement. Unloading it means it will be recalculated next time it is needed, which is a simple solution.
		node_metadata.transient_metadata.unload();
	}
	
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

/// Represents an input connector with index based on the [`DocumentNode::inputs`] index, not the visible input index
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum InputConnector {
	Node {node_id: NodeId, input_index: usize},
	Export(usize),
}

impl InputConnector {
	pub fn node(node_id: NodeId, input_index: usize ) -> Self {
		InputConnector::Node {node_id, input_index}
	}
	pub fn input_index(&self) -> usize {
		match self {
			InputConnector::Node {input_index, ..} => input_index,
			InputConnector::Export(input_index) => input_index,
		}
	}
}

/// Represents an output connector
#[derive(Debug, Clone, Default, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub enum OutputConnector {
	Node {node_id: NodeId, output_index: usize},
	Import(usize),
}

impl OutputConnector {
	pub fn node(node_id: NodeId, output_index: usize ) -> Self {
		OutputConnector::Node {node_id, output_index}
	}
	pub fn index(&self) -> usize {
		match self {
			OutputConnector::Node {output_index, ..} => output_index,
			OutputConnector::Import(output_index) => output_index,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Ports{ 
	input_ports: Vec<(usize, ClickTarget)>,
	output_ports: Vec<(usize, ClickTarget)>,
}

impl Ports {
	pub fn new() -> Ports {
		Ports {input_ports:Vec::new(), output_ports: Vec::new()}
	}

	fn insert_input_port_at_center(&mut self, input_index: usize, center: DVec2) {
		let subpath = Subpath::new_ellipse(center - DVec2::new(8., 8.), center + DVec2::new(8., 8.));
		self.input_ports.push((input_index, ClickTarget { subpath, stroke_width: 1. }));
	}

	fn insert_output_port_at_center(&mut self, output_index: usize, center: DVec2) {
		let subpath = Subpath::new_ellipse(center - DVec2::new(8., 8.), center + DVec2::new(8., 8.));
		self.output_ports.push((output_index, ClickTarget { subpath, stroke_width: 1. }));
	}

	fn insert_node_input(&mut self, input_index: usize, row_index: usize, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(0, 24. + 24. * row_index as f64);
		self.insert_input_port_at_center(input_index, center);
	}
	fn insert_node_output(&mut self, output_index: usize, row_index: usize, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(5.*24., 24. + 24. * row_index as f64);
		self.insert_output_port_at_center(output_index, center);
	}

	fn insert_layer_input(&mut self, input_index: usize, node_top_left: DVec2) {
		let center = if input_index == 0 {
			node_top_left + DVec2::new(2. * 24., 24. * 2. + 8.);
		} else {
			node_top_left + DVec2::new(0., 24. * 1);
		};
		let subpath = Subpath::new_ellipse(center - DVec2::new(8., 8.), center + DVec2::new(8., 8.));
		self.insert_input_port_at_center(input_index, center);
	}

	fn insert_layer_output(&mut self, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(2.*24., -8);
		self.insert_output_port_at_center(0, center);
	}

	pub fn clicked_input_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.input_ports.iter().find_map(|(port, click_target)| click_target.intersect_point(point, DAffine2::IDENTITY).then(|| *port))
	}

	pub fn clicked_output_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.output_ports.iter().find_map(|(port, click_target)| click_target.intersect_point(point, DAffine2::IDENTITY).then(|| *port))
	}

	pub fn get_input_port_position(&self, index: usize) -> Option<DVec2> {
		self.input_ports.iter().nth(index).and_then(|(_, click_target)| click_target.subpath.bounding_box().map(|bounds| bounds[0] + DVec2::new(8., 8.)))
	}

	pub fn get_output_port_position(&self, index: usize) -> Option<DVec2> {
		self.output_ports.iter().nth(index).and_then(|(_, click_target)| click_target.subpath.bounding_box().map(|bounds| bounds[0] + DVec2::new(8., 8.)))
	}
}

#[derive(PartialEq, Debug, Clone, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Previewing {
	/// If there is a node to restore the connection to the export for, then it is stored in the option.
	/// Otherwise, nothing gets restored and the primary export is disconnected.
	Yes { root_node_to_restore: Option<OutputConnector> },
	#[default]
	No,
}

/// All fields in NetworkMetadata should automatically be updated by using the network interface API. If a field is none then it should be calculated based on the network state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkMetadata {
	pub persistent_metadata: NodeNetworkPersistentMetadata,
	#[serde(skip)]
	pub transient_metadata: CurrentNodeNetworkTransientMetadata,
}

impl NodeNetworkMetadata {
	pub const GRID_SIZE: u32 = 24;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkPersistentMetadata {
	/// Node metadata must exist for every document node in the network 
	pub node_metadata: HashMap<NodeId, DocumentNodeMetadata>,
	/// Cached metadata for each node, which is calculated when adding a node to node_metadata
	/// Indicates whether the network is currently rendered with a particular node that is previewed, and if so, which connection should be restored when the preview ends.
	pub previewing: Previewing,
	// Stores the transform and navigation state for the network
	pub navigation_metadata: NavigationMetadata,
}


#[derive(Debug, Clone)]
pub enum CurrentNodeNetworkTransientMetadata {
	Loaded (NodeNetworkTransientMetadata),
	Unloaded,
}

impl CurrentNodeNetworkTransientMetadata {
	/// Always returns the Loaded variant, and creates it if it does not exist
	pub fn get_transient_network_metadata(&mut self, network_interface: &NodeNetworkInterface, use_document_network: bool) -> &NodeNetworkTransientMetadata {
		match self {
			CurrentNodeNetworkTransientMetadata::Loaded(node_network_transient_metadata) => node_network_transient_metadata,
			CurrentNodeNetworkTransientMetadata::Unloaded => {
				*self = CurrentNodeNetworkTransientMetadata::Loaded (NodeNetworkTransientMetadata::new(network_interface, use_document_network));
				self.get_transient_network_metadata(network_interface, use_document_network)
			}
		}
		self
	}

	/// Set the current transient metadata to unloaded
	pub fn unload(&mut self) {
		*self = CurrentNodeNetworkTransientMetadata::Unloaded;
	}
}

#[derive(Debug, Clone)]
pub struct NodeNetworkTransientMetadata {
	/// If some network calculation is too slow to compute for every usage, cache the data here
	/// Cache for the bounding box around all nodes in node graph space.
	// all_nodes_bounding_box: Option<Subpath<ManipulatorGroupId>>,
	/// Cache bounding box for all "groups of nodes", which will be used to prevent overlapping nodes
	// node_group_bounding_box: Vec<(Subpath<ManipulatorGroupId>, Vec<Nodes>)>,
	/// Cache for all outward wire connections - will most likely need to be added
	// outward_wires: HashMap<OutputConnector, Vec<InputConnector>>,
	/// TODO: Cache all wire paths instead of calculating in Graph.svelte
	// pub wire_paths: Vec<WirePath>
	/// Import node click targets, which may not exist, such as in the document network
	/// TODO: Delete this and replace with inputs placed on edges of the graph UI
	pub import_node_click_target: Option<(NodeId, ClickTarget)>,
	/// Import node click targets
	/// TODO: Delete this and replace with outputs placed on edges of the graph UI
	pub export_node_click_target: (NodeId, ClickTarget),
	/// All import connector click targets
	pub import_ports: Ports,
	/// All export connector click targets
	pub export_ports: Ports,
}

impl Default for CurrentNodeNetworkTransientMetadata {
    fn default() -> Self {
        CurrentNodeNetworkTransientMetadata::Unloaded
    }
}

impl NodeNetworkTransientMetadata {
	pub fn new(network_interface: &NodeNetworkInterface, use_document_network: bool) -> NodeNetworkTransientMetadata {
		NodeNetworkTransientMetadata {
			import_node_click_target: Self::import_node_click_target(network_interface, use_document_network),
			export_node_click_target: Self::export_node_click_target(network_interface, use_document_network),
			import_ports: Self::import_node_ports(network_interface, use_document_network),
			export_ports: Self::export_node_ports(network_interface, use_document_network),
		}
	}

	/// Returns none if network_path is empty, since the document network does not have an Imports node.
	fn import_node_click_target(network_interface: &NodeNetworkInterface, use_document_network: bool) -> Option<ClickTarget> {
		if network_interface.is_document_network() || use_document_network {
			return None
		};
		let network = network_interface.network(use_document_network).expect("Could not get nested network when creating NetworkMetadata");

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
			let width = 5 * NodeNetworkMetadata::GRID_SIZE;
			let height = output_row_count as u32 * NodeNetworkMetadata::GRID_SIZE;
			let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
			let radius = 3.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(node_top_left, node_bottom_right, [radius; 4]);
			ClickTarget { subpath, stroke_width: 1. }
		})
	}

	fn import_node_ports(network_interface: &NodeNetworkInterface, use_document_network: bool) -> Option<ClickTarget> {
		let network = network_interface.network(use_document_network).expect("Could not get nested network when creating NetworkMetadata");
		let import_top_left = network.imports_metadata.1.as_dvec2() * 24.;
		let mut import_ports = Ports::new();
		for output_index in 0..network.import_types.len() {
			// Skip first row since the first row is reserved for the "Exports" name
			import_ports.insert_node_output(output_index, output_index+1, import_top_left);
		}
		import_ports
	}

	fn export_node_click_target(network_interface: &NodeNetworkInterface, use_document_network: bool) -> ClickTarget {
		let network = network_interface.network(use_document_network).expect("Could not get nested network when creating NetworkMetadata");

		let node_top_left = network.exports_metadata.1.as_dvec2() * 24.;
		let input_row_count = network.exports.len() + 1;
		let width = 5 * NodeNetworkMetadata::GRID_SIZE;
		let height = input_row_count as u32 * NodeNetworkMetadata::GRID_SIZE;
		let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
		let radius = 3.;
		let subpath = bezier_rs::Subpath::new_rounded_rect(node_top_left, node_bottom_right, [radius; 4]);
		ClickTarget { subpath, stroke_width: 1. }
	}

	fn export_node_ports(network_interface: &NodeNetworkInterface, use_document_network: bool) -> Ports {
		let network = network_interface.network(use_document_network).expect("Could not get nested network when creating NetworkMetadata");
		let export_top_left = network.exports_metadata.1.as_dvec2() * 24.;
		let mut export_ports = Ports::new();
		for output_index in 0..network.exports.len() {
			// Skip first row since the first row is reserved for the "Exports" name
			export_ports.insert_node_input(output_index, output_index+1, export_top_left);
		}
	}
}
/// Utility function for providing a default boolean value to serde.
#[inline(always)]
fn return_true() -> bool {
	true
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentNodeMetadata {
	pub persistent_metadata: DocumentNodePersistentMetadata,
	#[serde(skip)]
	pub transient_metadata: CurrentDocumentNodeTransientMetadata,
}

/// Persistent metadata for each node in the network, which must be included when creating, serializing, and deserializing saving a node.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentNodePersistentMetadata {
	/// This should always be Some for nodes with a [`DocumentNodeImplementation::Network`], and none for [`DocumentNodeImplementation::ProtoNode`]
	pub network_metadata: Option<NodeNetworkMetadata>,
	/// The name of the node definition, as originally set by [`DocumentNodeDefinition`], used to display in the UI and to display the appropriate properties if no alias is set.
	/// Used during serialization/deserialization to prevent storing implementation or inputs (and possible other fields) if they are the same as the definition.
	pub reference: Option<&'static str>,
	/// A name chosen by the user for this instance of the node. Empty indicates no given name, in which case the reference name is displayed to the user in italics.
	#[serde(default)]
	pub alias: Option<String>,
	pub input_names: Vec<&'static str>,
	pub output_names: Vec<&'static str>,
	/// Indicates to the UI if a primary output should be drawn for this node.
	/// True for most nodes, but the Split Channels node is an example of a node that has multiple secondary outputs but no primary output.
	#[serde(default = "return_true")]
	pub has_primary_output: bool,
	// Metadata that is specific to either nodes or layers, which are chosen states for displaying as a left-to-right node or bottom-to-top layer.
	/// All fields in NodeTypePersistentMetadata should automatically be updated by using the network interface API
	pub node_type_metadata: NodeTypePersistentMetadata,
}
impl DocumentNodePersistentMetadata {

	pub fn is_layer(&self) -> bool {
		matches!(self.node_type_metadata, NodeTypePersistentMetadata::Layer(_))
	}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NodeTypePersistentMetadata {
	Layer(LayerPersistentMetadata),
	Node(NodePersistentMetadata),
}

/// All fields in LayerMetadata should automatically be updated by using the network interface API
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LayerPersistentMetadata {
	// TODO: Store click target for the preview button, which will appear when the node is a selected/(hovered?) layer node
	// preview_click_target: Option<ClickTarget>,
	/// Stores the position of a layer node, which can either be Absolute or Stack
	/// If performance is a concern then also cache the absolute position for each node
	pub position: LayerPosition,
	/// Represents the lock icon for locking/unlocking the node in the graph UI. When locked, a node cannot be moved in the graph UI.
	#[serde(default)]
	pub locked: bool,
}

/// A layer can either be position as Absolute or in a Stack
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LayerPosition {
	// Position of the node in grid spaces
	Absolute(IVec2),
	// A layer is in a Stack when it feeds into the secondary input of a layer input. The Y position stores the vertical distance between the layer and its parent.
	Stack(u32),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodePersistentMetadata {
	/// Stores the position of a non layer node, which can either be Absolute or Chain
	position: NodePosition,
}

/// A node can either be position as Absolute or in a Chain
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NodePosition {
	// Position of the node in grid spaces
	Absolute(IVec2),
	// In a chain the position is based on the number of nodes to the first layer node
	Chain,
}

/// CurrentDocumentNodeTransientMetadata can either be loaded or unloaded. It will be unloaded if it was deserialized from a saved file, in which case it must be recalculated.
pub enum CurrentDocumentNodeTransientMetadata {
	Loaded(DocumentNodeTransientMetadata),
	Unloaded,
}

impl Default for CurrentDocumentNodeTransientMetadata {
    fn default() -> Self {
        CurrentDocumentNodeTransientMetadata::Unloaded
    }
}

impl CurrentDocumentNodeTransientMetadata {
	/// Always returns the Loaded variant, and creates it if it does not exist using persistent metadata data from the network interface
	pub fn get_transient_node_metadata(&mut self, network_interface: &NodeNetworkInterface, node_id: &NodeId, use_document_network: bool) -> &DocumentNodeTransientMetadata {
		match self {
			CurrentDocumentNodeTransientMetadata::Loaded ( document_node_transient_metadata ) => document_node_transient_metadata,
			CurrentDocumentNodeTransientMetadata::Unloaded => {
				*self = CurrentDocumentNodeTransientMetadata::Loaded(DocumentNodeTransientMetadata::new(network_interface, node_id, use_document_network));
				self.get_transient_node_metadata(network_interface, node_id, use_document_network)
			}
		}
	}

	/// Set the current transient metadata to unloaded
	pub fn unload(&mut self) {
		*self = CurrentNodeNetworkTransientMetadata::Unloaded;
	}
}

/// Cached metadata that should be calculated when creating a node, and should be recalculated when modifying a node property that affects one of the cached fields.
#[derive(Debug, Clone)]
pub struct DocumentNodeTransientMetadata {
	/// If performance is a concern then also cache the absolute position for each node
	// pub cached_node_position: DVec2,
	/// Ensure node_click_target is kept in sync when modifying a node property that changes its size. Currently this is alias, inputs, is_layer, and metadata
	pub node_click_target: ClickTarget,
	/// Stores all port click targets in node graph space.
	pub port_click_targets: Ports,
	// Metadata that is specific to either nodes or layers, which are chosen states for displaying as a left-to-right node or bottom-to-top layer.
	pub node_type_metadata: NodeTypeTransientMetadata,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NodeTypeTransientMetadata {
	Layer(LayerTransientMetadata),
	Node, //No transient data is stored exclusively for nodes
}

/// All fields in TransientLayerMetadata should automatically be updated by using the network interface API
#[derive(Debug, Clone)]
pub struct LayerTransientMetadata {
	/// Cache for all visibility buttons. Should be automatically updated when update_click_target is called
	pub visibility_click_target: ClickTarget,
	// TODO: Store click target for the preview button, which will appear when the node is a selected/(hovered?) layer node
	// preview_click_target: ClickTarget,
	/// Stores the width in grid cell units for layer nodes from the left edge of the thumbnail (+12px padding since thumbnail ends between grid spaces) to the end of the node
	/// This is necessary since calculating the layer width through web_sys is very slow
	pub layer_width: u32,
	/// Stores the width in grid cell units for layer nodes from the left edge of the thumbnail to the end of the chain
	/// Should not be a performance concern to calculate when needed with get_chain_width.
	// chain_width: u32,
}

impl DocumentNodeTransientMetadata {
	// Create transient metadata using data from the document node and persistent node metadata
	pub fn new(network_interface: &NodeNetworkInterface, node_id: &NodeId, use_document_network: bool) -> DocumentNodeTransientMetadata {
		let Some(network_metadata) = network_interface.network_metadata(use_document_network) else {
			log::error!("Could not get nested network_metadata in get_transient_node_metadata");
			return None
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata in get_transient_node_metadata");
			return None
		};
		let Some(node_position) = network_interface.get_position(node_id, network_interface.outward_wires(use_document_network), use_document_network) else {
			log::error!("Could not get node position in new DocumentNodeTransientMetadata");
			return None
		};
		let Some(network) = network_interface.network(use_document_network) else {
			log::error!("Could not get network in new DocumentNodeTransientMetadata");
			return None
		};
		let Some(document_node) = network.nodes.get(node_id) else {
			log::error!("Could not get document node in new DocumentNodeTransientMetadata");
			return None
		};
		

		let node_top_left = node_position.as_dvec2() * 24.;
		let mut port_click_targets = Ports::new();
		if !node_metadata.persistent_metadata.is_layer() {
			// Create input/output click targets
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
			let mut output_row_count = if !node_metadata.persistent_metadata.has_primary_output { 1 } else { 0 };
			for output_index in 0..number_of_outputs {
				port_click_targets.insert_node_output(output_index, output_row_count, node_top_left);
				output_row_count += 1;
			}

			let height = std::cmp::max(input_row_count, output_row_count) as u32 * NodeNetworkMetadata::GRID_SIZE;
			let width = 5 * NodeNetworkMetadata::GRID_SIZE;
			let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);

			let radius = 3.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(node_top_left, node_bottom_right, [radius; 4]);
			let node_click_target = ClickTarget { subpath, stroke_width: 1. };

			DocumentNodeTransientMetadata { node_click_target, port_click_targets, node_type_metadata: NodeTypeTransientMetadata::Node }
		} else {
			// Layer inputs
			port_click_targets.insert_layer_input(0, node_top_left);
			if node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
				port_click_targets.insert_layer_input(1, node_top_left);
			}
			port_click_targets.insert_layer_output(node_top_left);

			let layer_width_cells = Self::layer_width_cells(node);
			let width = layer_width_cells * NodeNetworkMetadata::GRID_SIZE;
			let height = 2 * NodeNetworkMetadata::GRID_SIZE;

			// Update visibility button click target
			let visibility_offset = node_top_left + DVec2::new(width as f64, 24.);
			let subpath = Subpath::new_rounded_rect(DVec2::new(-12., -12.) + visibility_offset, DVec2::new(12., 12.) + visibility_offset, [3.; 4]);
			let stroke_width = 1.;
			let visibility_click_target = ClickTarget { subpath, stroke_width };

			// Create layer click target, which is contains the layer and the chain background
			let chain_width_grid_spaces = network_interface.get_chain_width(node_id, use_document_network);

			let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
			let chain_top_left = node_top_left - DVec2::new(chain_width_grid_spaces * NodeNetworkMetadata::GRID_SIZE, 0);
			let radius = 10.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(chain_top_left, node_bottom_right, [radius; 4]);
			let node_click_target = ClickTarget { subpath, stroke_width: 1. };

			DocumentNodeTransientMetadata {
				node_click_target,
				port_click_targets,
				node_type_metadata: NodeTypeTransientMetadata::Layer(LayerTransientMetadata{
					visibility_click_target,
					layer_width: layer_width_cells,
				})
			}
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

/// All persistent editor and Graphene data for a node. Used to serialize and deserialize a node, pass it through the editor, and create definitions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeTemplate {
	pub persistent_node_metadata: DocumentNodePersistentMetadata,
	pub document_node: DocumentNode,
}
