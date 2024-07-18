use crate::messages::{
	portfolio::document::{graph_operation::utility_types::ModifyInputsContext, node_graph::utility_types::FrontEndClickTargets},
	prelude::Responses,
};
use bezier_rs::Subpath;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::{
	concrete,
	document::{value::TaggedValue, DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork},
	Type,
};
use graphene_std::{
	renderer::{ClickTarget, Quad},
	vector::{PointId, VectorModificationType},
};
use interpreted_executor::{dynamic_executor::ResolvedDocumentNodeTypes, node_registry::NODE_REGISTRY};
use std::{
	collections::{HashMap, HashSet},
	hash::{DefaultHasher, Hash, Hasher},
};

use crate::messages::prelude::NodeGraphMessageHandler;

use super::{
	document_metadata::{DocumentMetadata, LayerNodeIdentifier, NodeRelations},
	misc::PTZ,
	nodes::SelectedNodes,
};

/// All network modifications should be done through this API, so the fields cannot be public. However, all fields within this struct can be public since it it not possible to have a public mutable reference.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkInterface {
	/// The node graph that generates this document's artwork. It recursively stores its sub-graphs, so this root graph is the whole snapshot of the document content.
	/// A mutable reference should never be created. It should only be mutated through custom setters which perform the necessary side effects to keep network_metadata in sync
	network: NodeNetwork,
	/// Stores all editor information for a NodeNetwork. For the network this includes viewport transforms, outward links, and bounding boxes. For nodes this includes click target, position, and alias
	network_metadata: NodeNetworkMetadata,
	/// Stores the document network's structural topology. Should automatically kept in sync by the setter methods when changes to the document network are made, which is why it was moved within the interface,
	#[serde(skip)]
	document_metadata: DocumentMetadata,
	/// All input/output types based on the compiled network.
	#[serde(skip)]
	pub resolved_types: ResolvedDocumentNodeTypes,
}

impl PartialEq for NodeNetworkInterface {
	fn eq(&self, other: &Self) -> bool {
		self.network == other.network && self.network_metadata == other.network_metadata
	}
}

// Public immutable getters for the network interface
impl NodeNetworkInterface {
	/// Gets the nested network based on network_path
	pub fn network(&self, network_path: &[NodeId]) -> Option<&NodeNetwork> {
		self.network.nested_network(network_path)
	}

	/// The network metadata should always exist for the current network
	pub fn network_metadata(&self, network_path: &[NodeId]) -> Option<&NodeNetworkMetadata> {
		self.network_metadata.nested_metadata(network_path)
	}

	pub fn document_metadata(&self) -> &DocumentMetadata {
		&self.document_metadata
	}

	/// Get the selected nodes for the network at the network_path
	pub fn selected_nodes(&self, network_path: &[NodeId]) -> Option<&SelectedNodes> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes");
			return None;
		};
		Some(&network_metadata.transient_metadata.selected_nodes)
	}

	/// Get the network which the encapsulating node of the currently viewed network is part of. Will always be None in the document network.
	pub fn get_encapsulating_network_metadata(&self, network_path: &[NodeId]) -> Option<&NodeNetworkMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		encapsulating_path.pop()?;
		let Some(parent_metadata) = self.network_metadata(&encapsulating_path) else {
			log::error!("Could not get parent network in encapsulating_node_metadata");
			return None;
		};
		Some(parent_metadata)
	}

	/// Get the node which encapsulates the currently viewed network. Will always be None in the document network.
	pub fn get_encapsulating_node(&self, network_path: &[NodeId]) -> Option<&DocumentNode> {
		let mut encapsulating_path = network_path.to_vec();
		let encapsulating_node_id = encapsulating_path.pop()?;
		let Some(parent_network) = self.network(&encapsulating_path) else {
			log::error!("Could not get parent network in get_encapsulating_node");
			return None;
		};
		let Some(encapsulating_node) = parent_network.nodes.get(&encapsulating_node_id) else {
			log::error!("Could not get encapsulating node in get_encapsulating_node");
			return None;
		};
		Some(encapsulating_node)
	}

	/// Get the node metadata for the node which encapsulates the currently viewed network. Will always be None in the document network.
	pub fn get_encapsulating_node_metadata(&self, network_path: &[NodeId]) -> Option<&DocumentNodeMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		let encapsulating_node_id = encapsulating_path.pop()?;
		let Some(parent_metadata) = self.network_metadata(&encapsulating_path) else {
			log::error!("Could not get parent network in encapsulating_node_metadata");
			return None;
		};
		let Some(encapsulating_node_metadata) = parent_metadata.persistent_metadata.node_metadata.get(&encapsulating_node_id) else {
			log::error!("Could not get encapsulating node metadata in encapsulating_node_metadata");
			return None;
		};
		Some(encapsulating_node_metadata)
	}

	/// Returns the first downstream layer from a node, inclusive. If the node is a layer, it will return itself
	pub fn downstream_layer(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<LayerNodeIdentifier> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network in downstream_layer");
			return None;
		};
		let outward_wires = self.collect_outward_wires(network_path);
		let mut id = *node_id;
		while !network_metadata.persistent_metadata.node_metadata.get(&node_id)?.persistent_metadata.is_layer() {
			id = outward_wires.get(&OutputConnector::node(id, 0))?.first()?.node_id()?;
		}
		Some(LayerNodeIdentifier::new(id, self))
	}

	pub fn get_chain_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> u32 {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in get_chain_width");
			return 0;
		};
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_chain_width");
			return 0;
		};
		assert!(
			network_metadata
				.persistent_metadata
				.node_metadata
				.get(node_id)
				.is_some_and(|node_metadata| node_metadata.persistent_metadata.is_layer()),
			"Node is not a layer node in get_chain_width"
		);
		let node = network.nodes.get(node_id).expect("Node not found in get_chain_width");
		if node.inputs.len() > 1 {
			let mut last_chain_node_distance = 0u32;
			// Iterate upstream from the layer, and get the number of nodes distance to the last node with Position::Chain
			for (index, (_, node_id)) in self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).enumerate() {
				if let Some(NodeTypePersistentMetadata::Node(node_persistent_metadata)) = network_metadata
					.persistent_metadata
					.node_metadata
					.get(&node_id)
					.map(|node_metadata| &node_metadata.persistent_metadata.node_type_metadata)
				{
					if matches!(node_persistent_metadata.position, NodePosition::Chain) {
						last_chain_node_distance = index as u32;
					}
				}
			}
			last_chain_node_distance * 8
		} else {
			// Layer with no inputs has no chain
			0
		}
	}

	/// Check if the specified node id is connected to the output
	pub fn connected_to_output(&self, target_node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in connected_to_output");
			return false;
		};
		// If the node is the output then return true
		if network
			.exports
			.iter()
			.any(|export| if let NodeInput::Node { node_id, .. } = export { node_id == target_node_id } else { false })
		{
			return true;
		}

		// Get the outputs
		let mut stack = network
			.exports
			.iter()
			.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { network.nodes.get(node_id) } else { None })
			.collect::<Vec<_>>();
		let mut already_visited = HashSet::new();
		already_visited.extend(
			network
				.exports
				.iter()
				.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { Some(node_id) } else { None }),
		);

		while let Some(node) = stack.pop() {
			for input in &node.inputs {
				if let &NodeInput::Node { node_id: ref_id, .. } = input {
					// Skip if already viewed
					if already_visited.contains(&ref_id) {
						continue;
					}
					// If the target node is used as input then return true
					if ref_id == *target_node_id {
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

	fn number_of_imports(&self, network_path: &[NodeId]) -> usize {
		if network_path.len() == 1 {
			return 1;
		};
		// TODO: Use network.import_types.len()
		if let Some(encapsulating_node) = self.get_encapsulating_node(network_path) {
			encapsulating_node.inputs.len()
		} else {
			// There is one(?) import to the document network, but the imports are not displayed
			1
		}
	}

	/// Collect a hashmap of all downstream inputs from an output.
	/// TODO: Only call when reloading transient network data for the outward wires
	pub fn collect_outward_wires(&self, network_path: &[NodeId]) -> HashMap<OutputConnector, Vec<InputConnector>> {
		let mut outward_wires = HashMap::new();
		let Some(network) = self.network(network_path) else {
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
			for output_index in 0..number_of_outputs {
				outward_wires.insert(OutputConnector::node(*node_id, output_index), Vec::new());
			}
		}
		// Initialize output connectors for the import node

		for import_index in 0..self.number_of_imports(network_path) {
			outward_wires.insert(OutputConnector::Import(import_index), Vec::new());
		}
		// Collect wires between all nodes and the Imports
		for (current_node_id, node) in network.nodes.iter() {
			// Collect wires between the nodes as well as exports
			for (input_index, input) in node.inputs.iter().chain(network.exports.iter()).enumerate() {
				if let NodeInput::Node { node_id, output_index, .. } = input {
					let outward_wires_entry = outward_wires
						.get_mut(&OutputConnector::node(*node_id, *output_index))
						.expect("All output connectors should be initialized");
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
	pub fn copy_nodes<'a>(&self, new_ids: &'a HashMap<NodeId, NodeId>, network_path: &[NodeId]) -> impl Iterator<Item = (NodeId, NodeTemplate)> + 'a {
		new_ids
			.iter()
			.filter_map(|(&node_id, &new)| self.create_node_template(node_id, network_path).map(|node_template| (new, node_id, node_template)))
			.map(move |(new, node_id, node)| (new, self.map_ids(node, &node_id, new_ids, network_path)))
			.collect::<Vec<_>>()
			.into_iter()
	}

	pub fn create_node_template(&self, node_id: NodeId, network_path: &[NodeId]) -> Option<NodeTemplate> {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in create_node_template");
			return None;
		};
		let Some(node) = network.nodes.get(&node_id) else {
			log::error!("Could not get node in create_node_template");
			return None;
		};
		let Some(node_metadata) = self
			.network_metadata(network_path)
			.and_then(|network_metadata| network_metadata.persistent_metadata.node_metadata.get(&node_id))
		else {
			log::error!("Could not get node_metadata in create_node_template");
			return None;
		};
		Some(NodeTemplate {
			persistent_node_metadata: node_metadata.persistent_metadata.clone(),
			document_node: node.clone(),
		})
	}

	/// Converts all node id inputs to a new id based on a HashMap.
	///
	/// If the node is not in the hashmap then a default input is found based on the compiled network, using the node_id passed as a parameter
	pub fn map_ids(&self, mut node_template: NodeTemplate, node_id: &NodeId, new_ids: &HashMap<NodeId, NodeId>, network_path: &[NodeId]) -> NodeTemplate {
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
					let tagged_value = TaggedValue::from_type(&self.get_input_type(&InputConnector::node(*node_id, input_index), network_path));
					*input = NodeInput::Value { tagged_value, exposed: true };
				}
			} else if let &mut NodeInput::Network { .. } = input {
				// Always disconnect network node input
				let tagged_value = TaggedValue::from_type(&self.get_input_type(&InputConnector::node(*node_id, input_index), network_path));
				*input = NodeInput::Value { tagged_value, exposed: true };
			}
		}
		node_template
	}

	pub fn get_input(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<&NodeInput> {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in get_input");
			return None;
		};
		match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(node) = network.nodes.get(&node_id) else {
					log::error!("Could not get node in get_input");
					return None;
				};
				node.inputs.get(*input_index)
			}
			InputConnector::Export(export_index) => network.exports.get(*export_index),
		}
	}

	/// Get the [`Type`] for any InputConnector
	pub fn get_input_type(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Type {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in get_tagged_value");
			return concrete!(());
		};

		// TODO: Store types for all document nodes, not just the compiled proto nodes, which currently skips isolated nodes
		let node_type_from_compiled_network = if let Some(node_id) = input_connector.node_id() {
			let node_id_path = [&network_path[..], &[node_id]].concat().clone();
			let input_type = self.resolved_types.inputs.get(&graph_craft::document::Source {
				node: node_id_path,
				index: input_connector.input_index(),
			});
			input_type.cloned()
		} else if let Some(encapsulating_node) = self.get_encapsulating_node(network_path) {
			let output_types = NodeGraphMessageHandler::get_output_types(encapsulating_node, &self.resolved_types, network_path);
			output_types.iter().nth(input_connector.input_index()).map_or_else(
				|| {
					warn!("Could not find output type for export node");
					Some(concrete!(()))
				},
				|output_type| output_type.clone().map_or(Some(concrete!(())), |output| Some(output)),
			)
		} else {
			Some(concrete!(graphene_core::ArtboardGroup))
		};

		node_type_from_compiled_network.unwrap_or_else(|| {
			// TODO: Once there is type inference (#1621), replace this workaround approach when disconnecting node inputs with NodeInput::Node(ToDefaultNode),
			// TODO: which would be a new node that implements the Default trait (i.e. `Default::default()`)

			// Resolve types from proto nodes in node_registry
			let Some(node_id) = input_connector.node_id() else {
				return concrete!(());
			};
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

			get_type_from_node(node, input_connector.input_index())
		})
	}

	/// Get the top left position in node graph coordinates for a node by recursively iterating downstream
	pub fn get_position(&self, node_id: &NodeId, outward_wires: &HashMap<OutputConnector, Vec<InputConnector>>, network_path: &[NodeId]) -> Option<IVec2> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_position");
			return None;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata in get_position");
			return None;
		};
		match &node_metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => {
				match layer_metadata.position {
					LayerPosition::Absolute(position) => return Some(position),
					LayerPosition::Stack(y_offset) => {
						// TODO: Use root node to restore if previewing
						let Some(downstream_node_connectors) = outward_wires.get(&OutputConnector::node(*node_id, 0)) else {
							log::error!("Could not get downstream node in get_position");
							return None;
						};
						let Some(downstream_node_id) = downstream_node_connectors.iter().find_map(|input_connector| {
							if let InputConnector::Node { node_id, input_index } = input_connector {
								if *input_index == 0 {
									Some(node_id)
								} else {
									None
								}
							} else {
								None
							}
						}) else {
							log::error!("Could not get downstream node input connector with input index 0");
							return None;
						};
						return self
							.get_position(downstream_node_id, &outward_wires, network_path)
							.map(|position| position + IVec2::new(0, y_offset as i32));
					}
				}
			}
			NodeTypePersistentMetadata::Node(node_metadata) => {
				match node_metadata.position {
					NodePosition::Absolute(position) => Some(position),
					NodePosition::Chain => {
						// Iterate through primary flow to find the first Layer
						let mut current_node_id = node_id;
						let mut node_distance_from_layer = 1;
						loop {
							// TODO: Use root node to restore if previewing
							let Some(downstream_node_connectors) = outward_wires.get(&OutputConnector::node(*node_id, 0)) else {
								log::error!("Could not get downstream node for node with Position::Chain");
								return None;
							};
							let Some(downstream_node_id) = downstream_node_connectors.iter().find_map(|input_connector| {
								if let InputConnector::Node { node_id, input_index } = input_connector {
									if *input_index == 0 {
										Some(node_id)
									} else {
										None
									}
								} else {
									None
								}
							}) else {
								log::error!("Could not get downstream node input connector with input index 0 for node with Position::Chain");
								return None;
							};
							let Some(downstream_node_metadata) = network_metadata.persistent_metadata.node_metadata.get(downstream_node_id) else {
								log::error!("Downstream node metadata not found in node_metadata for node with Position::Chain");
								return None;
							};
							if downstream_node_metadata.persistent_metadata.is_layer() {
								// Get the position of the layer
								let Some(layer_position) = self.get_position(downstream_node_id, &outward_wires, network_path) else {
									return None;
								};
								return Some(layer_position + IVec2::new(0, node_distance_from_layer * 8));
							}
							node_distance_from_layer += 1;
							current_node_id = downstream_node_id;
						}
					}
				}
			}
		}
	}

	pub fn get_upstream_output_connector(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<OutputConnector> {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in get_upstream_node_from_input");
			return None;
		};
		let input = match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(node) = network.nodes.get(&node_id) else {
					log::error!("Could not get node in get_upstream_node_from_input");
					return None;
				};
				node.inputs.get(*input_index)
			}
			InputConnector::Export(export_index) => network.exports.get(*export_index),
		};
		input.and_then(|input| match input {
			NodeInput::Node { node_id, output_index, .. } => Some(OutputConnector::node(*node_id, *output_index)),
			NodeInput::Network { import_index, .. } => Some(OutputConnector::Import(*import_index)),
			_ => None,
		})
	}

	pub fn previewing(&self, network_path: &[NodeId]) -> Previewing {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in previewing");
			return Previewing::No;
		};
		network_metadata.persistent_metadata.previewing
	}

	/// Returns the root node (the node that the solid line is connect to), or None if no nodes are connected to the output
	pub fn get_root_node(&self, network_path: &[NodeId]) -> Option<RootNode> {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in get_root_node");
			return None;
		};
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_root_node");
			return None;
		};
		match &network_metadata.persistent_metadata.previewing {
			Previewing::Yes { root_node_to_restore } => *root_node_to_restore,
			Previewing::No => network.exports.first().and_then(|export| {
				if let NodeInput::Node { node_id, output_index, .. } = export {
					Some(RootNode {
						node_id: *node_id,
						output_index: *output_index,
					})
				} else {
					None
				}
			}),
		}
	}

	pub fn persistent_node_metadata(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&DocumentNodePersistentMetadata> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata");
			return None;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata for node {node_id}");
			return None;
		};
		Some(&node_metadata.persistent_metadata)
	}

	pub fn get_reference(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<String> {
		self.persistent_node_metadata(&node_id, network_path)
			.and_then(|node_metadata| node_metadata.reference.as_ref().map(|reference| reference.to_string()))
	}

	pub fn get_display_name(&self, node_id: &NodeId, network_path: &[NodeId]) -> String {
		let Some(node_metadata) = self.persistent_node_metadata(&node_id, network_path) else {
			log::error!("Could not get node_metadata in get_alias");
			return "".to_string();
		};
		node_metadata.display_name.clone()
	}

	pub fn frontend_display_name(&self, node_id: &NodeId, network_path: &[NodeId]) -> String {
		let is_layer = self
			.persistent_node_metadata(node_id, network_path)
			.expect("Could not get persistent node metadata in untitled_layer_label")
			.is_layer();
		let reference = self.get_reference(node_id, network_path);
		let is_merge_node = reference.as_ref().is_some_and(|reference| reference == "Merge");
		if self.get_display_name(node_id, network_path).is_empty() {
			if is_layer && is_merge_node {
				"Untitled Layer".to_string()
			} else {
				reference.unwrap_or("Untitled node".to_string())
			}
		} else {
			self.get_display_name(node_id, network_path)
		}
	}

	pub fn is_locked(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(persistent_node_metadata) = self.persistent_node_metadata(node_id, network_path) else {
			log::error!("Could not get persistent node metadata in get_locked for node {node_id}");
			return false;
		};
		persistent_node_metadata.locked
	}

	pub fn is_visible(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network_metadata in is_visible");
			return false;
		};
		let Some(node) = network.nodes.get(node_id) else {
			log::error!("Could not get nested node_metadata in is_visible");
			return false;
		};
		node.visible
	}

	pub fn is_layer(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in is_layer");
			return false;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata in is_layer");
			return false;
		};
		node_metadata.persistent_metadata.is_layer()
	}

	pub fn is_eligible_to_be_layer(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in is_eligible_to_be_layer");
			return false;
		};

		let Some(node) = network.nodes.get(&node_id) else { return false };

		let input_count = node.inputs.iter().filter(|input| input.is_exposed_to_frontend(network_path.is_empty())).count();
		let output_count = if let graph_craft::document::DocumentNodeImplementation::Network(nested_network) = &node.implementation {
			nested_network.exports.len()
		} else {
			// Node is a Protonode, so it must have 1 output
			1
		};

		let outward_wires = self.collect_outward_wires(network_path).get(&OutputConnector::node(*node_id, 0)).cloned().unwrap_or(Vec::new());
		let has_single_output_wire = outward_wires.len() <= 1;

		// TODO: Eventually allow nodes at the bottom of a stack to be layers, where `input_count` is 0
		self.persistent_node_metadata(node_id, network_path).is_some_and(|node_metadata| node_metadata.has_primary_output)
			&& output_count == 1
			&& (input_count == 1 || input_count == 2)
			&& has_single_output_wire
	}

	pub fn has_primary_output(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(node_metadata) = self.persistent_node_metadata(node_id, network_path) else {
			log::error!("Could not get node_metadata in has_primary_output");
			return false;
		};
		node_metadata.has_primary_output
	}

	pub fn is_artboard(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(node_metadata) = self.persistent_node_metadata(node_id, network_path) else {
			log::error!("Could not get nested network_metadata in is_artboard");
			return false;
		};
		node_metadata
			.reference
			.as_ref()
			.is_some_and(|reference| reference == "Artboard" && self.connected_to_output(&node_id, &[]))
	}

	pub fn all_artboards(&self) -> HashSet<LayerNodeIdentifier> {
		self.network_metadata(&[])
			.unwrap()
			.persistent_metadata
			.node_metadata
			.iter()
			.filter_map(|(node_id, node_metadata)| {
				if node_metadata
					.persistent_metadata
					.reference
					.as_ref()
					.is_some_and(|reference| reference == "Artboard" && self.connected_to_output(node_id, &[]))
				{
					Some(LayerNodeIdentifier::new(*node_id, self))
				} else {
					None
				}
			})
			.collect()
	}

	/// Folders sorted from most nested to least nested
	pub fn folders_sorted_by_most_nested(&self, network_path: &[NodeId]) -> Vec<LayerNodeIdentifier> {
		if network_path.len() != 0 {
			log::error!("Currently can only get deepest common ancestor in the document network");
			return Vec::new();
		}
		let Some(selected_nodes) = self.selected_nodes(network_path) else {
			log::error!("Could not get selected nodes in deepest_common_ancestor");
			return Vec::new();
		};
		let mut folders: Vec<_> = selected_nodes
			.selected_layers(self.document_metadata())
			.filter(|layer| layer.has_children(&self.document_metadata()))
			.collect();
		folders.sort_by_cached_key(|a| std::cmp::Reverse(a.ancestors(self.document_metadata()).count()));
		folders
	}

	/// Calculates the document bounds in document space
	pub fn document_bounds_document_space(&self, include_artboards: bool) -> Option<[DVec2; 2]> {
		self.document_metadata
			.all_layers()
			.filter(|layer| include_artboards || !self.is_artboard(&layer.to_node(), &[]))
			.filter_map(|layer| self.document_metadata.bounding_box_document(layer))
			.reduce(Quad::combine_bounds)
	}

	/// Calculates the selected layer bounds in document space
	pub fn selected_bounds_document_space(&self, include_artboards: bool, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let Some(selected_nodes) = self.selected_nodes(network_path) else {
			log::error!("Could not get selected nodes in shallowest_unique_layers");
			return None;
		};
		selected_nodes
			.selected_layers(&self.document_metadata)
			.filter(|&layer| include_artboards || !self.is_artboard(&layer.to_node(), &[]))
			.filter_map(|layer| self.document_metadata.bounding_box_document(layer))
			.reduce(Quad::combine_bounds)
	}

	/// Layers excluding ones that are children of other layers in the list.
	/// TODO: Cache this
	pub fn shallowest_unique_layers(&self, network_path: &[NodeId]) -> impl Iterator<Item = LayerNodeIdentifier> {
		let mut sorted_layers = if let Some(selected_nodes) = self.selected_nodes(network_path) {
			selected_nodes
				.selected_layers(self.document_metadata())
				.map(|layer| {
					let mut layer_path = layer.ancestors(&self.document_metadata).collect::<Vec<_>>();
					layer_path.reverse();
					layer_path
				})
				.collect::<Vec<_>>()
		} else {
			log::error!("Could not get selected nodes in shallowest_unique_layers");
			Vec::new()
		};

		// Sorting here creates groups of similar UUID paths
		sorted_layers.sort();
		sorted_layers.dedup_by(|a, b| a.starts_with(b));
		sorted_layers.into_iter().map(|mut path| {
			let layer = path.pop().expect("Path should not be empty");
			assert!(
				layer != LayerNodeIdentifier::ROOT_PARENT,
				"The root parent cannot be selected, so it cannot be a shallowest selected layer"
			);
			layer
		})
	}

	/// Ancestor that is shared by all layers and that is deepest (more nested). Default may be the root. Skips selected non-folder, non-artboard layers
	pub fn deepest_common_ancestor(&self, network_path: &[NodeId], include_self: bool) -> Option<LayerNodeIdentifier> {
		if network_path.len() != 0 {
			log::error!("Currently can only get deepest common ancestor in the document network");
			return None;
		}
		let Some(selected_nodes) = self.selected_nodes(network_path) else {
			log::error!("Could not get selected nodes in deepest_common_ancestor");
			return None;
		};
		selected_nodes
			.selected_layers(&self.document_metadata)
			.map(|layer| {
				let mut layer_path = layer.ancestors(&self.document_metadata).collect::<Vec<_>>();
				layer_path.reverse();

				if !include_self || !self.is_artboard(&layer.to_node(), network_path) {
					layer_path.pop();
				}

				layer_path
			})
			.reduce(|mut a, b| {
				a.truncate(a.iter().zip(b.iter()).position(|(&a, &b)| a != b).unwrap_or_else(|| a.len().min(b.len())));
				a
			})
			.and_then(|layer| layer.last().copied())
	}

	/// Gives an iterator to all nodes connected to the given nodes (inclusive) by all inputs (primary or primary + secondary depending on `only_follow_primary` choice), traversing backwards upstream starting from the given node's inputs.
	pub fn upstream_flow_back_from_nodes(&self, mut node_ids: Vec<NodeId>, network_path: &[NodeId], mut flow_type: FlowType) -> impl Iterator<Item = (&DocumentNode, NodeId)> {
		let (Some(network), Some(network_metadata)) = (self.network(network_path), self.network_metadata(network_path)) else {
			log::error!("Could not get network or network_metadata in upstream_flow_back_from_nodes");
			return Vec::new().into_iter();
		};
		if matches!(flow_type, FlowType::LayerChildrenUpstreamFlow) {
			node_ids = node_ids
				.iter()
				.filter_map(move |node_id| network.nodes.get(&node_id).and_then(|node| node.inputs.get(1)).and_then(|input| input.as_node()))
				.collect::<Vec<_>>();
			flow_type = FlowType::UpstreamFlow;
		};
		FlowIter {
			stack: node_ids,
			network,
			network_metadata,
			flow_type,
		}.collect::<Vec<_>>() // TODO: Why is this necessary
		.into_iter()
	}

	/// In the network `X -> Y -> Z`, `is_node_upstream_of_another_by_primary_flow(Z, X)` returns true.
	pub fn is_node_upstream_of_another_by_horizontal_flow(&self, node: NodeId, network_path: &[NodeId], potentially_upstream_node: NodeId) -> bool {
		self.upstream_flow_back_from_nodes(vec![node], network_path, FlowType::HorizontalFlow)
			.any(|(_, id)| id == potentially_upstream_node)
	}

	#[cfg(not(target_arch = "wasm32"))]
	fn get_text_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<f64> {
		warn!("Failed to find width of {node_id:#?} due to non-wasm arch");
		None
	}

	#[cfg(target_arch = "wasm32")]
	fn get_text_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<f64> {
		let document = web_sys::window().unwrap().document().unwrap();
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

		let name = self.frontend_display_name(node_id, network_path);

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
}

// Private mutable getters for use within the network interface
impl NodeNetworkInterface {
	fn network_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetwork> {
		self.network.nested_network_mut(network_path)
	}

	fn network_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkMetadata> {
		self.network_metadata.nested_metadata_mut(network_path)
	}

	/// Mutably get the network which the encapsulating node of the currently viewed network is part of. Will always be None in the document network.
	pub fn get_encapsulating_network_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		encapsulating_path.pop()?;
		let Some(parent_metadata) = self.network_metadata_mut(&encapsulating_path) else {
			log::error!("Could not get parent network in encapsulating_node_metadata");
			return None;
		};
		Some(parent_metadata)
	}

	/// Mutably get the node which encapsulates the currently viewed network. Will always be None in the document network.
	fn get_encapsulating_node_mut(&mut self, network_path: &[NodeId]) -> Option<&mut DocumentNode> {
		let mut encapsulating_path = network_path.to_vec();
		let encapsulating_node_id = encapsulating_path.pop()?;
		let Some(parent_network) = self.network_mut(&encapsulating_path) else {
			log::error!("Could not get parent network in get_encapsulating_node_mut");
			return None;
		};
		let Some(encapsulating_node) = parent_network.nodes.get_mut(&encapsulating_node_id) else {
			log::error!("Could not get encapsulating node in get_encapsulating_node_mut");
			return None;
		};
		Some(encapsulating_node)
	}

	/// Get the node metadata for the node which encapsulates the currently viewed network. Will always be None in the document network.
	fn get_encapsulating_node_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut DocumentNodeMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		let encapsulating_node_id = encapsulating_path.pop()?;
		let Some(parent_metadata) = self.network_metadata_mut(&encapsulating_path) else {
			log::error!("Could not get parent network in encapsulating_node_metadata");
			return None;
		};
		let Some(encapsulating_node_metadata) = parent_metadata.persistent_metadata.node_metadata.get_mut(&encapsulating_node_id) else {
			log::error!("Could not get encapsulating node metadata in encapsulating_node_metadata");
			return None;
		};
		Some(encapsulating_node_metadata)
	}
}

// Public mutable getters for data that involves transient network metadata
impl NodeNetworkInterface {
	/// Mutably get the selected nodes for the network at the network_path
	pub fn selected_nodes_mut(&mut self, network_path: &[NodeId]) -> Option<&mut SelectedNodes> {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes");
			return None;
		};
		Some(&mut network_metadata.transient_metadata.selected_nodes)
	}

	pub fn get_import_ports(&mut self, network_path: &[NodeId]) -> Option<&Ports> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_import_ports");
			return None;
		};
		if !network_metadata.transient_metadata.import_ports.is_loaded() {
			self.load_import_ports(network_path);
		}
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_import_ports");
			return None;
		};
		let TransientMetadata::Loaded(ports) = &network_metadata.transient_metadata.import_ports else {
			log::error!("could not load import ports");
			return None;
		};
		Some(ports)
	}

	pub fn load_import_ports(&mut self, network_path: &[NodeId]) {
		let import_top_left = DVec2::new(0., 0.) * 24.;
		let mut import_ports = Ports::new();
		for import_index in 0..self.number_of_imports(network_path) {
			// Skip first row since the first row is reserved for the "Exports" name
			import_ports.insert_node_output(import_index, import_index + 1, import_top_left);
		}
		// If the network is the document network, then there are no import ports
		if network_path.len() == 0 {
			import_ports = Ports::new()
		}
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in load_import_node_ports");
			return;
		};
		network_metadata.transient_metadata.import_ports = TransientMetadata::Loaded(import_ports);
	}

	pub fn get_export_ports(&mut self, network_path: &[NodeId]) -> Option<&Ports> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_export_ports");
			return None;
		};
		if !network_metadata.transient_metadata.export_ports.is_loaded() {
			self.load_export_ports(network_path);
		}
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_export_ports");
			return None;
		};
		let TransientMetadata::Loaded(ports) = &network_metadata.transient_metadata.export_ports else {
			log::error!("could not load import ports");
			return None;
		};
		Some(ports)
	}

	pub fn load_export_ports(&mut self, network_path: &[NodeId]) {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return;
		};
		let export_top_left = DVec2::new(10., 0.) * 24.;
		let mut export_ports = Ports::new();
		for output_index in 0..network.exports.len() {
			// Skip first row since the first row is reserved for the "Exports" name
			export_ports.insert_node_input(output_index, output_index + 1, export_top_left);
		}
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return;
		};
		network_metadata.transient_metadata.export_ports = TransientMetadata::Loaded(export_ports);
	}

	pub fn load_layer_width(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let half_grid_cell_offset = 24. / 2.;
		let thumbnail_width = 3. * 24.;
		let gap_width = 8.;
		let text_width = self.get_text_width(node_id, network_path).unwrap_or_else(|| {
			log::error!("Could not get text width for node {node_id}");
			0.
		});
		let icon_width = 24.;
		let icon_overhang_width = icon_width / 2.;

		let text_right = half_grid_cell_offset + thumbnail_width + gap_width + text_width;
		let layer_width_pixels = text_right + gap_width + icon_width - icon_overhang_width;
		let layer_width = ((layer_width_pixels / 24.) as u32).max(8);

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in get_transient_node_metadata");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get nested node_metadata in get_transient_node_metadata");
			return;
		};

		// Ensure layer width is not loaded for a non layer node
		if node_metadata.persistent_metadata.is_layer() {
			// Set the entire transient node type metadata to be a layer, in case it was previously a node
			node_metadata.transient_metadata.node_type_metadata = NodeTypeTransientMetadata::Layer(LayerTransientMetadata {
				layer_width: TransientMetadata::Loaded(layer_width),
			});
		} else {
			log::warn!("Tried loading layer width for non layer node");
		}
	}

	pub fn get_layer_width(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<u32> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_transient_node_metadata");
			return None;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata in get_transient_node_metadata");
			return None;
		};
		if !node_metadata.persistent_metadata.is_layer() {
			log::error!("Cannot get layer width for non layer node");
			return None;
		}

		let layer_width_loaded = if let NodeTypeTransientMetadata::Layer(layer_metadata) = &node_metadata.transient_metadata.node_type_metadata {
			layer_metadata.layer_width.is_loaded()
		} else {
			false
		};
		if !layer_width_loaded {
			self.load_layer_width(node_id, network_path);
		}

		let network_metadata = self.network_metadata(network_path)?;
		let node_metadata = network_metadata.persistent_metadata.node_metadata.get(node_id)?;
		let NodeTypeTransientMetadata::Layer(layer_metadata) = &node_metadata.transient_metadata.node_type_metadata else {
			log::error!("Transient metadata should be layer metadata when getting layer width");
			return None;
		};
		let TransientMetadata::Loaded(layer_width) = layer_metadata.layer_width else {
			log::error!("Transient metadata was not loaded when getting layer width");
			return None;
		};

		Some(layer_width)
	}

	pub fn load_node_click_targets(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in new DocumentNodeTransientMetadata");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata in new DocumentNodeTransientMetadata");
			return;
		};
		let Some(node_position) = self.get_position(node_id, &self.collect_outward_wires(network_path), network_path) else {
			log::error!("Could not get node position in new DocumentNodeTransientMetadata");
			return;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in new DocumentNodeTransientMetadata");
			return;
		};
		let Some(document_node) = network.nodes.get(node_id) else {
			log::error!("Could not get document node in new DocumentNodeTransientMetadata");
			return;
		};

		let node_top_left = node_position.as_dvec2() * 24.;
		let mut port_click_targets = Ports::new();
		let document_node_click_targets = if !node_metadata.persistent_metadata.is_layer() {
			// Create input/output click targets
			let mut input_row_count = 0;
			for (input_index, input) in document_node.inputs.iter().enumerate() {
				if input.is_exposed() {
					port_click_targets.insert_node_input(input_index, input_row_count, node_top_left);
				}
				// Primary input row is always displayed, even if the input is not exposed
				if input_index == 0 || input.is_exposed() {
					input_row_count += 1;
				}
			}

			let number_of_outputs = if let DocumentNodeImplementation::Network(network) = &document_node.implementation {
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
			let node_click_target_top_left = node_top_left + DVec2::new(0., 12.);
			let node_click_target_bottom_right = node_click_target_top_left + DVec2::new(width as f64, height as f64);

			let radius = 3.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(node_click_target_top_left, node_click_target_bottom_right, [radius; 4]);
			let node_click_target = ClickTarget { subpath, stroke_width: 1. };

			DocumentNodeClickTargets {
				node_click_target,
				port_click_targets,
				node_type_metadata: NodeTypeClickTargets::Node,
			}
		} else {
			// Layer inputs
			port_click_targets.insert_layer_input(0, node_top_left);
			if document_node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
				port_click_targets.insert_layer_input(1, node_top_left);
			}
			port_click_targets.insert_layer_output(node_top_left);

			let layer_width_cells = self.get_layer_width(node_id, network_path).unwrap_or_else(|| {
				log::error!("Could not get layer width in get_transient_node_metadata");
				0
			});
			let width = layer_width_cells * NodeNetworkMetadata::GRID_SIZE;
			let height = 2 * NodeNetworkMetadata::GRID_SIZE;

			// Update visibility button click target
			let visibility_offset = node_top_left + DVec2::new(width as f64, 24.);
			let subpath = Subpath::new_rounded_rect(DVec2::new(-12., -12.) + visibility_offset, DVec2::new(12., 12.) + visibility_offset, [3.; 4]);
			let stroke_width = 1.;
			let visibility_click_target = ClickTarget { subpath, stroke_width };

			// Create layer click target, which is contains the layer and the chain background
			let chain_width_grid_spaces = self.get_chain_width(node_id, network_path);

			let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
			let chain_top_left = node_top_left - DVec2::new((chain_width_grid_spaces * NodeNetworkMetadata::GRID_SIZE) as f64, 0.0);
			let radius = 10.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(chain_top_left, node_bottom_right, [radius; 4]);
			let node_click_target = ClickTarget { subpath, stroke_width: 1. };

			DocumentNodeClickTargets {
				node_click_target,
				port_click_targets,
				node_type_metadata: NodeTypeClickTargets::Layer(LayerClickTargets { visibility_click_target }),
			}
		};

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in get_transient_node_metadata");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get nested node_metadata in get_transient_node_metadata");
			return;
		};
		node_metadata.transient_metadata.click_targets = TransientMetadata::Loaded(document_node_click_targets);
	}

	pub fn get_node_click_targets(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&DocumentNodeClickTargets> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_transient_node_metadata");
			return None;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata in get_transient_node_metadata");
			return None;
		};
		if !node_metadata.transient_metadata.click_targets.is_loaded() {
			self.load_node_click_targets(node_id, network_path)
		};
		let network_metadata = self.network_metadata(network_path)?;
		let node_metadata = network_metadata.persistent_metadata.node_metadata.get(node_id)?;
		let TransientMetadata::Loaded(click_target) = &node_metadata.transient_metadata.click_targets else {
			log::error!("Could not load node type metadata when getting click targets");
			return None;
		};
		Some(click_target)
	}
}

// Misc
impl NodeNetworkInterface {
	pub fn collect_front_end_click_targets(&mut self, network_path: &[NodeId]) -> FrontEndClickTargets {
		let mut all_node_click_targets = Vec::new();
		let mut port_click_targets = Vec::new();
		let mut visibility_click_targets = Vec::new();
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in collect_node_click_targets");
			return FrontEndClickTargets::default();
		};
		network_metadata
			.persistent_metadata
			.node_metadata
			.iter()
			.map(|(node_id, _)| node_id.clone())
			.collect::<Vec<_>>()
			.into_iter()
			.for_each(|node_id| {
				if let Some(node_click_targets) = self.get_node_click_targets(&node_id, network_path) {
					let mut node_path = String::new();
					node_click_targets.node_click_target.subpath.subpath_to_svg(&mut node_path, DAffine2::IDENTITY);
					all_node_click_targets.push((node_id, node_path));
					for port in node_click_targets.port_click_targets.get_click_targets() {
						let mut port_path = String::new();
						port.subpath.subpath_to_svg(&mut port_path, DAffine2::IDENTITY);
						port_click_targets.push(port_path);
					}
					if let NodeTypeClickTargets::Layer(layer_metadata) = &node_click_targets.node_type_metadata {
						let mut port_path = String::new();
						layer_metadata.visibility_click_target.subpath.subpath_to_svg(&mut port_path, DAffine2::IDENTITY);
						visibility_click_targets.push(port_path);
					}
				}
			});
		let mut layer_click_targets = Vec::new();
		let mut node_click_targets = Vec::new();
		all_node_click_targets.into_iter().for_each(|(node_id, path)| {
			if self.is_layer(&node_id, network_path) {
				layer_click_targets.push(path);
			} else {
				node_click_targets.push(path);
			}
		});
		FrontEndClickTargets {
			node_click_targets,
			layer_click_targets,
			port_click_targets,
			visibility_click_targets,
		}
	}

	pub fn set_document_to_viewport_transform(&mut self, transform: DAffine2) {
		let document_metadata = self.document_metadata_mut();
		document_metadata.document_to_viewport = transform;
	}

	pub fn vector_modify(&mut self, node_id: &NodeId, modification_type: VectorModificationType) {
		let Some(node) = self.network_mut(&[]).unwrap().nodes.get_mut(node_id) else {
			log::error!("Could not get node in get_vector_modification");
			return;
		};

		let [_, NodeInput::Value {
			tagged_value: TaggedValue::VectorModification(modification),
			..
		}] = node.inputs.as_mut_slice()
		else {
			panic!("Path node does not have modification input");
		};
		modification.modify(&modification_type);
	}

	pub fn get_node_graph_ptz_mut(&mut self, network_path: &[NodeId]) -> Option<&mut PTZ> {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in get_node_graph_ptz_mut");
			return None;
		};
		Some(&mut network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz)
	}

	/// Click target getter methods
	pub fn get_node_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<NodeId> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_node_from_click");
			return None;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in get_node_from_point");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let nodes = network.nodes.iter().map(|(node_id, _)| *node_id).collect::<Vec<_>>();
		let clicked_nodes = nodes
			.iter()
			.filter(|node_id| {
				self.get_node_click_targets(node_id, network_path)
					.is_some_and(|transient_node_metadata| transient_node_metadata.node_click_target.intersect_point(point, DAffine2::IDENTITY))
			})
			.cloned()
			.collect::<Vec<_>>();
		// Since nodes are placed on top of layer chains, find the first non layer node that was clicked, and if there way no non layer nodes clicked, then find the first layer node that was clicked
		clicked_nodes
			.iter()
			.find_map(|node_id| {
				let Some(node_metadata) = self.network_metadata.persistent_metadata.node_metadata.get(node_id) else {
					log::debug!("Could not get node_metadata for node {node_id}");
					return None;
				};
				if node_metadata.persistent_metadata.is_layer() {
					Some(node_id.clone())
				} else {
					None
				}
			})
			.or_else(|| clicked_nodes.into_iter().next())
	}

	pub fn get_visibility_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<NodeId> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_node_from_click");
			return None;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in get_node_from_point");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let node_ids: Vec<_> = network.nodes.iter().map(|(node_id, _)| *node_id).collect();

		node_ids
			.iter()
			.filter_map(|node_id| {
				self.get_node_click_targets(node_id, network_path).and_then(|transient_node_metadata| {
					if let NodeTypeClickTargets::Layer(layer) = &transient_node_metadata.node_type_metadata {
						layer.visibility_click_target.intersect_point(point, DAffine2::IDENTITY).then(|| *node_id)
					} else {
						None
					}
				})
			})
			.next()
	}

	pub fn get_input_connector_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<InputConnector> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_node_from_click");
			return None;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in get_node_from_point");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let nodes = network.nodes.iter().map(|(node_id, _)| *node_id).collect::<Vec<_>>();
		nodes
			.iter()
			.filter_map(|node_id| {
				self.get_node_click_targets(node_id, network_path)
					.and_then(|transient_node_metadata| {
						transient_node_metadata
							.port_click_targets
							.clicked_input_port_from_point(point)
							.map(|port| InputConnector::node(*node_id, port))
					})
					.or_else(|| {
						self.get_export_ports(network_path)
							.and_then(|export_ports| export_ports.clicked_input_port_from_point(point).map(|port| InputConnector::Export(port)))
					})
			})
			.next()
	}

	pub fn get_output_connector_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<OutputConnector> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in get_node_from_click");
			return None;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in get_node_from_point");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let nodes = network.nodes.iter().map(|(node_id, _)| *node_id).collect::<Vec<_>>();
		nodes
			.iter()
			.filter_map(|node_id| {
				self.get_node_click_targets(node_id, network_path)
					.and_then(|transient_node_metadata| {
						transient_node_metadata
							.port_click_targets
							.clicked_output_port_from_point(point)
							.map(|output_index| OutputConnector::node(*node_id, output_index))
					})
					.or_else(|| {
						self.get_export_ports(network_path)
							.and_then(|export_ports| export_ports.clicked_output_port_from_point(point).map(|output_index| OutputConnector::Import(output_index)))
					})
			})
			.next()
	}

	pub fn node_bounding_box(&mut self, node_id: NodeId, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		self.get_node_click_targets(&node_id, network_path)
			.and_then(|transient_node_metadata| transient_node_metadata.node_click_target.subpath.bounding_box())
	}

	pub fn get_input_position(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match input_connector {
			InputConnector::Node { node_id, input_index } => self
				.get_node_click_targets(&node_id, network_path)
				.and_then(|transient_node_metadata| transient_node_metadata.port_click_targets.get_input_port_position(*input_index)),
			InputConnector::Export(_import_index) => None, // TODO: Implement getting position for the new import connection UI
		}
	}
	pub fn get_output_position(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match output_connector {
			OutputConnector::Node { node_id, output_index } => self
				.get_node_click_targets(&node_id, network_path)
				.and_then(|transient_node_metadata| transient_node_metadata.port_click_targets.get_output_port_position(*output_index)),
			OutputConnector::Import(import_index) => None, // TODO: Implement getting position for the new import connection UI
		}
	}

	/// Get the combined bounding box of the click targets of the selected nodes in the node graph in viewport space
	pub fn selected_nodes_bounding_box_viewport(&mut self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		// Always get the bounding box for nodes in the currently viewed network
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes_bounding_box_viewport");
			return None;
		};
		let Some(selected_nodes) = self.selected_nodes(network_path) else {
			log::error!("Could not get selected nodes in selected_nodes_bounding_box_viewport");
			return None;
		};
		let node_graph_to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
		selected_nodes
			.selected_nodes()
			.cloned()
			.collect::<Vec<_>>()
			.iter()
			.filter_map(|node_id| {
				let Some(node_metadata) = self.network_metadata.persistent_metadata.node_metadata.get(&node_id) else {
					log::debug!("Could not get click target for node {node_id}");
					return None;
				};
				self.get_node_click_targets(&node_id, network_path)
					.and_then(|transient_node_metadata| transient_node_metadata.node_click_target.subpath.bounding_box_with_transform(node_graph_to_viewport))
			})
			.reduce(graphene_core::renderer::Quad::combine_bounds)
	}

	/// Gets the bounding box in viewport coordinates for each node in the node graph
	pub fn graph_bounds_viewport_space(&mut self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes_bounding_box_viewport");
			return None;
		};
		let nodes = network_metadata.persistent_metadata.node_metadata.iter().map(|(node_id, _)| *node_id).collect::<Vec<_>>();

		// Get bounding box around all nodes. Cache this data in transient network metadata if it is too slow to calculate on every frame.
		let bounds = nodes
			.iter()
			.filter_map(|node_id| {
				self.get_node_click_targets(node_id, network_path)
					.and_then(|transient_node_metadata| transient_node_metadata.node_click_target.subpath.bounding_box())
			})
			.reduce(Quad::combine_bounds);
		let bounding_box_subpath = bounds.map(|bounds| bezier_rs::Subpath::<PointId>::new_rect(bounds[0], bounds[1]));
		bounding_box_subpath
			.as_ref()
			.and_then(|bounding_box| bounding_box.bounding_box_with_transform(self.network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport))
	}

	pub fn collect_layer_widths(&mut self, network_path: &[NodeId]) -> HashMap<NodeId, u32> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in collect_layer_widths");
			return HashMap::new();
		};
		let nodes = network_metadata.persistent_metadata.node_metadata.iter().map(|(node_id, _)| *node_id).collect::<Vec<_>>();
		nodes
			.iter()
			.filter_map(|node_id| self.get_layer_width(node_id, network_path).map(|layer_width| (*node_id, layer_width)))
			.collect::<HashMap<NodeId, u32>>()
	}

	/// Loads the structure of layer nodes from a node graph.
	pub fn load_structure(&mut self) {
		self.document_metadata.structure = HashMap::from_iter([(LayerNodeIdentifier::ROOT_PARENT, NodeRelations::default())]);

		// Only load structure if there is a root node
		let Some(root_node) = self.get_root_node(&[]) else {
			return;
		};

		let Some(first_root_layer) = self.upstream_flow_back_from_nodes(vec![root_node.node_id], &[], FlowType::PrimaryFlow).find_map(|(_, node_id)| {
			if self.is_layer(&node_id, &[]) {
				Some(LayerNodeIdentifier::new(node_id, self))
			} else {
				None
			}
		}) else {
			return;
		};
		// Should refer to output node
		let mut awaiting_horizontal_flow = vec![(first_root_layer.to_node(), first_root_layer)];
		let mut awaiting_primary_flow = vec![];

		while let Some((horizontal_root_node_id, mut parent_layer_node)) = awaiting_horizontal_flow.pop() {
			let horizontal_flow_iter = self.upstream_flow_back_from_nodes(vec![horizontal_root_node_id], &[], FlowType::HorizontalFlow);
			let mut children = Vec::new();
			// Special handling for the root layer, since it should not be skipped
			if horizontal_root_node_id == first_root_layer.to_node() {
				for (_, current_node_id) in horizontal_flow_iter {
					if self.is_layer(&current_node_id, &[]) {
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, self);
						if !self.document_metadata.structure.contains_key(&current_layer_node) {
							if current_node_id == first_root_layer.to_node() {
								awaiting_primary_flow.push((current_node_id, LayerNodeIdentifier::ROOT_PARENT));
								children.push((LayerNodeIdentifier::ROOT_PARENT, current_layer_node));
							} else {
								awaiting_primary_flow.push((current_node_id, parent_layer_node));
								children.push((parent_layer_node, current_layer_node));
							}
							parent_layer_node = current_layer_node;
						}
					}
				}
			} else {
				// Skip the horizontal_root_node_id node
				for (_, current_node_id) in horizontal_flow_iter.skip(1) {
					if self.is_layer(&current_node_id, &[]) {
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, self);
						if !self.document_metadata.structure.contains_key(&current_layer_node) {
							awaiting_primary_flow.push((current_node_id, parent_layer_node));
							children.push((parent_layer_node, current_layer_node));
							parent_layer_node = current_layer_node;
						}
					}
				}
			}
			for (parent, child) in children {
				parent.push_child(self.document_metadata_mut(), child);
			}
			while let Some((primary_root_node_id, parent_layer_node)) = awaiting_primary_flow.pop() {
				let primary_flow_iter = self.upstream_flow_back_from_nodes(vec![primary_root_node_id], &[], FlowType::PrimaryFlow);
				// Skip the primary_root_node_id node
				let mut children = Vec::new();
				for (_, current_node_id) in primary_flow_iter.skip(1) {
					if self.is_layer(&current_node_id, &[]) {
						// Create a new layer for the top of each stack, and add it as a child to the previous parent
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, self);
						if !self.document_metadata.structure.contains_key(&current_layer_node) {
							children.push(current_layer_node);

							// The layer nodes for the horizontal flow is itself
							awaiting_horizontal_flow.push((current_node_id, current_layer_node));
						}
					}
				}
				for child in children {
					parent_layer_node.push_child(self.document_metadata_mut(), child);
				}
			}
		}

		let nodes: HashSet<NodeId> = self.network(&[]).unwrap().nodes.keys().cloned().collect::<HashSet<_>>();

		self.document_metadata.upstream_transforms.retain(|node, _| nodes.contains(node));
		self.document_metadata.vector_modify.retain(|node, _| nodes.contains(node));
		self.document_metadata.click_targets.retain(|layer, _| self.document_metadata.structure.contains_key(layer));
	}

	pub fn document_metadata_mut(&mut self) -> &mut DocumentMetadata {
		&mut self.document_metadata
	}
}

// Public mutable methods
impl NodeNetworkInterface {
	pub fn clear_transient_metadata(&mut self) {
		let mut stack = vec![&mut self.network_metadata];
		while let Some(network_metadata) = stack.pop() {
			network_metadata.transient_metadata = NodeNetworkTransientMetadata::default();
			network_metadata
				.persistent_metadata
				.node_metadata
				.values_mut()
				.for_each(|node_metadata| node_metadata.transient_metadata = DocumentNodeTransientMetadata::default());
			stack.extend(
				network_metadata
					.persistent_metadata
					.node_metadata
					.values_mut()
					.filter_map(|node_metadata| node_metadata.persistent_metadata.network_metadata.as_mut()),
			);
		}
	}
	pub fn set_transform(&mut self, transform: DAffine2, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network in set_transform");
			return;
		};
		network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport = transform;
	}

	/// Inserts a new export at insert index. If the insert index is -1 it is inserted at the end. The output_name is used by the encapsulating node.
	pub fn add_export(&mut self, default_value: TaggedValue, insert_index: isize, output_name: String, network_path: &[NodeId]) {
		// Set the parent node (if it exists) to be a non layer if it is no longer eligible to be a layer
		if let Some(parent_id) = network_path.last().cloned() {
			if !self.is_eligible_to_be_layer(&parent_id, network_path) {
				self.set_to_node_or_layer(&parent_id, network_path, false);
			}
		};

		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in add_export");
			return;
		};
		let input = NodeInput::Value {
			tagged_value: default_value,
			exposed: true,
		};

		if insert_index == -1 {
			network.exports.push(input);
		} else {
			network.exports.insert(insert_index as usize, input);
		}

		// There will not be an encapsulating node if the network is the document network
		if let Some(encapsulating_node_metadata) = self.get_encapsulating_node_metadata_mut(network_path) {
			if insert_index == -1 {
				encapsulating_node_metadata.persistent_metadata.output_names.push(output_name);
			} else {
				encapsulating_node_metadata.persistent_metadata.output_names.insert(insert_index as usize, output_name);
			}
		};

		let Some(network_metadata_mut) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in add_export");
			return;
		};

		// Update the export ports and outward wires for the current network
		network_metadata_mut.transient_metadata.export_ports.unload();
		network_metadata_mut.transient_metadata.outward_wires.unload();

		// Update the outward wires and bounding box for all nodes in the encapsulating network
		if let Some(encapsulating_network_metadata) = self.get_encapsulating_network_metadata_mut(network_path) {
			encapsulating_network_metadata.transient_metadata.outward_wires.unload();
			encapsulating_network_metadata.transient_metadata.all_nodes_bounding_box.unload();
		}

		// Update the click targets for the encapsulating node, if it exists. There is no encapsulating node if the network is the document network
		if let Some(encapsulating_node_metadata_mut) = self.get_encapsulating_node_metadata_mut(network_path) {
			encapsulating_node_metadata_mut.transient_metadata.click_targets.unload();
		};

		// If the export is inserted as the first input or second input, and the parent network is the document_network, then it may have affected the document metadata structure
		if network_path.len() == 1 && (insert_index == 0 || insert_index == 1) {
			self.load_structure();
		}
	}

	/// Inserts a new input at insert index. If the insert index is -1 it is inserted at the end. The output_name is used by the encapsulating node.
	pub fn add_input(&mut self, node_id: &NodeId, network_path: &[NodeId], default_value: TaggedValue, exposed: bool, insert_index: isize, input_name: String) {
		// Set the node to be a non layer if it is no longer eligible to be a layer
		if !self.is_eligible_to_be_layer(node_id, network_path) {
			self.set_to_node_or_layer(node_id, network_path, false);
		}

		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in insert_input");
			return;
		};
		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node in insert_input");
			return;
		};
		let input = NodeInput::Value { tagged_value: default_value, exposed };
		if insert_index == -1 {
			node.inputs.push(input);
		} else {
			node.inputs.insert(insert_index as usize, input);
		}

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in insert_input");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get node_metadata in insert_input");
			return;
		};
		if insert_index == -1 {
			node_metadata.persistent_metadata.input_names.push(input_name);
		} else {
			node_metadata.persistent_metadata.input_names.insert(insert_index as usize, input_name);
		}

		// Update the click targets for the node
		node_metadata.transient_metadata.click_targets.unload();

		// Update the transient network metadata bounding box for all nodes and outward wires
		network_metadata.transient_metadata.all_nodes_bounding_box.unload();
		network_metadata.transient_metadata.outward_wires.unload();

		// Update the internal network import ports and outwards connections (if has a network implementation)
		if let Some(internal_network) = &mut node_metadata.persistent_metadata.network_metadata {
			internal_network.transient_metadata.import_ports.unload();
			internal_network.transient_metadata.outward_wires.unload();
		}

		// If the input is inserted as the first input, then it may have affected the document metadata structure
		if network_path.len() == 0 && (insert_index == 0 || insert_index == 1) {
			self.load_structure();
		}
	}

	// TODO: Keep metadata in sync with the new implementation
	pub fn set_implementation(&mut self, node_id: &NodeId, network_path: &[NodeId], implementation: DocumentNodeImplementation) {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_implementation");
			return;
		};
		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node in set_implementation");
			return;
		};
		node.implementation = implementation;
	}

	pub fn replace_inputs(&mut self, node_id: &NodeId, inputs: Vec<NodeInput>, network_path: &[NodeId]) -> Vec<NodeInput> {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in replace_inputs");
			return Vec::new();
		};
		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node in replace_inputs");
			return Vec::new();
		};
		std::mem::replace(&mut node.inputs, inputs)
	}

	/// TODO: Ensure network metadata, positions, and other metadata is kept in sync
	/// Ensure layer is toggled to non layer if it is no longer eligibile
	pub fn set_input(&mut self, input_connector: InputConnector, input: NodeInput, network_path: &[NodeId]) {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_input");
			return;
		};
		match input_connector {
			InputConnector::Node { node_id, input_index } => todo!(),
			InputConnector::Export(export_index) => {
				let Some(export) = network.exports.get_mut(export_index) else {
					log::error!("Could not get export in set_input");
					return;
				};
				*export = input;
			}
		}
	}

	/// Ensure network metadata, positions, and other metadata is kept in sync
	pub fn disconnect_input(&mut self, input_connector: InputConnector, network_path: &[NodeId]) {
		let Some(network) = self.network(network_path) else {
			return;
		};

		let existing_input = match input_connector {
			InputConnector::Node { node_id, input_index } => network.nodes.get(&node_id).and_then(|node| node.inputs.get(input_index)),
			InputConnector::Export(input_index) => network.exports.get(input_index),
		};

		let Some(existing_input) = existing_input else {
			warn!("Could not find input for {input_connector:?} when disconnecting");
			return;
		};

		let tagged_value = TaggedValue::from_type(&self.get_input_type(&input_connector, network_path));

		let mut value_input = NodeInput::value(tagged_value, true);
		if let NodeInput::Value { exposed, .. } = &mut value_input {
			*exposed = existing_input.is_exposed();
		}
		if let InputConnector::Node { node_id, .. } = input_connector {
			self.set_input(input_connector, value_input, network_path);
		} else {
			// Since it is only possible to drag the solid line, if previewing then there must be a dashed connection, which becomes the new export
			if matches!(self.previewing(network_path), Previewing::Yes { .. }) {
				self.start_previewing_without_restore();
			}
			// If there is no preview, then disconnect
			else {
				self.set_input(input_connector, value_input, network_path);
			}
		}
	}

	pub fn create_wire(&mut self, _output_connector: OutputConnector, _input_connector: InputConnector, _network_path: &[NodeId]) {

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

	/// Used to insert a node template into the network.
	pub fn insert_node(&mut self, node_id: NodeId, network_path: &[NodeId], node_template: NodeTemplate) {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Network not found in insert_node");
			return;
		};
		network.nodes.insert(node_id.clone(), node_template.document_node);

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Network not found in insert_node");
			return;
		};
		let node_metadata = DocumentNodeMetadata {
			persistent_metadata: node_template.persistent_node_metadata,
			transient_metadata: DocumentNodeTransientMetadata::default(),
		};
		network_metadata.persistent_metadata.node_metadata.insert(node_id.clone(), node_metadata);
		// TODO: Update the bounding box around all nodes instead of unloading all data
		network_metadata.transient_metadata.all_nodes_bounding_box.unload();
	}

	/// Deletes all nodes in `node_ids` and any sole dependents in the horizontal chain if the node to delete is a layer node.
	/// The various side effects to external data (network metadata, selected nodes, rendering document) are added through responses
	/// TODO: Store network metadata, selected nodes, mutable reference to responses as fields in the interface?
	pub fn delete_nodes(&mut self, nodes_to_delete: Vec<NodeId>, reconnect: bool, network_path: &[NodeId]) {
		let Some(network) = self.network(network_path) else {
			return;
		};

		let outward_wires = self.collect_outward_wires(network_path);

		let mut delete_nodes = HashSet::new();
		for node_id in &nodes_to_delete {
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

			let _root_node = self.get_root_node(network_path);
			for (_, upstream_id) in self.upstream_flow_back_from_nodes(vec![*child_id], network_path, FlowType::UpstreamFlow) {
				// This does a downstream traversal starting from the current node, and ending at either a node in the `delete_nodes` set or the output.
				// If the traversal find as child node of a node in the `delete_nodes` set, then it is a sole dependent. If the output node is eventually reached, then it is not a sole dependent.
				let mut stack = vec![OutputConnector::node(upstream_id, 0)];
				let mut can_delete = true;

				while let Some(current_node) = stack.pop() {
					let current_node_id = current_node.node_id().expect("The current node in the delete stack cannot be the export");
					let Some(downstream_nodes) = outward_wires.get(&current_node) else { continue };
					for downstream_node in downstream_nodes {
						if let InputConnector::Node { node_id: downstream_id, .. } = downstream_node {
							let downstream_node_output = OutputConnector::node(*downstream_id, 0);
							if !delete_nodes.contains(&downstream_id) {
								stack.push(downstream_node_output);
							}
							// Continue traversing over the downstream sibling, if the current node is a sibling to a node that will be deleted
							else {
								for deleted_node_id in &nodes_to_delete {
									let Some(output_node) = network.nodes.get(&deleted_node_id) else { continue };
									let Some(input) = output_node.inputs.get(0) else { continue };

									if let NodeInput::Node { node_id, .. } = input {
										if *node_id == current_node_id {
											stack.push(OutputConnector::node(*deleted_node_id, 0));
										}
									}
								}
							}
						}
						// If the traversal reaches the export, then the current node is not a sole dependent
						else {
							can_delete = false;
						}
					}
				}
				if can_delete {
					delete_nodes.insert(upstream_id);
				}
			}
		}

		for delete_node_id in &delete_nodes {
			if !self.remove_references_from_network(*delete_node_id, reconnect, network_path) {
				log::error!("could not remove references from network");
				continue;
			}
			self.network_mut(&[]).unwrap().nodes.remove(delete_node_id);
			//node_graph.update_click_target(node_id, document_network, network_path.clone());
		}
	}

	/// Removes all references to the node with the given id from the network, and reconnects the input to the node below (or the next layer below if the node to be deleted is layer) if `reconnect` is true.
	pub fn remove_references_from_network(&mut self, deleting_node_id: NodeId, reconnect: bool, network_path: &[NodeId]) -> bool {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in remove_references_from_network");
			return false;
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

		//TODO: Handle exports (NOTE: // Do not reconnect export to import until (#1762) is solved)

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
		{
			let NodeInput::Node { node_id: upstream_node_id, .. } = input else { continue };
			if *upstream_node_id != deleting_node_id {
				continue;
			}

			// Do not reconnect to EditorApi network input in the document network.
			if network_path.len() == 0 && reconnect_to_input.as_ref().is_some_and(|reconnect| matches!(reconnect, NodeInput::Network { .. })) {
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
				let tagged_value = TaggedValue::from_type(&self.get_input_type(&InputConnector::node(node_id, input_index), network_path));
				let value_input = NodeInput::value(tagged_value, true);
				nodes_to_set_input.push((node_id, input_index, Some(value_input)));
			}
		}

		let Some(network) = self.network(network_path) else { return false };

		if let Previewing::Yes { root_node_to_restore } = self.previewing(network_path) {
			if let Some(root_node_to_restore) = root_node_to_restore {
				if root_node_to_restore.node_id == deleting_node_id {
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

	/// Start previewing with a restore node
	// pub fn start_previewing(&mut self, previous_node_id: NodeId, output_index: usize) {
	// 	let Some(network_metadata)
	// 	self.previewing = Previewing::Yes {
	// 		root_node_to_restore: Some(RootNode { id: previous_node_id, output_index }),
	// 	};
	// }

	pub fn start_previewing_without_restore(&mut self) {
		// Some logic will have to be performed to prevent the graph positions from being completely changed when the export changes to some previewed node
		// self.network.start_previewing_without_restore();
	}

	/// Sets the root node only if a node is being previewed
	// pub fn update_root_node(&mut self, node_id: NodeId, output_index: usize) {
	// 	if let Previewing::Yes { root_node_to_restore } = self.previewing {
	// 		// Only continue previewing if the new root node is not the same as the primary export. If it is the same, end the preview
	// 		if let Some(root_node_to_restore) = root_node_to_restore {
	// 			if root_node_to_restore.id != node_id {
	// 				self.start_previewing(node_id, output_index);
	// 			} else {
	// 				self.stop_preview();
	// 			}
	// 		} else {
	// 			self.stop_preview();
	// 		}
	// 	}
	// }

	/// Stops preview, does not reset export
	// pub fn stop_preview(&mut self) {
	// 	self.previewing = Previewing::No;
	// }

	pub fn set_display_name(&mut self, node_id: &NodeId, network_path: &[NodeId], display_name: String) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			return;
		};

		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		node_metadata.persistent_metadata.display_name = display_name.clone();

		// Keep the alias in sync with the `ToArtboard` name input
		if node_metadata.persistent_metadata.reference.as_ref().is_some_and(|reference| reference == "Artboard") {
			let Some(nested_network) = self.network_mut(network_path) else {
				return;
			};
			let Some(artboard_node) = nested_network.nodes.get_mut(node_id) else {
				return;
			};
			let DocumentNodeImplementation::Network(network) = &mut artboard_node.implementation else {
				return;
			};
			// Keep this in sync with the definition
			let Some(to_artboard) = network.nodes.get_mut(&NodeId(0)) else {
				return;
			};

			let label_index = 1;
			let label = if !display_name.is_empty() { display_name } else { "Artboard".to_string() };
			let label_input = NodeInput::value(TaggedValue::String(label), false);
			to_artboard.inputs[label_index] = label_input;
		}

		let is_layer = self.is_layer(node_id, network_path);

		//TODO: Recalculate transient metadata instead of unloading
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			return;
		};

		// If the node is a layer, then the width and click targets need to be recalculated
		if is_layer {
			if let NodeTypeTransientMetadata::Layer(layer_metadata) = &mut node_metadata.transient_metadata.node_type_metadata {
				layer_metadata.layer_width.unload();
			}
			node_metadata.transient_metadata.click_targets.unload();
		}
	}

	pub fn set_visibility(&mut self, node_id: &NodeId, network_path: &[NodeId], is_visible: bool) {
		let Some(network) = self.network_mut(network_path) else {
			return;
		};

		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		node.visible = is_visible;
	}

	pub fn set_locked(&mut self, node_id: &NodeId, network_path: &[NodeId], locked: bool) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			return;
		};

		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(&node_id) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		node_metadata.persistent_metadata.locked = locked;
	}

	pub fn set_to_node_or_layer(&mut self, node_id: &NodeId, network_path: &[NodeId], is_layer: bool) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in set_to_node_or_layer");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		// TODO: Ensure transient metadata, persistent metadata, and document metadata are correctly updated when switching between node and layer
		// node_metadata.persistent_metadata.node_type_metadata = if is_layer {
		// 	NodeTypePersistentMetadata::Layer
		// } else {
		// 	NodeTypePersistentMetadata::Node
		// };
		node_metadata.transient_metadata.click_targets.unload();
		if is_layer {
			node_metadata.transient_metadata.node_type_metadata = NodeTypeTransientMetadata::Layer(LayerTransientMetadata::default());
		} else {
			node_metadata.transient_metadata.node_type_metadata = NodeTypeTransientMetadata::Node;
		}
	}

	pub fn toggle_preview(&mut self, toggle_id: NodeId, network_path: &[NodeId]) {
		let Some(network) = self.network(network_path) else {
			return;
		};
		// If new_export is None then disconnect
		let mut new_export = None;
		let mut new_previewing_state = Previewing::No;
		if let Some(export) = network.exports.get(0) {
			// If there currently an export
			if let NodeInput::Node { node_id, output_index, .. } = export {
				let previous_export_id = *node_id;
				let previous_output_index = *output_index;

				// The export is clicked
				if *node_id == toggle_id {
					// If the current export is clicked and is being previewed end the preview and set either export back to root node or disconnect
					if let Previewing::Yes { root_node_to_restore } = self.previewing(network_path) {
						new_export = root_node_to_restore.map(|root_node| root_node.to_connector());
						new_previewing_state = Previewing::No;
					}
					// The export is clicked and there is no preview
					else {
						new_previewing_state = Previewing::Yes {
							root_node_to_restore: Some(RootNode {
								node_id: previous_export_id,
								output_index: previous_output_index,
							}),
						};
					}
				}
				// The export is not clicked
				else {
					new_export = Some(OutputConnector::node(toggle_id, 0));

					// There is currently a dashed line being drawn
					if let Previewing::Yes { root_node_to_restore } = self.previewing(network_path) {
						// There is also a solid line being drawn
						if let Some(root_node_to_restore) = root_node_to_restore {
							// If the node with the solid line is clicked, then start previewing that node without restore
							if root_node_to_restore.node_id == toggle_id {
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
						new_previewing_state = Previewing::Yes {
							root_node_to_restore: Some(RootNode {
								node_id: previous_export_id,
								output_index: previous_output_index,
							}),
						};
					}
				}
			}
			// The primary export is disconnected
			else {
				// Set node as export and cancel any preview
				new_export = Some(OutputConnector::node(toggle_id, 0));
				self.start_previewing_without_restore();
			}
		}
		match new_export {
			Some(new_export) => {
				self.create_wire(new_export, InputConnector::Export(0), network_path);
			}
			None => {
				self.disconnect_input(InputConnector::Export(0), network_path);
			}
		}
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			return;
		};
		network_metadata.persistent_metadata.previewing = new_previewing_state;
	}

	/// Sets the position of a node to an absolute position
	fn set_absolute_position(&mut self, node_id: &NodeId, network_path: &[NodeId], position: IVec2) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in shift_node");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if let NodeTypePersistentMetadata::Node(node_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			node_metadata.position = NodePosition::Absolute(position);
		} else if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			layer_metadata.position = LayerPosition::Absolute(position);
		}
	}

	/// Shifts a node by a certain offset without the auto layout system. If the node is a layer in a stack, the y_offset is shifted. If the node is a node in a chain, its position gets set to absolute.
	pub fn shift_node(&mut self, node_id: &NodeId, shift: IVec2, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in shift_node");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if let LayerPosition::Absolute(layer_position) = &mut layer_metadata.position {
				*layer_position += shift;
			} else if let LayerPosition::Stack(y_offset) = &mut layer_metadata.position {
				let shifted_y_offset = *y_offset as i32 + shift.y;
				// A layer can only be shifted to a positive y_offset
				*y_offset = shifted_y_offset.max(0) as u32;
			}
		} else if let NodeTypePersistentMetadata::Node(node_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if let NodePosition::Absolute(node_metadata) = &mut node_metadata.position {
				*node_metadata += shift;
			} else if let NodePosition::Chain = node_metadata.position {
				// TODO: Dont't break the chain when shifting a node left or right. Instead, shift the entire chain (?).
				// TODO: Instead of outward wires to the export being based on the export (which changes when previewing), it should be based on the root node.
				let position = self
					.get_position(node_id, &self.collect_outward_wires(network_path), network_path)
					.unwrap_or_else(|| {
						log::error!("Could not get position for node {node_id}");
						IVec2::new(0, 0)
					})
					.y + shift;
				self.set_absolute_position(node_id, network_path, position);
			}
		}
		//TODO: Update transient metadata based on the movement. Unloading it means it will be recalculated next time it is needed, which is a simple solution.
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in shift_node");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		node_metadata.transient_metadata.click_targets.unload();
	}

	/// Shifts all nodes upstream from a certain node by a certain offset, and rearranges the graph if necessary
	pub fn shift_upstream(&mut self, node_id: NodeId, shift: IVec2, shift_self: bool) {
		// TODO: node layout system and implementation

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
		// 		node.metadata().position += shift;
		// 		node_graph.update_click_target(node_id, document_network, network_path.clone());
		// 	}
		// }
	}

	/// Moves a node to the same position as another node, and shifts all upstream nodes
	pub fn move_node_to(&mut self, _node_id: &NodeId, _target_id: &NodeId) {}

	// Disconnect the layers primary output and the input to the last non layer node feeding into it through primary flow, reconnects, then moves the layer to the new layer and stack index
	pub fn move_layer_to_stack(&mut self, layer: LayerNodeIdentifier, parent: LayerNodeIdentifier, insert_index: usize, network_path: &[NodeId]) {
		// TODO: Run the auto layout system to make space for the new nodes
		let (post_node, pre_node) = ModifyInputsContext::get_post_node_with_index(self, parent, insert_index);
		let input = NodeInput::node(layer.to_node(), 0);
		self.set_input(post_node, input, network_path);
	}

	// Moves a node and all upstream children to the end of a layer chain
	pub fn move_node_to_chain(&mut self, _node_id: &NodeId, _parent: LayerNodeIdentifier) {
		// TODO: Run the auto layout system to make space for the new nodes
	}
}

#[derive(PartialEq)]
pub enum FlowType {
	/// Iterate over all upstream nodes from every input (the primary and all secondary).
	UpstreamFlow,
	/// Iterate over nodes connected to the primary input.
	PrimaryFlow,
	/// Iterate over the secondary input for layer nodes and primary input for non layer nodes.
	HorizontalFlow,
	/// Upstream flow starting from the secondary input of the layer. All node_ids must be layers.
	LayerChildrenUpstreamFlow,
}
/// Iterate over upstream nodes. The behavior changes based on the `flow_type` that's set.
/// - [`FlowType::UpstreamFlow`]: iterates over all upstream nodes from every input (the primary and all secondary).
/// - [`FlowType::PrimaryFlow`]: iterates along the horizontal inputs of nodes, so in the case of a node chain `a -> b -> c`, this would yield `c, b, a` if we started from `c`.
/// - [`FlowType::HorizontalFlow`]: iterates over the secondary input for layer nodes and primary input for non layer nodes.
/// - [`FlowType::LayerChildrenUpstreamFlow`]: iterates over all upstream nodes from the secondary input of the node.
struct FlowIter<'a> {
	stack: Vec<NodeId>,
	network: &'a NodeNetwork,
	network_metadata: &'a NodeNetworkMetadata,
	flow_type: FlowType,
}
impl<'a> Iterator for FlowIter<'a> {
	type Item = (&'a DocumentNode, NodeId);
	fn next(&mut self) -> Option<Self::Item> {
		loop {
			let node_id = self.stack.pop()?;

			// Special handling for iterating from ROOT_PARENT in load_structure`
			// TODO: Delete this
			if node_id == NodeId(u64::MAX) {
				panic!("ROOT_PARENT should not be iterated over in upstream_flow_back_from_nodes");
			}

			if let (Some(document_node), Some(node_metadata)) = (self.network.nodes.get(&node_id), self.network_metadata.persistent_metadata.node_metadata.get(&node_id)) {
				let skip = if self.flow_type == FlowType::HorizontalFlow && node_metadata.persistent_metadata.is_layer() {
					1
				} else {
					0
				};
				let take = if self.flow_type == FlowType::UpstreamFlow { usize::MAX } else { 1 };
				let inputs = document_node.inputs.iter().skip(skip).take(take);

				let node_ids = inputs.filter_map(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id) } else { None });

				self.stack.extend(node_ids);

				return Some((document_node, node_id));
			}
		}
	}
}

/// Represents an input connector with index based on the [`DocumentNode::inputs`] index, not the visible input index
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum InputConnector {
	Node { node_id: NodeId, input_index: usize },
	Export(usize),
}

impl Default for InputConnector {
	fn default() -> Self {
		InputConnector::Export(0)
	}
}

impl InputConnector {
	pub fn node(node_id: NodeId, input_index: usize) -> Self {
		InputConnector::Node { node_id, input_index }
	}
	pub fn input_index(&self) -> usize {
		match self {
			InputConnector::Node { input_index, .. } => *input_index,
			InputConnector::Export(input_index) => *input_index,
		}
	}
	pub fn node_id(&self) -> Option<NodeId> {
		match self {
			InputConnector::Node { node_id, .. } => Some(*node_id),
			_ => None,
		}
	}
}

/// Represents an output connector
/// TODO: Layer could also be a variant, since the output index is always one. Layer(NodeId)
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum OutputConnector {
	Node { node_id: NodeId, output_index: usize },
	Import(usize),
}

impl Default for OutputConnector {
	fn default() -> Self {
		OutputConnector::Import(0)
	}
}

impl OutputConnector {
	pub fn node(node_id: NodeId, output_index: usize) -> Self {
		OutputConnector::Node { node_id, output_index }
	}
	pub fn index(&self) -> usize {
		match self {
			OutputConnector::Node { output_index, .. } => *output_index,
			OutputConnector::Import(output_index) => *output_index,
		}
	}
	pub fn node_id(&self) -> Option<NodeId> {
		match self {
			OutputConnector::Node { node_id, .. } => Some(*node_id),
			_ => None,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Ports {
	input_ports: Vec<(usize, ClickTarget)>,
	output_ports: Vec<(usize, ClickTarget)>,
}

impl Ports {
	pub fn new() -> Ports {
		Ports {
			input_ports: Vec::new(),
			output_ports: Vec::new(),
		}
	}

	pub fn get_click_targets(&self) -> impl Iterator<Item = &ClickTarget> {
		self.input_ports
			.iter()
			.map(|(_, click_target)| click_target)
			.chain(self.output_ports.iter().map(|(_, click_target)| click_target))
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
		let center = node_top_left + DVec2::new(0., 24. + 24. * row_index as f64);
		self.insert_input_port_at_center(input_index, center);
	}
	fn insert_node_output(&mut self, output_index: usize, row_index: usize, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(5. * 24., 24. + 24. * row_index as f64);
		self.insert_output_port_at_center(output_index, center);
	}

	fn insert_layer_input(&mut self, input_index: usize, node_top_left: DVec2) {
		let center = if input_index == 0 {
			node_top_left + DVec2::new(2. * 24., 24. * 2. + 8.)
		} else {
			node_top_left + DVec2::new(0., 24. * 1.)
		};
		self.insert_input_port_at_center(input_index, center);
	}

	fn insert_layer_output(&mut self, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(2. * 24., -8.0);
		self.insert_output_port_at_center(0, center);
	}

	pub fn clicked_input_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.input_ports
			.iter()
			.find_map(|(port, click_target)| click_target.intersect_point(point, DAffine2::IDENTITY).then(|| *port))
	}

	pub fn clicked_output_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.output_ports
			.iter()
			.find_map(|(port, click_target)| click_target.intersect_point(point, DAffine2::IDENTITY).then(|| *port))
	}

	pub fn get_input_port_position(&self, index: usize) -> Option<DVec2> {
		self.input_ports
			.iter()
			.nth(index)
			.and_then(|(_, click_target)| click_target.subpath.bounding_box().map(|bounds| bounds[0] + DVec2::new(8., 8.)))
	}

	pub fn get_output_port_position(&self, index: usize) -> Option<DVec2> {
		self.output_ports
			.iter()
			.nth(index)
			.and_then(|(_, click_target)| click_target.subpath.bounding_box().map(|bounds| bounds[0] + DVec2::new(8., 8.)))
	}
}

#[derive(PartialEq, Debug, Clone, Copy, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct RootNode {
	pub node_id: NodeId,
	pub output_index: usize,
}

impl RootNode {
	pub fn to_connector(&self) -> OutputConnector {
		OutputConnector::Node {
			node_id: self.node_id,
			output_index: self.output_index,
		}
	}
}

#[derive(PartialEq, Debug, Clone, Copy, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum Previewing {
	/// If there is a node to restore the connection to the export for, then it is stored in the option.
	/// Otherwise, nothing gets restored and the primary export is disconnected.
	Yes { root_node_to_restore: Option<RootNode> },
	#[default]
	No,
}

/// All fields in NetworkMetadata should automatically be updated by using the network interface API. If a field is none then it should be calculated based on the network state.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkMetadata {
	pub persistent_metadata: NodeNetworkPersistentMetadata,
	#[serde(skip)]
	pub transient_metadata: NodeNetworkTransientMetadata,
}

impl PartialEq for NodeNetworkMetadata {
	fn eq(&self, other: &Self) -> bool {
		self.persistent_metadata == other.persistent_metadata
	}
}

impl NodeNetworkMetadata {
	pub const GRID_SIZE: u32 = 24;
	pub fn nested_metadata(&self, nested_path: &[NodeId]) -> Option<&Self> {
		let mut network_metadata = Some(self);

		for segment in nested_path {
			network_metadata = network_metadata
				.and_then(|network| network.persistent_metadata.node_metadata.get(segment))
				.and_then(|node| node.persistent_metadata.network_metadata.as_ref());
		}
		network_metadata
	}

	/// Get the mutable nested network given by the path of node ids
	pub fn nested_metadata_mut(&mut self, nested_path: &[NodeId]) -> Option<&mut Self> {
		let mut network_metadata = Some(self);

		for segment in nested_path {
			network_metadata = network_metadata
				.and_then(|network| network.persistent_metadata.node_metadata.get_mut(segment))
				.and_then(|node| node.persistent_metadata.network_metadata.as_mut());
		}
		network_metadata
	}
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkPersistentMetadata {
	/// Node metadata must exist for every document node in the network
	pub node_metadata: HashMap<NodeId, DocumentNodeMetadata>,
	/// Cached metadata for each node, which is calculated when adding a node to node_metadata
	/// Indicates whether the network is currently rendered with a particular node that is previewed, and if so, which connection should be restored when the preview ends.
	pub previewing: Previewing,
	// Stores the transform and navigation state for the network
	pub navigation_metadata: NavigationMetadata,
}

/// This is the same as Option, but more clear in the context of having cached metadata either being loaded or unloaded
#[derive(Debug, Default, Clone)]
pub enum TransientMetadata<T> {
	Loaded(T),
	#[default]
	Unloaded,
}

impl<T> TransientMetadata<T> {
	/// Set the current transient metadata to unloaded
	pub fn unload(&mut self) {
		*self = TransientMetadata::Unloaded;
	}

	pub fn is_loaded(&self) -> bool {
		matches!(self, TransientMetadata::Loaded(_))
	}
}

/// If some network calculation is too slow to compute for every usage, cache the data here
#[derive(Debug, Default, Clone)]
pub struct NodeNetworkTransientMetadata {
	pub selected_nodes: SelectedNodes,
	/// Cache for the bounding box around all nodes in node graph space.
	pub all_nodes_bounding_box: TransientMetadata<Subpath<PointId>>,
	/// Cache bounding box for all "groups of nodes", which will be used to prevent overlapping nodes
	// node_group_bounding_box: Vec<(Subpath<ManipulatorGroupId>, Vec<Nodes>)>,
	/// Cache for all outward wire connections - will most likely need to be added
	pub outward_wires: TransientMetadata<HashMap<OutputConnector, Vec<InputConnector>>>,
	/// TODO: Cache all wire paths instead of calculating in Graph.svelte
	// pub wire_paths: Vec<WirePath>
	/// All import connector click targets
	pub import_ports: TransientMetadata<Ports>,
	/// All export connector click targets
	pub export_ports: TransientMetadata<Ports>,
}

/// Utility function for providing a default boolean value to serde.
#[inline(always)]
fn return_true() -> bool {
	true
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DocumentNodeMetadata {
	pub persistent_metadata: DocumentNodePersistentMetadata,
	#[serde(skip)]
	pub transient_metadata: DocumentNodeTransientMetadata,
}

impl PartialEq for DocumentNodeMetadata {
	fn eq(&self, other: &Self) -> bool {
		self.persistent_metadata == other.persistent_metadata
	}
}

/// Persistent metadata for each node in the network, which must be included when creating, serializing, and deserializing saving a node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentNodePersistentMetadata {
	/// The name of the node definition, as originally set by [`DocumentNodeDefinition`], used to display in the UI and to display the appropriate properties if no display name is set.
	/// Used during serialization/deserialization to prevent storing implementation or inputs (and possible other fields) if they are the same as the definition.
	pub reference: Option<String>,
	/// A name chosen by the user for this instance of the node. Empty indicates no given name, in which case the reference name is displayed to the user in italics.
	#[serde(default)]
	pub display_name: String,
	/// TODO: Should input/output names always be the same length as the inputs/outputs of the DocumentNode?
	pub input_names: Vec<String>,
	pub output_names: Vec<String>,
	/// Indicates to the UI if a primary output should be drawn for this node.
	/// True for most nodes, but the Split Channels node is an example of a node that has multiple secondary outputs but no primary output.
	#[serde(default = "return_true")]
	pub has_primary_output: bool,
	/// Represents the lock icon for locking/unlocking the node in the graph UI. When locked, a node cannot be moved in the graph UI.
	#[serde(default)]
	pub locked: bool,
	/// Metadata that is specific to either nodes or layers, which are chosen states for displaying as a left-to-right node or bottom-to-top layer.
	/// All fields in NodeTypePersistentMetadata should automatically be updated by using the network interface API
	pub node_type_metadata: NodeTypePersistentMetadata,
	/// This should always be Some for nodes with a [`DocumentNodeImplementation::Network`], and none for [`DocumentNodeImplementation::ProtoNode`]
	pub network_metadata: Option<NodeNetworkMetadata>,
}

impl Default for DocumentNodePersistentMetadata {
	fn default() -> Self {
		DocumentNodePersistentMetadata {
			reference: None,
			display_name: String::new(),
			input_names: Vec::new(),
			output_names: Vec::new(),
			has_primary_output: true,
			locked: false,
			node_type_metadata: NodeTypePersistentMetadata::default(),
			network_metadata: None,
		}
	}
}

impl DocumentNodePersistentMetadata {
	pub fn is_layer(&self) -> bool {
		matches!(self.node_type_metadata, NodeTypePersistentMetadata::Layer(_))
	}
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NodeTypePersistentMetadata {
	Layer(LayerPersistentMetadata),
	Node(NodePersistentMetadata),
}

impl Default for NodeTypePersistentMetadata {
	fn default() -> Self {
		NodeTypePersistentMetadata::node(IVec2::ZERO)
	}
}

impl NodeTypePersistentMetadata {
	pub fn node(position: IVec2) -> NodeTypePersistentMetadata {
		NodeTypePersistentMetadata::Node(NodePersistentMetadata {
			position: NodePosition::Absolute(position),
		})
	}
	pub fn layer(position: IVec2) -> NodeTypePersistentMetadata {
		NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
			position: LayerPosition::Absolute(position),
		})
	}
}

/// All fields in LayerMetadata should automatically be updated by using the network interface API
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LayerPersistentMetadata {
	// TODO: Store click target for the preview button, which will appear when the node is a selected/(hovered?) layer node
	// preview_click_target: Option<ClickTarget>,
	/// Stores the position of a layer node, which can either be Absolute or Stack
	/// If performance is a concern then also cache the absolute position for each node
	pub position: LayerPosition,
}

/// A layer can either be position as Absolute or in a Stack
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LayerPosition {
	// Position of the node in grid spaces
	Absolute(IVec2),
	// A layer is in a Stack when it feeds into the secondary input of a layer input. The Y position stores the vertical distance between the layer and its parent.
	Stack(u32),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodePersistentMetadata {
	/// Stores the position of a non layer node, which can either be Absolute or Chain
	position: NodePosition,
}

/// A node can either be position as Absolute or in a Chain
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NodePosition {
	// Position of the node in grid spaces
	Absolute(IVec2),
	// In a chain the position is based on the number of nodes to the first layer node
	Chain,
}

/// Cached metadata that should be calculated when creating a node, and should be recalculated when modifying a node property that affects one of the cached fields.
#[derive(Debug, Default, Clone)]
pub struct DocumentNodeTransientMetadata {
	/// If performance is a concern then also cache the absolute position for each node
	// pub cached_node_position: DVec2,
	// The click targets are stored as a single struct since it is very rare for only one to be updated, and recomputing all click targets in one function is more efficient than storing them separately.
	pub click_targets: TransientMetadata<DocumentNodeClickTargets>,
	// Metadata that is specific to either nodes or layers, which are chosen states for displaying as a left-to-right node or bottom-to-top layer.
	pub node_type_metadata: NodeTypeTransientMetadata,
}

#[derive(Debug, Clone)]
pub struct DocumentNodeClickTargets {
	/// Ensure node_click_target is kept in sync when modifying a node property that changes its size. Currently this is alias, inputs, is_layer, and metadata
	pub node_click_target: ClickTarget,
	/// Stores all port click targets in node graph space.
	pub port_click_targets: Ports,
	// Click targets that are specific to either nodes or layers, which are chosen states for displaying as a left-to-right node or bottom-to-top layer.
	pub node_type_metadata: NodeTypeClickTargets,
}

#[derive(Debug, Default, Clone)]
pub enum NodeTypeTransientMetadata {
	Layer(LayerTransientMetadata),
	#[default]
	Node, // No transient data is stored exclusively for nodes
}

#[derive(Debug, Default, Clone)]
pub struct LayerTransientMetadata {
	// Stores the width in grid cell units for layer nodes from the left edge of the thumbnail (+12px padding since thumbnail ends between grid spaces) to the left end of the node
	/// This is necessary since calculating the layer width through web_sys is very slow
	pub layer_width: TransientMetadata<u32>,
	// Should not be a performance concern to calculate when needed with get_chain_width.
	// Stores the width in grid cell units for layer nodes from the left edge of the thumbnail to the end of the chain
	// chain_width: u32,
}

#[derive(Debug, Clone)]
pub enum NodeTypeClickTargets {
	Layer(LayerClickTargets),
	Node, // No transient click targets are stored exclusively for nodes
}

/// All fields in TransientLayerMetadata should automatically be updated by using the network interface API
#[derive(Debug, Clone)]
pub struct LayerClickTargets {
	/// Cache for all visibility buttons. Should be automatically updated when update_click_target is called
	pub visibility_click_target: ClickTarget,
	// TODO: Store click target for the preview button, which will appear when the node is a selected/(hovered?) layer node
	// preview_click_target: ClickTarget,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NavigationMetadata {
	/// The current pan, and zoom state of the viewport's view of the node graph.
	pub node_graph_ptz: PTZ,
	/// Transform from node graph space to viewport space.
	pub node_graph_to_viewport: DAffine2,
}

impl Default for NavigationMetadata {
	fn default() -> NavigationMetadata {
		//Default PTZ and transform
		NavigationMetadata {
			node_graph_ptz: PTZ::default(),
			node_graph_to_viewport: DAffine2::IDENTITY,
		}
	}
}

// PartialEq required by message handlers
/// All persistent editor and Graphene data for a node. Used to serialize and deserialize a node, pass it through the editor, and create definitions.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodeTemplate {
	pub document_node: DocumentNode,
	pub persistent_node_metadata: DocumentNodePersistentMetadata,
}
