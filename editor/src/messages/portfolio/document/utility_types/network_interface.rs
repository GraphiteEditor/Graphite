use super::document_metadata::{DocumentMetadata, LayerNodeIdentifier, NodeRelations};
use super::misc::PTZ;
use super::nodes::SelectedNodes;
use crate::consts::{EXPORTS_TO_RIGHT_EDGE_PIXEL_GAP, EXPORTS_TO_TOP_EDGE_PIXEL_GAP, GRID_SIZE, IMPORTS_TO_LEFT_EDGE_PIXEL_GAP, IMPORTS_TO_TOP_EDGE_PIXEL_GAP};
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{resolve_document_node_type, DocumentNodeDefinition};
use crate::messages::portfolio::document::node_graph::utility_types::{Direction, FrontendClickTargets, FrontendGraphDataType, FrontendGraphInput, FrontendGraphOutput};
use crate::messages::tool::common_functionality::graph_modification_utils;

use bezier_rs::Subpath;
use graph_craft::document::{value::TaggedValue, DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork, OldDocumentNodeImplementation, OldNodeNetwork};
use graph_craft::{concrete, Type};
use graphene_std::renderer::{ClickTarget, Quad};
use graphene_std::transform::Footprint;
use graphene_std::vector::{PointId, VectorData, VectorModificationType};
use interpreted_executor::{dynamic_executor::ResolvedDocumentNodeTypes, node_registry::NODE_REGISTRY};

use glam::{DAffine2, DVec2, IVec2};
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};

/// All network modifications should be done through this API, so the fields cannot be public. However, all fields within this struct can be public since it it not possible to have a public mutable reference.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkInterface {
	/// The node graph that generates this document's artwork. It recursively stores its sub-graphs, so this root graph is the whole snapshot of the document content.
	/// A public mutable reference should never be created. It should only be mutated through custom setters which perform the necessary side effects to keep network_metadata in sync
	network: NodeNetwork,
	/// Stores all editor information for a NodeNetwork. Should automatically kept in sync by the setter methods when changes to the document network are made.
	network_metadata: NodeNetworkMetadata,
	// TODO: Wrap in TransientMetadata Option
	/// Stores the document network's structural topology. Should automatically kept in sync by the setter methods when changes to the document network are made.
	#[serde(skip)]
	document_metadata: DocumentMetadata,
	/// All input/output types based on the compiled network.
	#[serde(skip)]
	pub resolved_types: ResolvedDocumentNodeTypes,
	/// Disallow aborting transactions whilst undoing to avoid #559.
	#[serde(skip)]
	transaction_status: TransactionStatus,
}

impl Clone for NodeNetworkInterface {
	fn clone(&self) -> Self {
		Self {
			network: self.network.clone(),
			network_metadata: self.network_metadata.clone(),
			document_metadata: Default::default(),
			resolved_types: Default::default(),
			transaction_status: TransactionStatus::Finished,
		}
	}
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

	pub fn node_metadata(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&DocumentNodeMetadata> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata");
			return None;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get(node_id) else {
			log::error!("Could not get nested node_metadata for node {node_id} in network {network_path:?}");
			return None;
		};
		Some(node_metadata)
	}

	pub fn document_metadata(&self) -> &DocumentMetadata {
		&self.document_metadata
	}

	pub fn transaction_status(&self) -> TransactionStatus {
		self.transaction_status
	}

	/// Get the selected nodes for the network at the network_path
	pub fn selected_nodes(&self, network_path: &[NodeId]) -> Option<SelectedNodes> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes");
			return None;
		};

		Some(
			network_metadata
				.persistent_metadata
				.selection_undo_history
				.back()
				.cloned()
				.unwrap_or_default()
				.filtered_selected_nodes(network_metadata.persistent_metadata.node_metadata.keys().cloned().collect()),
		)
	}

	/// Get the network which the encapsulating node of the currently viewed network is part of. Will always be None in the document network.
	pub fn encapsulating_network_metadata(&self, network_path: &[NodeId]) -> Option<&NodeNetworkMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		encapsulating_path.pop()?;
		let Some(parent_metadata) = self.network_metadata(&encapsulating_path) else {
			log::error!("Could not get parent network in encapsulating_node_metadata");
			return None;
		};
		Some(parent_metadata)
	}

	/// Get the node which encapsulates the currently viewed network. Will always be None in the document network.
	pub fn encapsulating_node(&self, network_path: &[NodeId]) -> Option<&DocumentNode> {
		let mut encapsulating_path = network_path.to_vec();
		let encapsulating_node_id = encapsulating_path.pop()?;
		let parent_network = self.network(&encapsulating_path)?;
		let Some(encapsulating_node) = parent_network.nodes.get(&encapsulating_node_id) else {
			log::error!("Could not get encapsulating node in encapsulating_node");
			return None;
		};
		Some(encapsulating_node)
	}

	/// Get the node metadata for the node which encapsulates the currently viewed network. Will always be None in the document network.
	pub fn encapsulating_node_metadata(&self, network_path: &[NodeId]) -> Option<&DocumentNodeMetadata> {
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

	/// Returns the first downstream layer(inclusive) from a node. If the node is a layer, it will return itself.
	pub fn downstream_layer(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<LayerNodeIdentifier> {
		let mut id = *node_id;
		while !self.is_layer(&id, network_path) {
			id = self.outward_wires(network_path)?.get(&OutputConnector::node(id, 0))?.first()?.node_id()?;
		}
		Some(LayerNodeIdentifier::new(id, self, network_path))
	}

	/// Returns all downstream layers (inclusive) from a node. If the node is a layer, it will return itself.
	pub fn downstream_layers(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Vec<NodeId> {
		let mut stack = vec![*node_id];
		let mut layers = Vec::new();
		while let Some(current_node) = stack.pop() {
			if self.is_layer(&current_node, network_path) {
				layers.push(current_node);
			} else {
				let Some(outward_wires) = self.outward_wires(network_path).and_then(|outward_wires| outward_wires.get(&OutputConnector::node(current_node, 0))) else {
					log::error!("Could not get outward wires in downstream_layer");
					return Vec::new();
				};
				stack.extend(outward_wires.iter().filter_map(|input_connector| input_connector.node_id()));
			}
		}
		layers
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
		// TODO: Use network.import_types.len()
		if let Some(encapsulating_node) = self.encapsulating_node(network_path) {
			encapsulating_node.inputs.len()
		} else {
			// There is one(?) import to the document network, but the imports are not displayed
			// I think this is zero now that the scope system has been added
			1
		}
	}

	fn number_of_displayed_imports(&self, network_path: &[NodeId]) -> usize {
		// TODO: Use network.import_types.len()
		if let Some(encapsulating_node) = self.encapsulating_node(network_path) {
			encapsulating_node.inputs.len()
		} else {
			// There is one(?) import to the document network, but the imports are not displayed
			0
		}
	}

	fn number_of_displayed_inputs(&self, node_id: &NodeId, network_path: &[NodeId]) -> usize {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in number_of_displayed_inputs");
			return 0;
		};
		let Some(node) = network.nodes.get(node_id) else {
			log::error!("Could not get node {node_id} in number_of_displayed_inputs");
			return 0;
		};
		node.inputs.iter().filter(|input| input.is_exposed_to_frontend(network_path.is_empty())).count()
	}

	pub fn number_of_inputs(&self, node_id: &NodeId, network_path: &[NodeId]) -> usize {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in number_of_inputs");
			return 0;
		};
		let Some(node) = network.nodes.get(node_id) else {
			log::error!("Could not get node {node_id} in number_of_inputs");
			return 0;
		};
		node.inputs.len()
	}

	pub fn number_of_outputs(&self, node_id: &NodeId, network_path: &[NodeId]) -> usize {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in number_of_outputs");
			return 0;
		};
		let Some(node) = network.nodes.get(node_id) else {
			log::error!("Could not get node {node_id} in number_of_outputs");
			return 0;
		};
		match &node.implementation {
			DocumentNodeImplementation::ProtoNode(_) => 1,
			DocumentNodeImplementation::Network(nested_network) => nested_network.exports.len(),
			DocumentNodeImplementation::Extract => 1,
		}
	}

	/// Creates a copy for each node by disconnecting nodes which are not connected to other copied nodes.
	/// Returns an iterator of all persistent metadata for a node and their ids
	pub fn copy_nodes<'a>(&'a mut self, new_ids: &'a HashMap<NodeId, NodeId>, network_path: &'a [NodeId]) -> impl Iterator<Item = (NodeId, NodeTemplate)> + 'a {
		let mut new_nodes = new_ids
			.iter()
			.filter_map(|(node_id, &new)| {
				self.create_node_template(node_id, network_path).and_then(|mut node_template| {
					let Some(outward_wires) = self.outward_wires(network_path) else {
						log::error!("Could not get outward wires in copy_nodes");
						return None;
					};
					// TODO: Get downstream connections from all outputs
					let Some(downstream_connections) = outward_wires.get(&OutputConnector::node(*node_id, 0)) else {
						log::error!("Could not get outward wires in copy_nodes");
						return None;
					};
					let has_selected_node_downstream = downstream_connections
						.iter()
						.any(|input_connector| input_connector.node_id().is_some_and(|upstream_id| new_ids.keys().any(|key| *key == upstream_id)));
					// If the copied node does not have a downstream connection to another copied node, then set the position to absolute
					if !has_selected_node_downstream {
						let Some(position) = self.position(node_id, network_path) else {
							log::error!("Could not get position in create_node_template");
							return None;
						};
						match &mut node_template.persistent_node_metadata.node_type_metadata {
							NodeTypePersistentMetadata::Layer(layer_metadata) => layer_metadata.position = LayerPosition::Absolute(position),
							NodeTypePersistentMetadata::Node(node_metadata) => node_metadata.position = NodePosition::Absolute(position),
						};
					}

					// If a chain node does not have a selected downstream layer, then set the position to absolute
					let downstream_layer = self.downstream_layer(node_id, network_path);
					if downstream_layer.map_or(true, |downstream_layer| new_ids.keys().all(|key| *key != downstream_layer.to_node())) {
						let Some(position) = self.position(node_id, network_path) else {
							log::error!("Could not get position in create_node_template");
							return None;
						};
						node_template.persistent_node_metadata.node_type_metadata = NodeTypePersistentMetadata::Node(NodePersistentMetadata {
							position: NodePosition::Absolute(position),
						});
					}

					// Shift all absolute nodes 2 to the right and 2 down
					// TODO: Remove 2x2 offset and replace with layout system to find space for new node
					match &mut node_template.persistent_node_metadata.node_type_metadata {
						NodeTypePersistentMetadata::Layer(layer_metadata) => {
							if let LayerPosition::Absolute(position) = &mut layer_metadata.position {
								*position += IVec2::new(2, 2)
							}
						}
						NodeTypePersistentMetadata::Node(node_metadata) => {
							if let NodePosition::Absolute(position) = &mut node_metadata.position {
								*position += IVec2::new(2, 2)
							}
						}
					}

					Some((new, *node_id, node_template))
				})
			})
			.collect::<Vec<_>>();

		for old_id in new_nodes.iter().map(|(_, old_id, _)| *old_id).collect::<Vec<_>>() {
			// Try set all selected nodes upstream of a layer to be chain nodes
			if self.is_layer(&old_id, network_path) {
				for valid_upstream_chain_node in self.valid_upstream_chain_nodes(&InputConnector::node(old_id, 1), network_path) {
					if let Some(node_template) = new_nodes.iter_mut().find_map(|(_, old_id, template)| (*old_id == valid_upstream_chain_node).then_some(template)) {
						match &mut node_template.persistent_node_metadata.node_type_metadata {
							NodeTypePersistentMetadata::Node(node_metadata) => node_metadata.position = NodePosition::Chain,
							NodeTypePersistentMetadata::Layer(_) => log::error!("Node cannot be a layer"),
						};
					}
				}
			}
		}
		new_nodes.into_iter().map(move |(new, node_id, node)| (new, self.map_ids(node, &node_id, new_ids, network_path)))
	}

	/// Create a node template from an existing node.
	pub fn create_node_template(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<NodeTemplate> {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in create_node_template");
			return None;
		};
		let Some(node) = network.nodes.get(node_id) else {
			log::error!("Could not get node {node_id} in create_node_template");
			return None;
		};
		let Some(node_metadata) = self.node_metadata(node_id, network_path).cloned() else {
			log::error!("Could not get node_metadata in create_node_template");
			return None;
		};

		Some(NodeTemplate {
			persistent_node_metadata: node_metadata.persistent_metadata,
			document_node: node.clone(),
		})
	}

	/// Converts all node id inputs to a new id based on a HashMap.
	///
	/// If the node is not in the hashmap then a default input is found based on the compiled network, using the node_id passed as a parameter
	pub fn map_ids(&mut self, mut node_template: NodeTemplate, node_id: &NodeId, new_ids: &HashMap<NodeId, NodeId>, network_path: &[NodeId]) -> NodeTemplate {
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
					let tagged_value = TaggedValue::from_type_or_none(&self.input_type(&InputConnector::node(*node_id, input_index), network_path).0);
					*input = NodeInput::value(tagged_value, true);
				}
			} else if let &mut NodeInput::Network { .. } = input {
				// Always disconnect network node input
				let tagged_value = TaggedValue::from_type_or_none(&self.input_type(&InputConnector::node(*node_id, input_index), network_path).0);
				*input = NodeInput::value(tagged_value, true);
			}
		}
		node_template
	}

	pub fn input_from_connector(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<&NodeInput> {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in input_from_connector");
			return None;
		};
		match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(node) = network.nodes.get(node_id) else {
					log::error!("Could not get node {node_id} in input_from_connector");
					return None;
				};
				node.inputs.get(*input_index)
			}
			InputConnector::Export(export_index) => network.exports.get(*export_index),
		}
	}

	/// Try and get the [`DocumentNodeDefinition`] for a node
	pub fn get_node_definition(&self, network_path: &[NodeId], node_id: NodeId) -> Option<&DocumentNodeDefinition> {
		let metadata = self.node_metadata(&node_id, network_path)?;
		resolve_document_node_type(metadata.persistent_metadata.reference.as_ref()?)
	}

	/// Try and get the [`Type`] for any [`InputConnector`] based on the `self.resolved_types`.
	fn node_type_from_compiled(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<(Type, TypeSource)> {
		let (node_id, input_index) = match *input_connector {
			InputConnector::Node { node_id, input_index } => (node_id, input_index),
			InputConnector::Export(export_index) => {
				let Some((encapsulating_node_id, encapsulating_node_id_path)) = network_path.split_last() else {
					// The outermost network export defaults to an ArtboardGroup.
					return Some((concrete!(graphene_core::ArtboardGroup), TypeSource::OuterMostExportDefault));
				};

				let output_type = self.output_types(encapsulating_node_id, encapsulating_node_id_path).into_iter().nth(export_index).flatten();
				if output_type.is_none() {
					warn!("Could not find output type for export node");
				}
				return output_type;
			}
		};
		let Some(current_network) = self.network(network_path) else {
			log::error!("Could not get current network in input_type");
			return None;
		};
		let Some(node) = current_network.nodes.get(&node_id) else {
			log::error!("Could not get node {node_id} in input_type");
			return None;
		};
		// If the input_connector is a NodeInput::Value, return the type of the tagged value.
		if let Some(value) = node.inputs.get(input_index).and_then(|input| input.as_value()) {
			return Some((value.ty(), TypeSource::TaggedValue));
		}
		let node_id_path = [network_path, &[node_id]].concat();
		match &node.implementation {
			DocumentNodeImplementation::Network(_nested_network) => {
				// Attempt to resolve where this import is within the nested network (it may be connected to the node or directly to an export)
				let outwards_wires = self.outward_wires(&node_id_path);
				let inputs_using_import = outwards_wires.and_then(|outwards_wires| outwards_wires.get(&OutputConnector::Import(input_index)));
				let first_input = inputs_using_import.and_then(|input| input.first()).copied();

				if inputs_using_import.is_some_and(|inputs| inputs.len() > 1) {
					warn!("Found multiple inputs using an import. Using the type of the first one.");
				}

				if let Some(input_connector) = first_input {
					self.node_type_from_compiled(&input_connector, &node_id_path)
				}
				// Nothing is connected to the import
				else {
					None
				}
			}
			DocumentNodeImplementation::ProtoNode(_) => {
				// If a node has manual composition, then offset the input index by 1 since the proto node also includes the type of the input passed through manual composition.
				let manual_composition_offset = if node.manual_composition.is_some() { 1 } else { 0 };
				self.resolved_types
					.types
					.get(node_id_path.as_slice())
					.and_then(|node_types| node_types.inputs.get(input_index + manual_composition_offset).cloned())
					.map(|node_types| (node_types, TypeSource::Compiled))
			}
			DocumentNodeImplementation::Extract => None,
		}
	}

	/// Guess the type from the node based on a document node default or a random protonode definition.
	fn guess_type_from_node(&mut self, network_path: &mut Vec<NodeId>, node_id: NodeId, input_index: usize) -> (Type, TypeSource) {
		// Try and get the default value from the document node definition
		if let Some(value) = self
			.get_node_definition(network_path, node_id)
			.and_then(|definition| definition.node_template.document_node.inputs.get(input_index))
			.and_then(|input| input.as_value())
		{
			return (value.ty(), TypeSource::DocumentNodeDefault);
		}

		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in input_type");
			return (concrete!(()), TypeSource::Error("could not get network"));
		};

		let Some(node) = network.nodes.get(&node_id) else {
			return (concrete!(()), TypeSource::Error("node id not in network"));
		};

		let node_id_path = [network_path.as_slice(), &[node_id]].concat();
		match &node.implementation {
			DocumentNodeImplementation::ProtoNode(protonode) => {
				let Some(node_types) = random_protonode_implementation(protonode) else {
					return (concrete!(()), TypeSource::Error("could not resolve protonode"));
				};

				let skip_footprint = if node.manual_composition.is_some() { 1 } else { 0 };

				let Some(input_type) = std::iter::once(node_types.call_argument.clone()).chain(node_types.inputs.clone()).nth(input_index + skip_footprint) else {
					log::error!("Could not get type");
					return (concrete!(()), TypeSource::Error("could not get the protonode's input"));
				};

				(input_type, TypeSource::RandomProtonodeImplementation)
			}
			DocumentNodeImplementation::Network(_network) => {
				// Attempt to resolve where this import is within the nested network
				let outwards_wires = self.outward_wires(&node_id_path);
				let inputs_using_import = outwards_wires.and_then(|outwards_wires| outwards_wires.get(&OutputConnector::Import(input_index)));
				let first_input = inputs_using_import.and_then(|input| input.first()).copied();

				if let Some(InputConnector::Node {
					node_id: child_id,
					input_index: child_input_index,
				}) = first_input
				{
					network_path.push(node_id);
					let result = self.guess_type_from_node(network_path, child_id, child_input_index);
					network_path.pop();
					return result;
				}

				// Input is disconnected
				(concrete!(()), TypeSource::Error("disconnected network input"))
			}
			_ => (concrete!(()), TypeSource::Error("implementation is not network or protonode")),
		}
	}

	/// Get the [`Type`] for any InputConnector
	pub fn input_type(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> (Type, TypeSource) {
		if let Some(result) = self.node_type_from_compiled(input_connector, network_path) {
			return result;
		}

		// Resolve types from proto nodes in node_registry
		let Some(node_id) = input_connector.node_id() else {
			return (concrete!(()), TypeSource::Error("input connector is not a node"));
		};

		// TODO: Once there is type inference (#1621), replace this workaround approach when disconnecting node inputs with NodeInput::Node(ToDefaultNode),
		// TODO: which would be a new node that implements the Default trait (i.e. `Default::default()`)
		self.guess_type_from_node(&mut network_path.to_vec(), node_id, input_connector.input_index())
	}

	/// Retrieves the output types for a given document node and its exports.
	///
	/// This function traverses the node and its nested network structure (if applicable) to determine
	/// the types of all outputs, including the primary output and any additional exports.
	///
	/// # Arguments
	///
	/// * `node` - A reference to the `DocumentNode` for which to determine output types.
	/// * `resolved_types` - A reference to `ResolvedDocumentNodeTypes` containing pre-resolved type information.
	/// * `node_id_path` - A slice of `NodeId`s representing the path to the current node in the document graph.
	///
	/// # Returns
	///
	/// A `Vec<Option<Type>>` where:
	/// - The first element is the primary output type of the node.
	/// - Subsequent elements are types of additional exports (if the node is a network).
	/// - `None` values indicate that a type couldn't be resolved for a particular output.
	///
	/// # Behavior
	///
	/// 1. Retrieves the primary output type from `resolved_types`.
	/// 2. If the node is a network:
	///    - Iterates through its exports (skipping the first/primary export).
	///    - For each export, traverses the network until reaching a protonode or terminal condition.
	///    - Determines the output type based on the final node/value encountered.
	/// 3. Collects and returns all resolved types.
	///
	/// # Note
	///
	/// This function assumes that export indices and node IDs always exist within their respective
	/// collections. It will panic if these assumptions are violated.
	pub fn output_types(&self, node_id: &NodeId, network_path: &[NodeId]) -> Vec<Option<(Type, TypeSource)>> {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in output_types");
			return Vec::new();
		};
		let Some(node) = network.nodes.get(node_id) else {
			log::error!("Could not get node {node_id} in output_types");
			return Vec::new();
		};

		let mut output_types = Vec::new();

		// If the node is not a protonode, get types by traversing across exports until a proto node is reached.
		match &node.implementation {
			graph_craft::document::DocumentNodeImplementation::Network(internal_network) => {
				for export in internal_network.exports.iter() {
					match export {
						NodeInput::Node {
							node_id: nested_node_id,
							output_index,
							..
						} => {
							let nested_output_types = self.output_types(nested_node_id, &[network_path, &[*node_id]].concat());
							let Some(nested_nodes_output_types) = nested_output_types.get(*output_index) else {
								log::error!("Could not get nested nodes output in output_types");
								return Vec::new();
							};
							output_types.push(nested_nodes_output_types.clone());
						}
						NodeInput::Value { tagged_value, .. } => {
							output_types.push(Some((tagged_value.ty(), TypeSource::TaggedValue)));
						}

						NodeInput::Network { .. } => {
							// https://github.com/GraphiteEditor/Graphite/issues/1762
							log::error!("Network input type cannot be connected to export");
							return Vec::new();
						}
						NodeInput::Scope(_) => todo!(),
						NodeInput::Inline(_) => todo!(),
						NodeInput::Reflection(_) => todo!(),
					}
				}
			}
			graph_craft::document::DocumentNodeImplementation::ProtoNode(protonode) => {
				let node_id_path = &[network_path, &[*node_id]].concat();
				let primary_output_type = self.resolved_types.types.get(node_id_path).map(|ty| (ty.output.clone(), TypeSource::Compiled)).or_else(|| {
					let node_types = random_protonode_implementation(protonode)?;
					Some((node_types.return_value.clone(), TypeSource::RandomProtonodeImplementation))
				});

				output_types.push(primary_output_type);
			}
			graph_craft::document::DocumentNodeImplementation::Extract => {
				output_types.push(Some((concrete!(()), TypeSource::Error("extract node"))));
			}
		}
		output_types
	}

	pub fn position(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<IVec2> {
		let top_left_position = self
			.node_click_targets(node_id, network_path)
			.and_then(|click_targets| click_targets.node_click_target.bounding_box())
			.map(|mut bounding_box| {
				if !self.is_layer(node_id, network_path) {
					bounding_box[0] -= DVec2::new(0., 12.);
				}
				(bounding_box[0] / 24.).as_ivec2()
			});
		top_left_position.map(|position| {
			if self.is_layer(node_id, network_path) {
				position + IVec2::new(self.chain_width(node_id, network_path) as i32, 0)
			} else {
				position
			}
		})
	}

	pub fn frontend_imports(&mut self, network_path: &[NodeId]) -> Option<Vec<(FrontendGraphOutput, i32, i32)>> {
		self.import_export_ports(network_path).cloned().map(|import_export_ports| {
			import_export_ports
				.output_ports
				.iter()
				.filter_map(|(import_index, click_target)| {
					// Get import name from parent node metadata input, which must match the number of imports.
					// Empty string means to use type, or "Import + index" if type can't be determined
					let import_name = self
						.encapsulating_node_metadata(network_path)
						.and_then(|encapsulating_metadata| encapsulating_metadata.persistent_metadata.input_names.get(*import_index).cloned())
						.unwrap_or_default();

					let mut import_metadata = None;

					if !network_path.is_empty() {
						let mut encapsulating_path = network_path.to_vec();
						let encapsulating_node_id = encapsulating_path.pop().unwrap();

						let (input_type, type_source) = self.input_type(&InputConnector::node(encapsulating_node_id, *import_index), &encapsulating_path);
						let data_type = FrontendGraphDataType::with_type(&input_type);

						let import_name = if import_name.is_empty() { input_type.clone().nested_type().to_string() } else { import_name };

						let connected_to = self
							.outward_wires(network_path)
							.and_then(|outward_wires| outward_wires.get(&OutputConnector::Import(*import_index)))
							.cloned()
							.unwrap_or_else(|| {
								log::error!("Could not get OutputConnector::Import({import_index}) in outward wires");
								Vec::new()
							});

						import_metadata = Some((
							FrontendGraphOutput {
								data_type,
								name: import_name,
								resolved_type: Some(format!("{input_type:?} from {type_source:?}")),
								connected_to,
							},
							click_target,
						));
					}
					import_metadata
				})
				.filter_map(|(import_index, output_port)| output_port.bounding_box().map(|bounding_box| (import_index, bounding_box[0].x as i32, bounding_box[0].y as i32)))
				.collect::<Vec<_>>()
		})
	}

	pub fn frontend_exports(&mut self, network_path: &[NodeId]) -> Option<Vec<(FrontendGraphInput, i32, i32)>> {
		self.import_export_ports(network_path).cloned().map(|import_export_ports| {
			import_export_ports
				.input_ports
				.iter()
				.filter_map(|(export_index, click_target)| {
					let Some(network) = self.network(network_path) else {
						log::error!("Could not get network in frontend_exports");
						return None;
					};

					let Some(export) = network.exports.get(*export_index) else {
						log::error!("Could not get export {export_index} in frontend_exports");
						return None;
					};

					let (frontend_data_type, input_type) = if let NodeInput::Node { node_id, output_index, .. } = export {
						let output_types = self.output_types(node_id, network_path);

						if let Some((output_type, type_source)) = output_types.get(*output_index).cloned().flatten() {
							(FrontendGraphDataType::with_type(&output_type), Some((output_type, type_source)))
						} else {
							(FrontendGraphDataType::General, None)
						}
					} else if let NodeInput::Value { tagged_value, .. } = export {
						(FrontendGraphDataType::with_type(&tagged_value.ty()), Some((tagged_value.ty(), TypeSource::TaggedValue)))
					// TODO: Get type from parent node input when <https://github.com/GraphiteEditor/Graphite/issues/1762> is possible
					// else if let NodeInput::Network { import_type, .. } = export {
					// 	(FrontendGraphDataType::with_type(import_type), Some(import_type.clone()))
					// }
					} else {
						(FrontendGraphDataType::General, None)
					};

					// First import index is visually connected to the root node instead of its actual export input so previewing does not change the connection
					let connected_to = if *export_index == 0 {
						self.root_node(network_path).map(|root_node| OutputConnector::node(root_node.node_id, root_node.output_index))
					} else if let NodeInput::Node { node_id, output_index, .. } = export {
						Some(OutputConnector::node(*node_id, *output_index))
					} else if let NodeInput::Network { import_index, .. } = export {
						Some(OutputConnector::Import(*import_index))
					} else {
						None
					};

					// Get export name from parent node metadata input, which must match the number of exports.
					// Empty string means to use type, or "Export + index" if type can't be determined
					let export_name = if network_path.is_empty() {
						"Canvas".to_string()
					} else {
						self.encapsulating_node_metadata(network_path)
							.and_then(|encapsulating_metadata| encapsulating_metadata.persistent_metadata.output_names.get(*export_index).cloned())
							.unwrap_or_default()
					};

					let export_name = if !export_name.is_empty() {
						export_name
					} else {
						input_type
							.clone()
							.map(|(input_type, _)| input_type.nested_type().to_string())
							.unwrap_or(format!("Export {}", export_index + 1))
					};

					Some((
						FrontendGraphInput {
							data_type: frontend_data_type,
							name: export_name,
							resolved_type: input_type.map(|(export_type, source)| format!("{export_type:?} from {source:?}")),
							connected_to,
						},
						click_target,
					))
				})
				.filter_map(|(export_metadata, output_port)| output_port.bounding_box().map(|bounding_box| (export_metadata, bounding_box[0].x as i32, bounding_box[0].y as i32)))
				.collect::<Vec<_>>()
		})
	}

	pub fn frontend_import_modify(&mut self, network_path: &[NodeId]) -> Option<(i32, i32)> {
		(!network_path.is_empty())
			.then(|| {
				self.modify_import_export(network_path).and_then(|modify_import_export_click_target| {
					modify_import_export_click_target
						.add_export
						.bounding_box()
						.map(|bounding_box| (bounding_box[0].x as i32, bounding_box[0].y as i32))
				})
			})
			.flatten()
	}

	pub fn frontend_export_modify(&mut self, network_path: &[NodeId]) -> Option<(i32, i32)> {
		(!network_path.is_empty())
			.then(|| {
				self.modify_import_export(network_path).and_then(|modify_import_export_click_target| {
					modify_import_export_click_target
						.add_import
						.bounding_box()
						.map(|bounding_box| (bounding_box[0].x as i32, bounding_box[0].y as i32))
				})
			})
			.flatten()
	}

	pub fn height_from_click_target(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<u32> {
		let mut node_height: Option<u32> = self
			.node_click_targets(node_id, network_path)
			.and_then(|click_targets| click_targets.node_click_target.bounding_box())
			.map(|bounding_box| ((bounding_box[1].y - bounding_box[0].y) / 24.) as u32);
		if !self.is_layer(node_id, network_path) {
			node_height = node_height.map(|height| height + 1);
		}
		node_height
	}

	// All chain nodes and branches from the chain which are sole dependents of the layer
	pub fn upstream_nodes_below_layer(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> HashSet<NodeId> {
		// Every upstream node below layer must be a sole dependent
		let mut upstream_nodes_below_layer = HashSet::new();

		let mut potential_upstream_nodes = HashSet::new();
		for chain_node in self
			.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow)
			.skip(1)
			.take_while(|node_id| self.is_chain(node_id, network_path))
			.collect::<Vec<_>>()
		{
			upstream_nodes_below_layer.insert(chain_node);
			let Some(chain_node) = self.network(network_path).and_then(|network| network.nodes.get(&chain_node)) else {
				log::error!("Could not get node {node_id} in upstream_nodes_below_layer");
				continue;
			};
			potential_upstream_nodes.extend(
				chain_node
					.inputs
					.iter()
					.filter(|input| input.is_exposed_to_frontend(network_path.is_empty()))
					.skip(1)
					.filter_map(|node_input| node_input.as_node()),
			)
		}

		// Get the node feeding into the left input of the chain
		let mut current_node_id = *node_id;
		loop {
			let Some(current_node) = self.network(network_path).and_then(|network| network.nodes.get(&current_node_id)) else {
				log::error!("Could not get node {node_id} in upstream_nodes_below_layer");
				break;
			};
			if let Some(primary_node_id) = current_node
				.inputs
				.iter()
				.filter(|input| input.is_exposed_to_frontend(network_path.is_empty()))
				.nth(if self.is_layer(&current_node_id, network_path) { 1 } else { 0 })
				.and_then(|left_input| left_input.as_node())
			{
				if self.is_chain(&primary_node_id, network_path) {
					current_node_id = primary_node_id;
				} else {
					potential_upstream_nodes.insert(primary_node_id);
					break;
				}
			} else {
				break;
			}
		}

		for potential_upstream_node in potential_upstream_nodes {
			// The upstream chain cannot be added if there is some node upstream from an input that is not a sole dependent
			let mut upstream_chain_can_be_added = true;
			// Collect a vec of nodes that are sole dependents while iterating
			let mut sole_dependents = HashSet::new();

			for upstream_node_from_input in self
				.upstream_flow_back_from_nodes(vec![potential_upstream_node], network_path, FlowType::UpstreamFlow)
				.collect::<Vec<_>>()
			{
				let number_of_outputs = self.number_of_outputs(&upstream_node_from_input, network_path);

				// A node is a sole dependent if all outputs are sole dependents, and there are no dead ends
				let mut all_outputs_are_sole_dependents = true;
				let mut dead_ends = 0;

				for output_index in 0..number_of_outputs {
					let downstream_connections = {
						let Some(outward_wires) = self.outward_wires(network_path) else {
							log::error!("Could not get outward wires in upstream_nodes_below_layer");
							continue;
						};
						outward_wires.get(&OutputConnector::node(upstream_node_from_input, output_index)).cloned()
					};
					let Some(downstream_connections) = downstream_connections else {
						log::error!("Could not get outward wires in upstream_nodes_below_layer");
						continue;
					};
					let mut current_output_is_sole_dependent = true;
					let mut stack = downstream_connections;
					while let Some(current_downstream_connection) = stack.pop() {
						// Iterate downstream. If a sole dependent or chain_node_id is reached, then stop the iteration. If the exports is eventually reached, then it is not a sole dependent
						match &current_downstream_connection {
							InputConnector::Node {
								node_id: downstream_node_id,
								input_index,
							} => {
								// Stop iterating once the downstream node is the left input to the chain or a sole dependent
								if !(sole_dependents.contains(downstream_node_id) || downstream_node_id == node_id && *input_index == 1) {
									// Continue iterating downstream for the downstream node
									let number_of_outputs = self.number_of_outputs(downstream_node_id, network_path);
									let Some(outward_wires) = self.outward_wires(network_path) else {
										log::error!("Could not get outward wires in upstream_nodes_below_layer");
										continue;
									};
									let mut has_downstream_connections = false;
									for output_index in 0..number_of_outputs {
										let Some(downstream_connections) = outward_wires.get(&OutputConnector::node(*downstream_node_id, output_index)) else {
											log::error!("Could not get outward wires in upstream_nodes_below_layer");
											continue;
										};
										if !downstream_connections.is_empty() {
											has_downstream_connections = true;
										}
										stack.extend(downstream_connections.clone());
									}
									if !has_downstream_connections {
										dead_ends += 1;
									}
								}
							}
							InputConnector::Export(_) => current_output_is_sole_dependent = false,
						}
					}
					if !current_output_is_sole_dependent || dead_ends != 0 {
						all_outputs_are_sole_dependents = false;
						break;
					}
				}
				if all_outputs_are_sole_dependents && dead_ends == 0 {
					sole_dependents.insert(upstream_node_from_input);
				} else {
					upstream_chain_can_be_added = false;
					break;
				}
			}

			if upstream_chain_can_be_added {
				upstream_nodes_below_layer.extend(sole_dependents)
			}
		}
		upstream_nodes_below_layer
	}

	pub fn previewing(&self, network_path: &[NodeId]) -> Previewing {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in previewing");
			return Previewing::No;
		};
		network_metadata.persistent_metadata.previewing
	}

	/// Returns the root node (the node that the solid line is connect to), or None if no nodes are connected to the output
	pub fn root_node(&self, network_path: &[NodeId]) -> Option<RootNode> {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in root_node");
			return None;
		};
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in root_node");
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

	pub fn reference(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<String> {
		self.node_metadata(node_id, network_path)
			.and_then(|node_metadata| node_metadata.persistent_metadata.reference.as_ref().map(|reference| reference.to_string()))
	}

	// None means that the type will be used
	pub fn input_name(&self, node_id: &NodeId, index: usize, network_path: &[NodeId]) -> Option<String> {
		self.node_metadata(node_id, network_path)
			.and_then(|node_metadata| node_metadata.persistent_metadata.input_names.get(index))
			.cloned()
			.filter(|s| !s.is_empty())
	}

	// Use frontend display name instead
	fn display_name(&self, node_id: &NodeId, network_path: &[NodeId]) -> String {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get node_metadata in display_name");
			return "".to_string();
		};
		node_metadata.persistent_metadata.display_name.clone()
	}

	pub fn frontend_display_name(&self, node_id: &NodeId, network_path: &[NodeId]) -> String {
		let is_layer = self
			.node_metadata(node_id, network_path)
			.expect("Could not get persistent node metadata in untitled_layer_label")
			.persistent_metadata
			.is_layer();
		let reference = self.reference(node_id, network_path);
		let is_merge_node = reference.as_ref().is_some_and(|reference| reference == "Merge");
		if self.display_name(node_id, network_path).is_empty() {
			if is_layer && is_merge_node {
				"Untitled Layer".to_string()
			} else {
				reference.unwrap_or("Untitled node".to_string())
			}
		} else {
			self.display_name(node_id, network_path)
		}
	}

	pub fn is_locked(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get persistent node metadata in is_locked for node {node_id}");
			return false;
		};
		node_metadata.persistent_metadata.locked
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
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in is_layer");
			return false;
		};
		node_metadata.persistent_metadata.is_layer()
	}

	pub fn has_primary_output(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get node_metadata in has_primary_output");
			return false;
		};
		node_metadata.persistent_metadata.has_primary_output
	}

	pub fn is_absolute(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get node_metadata in is_absolute");
			return false;
		};
		match &node_metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => matches!(layer_metadata.position, LayerPosition::Absolute(_)),
			NodeTypePersistentMetadata::Node(node_metadata) => matches!(node_metadata.position, NodePosition::Absolute(_)),
		}
	}

	pub fn is_chain(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get node_metadata in is_chain");
			return false;
		};
		match &node_metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Node(node_metadata) => matches!(node_metadata.position, NodePosition::Chain),
			_ => false,
		}
	}

	pub fn is_stack(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get node_metadata in is_stack");
			return false;
		};
		match &node_metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => matches!(layer_metadata.position, LayerPosition::Stack(_)),
			_ => false,
		}
	}

	pub fn is_artboard(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.reference(node_id, network_path)
			.as_ref()
			.is_some_and(|reference| reference == "Artboard" && self.connected_to_output(node_id, &[]))
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
					.is_some_and(|reference| reference == "Artboard" && self.connected_to_output(node_id, &[]) && self.is_layer(node_id, &[]))
				{
					Some(LayerNodeIdentifier::new(*node_id, self, &[]))
				} else {
					None
				}
			})
			.collect()
	}

	/// Folders sorted from most nested to least nested
	pub fn folders_sorted_by_most_nested(&self, network_path: &[NodeId]) -> Vec<LayerNodeIdentifier> {
		if !network_path.is_empty() {
			log::error!("Currently can only get deepest common ancestor in the document network");
			return Vec::new();
		}
		let Some(selected_nodes) = self.selected_nodes(network_path) else {
			log::error!("Could not get selected nodes in deepest_common_ancestor");
			return Vec::new();
		};
		let mut folders: Vec<_> = selected_nodes
			.selected_layers(self.document_metadata())
			.filter(|layer| layer.has_children(self.document_metadata()))
			.collect();
		folders.sort_by_cached_key(|a| std::cmp::Reverse(a.ancestors(self.document_metadata()).count()));
		folders
	}

	/// Calculates the document bounds in document space
	pub fn document_bounds_document_space(&self, include_artboards: bool) -> Option<[DVec2; 2]> {
		self.document_metadata
			.all_layers()
			.filter(|layer| include_artboards || !self.is_artboard(&layer.to_node(), &[]))
			.filter_map(|layer| {
				if !self.is_artboard(&layer.to_node(), &[]) {
					if let Some(artboard_node_identifier) = layer
						.ancestors(self.document_metadata())
						.find(|ancestor| *ancestor != LayerNodeIdentifier::ROOT_PARENT && self.is_artboard(&ancestor.to_node(), &[]))
					{
						let artboard = self.network(&[]).unwrap().nodes.get(&artboard_node_identifier.to_node());
						let clip_input = artboard.unwrap().inputs.get(5).unwrap();
						if let NodeInput::Value { tagged_value, .. } = clip_input {
							if tagged_value.to_primitive_string() == "true" {
								return Some(Quad::constraint_bounds(
									self.document_metadata.bounding_box_document(layer).unwrap_or_default(),
									self.document_metadata.bounding_box_document(artboard_node_identifier).unwrap_or_default(),
								));
							}
						}
					}
				}
				self.document_metadata.bounding_box_document(layer)
			})
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

	pub fn shallowest_unique_layers_sorted(&self, network_path: &[NodeId]) -> Vec<LayerNodeIdentifier> {
		let all_layers_to_group = self.shallowest_unique_layers(network_path).collect::<Vec<_>>();
		// Ensure nodes are grouped in the correct order
		let mut all_layers_to_group_sorted = Vec::new();
		for descendant in LayerNodeIdentifier::ROOT_PARENT.descendants(self.document_metadata()) {
			if all_layers_to_group.contains(&descendant) {
				all_layers_to_group_sorted.push(descendant);
			};
		}
		all_layers_to_group_sorted
	}

	/// Ancestor that is shared by all layers and that is deepest (more nested). Default may be the root. Skips selected non-folder, non-artboard layers
	pub fn deepest_common_ancestor(&self, network_path: &[NodeId], include_self: bool) -> Option<LayerNodeIdentifier> {
		if !network_path.is_empty() {
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

	/// Gives an iterator to all nodes connected to the given nodes by all inputs (primary or primary + secondary depending on `only_follow_primary` choice), traversing backwards upstream starting from the given node's inputs.
	pub fn upstream_flow_back_from_nodes<'a>(&'a self, mut node_ids: Vec<NodeId>, network_path: &'a [NodeId], mut flow_type: FlowType) -> impl Iterator<Item = NodeId> + 'a {
		let (Some(network), Some(network_metadata)) = (self.network(network_path), self.network_metadata(network_path)) else {
			log::error!("Could not get network or network_metadata in upstream_flow_back_from_nodes");
			return FlowIter {
				stack: Vec::new(),
				network: &self.network,
				network_metadata: &self.network_metadata,
				flow_type: FlowType::UpstreamFlow,
			};
		};
		if matches!(flow_type, FlowType::LayerChildrenUpstreamFlow) {
			node_ids = node_ids
				.iter()
				.filter_map(move |node_id| {
					if self.is_layer(node_id, network_path) {
						network.nodes.get(node_id).and_then(|node| node.inputs.get(1)).and_then(|input| input.as_node())
					} else {
						Some(*node_id)
					}
				})
				.collect::<Vec<_>>();
			flow_type = FlowType::UpstreamFlow;
		};
		FlowIter {
			stack: node_ids,
			network,
			network_metadata,
			flow_type,
		}
	}

	pub fn upstream_output_connector(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<OutputConnector> {
		let input = self.input_from_connector(input_connector, network_path);
		input.and_then(|input| match input {
			NodeInput::Node { node_id, output_index, .. } => Some(OutputConnector::node(*node_id, *output_index)),
			NodeInput::Network { import_index, .. } => Some(OutputConnector::Import(*import_index)),
			_ => None,
		})
	}

	/// In the network `X -> Y -> Z`, `is_node_upstream_of_another_by_primary_flow(Z, X)` returns true.
	pub fn is_node_upstream_of_another_by_horizontal_flow(&self, node: NodeId, network_path: &[NodeId], potentially_upstream_node: NodeId) -> bool {
		self.upstream_flow_back_from_nodes(vec![node], network_path, FlowType::HorizontalFlow)
			.any(|id| id == potentially_upstream_node)
	}

	#[cfg(not(target_arch = "wasm32"))]
	fn text_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<f64> {
		warn!("Failed to find width of {node_id:#?} in network_path {network_path:?} due to non-wasm arch");
		Some(0.)
	}

	#[cfg(target_arch = "wasm32")]
	fn text_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<f64> {
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

	pub fn from_old_network(old_network: OldNodeNetwork) -> Self {
		let mut node_network = NodeNetwork::default();
		let mut network_metadata = NodeNetworkMetadata::default();
		let mut stack = vec![(Vec::new(), old_network)];
		while let Some((network_path, old_network)) = stack.pop() {
			let Some(nested_network) = node_network.nested_network_mut(&network_path) else {
				log::error!("Could not get nested network in from_old_network");
				continue;
			};
			nested_network.exports = old_network.exports;
			nested_network.scope_injections = old_network.scope_injections.into_iter().collect();
			let Some(nested_network_metadata) = network_metadata.nested_metadata_mut(&network_path) else {
				log::error!("Could not get nested network in from_old_network");
				continue;
			};
			nested_network_metadata.persistent_metadata.previewing = Previewing::No;
			for (node_id, old_node) in old_network.nodes {
				let mut node = DocumentNode::default();
				let mut node_metadata = DocumentNodeMetadata::default();

				node.inputs = old_node.inputs;
				node.manual_composition = old_node.manual_composition;
				node.visible = old_node.visible;
				node.skip_deduplication = old_node.skip_deduplication;
				node.original_location = old_node.original_location;
				node_metadata.persistent_metadata.display_name = old_node.alias;
				node_metadata.persistent_metadata.reference = if old_node.name.is_empty() { None } else { Some(old_node.name) };
				node_metadata.persistent_metadata.has_primary_output = old_node.has_primary_output;
				node_metadata.persistent_metadata.locked = old_node.locked;
				node_metadata.persistent_metadata.node_type_metadata = if old_node.is_layer {
					NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
						position: LayerPosition::Absolute(old_node.metadata.position),
						owned_nodes: TransientMetadata::Unloaded,
					})
				} else {
					NodeTypePersistentMetadata::Node(NodePersistentMetadata {
						position: NodePosition::Absolute(old_node.metadata.position),
					})
				};

				match old_node.implementation {
					OldDocumentNodeImplementation::ProtoNode(protonode) => {
						node.implementation = DocumentNodeImplementation::ProtoNode(protonode);
					}
					OldDocumentNodeImplementation::Network(old_network) => {
						node.implementation = DocumentNodeImplementation::Network(NodeNetwork::default());
						node_metadata.persistent_metadata.network_metadata = Some(NodeNetworkMetadata::default());
						let mut nested_path = network_path.clone();
						nested_path.push(node_id);
						stack.push((nested_path, old_network));
					}
					OldDocumentNodeImplementation::Extract => {
						node.implementation = DocumentNodeImplementation::Extract;
					}
				}

				nested_network.nodes.insert(node_id, node);
				nested_network_metadata.persistent_metadata.node_metadata.insert(node_id, node_metadata);
			}
		}
		Self {
			network: node_network,
			network_metadata,
			document_metadata: DocumentMetadata::default(),
			resolved_types: ResolvedDocumentNodeTypes::default(),
			transaction_status: TransactionStatus::Finished,
		}
	}
}

/// Gets the type for a random protonode implementation (used if there is no type from the compiled network)
fn random_protonode_implementation(protonode: &graph_craft::ProtoNodeIdentifier) -> Option<&graphene_std::NodeIOTypes> {
	let mut protonode = protonode.clone();
	// TODO: Remove
	if let Some((path, _generics)) = protonode.name.split_once('<') {
		protonode = path.to_string().to_string().into();
	}
	let Some(node_io_hashmap) = NODE_REGISTRY.get(&protonode) else {
		log::error!("Could not get hashmap for proto node: {protonode:?}");
		return None;
	};

	let node_types = node_io_hashmap.keys().min_by_key(|node_io_types| {
		let mut hasher = DefaultHasher::new();
		node_io_types.hash(&mut hasher);
		hasher.finish()
	});

	if node_types.is_none() {
		log::error!("Could not get node_types from hashmap");
	};
	node_types
}

// Private mutable getters for use within the network interface
impl NodeNetworkInterface {
	fn network_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetwork> {
		self.network.nested_network_mut(network_path)
	}

	fn network_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkMetadata> {
		self.network_metadata.nested_metadata_mut(network_path)
	}

	fn node_metadata_mut(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&mut DocumentNodeMetadata> {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata");
			return None;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get nested node_metadata for node {node_id} in network {network_path:?}");
			return None;
		};
		Some(node_metadata)
	}

	/// Mutably get the network which the encapsulating node of the currently viewed network is part of. Will always be None in the document network.
	fn encapsulating_network_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		encapsulating_path.pop()?;
		let Some(parent_metadata) = self.network_metadata_mut(&encapsulating_path) else {
			log::error!("Could not get parent network in encapsulating_node_metadata");
			return None;
		};
		Some(parent_metadata)
	}

	/// Mutably get the node which encapsulates the currently viewed network. Will always be None in the document network.
	// fn encapsulating_node_mut(&mut self, network_path: &[NodeId]) -> Option<&mut DocumentNode> {
	// 	let mut encapsulating_path = network_path.to_vec();
	// 	let encapsulating_node_id = encapsulating_path.pop()?;
	// 	let Some(parent_network) = self.network_mut(&encapsulating_path) else {
	// 		log::error!("Could not get parent network in encapsulating_node_mut");
	// 		return None;
	// 	};
	// 	let Some(encapsulating_node) = parent_network.nodes.mut(&encapsulating_node_id) else {
	// 		log::error!("Could not get encapsulating node in encapsulating_node_mut");
	// 		return None;
	// 	};
	// 	Some(encapsulating_node)
	// }

	/// Get the node metadata for the node which encapsulates the currently viewed network. Will always be None in the document network.
	fn encapsulating_node_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut DocumentNodeMetadata> {
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
// Mutable methods never recalculate the transient metadata, they only unload it. Loading metadata should only be done by the getter.
impl NodeNetworkInterface {
	pub fn start_transaction(&mut self) {
		self.transaction_status = TransactionStatus::Started;
	}

	pub fn transaction_modified(&mut self) {
		if self.transaction_status == TransactionStatus::Started {
			self.transaction_status = TransactionStatus::Modified;
		}
	}

	pub fn finish_transaction(&mut self) {
		self.transaction_status = TransactionStatus::Finished;
	}

	/// Mutably get the selected nodes for the network at the network_path. Every time they are mutated, the transient metadata for the top of the stack gets unloaded.
	pub fn selected_nodes_mut(&mut self, network_path: &[NodeId]) -> Option<&mut SelectedNodes> {
		self.unload_stack_dependents(network_path);
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes");
			return None;
		};

		let last_selection_state = network_metadata.persistent_metadata.selection_undo_history.back().cloned().unwrap_or_default();

		network_metadata.persistent_metadata.selection_undo_history.push_back(last_selection_state);
		network_metadata.persistent_metadata.selection_redo_history.clear();

		if network_metadata.persistent_metadata.selection_undo_history.len() > crate::consts::MAX_UNDO_HISTORY_LEN {
			network_metadata.persistent_metadata.selection_undo_history.pop_front();
		}
		network_metadata.persistent_metadata.selection_undo_history.back_mut()
	}

	pub fn selection_step_back(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selection_step_back");
			return;
		};

		if let Some(selection_state) = network_metadata.persistent_metadata.selection_undo_history.pop_back() {
			network_metadata.persistent_metadata.selection_redo_history.push_front(selection_state);
		}
	}

	pub fn selection_step_forward(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in selection_step_forward");
			return;
		};

		if let Some(selection_state) = network_metadata.persistent_metadata.selection_redo_history.pop_front() {
			network_metadata.persistent_metadata.selection_undo_history.push_back(selection_state);
		}
	}

	fn stack_dependents(&mut self, network_path: &[NodeId]) -> Option<&HashMap<NodeId, LayerOwner>> {
		self.try_load_stack_dependents(network_path);
		self.try_get_stack_dependents(network_path)
	}

	fn try_load_stack_dependents(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in stack_dependents");
			return;
		};

		if !network_metadata.transient_metadata.stack_dependents.is_loaded() {
			self.load_stack_dependents(network_path);
		}
	}

	fn try_get_stack_dependents(&self, network_path: &[NodeId]) -> Option<&HashMap<NodeId, LayerOwner>> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in try_get_stack_dependents");
			return None;
		};
		let TransientMetadata::Loaded(stack_dependents) = &network_metadata.transient_metadata.stack_dependents else {
			log::error!("could not load stack_dependents");
			return None;
		};
		Some(stack_dependents)
	}

	// This function always has to be in sync with the selected nodes.
	fn load_stack_dependents(&mut self, network_path: &[NodeId]) {
		let Some(selected_nodes) = self.selected_nodes(network_path) else {
			log::error!("Could not get selected nodes in load_stack_dependents");
			return;
		};

		let mut selected_layers = selected_nodes.selected_nodes().filter(|node_id| self.is_layer(node_id, network_path)).cloned().collect::<HashSet<_>>();

		// Deselect all layers that are upstream of other selected layers
		let mut removed_layers = Vec::new();
		for layer in selected_layers.clone() {
			if removed_layers.contains(&layer) {
				continue;
			}
			for upstream_node in self.upstream_flow_back_from_nodes(vec![layer], network_path, FlowType::UpstreamFlow).skip(1) {
				if selected_layers.remove(&upstream_node) {
					removed_layers.push(upstream_node)
				}
			}
		}

		// Get a unique list of the top of each stack for each layer
		let mut stack_tops = HashSet::new();

		for layer in &selected_layers {
			let mut current_node = *layer;
			loop {
				if self.is_layer(&current_node, network_path) && self.is_absolute(&current_node, network_path) {
					stack_tops.insert(current_node);
					break;
				};
				let Some(outward_wires) = self.outward_wires(network_path) else {
					log::error!("Cannot load outward wires in load_stack_dependents");
					return;
				};
				let Some(layer_outward_wires) = outward_wires.get(&OutputConnector::node(current_node, 0)) else {
					log::error!("Could not get outward_wires for layer {current_node}");
					break;
				};
				match layer_outward_wires.first() {
					Some(downstream_input) => {
						let Some(downstream_node) = downstream_input.node_id() else {
							log::error!("Node connected to export should be absolute");
							break;
						};
						current_node = downstream_node
					}
					None => break,
				}
			}
		}

		let mut stack_dependents = HashMap::new();
		let mut owned_sole_dependents = HashSet::new();
		// Loop through all layers below the stack_tops, and set sole dependents upstream from that layer to be owned by that layer. Ensure LayerOwner is kept in sync.
		for stack_top in &stack_tops {
			for upstream_stack_layer in self
				.upstream_flow_back_from_nodes(vec![*stack_top], network_path, FlowType::PrimaryFlow)
				.take_while(|upstream_node| self.is_layer(upstream_node, network_path))
				.collect::<Vec<_>>()
			{
				for upstream_layer in self.upstream_flow_back_from_nodes(vec![upstream_stack_layer], network_path, FlowType::UpstreamFlow).collect::<Vec<_>>() {
					if !self.is_layer(&upstream_layer, network_path) {
						continue;
					}
					let mut new_owned_nodes = HashSet::new();
					for layer_sole_dependent in &self.upstream_nodes_below_layer(&upstream_layer, network_path) {
						stack_dependents.insert(*layer_sole_dependent, LayerOwner::Layer(upstream_layer));
						owned_sole_dependents.insert(*layer_sole_dependent);
						new_owned_nodes.insert(*layer_sole_dependent);
					}
					let Some(layer_node) = self.node_metadata_mut(&upstream_layer, network_path) else {
						log::error!("Could not get layer node in load_stack_dependents");
						continue;
					};
					let NodeTypePersistentMetadata::Layer(LayerPersistentMetadata { owned_nodes, .. }) = &mut layer_node.persistent_metadata.node_type_metadata else {
						log::error!("upstream layer should be a layer");
						return;
					};
					*owned_nodes = TransientMetadata::Loaded(new_owned_nodes);
				}
			}
		}

		// Set any sole dependents of the stack top that are not dependents of a layer in the stack to LayerOwner::None. These nodes will be pushed as blocks when a layer is shifted.
		for stack_top in &stack_tops {
			let mut sole_dependents = HashSet::new();
			let mut not_sole_dependents = HashSet::new();
			sole_dependents.insert(*stack_top);
			for upstream_node in self.upstream_flow_back_from_nodes(vec![*stack_top], network_path, FlowType::UpstreamFlow).collect::<Vec<_>>() {
				let mut stack = vec![upstream_node];
				let mut is_sole_dependent = true;
				while let Some(current_node) = stack.pop() {
					if not_sole_dependents.contains(&current_node) {
						is_sole_dependent = false;
						break;
					}
					if !sole_dependents.contains(&current_node) {
						let mut has_outward_wire = false;
						for output_index in 0..self.number_of_outputs(&current_node, network_path) {
							let Some(outward_wires) = self.outward_wires(network_path) else {
								log::error!("Cannot load outward wires in load_stack_dependents");
								continue;
							};
							let Some(outward_wires) = outward_wires.get(&OutputConnector::node(current_node, output_index)) else {
								log::error!("Cannot load outward wires in load_stack_dependents");
								continue;
							};
							for downstream_input in outward_wires {
								has_outward_wire = true;
								match downstream_input {
									InputConnector::Node { node_id, .. } => stack.push(*node_id),
									InputConnector::Export(_) => is_sole_dependent = false,
								}
							}
						}
						if !has_outward_wire {
							is_sole_dependent = false;
						}
					}
					if !is_sole_dependent {
						break;
					}
				}

				if is_sole_dependent {
					sole_dependents.insert(upstream_node);
				} else {
					not_sole_dependents.insert(upstream_node);
				}
			}

			for sole_dependent in sole_dependents {
				if !owned_sole_dependents.contains(&sole_dependent) {
					stack_dependents.insert(sole_dependent, LayerOwner::None(0));
				}
			}
		}

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return;
		};

		network_metadata.transient_metadata.stack_dependents = TransientMetadata::Loaded(stack_dependents);
	}

	pub fn unload_stack_dependents(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_stack_dependents");
			return;
		};
		network_metadata.transient_metadata.stack_dependents.unload();
	}

	/// Resets all the offsets for nodes with no LayerOwner when the drag ends
	pub fn unload_stack_dependents_y_offset(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_stack_dependents_y_offset");
			return;
		};

		if let TransientMetadata::Loaded(stack_dependents) = &mut network_metadata.transient_metadata.stack_dependents {
			for layer_owner in stack_dependents.values_mut() {
				if let LayerOwner::None(offset) = layer_owner {
					*offset = 0;
				}
			}
		}
	}

	pub fn import_export_ports(&mut self, network_path: &[NodeId]) -> Option<&Ports> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in export_ports");
			return None;
		};
		if !network_metadata.transient_metadata.import_export_ports.is_loaded() {
			self.load_import_export_ports(network_path);
		}
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in export_ports");
			return None;
		};
		let TransientMetadata::Loaded(ports) = &network_metadata.transient_metadata.import_export_ports else {
			log::error!("could not load import ports");
			return None;
		};
		Some(ports)
	}

	pub fn load_import_export_ports(&mut self, network_path: &[NodeId]) {
		//let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2();
		let Some(all_nodes_bounding_box) = self.all_nodes_bounding_box(network_path).cloned() else {
			log::error!("Could not get all nodes bounding box in load_export_ports");
			return;
		};
		let Some(rounded_network_edge_distance) = self.rounded_network_edge_distance(network_path).cloned() else {
			log::error!("Could not get rounded_network_edge_distance in load_export_ports");
			return;
		};
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in load_export_ports");
			return;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return;
		};

		let mut import_export_ports = Ports::new();

		let viewport_top_right = network_metadata
			.persistent_metadata
			.navigation_metadata
			.node_graph_to_viewport
			.inverse()
			.transform_point2(rounded_network_edge_distance.exports_to_edge_distance);
		let offset_from_top_right = if network
			.exports
			.first()
			.is_some_and(|export| export.as_node().is_some_and(|export_node| self.is_layer(&export_node, network_path)))
		{
			DVec2::new(2. * GRID_SIZE as f64, -2. * GRID_SIZE as f64)
		} else {
			DVec2::new(4. * GRID_SIZE as f64, 0.)
		};

		let bounding_box_top_right = DVec2::new((all_nodes_bounding_box[1].x / 24. + 0.5).floor() * 24., (all_nodes_bounding_box[0].y / 24. + 0.5).floor() * 24.) + offset_from_top_right;
		let export_top_right = DVec2::new(viewport_top_right.x.max(bounding_box_top_right.x), viewport_top_right.y.min(bounding_box_top_right.y));
		for input_index in 0..network.exports.len() {
			import_export_ports.insert_input_port_at_center(input_index, export_top_right + DVec2::new(0., input_index as f64 * 24.));
		}

		let viewport_top_left = network_metadata
			.persistent_metadata
			.navigation_metadata
			.node_graph_to_viewport
			.inverse()
			.transform_point2(rounded_network_edge_distance.imports_to_edge_distance);

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
		let import_top_left = DVec2::new(viewport_top_left.x.min(bounding_box_top_left.x), viewport_top_left.y.min(bounding_box_top_left.y));
		for output_index in 0..self.number_of_displayed_imports(network_path) {
			import_export_ports.insert_output_port_at_center(output_index, import_top_left + DVec2::new(0., output_index as f64 * 24.));
		}
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return;
		};

		network_metadata.transient_metadata.import_export_ports = TransientMetadata::Loaded(import_export_ports);
	}

	fn unload_import_export_ports(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_export_ports");
			return;
		};
		network_metadata.transient_metadata.import_export_ports.unload();
	}

	pub fn modify_import_export(&mut self, network_path: &[NodeId]) -> Option<&ModifyImportExportClickTarget> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in modify_import_export");
			return None;
		};
		if !network_metadata.transient_metadata.modify_import_export.is_loaded() {
			self.load_modify_import_export(network_path);
		}
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in modify_import_export");
			return None;
		};
		let TransientMetadata::Loaded(click_targets) = &network_metadata.transient_metadata.modify_import_export else {
			log::error!("could not load modify import export ports");
			return None;
		};
		Some(click_targets)
	}

	pub fn load_modify_import_export(&mut self, network_path: &[NodeId]) {
		let Some(all_nodes_bounding_box) = self.all_nodes_bounding_box(network_path).cloned() else {
			log::error!("Could not get all nodes bounding box in load_export_ports");
			return;
		};
		let Some(rounded_network_edge_distance) = self.rounded_network_edge_distance(network_path).cloned() else {
			log::error!("Could not get rounded_network_edge_distance in load_export_ports");
			return;
		};
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in load_export_ports");
			return;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return;
		};

		let viewport_top_right = network_metadata
			.persistent_metadata
			.navigation_metadata
			.node_graph_to_viewport
			.inverse()
			.transform_point2(rounded_network_edge_distance.exports_to_edge_distance);
		let offset_from_top_right = if network
			.exports
			.first()
			.is_some_and(|export| export.as_node().is_some_and(|export_node| self.is_layer(&export_node, network_path)))
		{
			DVec2::new(2. * GRID_SIZE as f64, -2. * GRID_SIZE as f64)
		} else {
			DVec2::new(4. * GRID_SIZE as f64, 0.)
		};

		let bounding_box_top_right = DVec2::new((all_nodes_bounding_box[1].x / 24. + 0.5).floor() * 24., (all_nodes_bounding_box[0].y / 24. + 0.5).floor() * 24.) + offset_from_top_right;
		let export_top_right = DVec2::new(viewport_top_right.x.max(bounding_box_top_right.x), viewport_top_right.y.min(bounding_box_top_right.y));
		let add_export_center = export_top_right + DVec2::new(0., network.exports.len() as f64 * 24.);
		let add_export = ClickTarget::new(Subpath::new_ellipse(add_export_center - DVec2::new(8., 8.), add_export_center + DVec2::new(8., 8.)), 0.);

		let viewport_top_left = network_metadata
			.persistent_metadata
			.navigation_metadata
			.node_graph_to_viewport
			.inverse()
			.transform_point2(rounded_network_edge_distance.imports_to_edge_distance);

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
		let import_top_left = DVec2::new(viewport_top_left.x.min(bounding_box_top_left.x), viewport_top_left.y.min(bounding_box_top_left.y));
		let add_import_center = import_top_left + DVec2::new(0., self.number_of_displayed_imports(network_path) as f64 * 24.);
		let add_import = ClickTarget::new(Subpath::new_ellipse(add_import_center - DVec2::new(8., 8.), add_import_center + DVec2::new(8., 8.)), 0.);

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get current network in load_modify_import_export");
			return;
		};

		network_metadata.transient_metadata.modify_import_export = TransientMetadata::Loaded(ModifyImportExportClickTarget {
			add_export,
			add_import,
			remove_imports: Vec::new(),
			remove_exports: Vec::new(),
			move_import: Vec::new(),
			move_export: Vec::new(),
		});
	}

	fn unload_modify_import_export(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_export_ports");
			return;
		};
		network_metadata.transient_metadata.modify_import_export.unload();
	}

	pub fn rounded_network_edge_distance(&mut self, network_path: &[NodeId]) -> Option<&NetworkEdgeDistance> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in rounded_network_edge_distance");
			return None;
		};
		if !network_metadata.transient_metadata.rounded_network_edge_distance.is_loaded() {
			self.load_rounded_network_edge_distance(network_path);
		}
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in rounded_network_edge_distance");
			return None;
		};
		let TransientMetadata::Loaded(rounded_network_edge_distance) = &network_metadata.transient_metadata.rounded_network_edge_distance else {
			log::error!("could not load import rounded_network_edge_distance");
			return None;
		};
		Some(rounded_network_edge_distance)
	}

	fn load_rounded_network_edge_distance(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network in set_grid_aligned_edges");
			return;
		};
		// When setting the edges to be grid aligned, update the pixel offset to ensure the next pan starts from the snapped import/export position
		let node_graph_to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
		// TODO: Eventually replace node graph top right with the footprint when trying to get the network edge distance
		let node_graph_top_right = network_metadata.persistent_metadata.navigation_metadata.node_graph_top_right;

		let target_exports_distance = node_graph_to_viewport.inverse().transform_point2(DVec2::new(
			node_graph_top_right.x - EXPORTS_TO_RIGHT_EDGE_PIXEL_GAP as f64,
			node_graph_top_right.y + EXPORTS_TO_TOP_EDGE_PIXEL_GAP as f64,
		));

		let target_imports_distance = node_graph_to_viewport
			.inverse()
			.transform_point2(DVec2::new(IMPORTS_TO_LEFT_EDGE_PIXEL_GAP as f64, IMPORTS_TO_TOP_EDGE_PIXEL_GAP as f64));

		let rounded_exports_distance = DVec2::new((target_exports_distance.x / 24. + 0.5).floor() * 24., (target_exports_distance.y / 24. + 0.5).floor() * 24.);
		let rounded_imports_distance = DVec2::new((target_imports_distance.x / 24. + 0.5).floor() * 24., (target_imports_distance.y / 24. + 0.5).floor() * 24.);

		let rounded_viewport_exports_distance = node_graph_to_viewport.transform_point2(rounded_exports_distance);
		let rounded_viewport_imports_distance = node_graph_to_viewport.transform_point2(rounded_imports_distance);

		let network_edge_distance = NetworkEdgeDistance {
			exports_to_edge_distance: rounded_viewport_exports_distance,
			imports_to_edge_distance: rounded_viewport_imports_distance,
		};
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get current network in load_export_ports");
			return;
		};
		network_metadata.transient_metadata.rounded_network_edge_distance = TransientMetadata::Loaded(network_edge_distance);
	}

	fn unload_rounded_network_edge_distance(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_export_ports");
			return;
		};
		network_metadata.transient_metadata.rounded_network_edge_distance.unload();
	}

	fn owned_nodes(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&HashSet<NodeId>> {
		let layer_node = self.node_metadata(node_id, network_path)?;
		let NodeTypePersistentMetadata::Layer(LayerPersistentMetadata { owned_nodes, .. }) = &layer_node.persistent_metadata.node_type_metadata else {
			return None;
		};
		let TransientMetadata::Loaded(owned_nodes) = owned_nodes else {
			return None;
		};
		Some(owned_nodes)
	}

	pub fn all_nodes_bounding_box(&mut self, network_path: &[NodeId]) -> Option<&[DVec2; 2]> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in all_nodes_bounding_box");
			return None;
		};

		if !network_metadata.transient_metadata.all_nodes_bounding_box.is_loaded() {
			self.load_all_nodes_bounding_box(network_path);
		}

		let network_metadata = self.network_metadata(network_path)?;

		let TransientMetadata::Loaded(bounding_box) = &network_metadata.transient_metadata.all_nodes_bounding_box else {
			log::error!("could not load all nodes bounding box");
			return None;
		};

		Some(bounding_box)
	}

	pub fn load_all_nodes_bounding_box(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in load_all_nodes_bounding_box");
			return;
		};
		let nodes = network_metadata.persistent_metadata.node_metadata.keys().copied().collect::<Vec<_>>();

		let all_nodes_bounding_box = nodes
			.iter()
			.filter_map(|node_id| {
				self.node_click_targets(node_id, network_path)
					.and_then(|transient_node_metadata| transient_node_metadata.node_click_target.bounding_box())
			})
			.reduce(Quad::combine_bounds)
			.unwrap_or([DVec2::new(0., 0.), DVec2::new(0., 0.)]);

		let Some(network_metadata) = self.network_metadata_mut(network_path) else { return };

		network_metadata.transient_metadata.all_nodes_bounding_box = TransientMetadata::Loaded(all_nodes_bounding_box);
	}

	pub fn unload_all_nodes_bounding_box(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_all_nodes_bounding_box");
			return;
		};
		network_metadata.transient_metadata.all_nodes_bounding_box.unload();
		network_metadata.transient_metadata.import_export_ports.unload();
	}

	pub fn outward_wires(&mut self, network_path: &[NodeId]) -> Option<&HashMap<OutputConnector, Vec<InputConnector>>> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in outward_wires");
			return None;
		};

		if !network_metadata.transient_metadata.outward_wires.is_loaded() {
			self.load_outward_wires(network_path);
		}

		let network_metadata = self.network_metadata(network_path)?;

		let TransientMetadata::Loaded(outward_wires) = &network_metadata.transient_metadata.outward_wires else {
			log::error!("could not load outward wires");
			return None;
		};

		Some(outward_wires)
	}

	fn load_outward_wires(&mut self, network_path: &[NodeId]) {
		let mut outward_wires = HashMap::new();
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in load_outward_wires");
			return;
		};
		// Initialize all output connectors for nodes
		for (node_id, _) in network.nodes.iter() {
			let number_of_outputs = self.number_of_outputs(node_id, network_path);
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
			for (input_index, input) in node.inputs.iter().enumerate() {
				if let NodeInput::Node { node_id, output_index, .. } = input {
					// If this errors then there is an input to a node that does not exist
					let outward_wires_entry = outward_wires.get_mut(&OutputConnector::node(*node_id, *output_index)).unwrap_or_else(|| {
						panic!(
							"Output connector {:?} should be initialized for each node output from a node",
							OutputConnector::node(*node_id, *output_index)
						)
					});
					outward_wires_entry.push(InputConnector::node(*current_node_id, input_index));
				} else if let NodeInput::Network { import_index, .. } = input {
					let outward_wires_entry = outward_wires
						.get_mut(&OutputConnector::Import(*import_index))
						.unwrap_or_else(|| panic!("Output connector {:?} should be initialized for each import from a node", OutputConnector::Import(*import_index)));
					outward_wires_entry.push(InputConnector::node(*current_node_id, input_index));
				}
			}
		}
		for (export_index, export) in network.exports.iter().enumerate() {
			if let NodeInput::Node { node_id, output_index, .. } = export {
				let outward_wires_entry = outward_wires.get_mut(&OutputConnector::node(*node_id, *output_index)).unwrap_or_else(|| {
					panic!(
						"Output connector {:?} should be initialized for each node input from exports",
						OutputConnector::node(*node_id, *output_index)
					)
				});
				outward_wires_entry.push(InputConnector::Export(export_index));
			} else if let NodeInput::Network { import_index, .. } = export {
				let outward_wires_entry = outward_wires
					.get_mut(&OutputConnector::Import(*import_index))
					.unwrap_or_else(|| panic!("Output connector {:?} should be initialized between imports and exports", OutputConnector::Import(*import_index)));
				outward_wires_entry.push(InputConnector::Export(export_index));
			}
		}

		let Some(network_metadata) = self.network_metadata_mut(network_path) else { return };

		network_metadata.transient_metadata.outward_wires = TransientMetadata::Loaded(outward_wires);
	}

	fn unload_outward_wires(&mut self, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in unload_outward_wires");
			return;
		};
		network_metadata.transient_metadata.outward_wires.unload();
	}

	pub fn layer_width(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<u32> {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in layer_width");
			return None;
		};
		if !node_metadata.persistent_metadata.is_layer() {
			log::error!("Cannot get layer width for non layer node {node_id} in network {network_path:?}");
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

		let node_metadata = self.node_metadata(node_id, network_path)?;
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

	pub fn load_layer_width(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let left_thumbnail_padding = GRID_SIZE as f64 / 2.;
		let thumbnail_width = 3. * GRID_SIZE as f64;
		let gap_width = 8.;
		let text_width = self.text_width(node_id, network_path).unwrap_or_else(|| {
			log::error!("Could not get text width for node {node_id}");
			0.
		});

		let grip_padding = 4.;
		let grip_width = 8.;
		let icon_overhang_width = GRID_SIZE as f64 / 2.;

		let layer_width_pixels = left_thumbnail_padding + thumbnail_width + gap_width + text_width + grip_padding + grip_width + icon_overhang_width;
		let layer_width = ((layer_width_pixels / 24.).ceil() as u32).max(8);

		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in load_layer_width");
			return;
		};

		// Ensure layer width is not loaded for a non layer node
		if node_metadata.persistent_metadata.is_layer() {
			if let NodeTypeTransientMetadata::Layer(layer_metadata) = &mut node_metadata.transient_metadata.node_type_metadata {
				layer_metadata.layer_width = TransientMetadata::Loaded(layer_width);
			} else {
				// Set the entire transient node type metadata to be a layer, in case it was previously a node
				node_metadata.transient_metadata.node_type_metadata = NodeTypeTransientMetadata::Layer(LayerTransientMetadata {
					layer_width: TransientMetadata::Loaded(layer_width),
				});
			}
		} else {
			log::warn!("Tried loading layer width for non layer node");
		}
	}

	/// Unloads layer width if the node is a layer
	pub fn try_unload_layer_width(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let is_layer = self.is_layer(node_id, network_path);

		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			return;
		};

		// If the node is a layer, then the width and click targets need to be recalculated
		if is_layer {
			if let NodeTypeTransientMetadata::Layer(layer_metadata) = &mut node_metadata.transient_metadata.node_type_metadata {
				layer_metadata.layer_width.unload();
			}
		}
	}

	pub fn node_click_targets(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&DocumentNodeClickTargets> {
		self.try_load_node_click_targets(node_id, network_path);
		self.try_get_node_click_targets(node_id, network_path)
	}

	fn try_load_node_click_targets(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in node_click_targets");
			return;
		};
		if !node_metadata.transient_metadata.click_targets.is_loaded() {
			self.load_node_click_targets(node_id, network_path)
		};
	}

	fn try_get_node_click_targets(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&DocumentNodeClickTargets> {
		let node_metadata = self.node_metadata(node_id, network_path)?;
		let TransientMetadata::Loaded(click_target) = &node_metadata.transient_metadata.click_targets else {
			log::error!("Could not load node type metadata when getting click targets");
			return None;
		};
		Some(click_target)
	}

	pub fn load_node_click_targets(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(node_position) = self.position_from_downstream_node(node_id, network_path) else {
			log::error!("Could not get node position in load_node_click_targets for node {node_id}");
			return;
		};
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in load_node_click_targets");
			return;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in load_node_click_targets");
			return;
		};
		let Some(document_node) = network.nodes.get(node_id) else {
			log::error!("Could not get document node in load_node_click_targets");
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

			let height = input_row_count.max(output_row_count).max(1) as u32 * crate::consts::GRID_SIZE;
			let width = 5 * crate::consts::GRID_SIZE;
			let node_click_target_top_left = node_top_left + DVec2::new(0., 12.);
			let node_click_target_bottom_right = node_click_target_top_left + DVec2::new(width as f64, height as f64);

			let radius = 3.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(node_click_target_top_left, node_click_target_bottom_right, [radius; 4]);
			let node_click_target = ClickTarget::new(subpath, 0.);

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

			let layer_width_cells = self.layer_width(node_id, network_path).unwrap_or_else(|| {
				log::error!("Could not get layer width in load_node_click_targets");
				0
			});
			let width = layer_width_cells * crate::consts::GRID_SIZE;
			let height = 2 * crate::consts::GRID_SIZE;

			// Update visibility button click target
			let visibility_offset = node_top_left + DVec2::new(width as f64, 24.);
			let subpath = Subpath::new_rounded_rect(DVec2::new(-12., -12.) + visibility_offset, DVec2::new(12., 12.) + visibility_offset, [3.; 4]);
			let visibility_click_target = ClickTarget::new(subpath, 0.);

			// Update grip button click target, which is positioned to the left of the left most icon
			let grip_offset_right_edge = node_top_left + DVec2::new(width as f64 - (GRID_SIZE as f64) / 2., 24.);
			let subpath = Subpath::new_rounded_rect(DVec2::new(-8., -12.) + grip_offset_right_edge, DVec2::new(0., 12.) + grip_offset_right_edge, [0.; 4]);
			let grip_click_target = ClickTarget::new(subpath, 0.);

			// Create layer click target, which is contains the layer and the chain background
			let chain_width_grid_spaces = self.chain_width(node_id, network_path);

			let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
			let chain_top_left = node_top_left - DVec2::new((chain_width_grid_spaces * crate::consts::GRID_SIZE) as f64, 0.);
			let radius = 10.;
			let subpath = bezier_rs::Subpath::new_rounded_rect(chain_top_left, node_bottom_right, [radius; 4]);
			let node_click_target = ClickTarget::new(subpath, 0.);

			DocumentNodeClickTargets {
				node_click_target,
				port_click_targets,
				node_type_metadata: NodeTypeClickTargets::Layer(LayerClickTargets {
					visibility_click_target,
					grip_click_target,
				}),
			}
		};

		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in load_node_click_targets");
			return;
		};
		node_metadata.transient_metadata.click_targets = TransientMetadata::Loaded(document_node_click_targets);
	}

	pub fn node_bounding_box(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		self.node_click_targets(node_id, network_path)
			.and_then(|transient_node_metadata| transient_node_metadata.node_click_target.bounding_box())
	}

	pub fn try_get_node_bounding_box(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		self.try_get_node_click_targets(node_id, network_path)
			.and_then(|transient_node_metadata| transient_node_metadata.node_click_target.bounding_box())
	}

	pub fn try_load_all_node_click_targets(&mut self, network_path: &[NodeId]) {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get network in load_all_node_click_targets");
			return;
		};
		for node_id in network.nodes.keys().cloned().collect::<Vec<_>>() {
			self.try_load_node_click_targets(&node_id, network_path);
		}
	}

	/// Get the top left position in node graph coordinates for a node by recursively iterating downstream through cached positions, which means the iteration can be broken once a known position is reached.
	pub fn position_from_downstream_node(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<IVec2> {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in position_from_downstream_node");
			return None;
		};
		match &node_metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => {
				match layer_metadata.position {
					LayerPosition::Absolute(position) => Some(position),
					LayerPosition::Stack(y_offset) => {
						let Some(downstream_node_connectors) = self
							.outward_wires(network_path)
							.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(*node_id, 0)))
							.cloned()
						else {
							log::error!("Could not get downstream node in position_from_downstream_node");
							return None;
						};
						let downstream_connector = downstream_node_connectors
							.iter()
							.find_map(|input_connector| input_connector.node_id().map(|node_id| (node_id, input_connector.input_index())));

						let Some((downstream_node_id, _)) = downstream_connector else {
							log::error!("Could not get downstream node input connector for node {node_id}");
							return None;
						};
						// Get the height of the node to ensure nodes do not overlap
						let Some(downstream_node_height) = self.height_from_click_target(&downstream_node_id, network_path) else {
							log::error!("Could not get click target height in position_from_downstream_node");
							return None;
						};
						self.position(&downstream_node_id, network_path)
							.map(|position| position + IVec2::new(0, 1 + downstream_node_height as i32 + y_offset as i32))
					}
				}
			}
			NodeTypePersistentMetadata::Node(node_metadata) => {
				match node_metadata.position {
					NodePosition::Absolute(position) => Some(position),
					NodePosition::Chain => {
						// Iterate through primary flow to find the first Layer
						let mut current_node_id = *node_id;
						let mut node_distance_from_layer = 1;
						loop {
							// TODO: Use root node to restore if previewing
							let Some(downstream_node_connectors) = self
								.outward_wires(network_path)
								.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(current_node_id, 0)))
								.cloned()
							else {
								log::error!("Could not get downstream node for node {node_id} with Position::Chain");
								return None;
							};
							let Some(downstream_node_id) = downstream_node_connectors.iter().find_map(|input_connector| {
								if let InputConnector::Node { node_id, input_index } = input_connector {
									let downstream_input_index = if self.is_layer(node_id, network_path) { 1 } else { 0 };
									if *input_index == downstream_input_index {
										Some(node_id)
									} else {
										None
									}
								} else {
									None
								}
							}) else {
								log::error!("Could not get downstream node input connector with input index 1 for node with Position::Chain");
								return None;
							};
							let Some(downstream_node_metadata) = self.network_metadata(network_path)?.persistent_metadata.node_metadata.get(downstream_node_id) else {
								log::error!("Downstream node metadata not found in node_metadata for node with Position::Chain");
								return None;
							};
							if downstream_node_metadata.persistent_metadata.is_layer() {
								// Get the position of the layer
								let layer_position = self.position(downstream_node_id, network_path)?;
								return Some(layer_position + IVec2::new(-node_distance_from_layer * 7, 0));
							}
							node_distance_from_layer += 1;
							current_node_id = *downstream_node_id;
						}
					}
				}
			}
		}
	}

	pub fn unload_node_click_targets(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in unload_node_click_target");
			return;
		};
		node_metadata.transient_metadata.click_targets.unload();
	}

	pub fn unload_upstream_node_click_targets(&mut self, node_ids: Vec<NodeId>, network_path: &[NodeId]) {
		let upstream_nodes = self.upstream_flow_back_from_nodes(node_ids, network_path, FlowType::UpstreamFlow).collect::<Vec<_>>();

		for upstream_id in &upstream_nodes {
			let Some(node_metadata) = self.node_metadata_mut(upstream_id, network_path) else {
				log::error!("Could not get node_metadata for node {upstream_id}");
				return;
			};
			node_metadata.transient_metadata.click_targets.unload();
		}
	}

	pub fn unload_all_nodes_click_targets(&mut self, network_path: &[NodeId]) {
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in unload_all_nodes_click_targets");
			return;
		};
		let upstream_nodes = network.nodes.keys().cloned().collect::<Vec<_>>();

		for upstream_id in &upstream_nodes {
			let Some(node_metadata) = self.node_metadata_mut(upstream_id, network_path) else {
				log::error!("Could not get node_metadata for node {upstream_id}");
				return;
			};
			node_metadata.transient_metadata.click_targets.unload();
		}
	}
}

// Helper functions for mutable getters
impl NodeNetworkInterface {
	pub fn upstream_chain_nodes(&mut self, network_path: &[NodeId]) -> Vec<NodeId> {
		let Some(selected_nodes) = self.selected_nodes(network_path) else {
			log::error!("Could not get selected nodes in upstream_chain_nodes");
			return Vec::new();
		};
		let mut all_selected_nodes = selected_nodes.selected_nodes().cloned().collect::<Vec<_>>();
		for selected_node_id in selected_nodes.selected_nodes() {
			if self.is_layer(selected_node_id, network_path) {
				let unique_upstream_chain = self
					.upstream_flow_back_from_nodes(vec![*selected_node_id], network_path, FlowType::HorizontalFlow)
					.skip(1)
					.take_while(|node_id| self.is_chain(node_id, network_path))
					.filter(|upstream_node| all_selected_nodes.iter().all(|new_selected_node| new_selected_node != upstream_node))
					.collect::<Vec<_>>();
				all_selected_nodes.extend(unique_upstream_chain);
			}
		}
		all_selected_nodes
	}

	pub fn collect_frontend_click_targets(&mut self, network_path: &[NodeId]) -> FrontendClickTargets {
		let mut all_node_click_targets = Vec::new();
		let mut port_click_targets = Vec::new();
		let mut icon_click_targets = Vec::new();
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in collect_frontend_click_targets");
			return FrontendClickTargets::default();
		};
		network_metadata.persistent_metadata.node_metadata.keys().copied().collect::<Vec<_>>().into_iter().for_each(|node_id| {
			if let (Some(import_export_click_targets), Some(node_click_targets)) = (self.import_export_ports(network_path).cloned(), self.node_click_targets(&node_id, network_path)) {
				let mut node_path = String::new();

				let _ = node_click_targets.node_click_target.subpath().subpath_to_svg(&mut node_path, DAffine2::IDENTITY);
				all_node_click_targets.push((node_id, node_path));
				for port in node_click_targets.port_click_targets.click_targets().chain(import_export_click_targets.click_targets()) {
					let mut port_path = String::new();
					let _ = port.subpath().subpath_to_svg(&mut port_path, DAffine2::IDENTITY);
					port_click_targets.push(port_path);
				}
				if let NodeTypeClickTargets::Layer(layer_metadata) = &node_click_targets.node_type_metadata {
					let mut port_path = String::new();
					let _ = layer_metadata.visibility_click_target.subpath().subpath_to_svg(&mut port_path, DAffine2::IDENTITY);
					icon_click_targets.push(port_path);
					let mut port_path = String::new();
					let _ = layer_metadata.grip_click_target.subpath().subpath_to_svg(&mut port_path, DAffine2::IDENTITY);
					icon_click_targets.push(port_path);
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

		let bounds = self.all_nodes_bounding_box(network_path).cloned().unwrap_or([DVec2::ZERO, DVec2::ZERO]);
		let rect = bezier_rs::Subpath::<PointId>::new_rect(bounds[0], bounds[1]);
		let mut all_nodes_bounding_box = String::new();
		let _ = rect.subpath_to_svg(&mut all_nodes_bounding_box, DAffine2::IDENTITY);

		let Some(rounded_network_edge_distance) = self.rounded_network_edge_distance(network_path).cloned() else {
			log::error!("Could not get rounded_network_edge_distance in collect_frontend_click_targets");
			return FrontendClickTargets::default();
		};
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in collect_frontend_click_targets");
			return FrontendClickTargets::default();
		};
		let import_exports_viewport_top_left = rounded_network_edge_distance.imports_to_edge_distance;
		let import_exports_viewport_bottom_right = rounded_network_edge_distance.exports_to_edge_distance;

		let node_graph_top_left = network_metadata
			.persistent_metadata
			.navigation_metadata
			.node_graph_to_viewport
			.inverse()
			.transform_point2(import_exports_viewport_top_left);
		let node_graph_bottom_right = network_metadata
			.persistent_metadata
			.navigation_metadata
			.node_graph_to_viewport
			.inverse()
			.transform_point2(import_exports_viewport_bottom_right);

		let import_exports_target = bezier_rs::Subpath::<PointId>::new_rect(node_graph_top_left, node_graph_bottom_right);
		let mut import_exports_bounding_box = String::new();
		let _ = import_exports_target.subpath_to_svg(&mut import_exports_bounding_box, DAffine2::IDENTITY);

		FrontendClickTargets {
			node_click_targets,
			layer_click_targets,
			port_click_targets,
			icon_click_targets,
			all_nodes_bounding_box,
			import_exports_bounding_box,
		}
	}

	pub fn set_document_to_viewport_transform(&mut self, transform: DAffine2) {
		self.document_metadata.document_to_viewport = transform;
	}

	pub fn is_eligible_to_be_layer(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		let input_count = self.number_of_displayed_inputs(node_id, network_path);
		let output_count = self.number_of_outputs(node_id, network_path);

		self.node_metadata(node_id, network_path)
			.is_some_and(|node_metadata| node_metadata.persistent_metadata.has_primary_output)
			&& output_count == 1
			&& (input_count <= 2)
	}

	pub fn node_graph_ptz(&self, network_path: &[NodeId]) -> Option<&PTZ> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in node_graph_ptz_mut");
			return None;
		};
		Some(&network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz)
	}

	pub fn node_graph_ptz_mut(&mut self, network_path: &[NodeId]) -> Option<&mut PTZ> {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in node_graph_ptz_mut");
			return None;
		};
		Some(&mut network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz)
	}

	// TODO: Optimize getting click target intersections from click by using a spacial data structure like a quadtree instead of linear search
	/// Click target getter methods
	pub fn node_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<NodeId> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in node_from_click");
			return None;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in node_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let nodes = network.nodes.keys().copied().collect::<Vec<_>>();
		let clicked_nodes = nodes
			.iter()
			.filter(|node_id| {
				self.node_click_targets(node_id, network_path)
					.is_some_and(|transient_node_metadata| transient_node_metadata.node_click_target.intersect_point_no_stroke(point))
			})
			.cloned()
			.collect::<Vec<_>>();
		// Since nodes are placed on top of layer chains, find the first non layer node that was clicked, and if there way no non layer nodes clicked, then find the first layer node that was clicked
		clicked_nodes
			.iter()
			.find_map(|node_id| {
				let Some(node_metadata) = self.network_metadata(network_path)?.persistent_metadata.node_metadata.get(node_id) else {
					log::error!("Could not get node_metadata for node {node_id}");
					return None;
				};
				if !node_metadata.persistent_metadata.is_layer() {
					Some(*node_id)
				} else {
					None
				}
			})
			.or_else(|| clicked_nodes.into_iter().next())
	}

	pub fn layer_click_target_from_click(&mut self, click: DVec2, click_target_type: LayerClickTargetTypes, network_path: &[NodeId]) -> Option<NodeId> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in visibility_from_click");
			return None;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in visibility_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let node_ids: Vec<_> = network.nodes.keys().copied().collect();

		node_ids
			.iter()
			.filter_map(|node_id| {
				self.node_click_targets(node_id, network_path).and_then(|transient_node_metadata| {
					if let NodeTypeClickTargets::Layer(layer) = &transient_node_metadata.node_type_metadata {
						match click_target_type {
							LayerClickTargetTypes::Visibility => layer.visibility_click_target.intersect_point_no_stroke(point).then_some(*node_id),
							LayerClickTargetTypes::Grip => layer.grip_click_target.intersect_point_no_stroke(point).then_some(*node_id),
						}
					} else {
						None
					}
				})
			})
			.next()
	}

	pub fn input_connector_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<InputConnector> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in input_connector_from_click");
			return None;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in input_connector_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		network
			.nodes
			.keys()
			.copied()
			.collect::<Vec<_>>()
			.iter()
			.filter_map(|node_id| {
				self.node_click_targets(node_id, network_path)
					.and_then(|transient_node_metadata| {
						transient_node_metadata
							.port_click_targets
							.clicked_input_port_from_point(point)
							.map(|port| InputConnector::node(*node_id, port))
					})
					.or_else(|| {
						self.import_export_ports(network_path)
							.and_then(|import_export_ports| import_export_ports.clicked_input_port_from_point(point).map(InputConnector::Export))
					})
			})
			.next()
	}

	pub fn output_connector_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<OutputConnector> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in output_connector_from_click");
			return None;
		};
		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in output_connector_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let nodes = network.nodes.keys().copied().collect::<Vec<_>>();
		nodes
			.iter()
			.filter_map(|node_id| {
				self.node_click_targets(node_id, network_path)
					.and_then(|transient_node_metadata| {
						transient_node_metadata
							.port_click_targets
							.clicked_output_port_from_point(point)
							.map(|output_index| OutputConnector::node(*node_id, output_index))
					})
					.or_else(|| {
						self.import_export_ports(network_path)
							.and_then(|import_export_ports| import_export_ports.clicked_output_port_from_point(point).map(OutputConnector::Import))
					})
			})
			.next()
	}

	pub fn input_position(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match input_connector {
			InputConnector::Node { node_id, input_index } => {
				// Get the displayed index from the input index
				let Some(network) = self.network(network_path) else {
					log::error!("Could not get network in input_position");
					return None;
				};
				let Some(node) = network.nodes.get(node_id) else {
					log::error!("Could not get node in input_position");
					return None;
				};
				let mut displayed_index = 0;
				for i in 0..*input_index {
					if node.inputs[i].is_exposed_to_frontend(network_path.is_empty()) {
						displayed_index += 1;
					}
				}
				self.node_click_targets(node_id, network_path)
					.and_then(|transient_node_metadata| transient_node_metadata.port_click_targets.input_port_position(displayed_index))
			}
			InputConnector::Export(export_index) => self
				.import_export_ports(network_path)
				.and_then(|import_export_ports| import_export_ports.input_port_position(*export_index)),
		}
	}

	pub fn output_position(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match output_connector {
			OutputConnector::Node { node_id, output_index } => self
				.node_click_targets(node_id, network_path)
				.and_then(|transient_node_metadata| transient_node_metadata.port_click_targets.output_port_position(*output_index)),
			OutputConnector::Import(import_index) => self
				.import_export_ports(network_path)
				.and_then(|import_export_ports| import_export_ports.output_port_position(*import_index)),
		}
	}

	/// Get the combined bounding box of the click targets of the selected nodes in the node graph in viewport space
	pub fn selected_nodes_bounding_box_viewport(&mut self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		// Always get the bounding box for nodes in the currently viewed network
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes_bounding_box_viewport");
			return None;
		};
		let node_graph_to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
		self.selected_nodes_bounding_box(network_path)
			.map(|[a, b]| [node_graph_to_viewport.transform_point2(a), node_graph_to_viewport.transform_point2(b)])
	}

	/// Get the combined bounding box of the click targets of the selected nodes in the node graph in layer space
	pub fn selected_nodes_bounding_box(&mut self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let Some(selected_nodes) = self.selected_nodes(network_path) else {
			log::error!("Could not get selected nodes in selected_nodes_bounding_box_viewport");
			return None;
		};
		selected_nodes
			.selected_nodes()
			.cloned()
			.collect::<Vec<_>>()
			.iter()
			.filter_map(|node_id| {
				self.node_click_targets(node_id, network_path)
					.and_then(|transient_node_metadata| transient_node_metadata.node_click_target.bounding_box())
			})
			.reduce(graphene_core::renderer::Quad::combine_bounds)
	}

	/// Gets the bounding box in viewport coordinates for each node in the node graph
	pub fn graph_bounds_viewport_space(&mut self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let bounds = *self.all_nodes_bounding_box(network_path)?;
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in graph_bounds_viewport_space");
			return None;
		};

		let bounding_box_subpath = bezier_rs::Subpath::<PointId>::new_rect(bounds[0], bounds[1]);
		bounding_box_subpath.bounding_box_with_transform(network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport)
	}

	pub fn collect_layer_widths(&mut self, network_path: &[NodeId]) -> (HashMap<NodeId, u32>, HashMap<NodeId, u32>, HashMap<NodeId, bool>) {
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

	pub fn compute_modified_vector(&self, layer: LayerNodeIdentifier) -> Option<VectorData> {
		let graph_layer = graph_modification_utils::NodeGraphLayer::new(layer, self);

		if let Some(vector_data) = graph_layer.upstream_node_id_from_name("Path").and_then(|node| self.document_metadata.vector_modify.get(&node)) {
			let mut modified = vector_data.clone();
			if let Some(TaggedValue::VectorModification(modification)) = graph_layer.find_input("Path", 1) {
				modification.apply(&mut modified);
			}
			return Some(modified);
		}

		self.document_metadata
			.click_targets
			.get(&layer)
			.map(|click| click.iter().map(ClickTarget::subpath))
			.map(|subpaths| VectorData::from_subpaths(subpaths, true))
	}

	/// Loads the structure of layer nodes from a node graph.
	pub fn load_structure(&mut self) {
		self.document_metadata.structure = HashMap::from_iter([(LayerNodeIdentifier::ROOT_PARENT, NodeRelations::default())]);

		// Only load structure if there is a root node
		let Some(root_node) = self.root_node(&[]) else {
			return;
		};

		let Some(first_root_layer) = self.upstream_flow_back_from_nodes(vec![root_node.node_id], &[], FlowType::PrimaryFlow).find_map(|node_id| {
			if self.is_layer(&node_id, &[]) {
				Some(LayerNodeIdentifier::new(node_id, self, &[]))
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
				for current_node_id in horizontal_flow_iter {
					if self.is_layer(&current_node_id, &[]) {
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, self, &[]);
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
				for current_node_id in horizontal_flow_iter.skip(1) {
					if self.is_layer(&current_node_id, &[]) {
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, self, &[]);
						if !self.document_metadata.structure.contains_key(&current_layer_node) {
							awaiting_primary_flow.push((current_node_id, parent_layer_node));
							children.push((parent_layer_node, current_layer_node));
							parent_layer_node = current_layer_node;
						}
					}
				}
			}

			for (parent, child) in children {
				parent.push_child(&mut self.document_metadata, child);
			}

			while let Some((primary_root_node_id, parent_layer_node)) = awaiting_primary_flow.pop() {
				let primary_flow_iter = self.upstream_flow_back_from_nodes(vec![primary_root_node_id], &[], FlowType::PrimaryFlow);
				// Skip the primary_root_node_id node
				let mut children = Vec::new();
				for current_node_id in primary_flow_iter.skip(1) {
					if self.is_layer(&current_node_id, &[]) {
						// Create a new layer for the top of each stack, and add it as a child to the previous parent
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, self, &[]);
						if !self.document_metadata.structure.contains_key(&current_layer_node) {
							children.push(current_layer_node);

							// The layer nodes for the horizontal flow is itself
							awaiting_horizontal_flow.push((current_node_id, current_layer_node));
						}
					}
				}
				for child in children {
					parent_layer_node.push_child(&mut self.document_metadata, child);
				}
			}
		}

		let nodes: HashSet<NodeId> = self.network(&[]).unwrap().nodes.keys().cloned().collect::<HashSet<_>>();

		self.document_metadata.upstream_transforms.retain(|node, _| nodes.contains(node));
		self.document_metadata.vector_modify.retain(|node, _| nodes.contains(node));
		self.document_metadata.click_targets.retain(|layer, _| self.document_metadata.structure.contains_key(layer));
	}

	/// Update the cached transforms of the layers
	pub fn update_transforms(&mut self, new_upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>) {
		self.document_metadata.upstream_transforms = new_upstream_transforms;
	}

	/// Update the cached click targets of the layers
	pub fn update_click_targets(&mut self, new_click_targets: HashMap<LayerNodeIdentifier, Vec<ClickTarget>>) {
		self.document_metadata.click_targets = new_click_targets;
	}

	/// Update the cached clip targets of the layers
	pub fn update_clip_targets(&mut self, new_clip_targets: HashSet<NodeId>) {
		self.document_metadata.clip_targets = new_clip_targets;
	}

	/// Update the vector modify of the layers
	pub fn update_vector_modify(&mut self, new_vector_modify: HashMap<NodeId, VectorData>) {
		self.document_metadata.vector_modify = new_vector_modify;
	}
}

// Public mutable methods
impl NodeNetworkInterface {
	pub fn copy_all_navigation_metadata(&mut self, other_interface: &NodeNetworkInterface) {
		let mut stack = vec![vec![]];
		while let Some(path) = stack.pop() {
			let Some(self_network_metadata) = self.network_metadata_mut(&path) else {
				continue;
			};
			if let Some(other_network_metadata) = other_interface.network_metadata(&path) {
				self_network_metadata.persistent_metadata.navigation_metadata = other_network_metadata.persistent_metadata.navigation_metadata.clone();
			}

			stack.extend(self_network_metadata.persistent_metadata.node_metadata.keys().map(|node_id| {
				let mut current_path = path.clone();
				current_path.push(*node_id);
				current_path
			}));
		}
	}

	pub fn set_transform(&mut self, transform: DAffine2, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network in set_transform");
			return;
		};
		network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport = transform;
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
	}

	// This should be run whenever the pan ends, a zoom occurs, or the network is opened
	pub fn set_grid_aligned_edges(&mut self, node_graph_top_right: DVec2, network_path: &[NodeId]) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in set_grid_aligned_edges");
			return;
		};
		network_metadata.persistent_metadata.navigation_metadata.node_graph_top_right = node_graph_top_right;
		self.unload_rounded_network_edge_distance(network_path);
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
	}

	pub fn vector_modify(&mut self, node_id: &NodeId, modification_type: VectorModificationType) {
		let Some(node) = self.network_mut(&[]).unwrap().nodes.get_mut(node_id) else {
			log::error!("Could not get node in vector_modification");
			return;
		};
		{
			let mut value = node.inputs.get_mut(1).and_then(|input| input.as_value_mut());
			let Some(TaggedValue::VectorModification(ref mut modification)) = value.as_deref_mut() else {
				panic!("Path node does not have modification input");
			};

			modification.modify(&modification_type);
		}
		self.transaction_modified();
	}

	/// Inserts a new export at insert index. If the insert index is -1 it is inserted at the end. The output_name is used by the encapsulating node.
	pub fn add_export(&mut self, default_value: TaggedValue, insert_index: isize, output_name: String, network_path: &[NodeId]) {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in add_export");
			return;
		};

		let input = NodeInput::value(default_value, true);
		if insert_index == -1 {
			network.exports.push(input);
		} else {
			network.exports.insert(insert_index as usize, input);
		}

		self.transaction_modified();

		let mut encapsulating_path = network_path.to_vec();
		// Set the parent node (if it exists) to be a non layer if it is no longer eligible to be a layer
		if let Some(parent_id) = encapsulating_path.pop() {
			if !self.is_eligible_to_be_layer(&parent_id, &encapsulating_path) && self.is_layer(&parent_id, &encapsulating_path) {
				self.set_to_node_or_layer(&parent_id, &encapsulating_path, false);
			}
		};

		// There will not be an encapsulating node if the network is the document network
		if let Some(encapsulating_node_metadata) = self.encapsulating_node_metadata_mut(network_path) {
			if insert_index == -1 {
				encapsulating_node_metadata.persistent_metadata.output_names.push(output_name);
			} else {
				encapsulating_node_metadata.persistent_metadata.output_names.insert(insert_index as usize, output_name);
			}
		};

		// Update the export ports and outward wires for the current network
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
		self.unload_outward_wires(network_path);

		// Update the outward wires and bounding box for all nodes in the encapsulating network
		if let Some(encapsulating_network_metadata) = self.encapsulating_network_metadata_mut(network_path) {
			encapsulating_network_metadata.transient_metadata.outward_wires.unload();
			encapsulating_network_metadata.transient_metadata.all_nodes_bounding_box.unload();
		}

		// Update the click targets for the encapsulating node, if it exists. There is no encapsulating node if the network is the document network
		if let Some(encapsulating_node_metadata_mut) = self.encapsulating_node_metadata_mut(network_path) {
			encapsulating_node_metadata_mut.transient_metadata.click_targets.unload();
		};

		// If the export is inserted as the first input or second input, and the parent network is the document_network, then it may have affected the document metadata structure
		if network_path.len() == 1 && (insert_index == 0 || insert_index == 1) {
			self.load_structure();
		}
	}

	/// Inserts a new input at insert index. If the insert index is -1 it is inserted at the end. The output_name is used by the encapsulating node.
	pub fn add_import(&mut self, default_value: TaggedValue, exposed: bool, insert_index: isize, input_name: String, network_path: &[NodeId]) {
		let mut encapsulating_network_path = network_path.to_vec();
		let Some(node_id) = encapsulating_network_path.pop() else {
			log::error!("Cannot add import for document network");
			return;
		};
		// Set the node to be a non layer if it is no longer eligible to be a layer
		if !self.is_eligible_to_be_layer(&node_id, &encapsulating_network_path) && self.is_layer(&node_id, &encapsulating_network_path) {
			self.set_to_node_or_layer(&node_id, &encapsulating_network_path, false);
		}

		let Some(network) = self.network_mut(&encapsulating_network_path) else {
			log::error!("Could not get nested network in insert_input");
			return;
		};
		let Some(node) = network.nodes.get_mut(&node_id) else {
			log::error!("Could not get node in insert_input");
			return;
		};

		let input = NodeInput::value(default_value, exposed);
		if insert_index == -1 {
			node.inputs.push(input);
		} else {
			node.inputs.insert(insert_index as usize, input);
		}

		self.transaction_modified();

		let Some(node_metadata) = self.node_metadata_mut(&node_id, &encapsulating_network_path) else {
			log::error!("Could not get node_metadata in insert_input");
			return;
		};
		if insert_index == -1 {
			node_metadata.persistent_metadata.input_names.push(input_name);
		} else {
			node_metadata.persistent_metadata.input_names.insert(insert_index as usize, input_name);
		}

		// Update the internal network import ports and outwards connections (if has a network implementation)
		if let Some(internal_network) = &mut node_metadata.persistent_metadata.network_metadata {
			internal_network.transient_metadata.import_export_ports.unload();
			internal_network.transient_metadata.outward_wires.unload();
		}

		// Update the click targets for the node
		self.unload_node_click_targets(&node_id, &encapsulating_network_path);

		// Update the transient network metadata bounding box for all nodes and outward wires
		self.unload_all_nodes_bounding_box(&encapsulating_network_path);

		// Unload the metadata for the nested network
		self.unload_outward_wires(network_path);
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);

		// If the input is inserted as the first input, then it may have affected the document metadata structure
		if encapsulating_network_path.is_empty() && (insert_index == 0 || insert_index == 1) {
			self.load_structure();
		}
	}

	/// Keep metadata in sync with the new implementation if this is used by anything other than the upgrade scripts
	pub fn replace_implementation(&mut self, node_id: &NodeId, network_path: &[NodeId], implementation: DocumentNodeImplementation) {
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

	// TODO: Eventually remove this (probably starting late 2024)
	/// Keep metadata in sync with the new implementation if this is used by anything other than the upgrade scripts
	pub fn replace_implementation_metadata(&mut self, node_id: &NodeId, network_path: &[NodeId], metadata: DocumentNodePersistentMetadata) {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get network metdata in set implementation");
			return;
		};
		let Some(node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(node_id) else {
			log::error!("Could not get persistent node metadata for node {node_id} in set implementation");
			return;
		};
		node_metadata.persistent_metadata.network_metadata = metadata.network_metadata;
	}

	/// Keep metadata in sync with the new implementation if this is used by anything other than the upgrade scripts
	pub fn set_manual_compostion(&mut self, node_id: &NodeId, network_path: &[NodeId], manual_composition: Option<Type>) {
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_implementation");
			return;
		};
		let Some(node) = network.nodes.get_mut(node_id) else {
			log::error!("Could not get node in set_implementation");
			return;
		};
		node.manual_composition = manual_composition;
	}

	/// Keep metadata in sync with the new implementation if this is used by anything other than the upgrade scripts
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

	pub fn set_input(&mut self, input_connector: &InputConnector, new_input: NodeInput, network_path: &[NodeId]) {
		if matches!(input_connector, InputConnector::Export(_)) && matches!(new_input, NodeInput::Network { .. }) {
			// TODO: Add support for flattening NodeInput::Network exports in flatten_with_fns https://github.com/GraphiteEditor/Graphite/issues/1762
			log::error!("Cannot connect a network to an export, see https://github.com/GraphiteEditor/Graphite/issues/1762");
			return;
		}

		let Some(previous_input) = self.input_from_connector(input_connector, network_path).cloned() else {
			log::error!("Could not get previous input in set_input");
			return;
		};

		// When changing a NodeInput::Node to a NodeInput::Node, the input should first be disconnected to ensure proper side effects
		if (matches!(previous_input, NodeInput::Node { .. }) && matches!(new_input, NodeInput::Node { .. })) {
			self.disconnect_input(input_connector, network_path);
			self.set_input(input_connector, new_input, network_path);
			return;
		}

		// If the previous input is connected to a chain node, then set all upstream chain nodes to absolute position
		if let NodeInput::Node { node_id: previous_upstream_id, .. } = &previous_input {
			if self.is_chain(previous_upstream_id, network_path) {
				self.set_upstream_chain_to_absolute(previous_upstream_id, network_path);
			}
		}
		if let NodeInput::Node { node_id: new_upstream_id, .. } = &new_input {
			// If the new input is connected to a chain node, then break its chain
			if self.is_chain(new_upstream_id, network_path) {
				self.set_upstream_chain_to_absolute(new_upstream_id, network_path);
			}
		}

		let previous_metadata = if let NodeInput::Node { node_id, .. } = &previous_input {
			self.position(node_id, network_path).map(|position| (*node_id, position))
		} else {
			None
		};

		let Some(network) = self.network_mut(network_path) else {
			log::error!("Could not get nested network in set_input");
			return;
		};

		let old_input = match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(node) = network.nodes.get_mut(node_id) else {
					log::error!("Could not get node in set_input");
					return;
				};
				let Some(input) = node.inputs.get_mut(*input_index) else {
					log::error!("Could not get input in set_input");
					return;
				};
				std::mem::replace(input, new_input.clone())
			}
			InputConnector::Export(export_index) => {
				let Some(export) = network.exports.get_mut(*export_index) else {
					log::error!("Could not get export in set_input");
					return;
				};
				std::mem::replace(export, new_input.clone())
			}
		};

		if old_input == new_input {
			return;
		};

		// Ensure the network is not cyclic
		if !network.is_acyclic() {
			self.set_input(input_connector, old_input, network_path);
			return;
		}

		self.transaction_modified();

		// Ensure layer is toggled to non layer if it is no longer eligible to be a layer
		if let InputConnector::Node { node_id, .. } = &input_connector {
			if !self.is_eligible_to_be_layer(node_id, network_path) && self.is_layer(node_id, network_path) {
				self.set_to_node_or_layer(node_id, network_path, false);
			}
		}

		// Side effects
		match (&old_input, &new_input) {
			// If a node input is exposed or hidden reload the click targets and update the bounding box for all nodes
			(NodeInput::Value { exposed: new_exposed, .. }, NodeInput::Value { exposed: old_exposed, .. }) => {
				if let InputConnector::Node { node_id, .. } = input_connector {
					if new_exposed != old_exposed {
						self.unload_upstream_node_click_targets(vec![*node_id], network_path);
						self.unload_all_nodes_bounding_box(network_path);
					}
				}
			}
			(_, NodeInput::Node { node_id: upstream_node_id, .. }) => {
				// Load structure if the change is to the document network and to the first or second
				if network_path.is_empty() {
					if matches!(input_connector, InputConnector::Export(0)) {
						self.load_structure();
					} else if let InputConnector::Node { node_id, input_index } = &input_connector {
						// If the connection is made to the first or second input of a node connected to the output, then load the structure
						if self.connected_to_output(node_id, network_path) && (*input_index == 0 || *input_index == 1) {
							self.load_structure();
						}
					}
				}
				self.unload_outward_wires(network_path);
				// Layout system
				let Some(current_node_position) = self.position(upstream_node_id, network_path) else {
					log::error!("Could not get current node position in set_input for node {upstream_node_id}");
					return;
				};
				let Some(node_metadata) = self.node_metadata(upstream_node_id, network_path) else {
					log::error!("Could not get node_metadata in set_input");
					return;
				};
				match &node_metadata.persistent_metadata.node_type_metadata {
					NodeTypePersistentMetadata::Layer(_) => {
						match &input_connector {
							InputConnector::Export(_) => {
								// If a layer is connected to the exports, it should be set to absolute position without being moved.
								self.set_absolute_position(upstream_node_id, current_node_position, network_path)
							}
							InputConnector::Node {
								node_id: downstream_node_id,
								input_index,
							} => {
								// If a layer has a single connection to the bottom of another layer, it should be set to stack positioning
								let Some(downstream_node_metadata) = self.node_metadata(downstream_node_id, network_path) else {
									log::error!("Could not get downstream node_metadata in set_input");
									return;
								};
								match &downstream_node_metadata.persistent_metadata.node_type_metadata {
									NodeTypePersistentMetadata::Layer(_) => {
										// If the layer feeds into the bottom input of layer, and has no other outputs, set its position to stack at its previous y position
										let multiple_outward_wires = self
											.outward_wires(network_path)
											.and_then(|all_outward_wires| all_outward_wires.get(&OutputConnector::node(*upstream_node_id, 0)))
											.is_some_and(|outward_wires| outward_wires.len() > 1);
										if *input_index == 0 && !multiple_outward_wires {
											self.set_stack_position_calculated_offset(upstream_node_id, downstream_node_id, network_path);
										} else {
											self.set_absolute_position(upstream_node_id, current_node_position, network_path);
										}
									}
									NodeTypePersistentMetadata::Node(_) => {
										// If the layer feeds into a node, set its y offset to 0
										self.set_absolute_position(upstream_node_id, current_node_position, network_path);
									}
								}
							}
						}
					}
					NodeTypePersistentMetadata::Node(_) => {}
				}
				self.unload_upstream_node_click_targets(vec![*upstream_node_id], network_path);
				self.unload_stack_dependents(network_path);

				self.try_set_upstream_to_chain(input_connector, network_path);
			}
			// If a connection is made to the imports
			(NodeInput::Value { .. } | NodeInput::Scope { .. } | NodeInput::Inline { .. }, NodeInput::Network { .. }) => {
				self.unload_outward_wires(network_path);
			}
			// If a connection to the imports is disconnected
			(NodeInput::Network { .. }, NodeInput::Value { .. } | NodeInput::Scope { .. } | NodeInput::Inline { .. }) => {
				self.unload_outward_wires(network_path);
			}
			// If a node is disconnected.
			(NodeInput::Node { .. }, NodeInput::Value { .. } | NodeInput::Scope { .. } | NodeInput::Inline { .. }) => {
				self.unload_outward_wires(network_path);

				if let Some((old_upstream_node_id, previous_position)) = previous_metadata {
					let old_upstream_node_is_layer = self.is_layer(&old_upstream_node_id, network_path);
					let Some(outward_wires) = self
						.outward_wires(network_path)
						.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(old_upstream_node_id, 0)))
					else {
						log::error!("Could not get outward wires in set_input");
						return;
					};
					// If it is a layer and is connected to a single layer, set its position to stack at its previous y position
					if old_upstream_node_is_layer && outward_wires.len() == 1 && outward_wires[0].input_index() == 0 {
						if let Some(downstream_node_id) = outward_wires[0].node_id() {
							if self.is_layer(&downstream_node_id, network_path) {
								self.set_stack_position_calculated_offset(&old_upstream_node_id, &downstream_node_id, network_path);
								self.unload_upstream_node_click_targets(vec![old_upstream_node_id], network_path);
							}
						}
					}
					// If it is a node and is eligible to be in a chain, then set it to chain positioning
					else if !old_upstream_node_is_layer {
						self.try_set_node_to_chain(&old_upstream_node_id, network_path);
					}
					// If a node was previously connected, and it is no longer connected to any nodes, then set its position to absolute at its previous position
					else {
						self.set_absolute_position(&old_upstream_node_id, previous_position, network_path);
					}
				}
				// Load structure if the change is to the document network and to the first or second
				if network_path.is_empty() {
					if matches!(input_connector, InputConnector::Export(0)) {
						self.load_structure();
					} else if let InputConnector::Node { node_id, input_index } = &input_connector {
						// If the connection is made to the first or second input of a node connected to the output, then load the structure
						if self.connected_to_output(node_id, network_path) && (*input_index == 0 || *input_index == 1) {
							self.load_structure();
						}
					}
				}
				self.unload_stack_dependents(network_path);
			}
			_ => {}
		}
	}

	/// Ensure network metadata, positions, and other metadata is kept in sync
	pub fn disconnect_input(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) {
		let Some(current_input) = self.input_from_connector(input_connector, network_path).cloned() else {
			log::error!("Could not get current input in disconnect_input");
			return;
		};
		// Do not disconnect an already disconnected input
		if matches!(current_input, NodeInput::Value { .. }) {
			return;
		}

		if let NodeInput::Node {
			node_id: upstream_node_id,
			output_index,
			..
		} = &current_input
		{
			// If the node upstream from the disconnected input is a chain, then break the chain by setting it to absolute positioning
			if self.is_chain(upstream_node_id, network_path) {
				self.set_upstream_chain_to_absolute(upstream_node_id, network_path);
			}
			// If the node upstream from the disconnected input has an outward wire to the bottom of a layer, set it back to stack positioning
			if self.is_layer(upstream_node_id, network_path) {
				let Some(outward_wires) = self
					.outward_wires(network_path)
					.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(*upstream_node_id, *output_index)))
				else {
					log::error!("Could not get outward wires in disconnect_input");
					return;
				};
				let mut other_outward_wires = outward_wires.iter().filter(|outward_wire| *outward_wire != input_connector);
				if let Some(other_outward_wire) = other_outward_wires.next().cloned() {
					if other_outward_wires.next().is_none() {
						if let InputConnector::Node {
							node_id: downstream_node_id,
							input_index,
						} = other_outward_wire
						{
							if self.is_layer(&downstream_node_id, network_path) && input_index == 0 {
								self.set_stack_position_calculated_offset(upstream_node_id, &downstream_node_id, network_path);
							}
						}
					}
				}
			}
		}

		let tagged_value = TaggedValue::from_type_or_none(&self.input_type(input_connector, network_path).0);

		let value_input = NodeInput::value(tagged_value, true);

		self.set_input(input_connector, value_input, network_path);
	}

	pub fn create_wire(&mut self, output_connector: &OutputConnector, input_connector: &InputConnector, network_path: &[NodeId]) {
		let input = match output_connector {
			OutputConnector::Node { node_id, output_index } => NodeInput::node(*node_id, *output_index),
			OutputConnector::Import(import_index) => NodeInput::Network {
				import_type: graph_craft::generic!(T),
				import_index: *import_index,
			},
		};

		self.set_input(input_connector, input, network_path);
	}

	/// Used to insert a group of nodes into the network
	pub fn insert_node_group(&mut self, nodes: Vec<(NodeId, NodeTemplate)>, new_ids: HashMap<NodeId, NodeId>, network_path: &[NodeId]) {
		for (old_node_id, mut node_template) in nodes {
			// Get the new node template
			node_template = self.map_ids(node_template, &old_node_id, &new_ids, network_path);
			// Insert node into network
			let node_id = *new_ids.get(&old_node_id).unwrap();
			let Some(network) = self.network_mut(network_path) else {
				log::error!("Network not found in insert_node");
				return;
			};

			network.nodes.insert(node_id, node_template.document_node);
			self.transaction_modified();

			let Some(network_metadata) = self.network_metadata_mut(network_path) else {
				log::error!("Network not found in insert_node");
				return;
			};
			let node_metadata = DocumentNodeMetadata {
				persistent_metadata: node_template.persistent_node_metadata,
				transient_metadata: DocumentNodeTransientMetadata::default(),
			};
			network_metadata.persistent_metadata.node_metadata.insert(node_id, node_metadata);
		}
		for new_node_id in new_ids.values() {
			self.unload_node_click_targets(new_node_id, network_path);
		}
		self.unload_all_nodes_bounding_box(network_path);
		self.unload_outward_wires(network_path);
	}

	/// Used to insert a node template with no node/network inputs into the network.
	pub fn insert_node(&mut self, node_id: NodeId, node_template: NodeTemplate, network_path: &[NodeId]) {
		let has_node_or_network_input = node_template
			.document_node
			.inputs
			.iter()
			.all(|input| !(matches!(input, NodeInput::Node { .. }) || matches!(input, NodeInput::Network { .. })));
		assert!(has_node_or_network_input, "Cannot insert node with node or network inputs. Use insert_node_group instead");
		let Some(network) = self.network_mut(network_path) else {
			log::error!("Network not found in insert_node");
			return;
		};

		network.nodes.insert(node_id, node_template.document_node);
		self.transaction_modified();

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Network not found in insert_node");
			return;
		};
		let node_metadata = DocumentNodeMetadata {
			persistent_metadata: node_template.persistent_node_metadata,
			transient_metadata: DocumentNodeTransientMetadata::default(),
		};
		network_metadata.persistent_metadata.node_metadata.insert(node_id, node_metadata);

		self.unload_all_nodes_bounding_box(network_path);
		self.unload_node_click_targets(&node_id, network_path)
	}

	/// Deletes all nodes in `node_ids` and any sole dependents in the horizontal chain if the node to delete is a layer node.
	pub fn delete_nodes(&mut self, nodes_to_delete: Vec<NodeId>, delete_children: bool, network_path: &[NodeId]) {
		let Some(outward_wires) = self.outward_wires(network_path).cloned() else {
			log::error!("Could not get outward wires in delete_nodes");
			return;
		};
		let Some(network) = self.network(network_path) else {
			return;
		};

		let mut delete_nodes = HashSet::new();
		for node_id in &nodes_to_delete {
			delete_nodes.insert(*node_id);

			if !delete_children {
				continue;
			};

			for upstream_id in self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::LayerChildrenUpstreamFlow) {
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
							if !delete_nodes.contains(downstream_id) {
								stack.push(downstream_node_output);
							}
							// Continue traversing over the downstream sibling, if the current node is a sibling to a node that will be deleted and it is a layer
							else {
								for deleted_node_id in &nodes_to_delete {
									let Some(downstream_node) = network.nodes.get(deleted_node_id) else { continue };
									let Some(input) = downstream_node.inputs.first() else { continue };

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
			let upstream_chain_nodes = self
				.upstream_flow_back_from_nodes(vec![*delete_node_id], network_path, FlowType::PrimaryFlow)
				.skip(1)
				.take_while(|upstream_node| self.is_chain(upstream_node, network_path))
				.collect::<Vec<_>>();

			if !self.remove_references_from_network(delete_node_id, network_path) {
				log::error!("could not remove references from network");
				continue;
			}

			for input_index in 0..self.number_of_displayed_inputs(delete_node_id, network_path) {
				self.disconnect_input(&InputConnector::node(*delete_node_id, input_index), network_path);
			}

			let Some(network) = self.network_mut(network_path) else {
				log::error!("Could not get nested network in delete_nodes");
				continue;
			};

			network.nodes.remove(delete_node_id);
			self.transaction_modified();

			let Some(network_metadata) = self.network_metadata_mut(network_path) else {
				log::error!("Could not get nested network_metadata in delete_nodes");
				continue;
			};
			network_metadata.persistent_metadata.node_metadata.remove(delete_node_id);
			for previous_chain_node in upstream_chain_nodes {
				self.set_chain_position(&previous_chain_node, network_path);
			}
		}
		self.unload_all_nodes_bounding_box(network_path);
		// Instead of unloaded all node click targets, just unload the nodes upstream from the deleted nodes. unload_upstream_node_click_targets will not work since the nodes have been deleted.
		self.unload_all_nodes_click_targets(network_path);
		let Some(selected_nodes) = self.selected_nodes_mut(network_path) else {
			log::error!("Could not get selected nodes in NodeGraphMessage::DeleteNodes");
			return;
		};
		selected_nodes.retain_selected_nodes(|node_id| !nodes_to_delete.contains(node_id));
	}

	/// Removes all references to the node with the given id from the network, and reconnects the input to the node below.
	pub fn remove_references_from_network(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		// TODO: Add more logic to support retaining preview when removing references. Since there are so many edge cases/possible crashes, for now the preview is ended.
		self.stop_previewing(network_path);

		let Some(network) = self.network(network_path) else {
			log::error!("Could not get nested network in remove_references_from_network");
			return false;
		};

		// Check whether the being-deleted node's first (primary) input is a node
		let reconnect_to_input = network.nodes.get(node_id).and_then(|node| {
			node.inputs
				.iter()
				.find(|input| input.is_exposed_to_frontend(network_path.is_empty()))
				.filter(|input| matches!(input, NodeInput::Node { .. } | NodeInput::Network { .. }))
				.cloned()
		});
		// Get all upstream references
		let number_of_outputs = self.number_of_outputs(node_id, network_path);
		let Some(all_outward_wires) = self.outward_wires(network_path) else {
			log::error!("Could not get outward wires in remove_references_from_network");
			return false;
		};
		let mut downstream_inputs_to_disconnect = Vec::new();
		for output_index in 0..number_of_outputs {
			if let Some(outward_wires) = all_outward_wires.get(&OutputConnector::node(*node_id, output_index)) {
				downstream_inputs_to_disconnect.extend(outward_wires.clone());
			}
		}

		let mut reconnect_node = None;

		for downstream_input in &downstream_inputs_to_disconnect {
			self.disconnect_input(downstream_input, network_path);
			// Prevent reconnecting export to import until https://github.com/GraphiteEditor/Graphite/issues/1762 is solved
			if !(matches!(reconnect_to_input, Some(NodeInput::Network { .. })) && matches!(downstream_input, InputConnector::Export(_))) {
				if let Some(reconnect_input) = &reconnect_to_input {
					reconnect_node = reconnect_input.as_node().and_then(|node_id| if self.is_stack(&node_id, network_path) { Some(node_id) } else { None });
					self.disconnect_input(&InputConnector::node(*node_id, 0), network_path);
					self.set_input(downstream_input, reconnect_input.clone(), network_path);
				}
			}
		}

		// Shift the reconnected node up to collapse space
		if let Some(reconnect_node) = &reconnect_node {
			let Some(reconnected_node_position) = self.position(reconnect_node, network_path) else {
				log::error!("Could not get reconnected node position in remove_references_from_network");
				return false;
			};
			let Some(disconnected_node_position) = self.position(node_id, network_path) else {
				log::error!("Could not get disconnected node position in remove_references_from_network");
				return false;
			};
			let max_shift_distance = reconnected_node_position.y - disconnected_node_position.y;

			let upstream_nodes = self.upstream_flow_back_from_nodes(vec![*reconnect_node], network_path, FlowType::PrimaryFlow).collect::<Vec<_>>();

			// Select the reconnect node to move to ensure the shifting works correctly
			let Some(selected_nodes) = self.selected_nodes_mut(network_path) else {
				log::error!("Could not get selected nodes in remove_references_from_network");
				return false;
			};

			let old_selected_nodes = selected_nodes.replace_with(upstream_nodes);

			// Shift up until there is either a collision or the disconnected node position is reached
			let mut current_shift_distance = 0;
			while self.check_collision_with_stack_dependents(reconnect_node, -1, network_path).is_empty() && max_shift_distance > current_shift_distance {
				self.shift_selected_nodes(Direction::Up, false, network_path);
				current_shift_distance += 1;
			}

			let _ = self.selected_nodes_mut(network_path).unwrap().replace_with(old_selected_nodes);
		}

		true
	}

	pub fn start_previewing_without_restore(&mut self, network_path: &[NodeId]) {
		// Some logic will have to be performed to prevent the graph positions from being completely changed when the export changes to some previewed node
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in start_previewing_without_restore");
			return;
		};
		network_metadata.persistent_metadata.previewing = Previewing::Yes { root_node_to_restore: None };
	}

	fn stop_previewing(&mut self, network_path: &[NodeId]) {
		if let Previewing::Yes {
			root_node_to_restore: Some(root_node_to_restore),
		} = self.previewing(network_path)
		{
			self.set_input(
				&InputConnector::Export(0),
				NodeInput::node(root_node_to_restore.node_id, root_node_to_restore.output_index),
				network_path,
			);
		}
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in stop_previewing");
			return;
		};
		network_metadata.persistent_metadata.previewing = Previewing::No;
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

	pub fn set_display_name(&mut self, node_id: &NodeId, display_name: String, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		if node_metadata.persistent_metadata.display_name == display_name {
			return;
		}

		node_metadata.persistent_metadata.display_name.clone_from(&display_name);

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

		self.transaction_modified();
		self.try_unload_layer_width(node_id, network_path);
		self.unload_node_click_targets(node_id, network_path);
	}

	pub fn set_pinned(&mut self, node_id: &NodeId, network_path: &[NodeId], pinned: bool) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node {node_id} in set_pinned");
			return;
		};

		node_metadata.persistent_metadata.pinned = pinned;
		self.transaction_modified();
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
		self.transaction_modified();
	}

	pub fn set_locked(&mut self, node_id: &NodeId, network_path: &[NodeId], locked: bool) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node {node_id} in set_visibility");
			return;
		};

		node_metadata.persistent_metadata.locked = locked;
		self.transaction_modified();
	}

	pub fn set_to_node_or_layer(&mut self, node_id: &NodeId, network_path: &[NodeId], is_layer: bool) {
		// If a layer is set to a node, set upstream nodes to absolute position, and upstream siblings to absolute position
		let child_id = { self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).nth(1) };
		let upstream_sibling_id = { self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::PrimaryFlow).nth(1) };
		match (self.is_layer(node_id, network_path), is_layer) {
			(true, false) => {
				if let Some(child_id) = child_id {
					self.set_upstream_chain_to_absolute(&child_id, network_path);
				}
				if let Some(upstream_sibling_id) = upstream_sibling_id {
					let Some(upstream_sibling_position) = self.position(&upstream_sibling_id, network_path) else {
						log::error!("Could not get upstream sibling position in set_to_node_or_layer");
						return;
					};
					self.set_absolute_position(&upstream_sibling_id, upstream_sibling_position, network_path);
				}
			}
			(false, true) => {
				// If a node is set to a layer
				if let Some(upstream_sibling_id) = upstream_sibling_id {
					// If the upstream sibling layer has a single output, then set it to stack position
					if self.is_layer(&upstream_sibling_id, network_path)
						&& self
							.outward_wires(network_path)
							.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(upstream_sibling_id, 0)))
							.is_some_and(|outward_wires| outward_wires.len() == 1)
					{
						self.set_stack_position_calculated_offset(&upstream_sibling_id, node_id, network_path);
					} else {
						self.set_upstream_chain_to_absolute(&upstream_sibling_id, network_path);
					}
				}
			}
			_ => return,
		};

		let Some(position) = self.position(node_id, network_path) else {
			log::error!("Could not get position in set_to_node_or_layer");
			return;
		};

		let single_downstream_layer_position = self
			.outward_wires(network_path)
			.and_then(|outward_wires| {
				outward_wires
					.get(&OutputConnector::node(*node_id, 0))
					.and_then(|outward_wires| (outward_wires.len() == 1).then(|| outward_wires[0]))
					.and_then(|downstream_connector| if downstream_connector.input_index() == 0 { downstream_connector.node_id() } else { None })
			})
			.filter(|downstream_node_id| self.is_layer(downstream_node_id, network_path))
			.and_then(|downstream_layer| self.position(&downstream_layer, network_path));

		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};

		// First set the position to absolute
		node_metadata.persistent_metadata.node_type_metadata = if is_layer {
			NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::Absolute(position),
				owned_nodes: TransientMetadata::Unloaded,
			})
		} else {
			NodeTypePersistentMetadata::Node(NodePersistentMetadata {
				position: NodePosition::Absolute(position),
			})
		};

		// Try build the chain
		if is_layer {
			self.try_set_upstream_to_chain(&InputConnector::node(*node_id, 1), network_path);
		} else {
			self.try_set_node_to_chain(node_id, network_path);
		}

		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		// Set the position to stack if necessary
		if let Some(downstream_position) = is_layer.then_some(single_downstream_layer_position).flatten() {
			node_metadata.persistent_metadata.node_type_metadata = NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::Stack((position.y - downstream_position.y - 3).max(0) as u32),
				owned_nodes: TransientMetadata::Unloaded,
			})
		}

		if is_layer {
			node_metadata.transient_metadata.node_type_metadata = NodeTypeTransientMetadata::Layer(LayerTransientMetadata::default());
		} else {
			node_metadata.transient_metadata.node_type_metadata = NodeTypeTransientMetadata::Node;
		}

		self.transaction_modified();
		self.unload_stack_dependents(network_path);
		self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		self.unload_all_nodes_bounding_box(network_path);
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
		self.load_structure();
	}

	pub fn toggle_preview(&mut self, toggle_id: NodeId, network_path: &[NodeId]) {
		let Some(network) = self.network(network_path) else {
			return;
		};
		// If new_export is None then disconnect
		let mut new_export = None;
		let mut new_previewing_state = Previewing::No;
		if let Some(export) = network.exports.first() {
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
							} else {
								// Root node to restore does not change
								new_previewing_state = Previewing::Yes {
									root_node_to_restore: Some(root_node_to_restore),
								};
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
				self.start_previewing_without_restore(network_path);
			}
		}
		match new_export {
			Some(new_export) => {
				self.create_wire(&new_export, &InputConnector::Export(0), network_path);
			}
			None => {
				self.disconnect_input(&InputConnector::Export(0), network_path);
			}
		}
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			return;
		};
		network_metadata.persistent_metadata.previewing = new_previewing_state;
	}

	/// Sets the position of a node to an absolute position
	fn set_absolute_position(&mut self, node_id: &NodeId, position: IVec2, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};

		if let NodeTypePersistentMetadata::Node(node_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if node_metadata.position == NodePosition::Absolute(position) {
				return;
			}
			node_metadata.position = NodePosition::Absolute(position);
			self.transaction_modified();
		} else if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if layer_metadata.position == LayerPosition::Absolute(position) {
				return;
			}
			layer_metadata.position = LayerPosition::Absolute(position);
			self.transaction_modified();
		}
	}

	/// Sets the position of a layer to a stack position
	pub fn set_stack_position(&mut self, node_id: &NodeId, y_offset: u32, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if layer_metadata.position == LayerPosition::Stack(y_offset) {
				return;
			}
			layer_metadata.position = LayerPosition::Stack(y_offset);
			self.transaction_modified();
		} else {
			log::error!("Could not set stack position for non layer node {node_id}");
		}
	}

	/// Sets the position of a node to a stack position without changing its y offset
	pub fn set_stack_position_calculated_offset(&mut self, node_id: &NodeId, downstream_layer: &NodeId, network_path: &[NodeId]) {
		let Some(node_position) = self.position(node_id, network_path) else {
			log::error!("Could not get node position for node {node_id}");
			return;
		};
		let Some(downstream_position) = self.position(downstream_layer, network_path) else {
			log::error!("Could not get downstream position for node {downstream_layer}");
			return;
		};

		self.set_stack_position(node_id, (node_position.y - downstream_position.y - 3).max(0) as u32, network_path);
	}

	/// Sets the position of a node to a chain position
	pub fn set_chain_position(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		// Set any absolute nodes to chain positioning
		if let NodeTypePersistentMetadata::Node(NodePersistentMetadata { position }) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if *position == NodePosition::Chain {
				return;
			}
			*position = NodePosition::Chain;
			self.transaction_modified();
		}
		// If there is an upstream layer then stop breaking the chain
		else {
			log::error!("Could not set chain position for layer node {node_id}");
		}
		self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		// Reload click target of the layer which encapsulate the chain
		if let Some(downstream_layer) = self.downstream_layer(node_id, network_path) {
			self.unload_node_click_targets(&downstream_layer.to_node(), network_path);
		}
		self.unload_all_nodes_bounding_box(network_path);
	}

	fn valid_upstream_chain_nodes(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Vec<NodeId> {
		let InputConnector::Node {
			node_id: input_connector_node_id,
			input_index,
		} = input_connector
		else {
			return Vec::new();
		};
		let mut set_position_to_chain = Vec::new();
		if self.is_layer(input_connector_node_id, network_path) && *input_index == 1 || self.is_chain(input_connector_node_id, network_path) && *input_index == 0 {
			let mut downstream_id = *input_connector_node_id;
			for upstream_node in self
				.upstream_flow_back_from_nodes(vec![*input_connector_node_id], network_path, FlowType::HorizontalFlow)
				.skip(1)
				.collect::<Vec<_>>()
			{
				if self.is_layer(&upstream_node, network_path) {
					break;
				}
				if !self.has_primary_output(&upstream_node, network_path) {
					break;
				}
				let Some(outward_wires) = self.outward_wires(network_path).and_then(|outward_wires| outward_wires.get(&OutputConnector::node(upstream_node, 0))) else {
					log::error!("Could not get outward wires in try_set_upstream_to_chain");
					break;
				};
				if outward_wires.len() != 1 {
					break;
				}
				let downstream_position = self.position(&downstream_id, network_path);
				let upstream_node_position = self.position(&upstream_node, network_path);
				if let (Some(input_connector_position), Some(new_upstream_node_position)) = (downstream_position, upstream_node_position) {
					if input_connector_position.y == new_upstream_node_position.y
						&& new_upstream_node_position.x >= input_connector_position.x - 9
						&& new_upstream_node_position.x <= input_connector_position.x
					{
						set_position_to_chain.push(upstream_node);
					} else {
						break;
					}
				} else {
					break;
				}
				downstream_id = upstream_node;
			}
		}
		set_position_to_chain
	}

	/// Input connector is the input to the layer
	pub fn try_set_upstream_to_chain(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) {
		// If the new input is to a non layer node on the same y position as the input connector, or the input connector is the side input of a layer, then set it to chain position

		let valid_upstream_chain_nodes = self.valid_upstream_chain_nodes(input_connector, network_path);

		for node_id in &valid_upstream_chain_nodes {
			self.set_chain_position(node_id, network_path);
		}
		// Reload click target of the layer which used to encapsulate the node
		if !valid_upstream_chain_nodes.is_empty() {
			let mut downstream_layer = Some(input_connector.node_id().unwrap());
			while let Some(downstream_layer_id) = downstream_layer {
				if downstream_layer_id == input_connector.node_id().unwrap() || !self.is_layer(&downstream_layer_id, network_path) {
					let Some(outward_wires) = self.outward_wires(network_path) else {
						log::error!("Could not get outward wires in try_set_upstream_to_chain");
						downstream_layer = None;
						break;
					};
					downstream_layer = outward_wires
						.get(&OutputConnector::node(downstream_layer_id, 0))
						.and_then(|outward_wires| if outward_wires.len() == 1 { outward_wires[0].node_id() } else { None });
				} else {
					break;
				}
			}
			if let Some(downstream_layer) = downstream_layer {
				self.unload_node_click_targets(&downstream_layer, network_path);
			}
		}
	}

	fn try_set_node_to_chain(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		if let Some(outward_wires) = self
			.outward_wires(network_path)
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(*node_id, 0)))
			.cloned()
		{
			if outward_wires.len() == 1 {
				self.try_set_upstream_to_chain(&outward_wires[0], network_path)
			}
		}
	}

	pub fn force_set_upstream_to_chain(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		for upstream_id in self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).collect::<Vec<_>>().iter() {
			if !self.is_layer(upstream_id, network_path)
				&& self.has_primary_output(node_id, network_path)
				&& self
					.outward_wires(network_path)
					.is_some_and(|outward_wires| outward_wires.get(&OutputConnector::node(*upstream_id, 0)).is_some_and(|outward_wires| outward_wires.len() == 1))
			{
				self.set_chain_position(upstream_id, network_path);
			}
			// If there is an upstream layer then stop breaking the chain
			else {
				break;
			}
		}
	}

	/// node_id is the first chain node, not the layer
	fn set_upstream_chain_to_absolute(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(downstream_layer) = self.downstream_layer(node_id, network_path) else {
			log::error!("Could not get downstream layer in set_upstream_chain_to_absolute");
			return;
		};
		for upstream_id in self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).collect::<Vec<_>>().iter() {
			let Some(previous_position) = self.position(upstream_id, network_path) else {
				log::error!("Could not get position in set_upstream_chain_to_absolute");
				return;
			};
			// Set any chain nodes to absolute positioning
			if self.is_chain(upstream_id, network_path) {
				self.set_absolute_position(upstream_id, previous_position, network_path);
				// Reload click target of the layer which used to encapsulate the chain
				self.unload_node_click_targets(&downstream_layer.to_node(), network_path);
			}
			// If there is an upstream layer then stop breaking the chain
			else {
				break;
			}
		}
	}

	pub fn nodes_sorted_top_to_bottom<'a>(&mut self, node_ids: impl Iterator<Item = &'a NodeId>, network_path: &[NodeId]) -> Option<Vec<NodeId>> {
		let mut node_ids_with_position = node_ids
			.filter_map(|&node_id| {
				let Some(position) = self.position(&node_id, network_path) else {
					log::error!("Could not get position for node {node_id} in shift_selected_nodes");
					return None;
				};
				Some((node_id, position.y))
			})
			.collect::<Vec<(NodeId, i32)>>();

		node_ids_with_position.sort_unstable_by(|a, b| a.1.cmp(&b.1));
		Some(node_ids_with_position.into_iter().map(|(node_id, _)| node_id).collect::<Vec<_>>())
	}

	/// Used when moving layer by the layer panel, does not run any pushing logic. Moves all sole dependents of the layer as well.
	/// Ensure that the layer is absolute position.
	pub fn shift_absolute_node_position(&mut self, layer: &NodeId, shift: IVec2, network_path: &[NodeId]) {
		if shift == IVec2::ZERO {
			return;
		}
		let mut nodes_to_shift = self.upstream_nodes_below_layer(layer, network_path);
		nodes_to_shift.insert(*layer);

		for node_id in nodes_to_shift {
			let Some(node_to_shift_metadata) = self.node_metadata_mut(&node_id, network_path) else {
				log::error!("Could not get node metadata for node {node_id} in set_layer_position");
				continue;
			};
			match &mut node_to_shift_metadata.persistent_metadata.node_type_metadata {
				NodeTypePersistentMetadata::Layer(layer_metadata) => {
					if let LayerPosition::Absolute(layer_position) = &mut layer_metadata.position {
						*layer_position += shift;
					}
				}
				NodeTypePersistentMetadata::Node(node_metadata) => {
					if let NodePosition::Absolute(node_position) = &mut node_metadata.position {
						*node_position += shift;
					}
				}
			}
		}
		self.transaction_modified();
		self.unload_upstream_node_click_targets(vec![*layer], network_path);
	}

	pub fn shift_selected_nodes(&mut self, direction: Direction, shift_without_push: bool, network_path: &[NodeId]) {
		let Some(mut node_ids) = self.selected_nodes(network_path).map(|selected_nodes| selected_nodes.selected_nodes().cloned().collect::<HashSet<_>>()) else {
			log::error!("Could not get selected nodes in shift_selected_nodes");
			return;
		};
		if !shift_without_push {
			for node_id in node_ids.clone() {
				if self.is_layer(&node_id, network_path) {
					if let Some(owned_nodes) = self.owned_nodes(&node_id, network_path) {
						for owned_node in owned_nodes {
							node_ids.remove(owned_node);
						}
					};
				}
			}
		}

		for selected_node in &node_ids.clone() {
			// Deselect chain nodes upstream from a selected layer
			if self.is_chain(selected_node, network_path)
				&& self
					.downstream_layer(selected_node, network_path)
					.is_some_and(|downstream_layer| node_ids.contains(&downstream_layer.to_node()))
			{
				node_ids.remove(selected_node);
			}
		}

		// If shifting up without a push, cancel the shift if there is a stack node that cannot move up
		if direction == Direction::Up && shift_without_push {
			for node_id in &node_ids {
				let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
					log::error!("Could not get node metadata for node {node_id} in shift_selected_nodes");
					return;
				};
				if let NodeTypePersistentMetadata::Layer(layer_metadata) = &node_metadata.persistent_metadata.node_type_metadata {
					if let LayerPosition::Stack(offset) = layer_metadata.position {
						// If the upstream layer is selected, then skip
						let Some(outward_wires) = self.outward_wires(network_path).and_then(|outward_wires| outward_wires.get(&OutputConnector::node(*node_id, 0))) else {
							log::error!("Could not get outward wires in shift_selected_nodes");
							return;
						};
						if let Some(upstream_node) = outward_wires.first() {
							if node_ids.contains(&upstream_node.node_id().expect("Stack layer should have downstream layer")) {
								continue;
							}
						}
						// Offset cannot be negative, so cancel the shift
						if offset == 0 {
							return;
						}
					}
				}
			}
		}

		let Some(mut sorted_node_ids) = self.nodes_sorted_top_to_bottom(node_ids.iter(), network_path) else {
			return;
		};

		if sorted_node_ids.len() != node_ids.len() {
			log::error!("Could not get position for all nodes in shift_selected_nodes");
			return;
		}

		// If shifting down, then the lowest node (greatest y value) should be shifted first
		if direction == Direction::Down {
			sorted_node_ids.reverse();
		}

		// Ensure the top of each stack is only shifted left/right once (this is only for performance)
		let mut shifted_absolute_layers = Vec::new();

		let mut shifted_nodes = HashSet::new();

		let shift_sign = if direction == Direction::Left || direction == Direction::Up { -1 } else { 1 };

		for node_id in &sorted_node_ids {
			match direction {
				Direction::Left | Direction::Right => {
					// If the node is a non layer, then directly shift it
					if !self.is_layer(node_id, network_path) {
						self.try_shift_node(node_id, IVec2::new(shift_sign, 0), &mut shifted_nodes, network_path);
					} else {
						// Get the downstream absolute layer (inclusive)
						let mut downstream_absolute_layer = *node_id;
						loop {
							if self.is_absolute(&downstream_absolute_layer, network_path) {
								break;
							}
							let Some(downstream_node) = self
								.outward_wires(network_path)
								.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(downstream_absolute_layer, 0)))
								.and_then(|downstream_nodes| downstream_nodes.first())
								.and_then(|downstream_node| downstream_node.node_id())
							else {
								log::error!("Could not get downstream node when deselecting stack layer in shift_selected_nodes");
								break;
							};
							downstream_absolute_layer = downstream_node;
						}

						// Shift the upstream nodes below the stack layers only once
						if !shifted_absolute_layers.contains(&downstream_absolute_layer) {
							shifted_absolute_layers.push(downstream_absolute_layer);

							self.try_shift_node(&downstream_absolute_layer, IVec2::new(shift_sign, 0), &mut shifted_nodes, network_path);

							if !shift_without_push {
								for stack_nodes in self
									.upstream_flow_back_from_nodes(vec![downstream_absolute_layer], network_path, FlowType::PrimaryFlow)
									.take_while(|layer| self.is_layer(layer, network_path))
									.collect::<Vec<_>>()
								{
									for sole_dependent in &self.upstream_nodes_below_layer(&stack_nodes, network_path) {
										if self.is_absolute(sole_dependent, network_path) {
											self.try_shift_node(sole_dependent, IVec2::new(shift_sign, 0), &mut shifted_nodes, network_path);
										}
									}
								}
							}
						}
					}
				}
				Direction::Up | Direction::Down => {
					if !shift_without_push && self.is_layer(node_id, network_path) {
						self.shift_node_or_parent(node_id, shift_sign, &mut shifted_nodes, network_path);
					} else if !shifted_nodes.contains(node_id) {
						shifted_nodes.insert(*node_id);
						self.shift_node(node_id, IVec2::new(0, shift_sign), network_path);

						let Some(network_metadata) = self.network_metadata_mut(network_path) else {
							log::error!("Could not get nested network_metadata in export_ports");
							continue;
						};
						if let TransientMetadata::Loaded(stack_dependents) = &mut network_metadata.transient_metadata.stack_dependents {
							if let Some(LayerOwner::None(offset)) = stack_dependents.get_mut(node_id) {
								*offset += shift_sign;
								self.transaction_modified();
							};
						};

						// Shift the upstream layer so that it stays in the same place
						if self.is_layer(node_id, network_path) {
							let upstream_layer = {
								self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::PrimaryFlow)
									.nth(1)
									.filter(|upstream_node| self.is_stack(upstream_node, network_path))
							};
							if let Some(upstream_layer) = upstream_layer {
								self.shift_node(&upstream_layer, IVec2::new(0, -shift_sign), network_path);
							}
						}
					}
				}
			}
		}

		let Some(stack_dependents) = self
			.stack_dependents(network_path)
			.map(|stack_dependents| stack_dependents.iter().map(|(node_id, owner)| (*node_id, owner.clone())).collect::<Vec<_>>())
		else {
			log::error!("Could not load stack dependents in shift_selected_nodes");
			return;
		};

		let mut stack_dependents_with_position = stack_dependents
			.iter()
			.filter_map(|(node_id, owner)| {
				let LayerOwner::None(offset) = owner else {
					return None;
				};
				if *offset == 0 {
					return None;
				}
				if self.selected_nodes(network_path).is_some_and(|selected_nodes| {
					selected_nodes
						.selected_nodes()
						.any(|selected_node| selected_node == node_id || self.owned_nodes(node_id, network_path).is_some_and(|owned_nodes| owned_nodes.contains(selected_node)))
				}) {
					return None;
				};
				let Some(position) = self.position(node_id, network_path) else {
					log::error!("Could not get position for node {node_id} in shift_selected_nodes");
					return None;
				};
				Some((*node_id, *offset, position.y))
			})
			.collect::<Vec<(NodeId, i32, i32)>>();

		stack_dependents_with_position.sort_unstable_by(|a, b| {
			a.1.signum().cmp(&b.1.signum()).then_with(|| {
				// If the node has a positive offset, then it is shifted up, so shift the top nodes first
				if a.1.signum() == 1 {
					a.2.cmp(&b.2)
				} else {
					b.2.cmp(&a.2)
				}
			})
		});

		// Try shift every node that is offset from its original position
		for (node_id, mut offset, _) in stack_dependents_with_position.iter() {
			while offset != 0 {
				if self.check_collision_with_stack_dependents(node_id, -offset.signum(), network_path).is_empty() {
					self.vertical_shift_with_push(node_id, -offset.signum(), &mut HashSet::new(), network_path);
					offset += -offset.signum();
				} else {
					break;
				}
			}
		}
	}

	fn try_shift_node(&mut self, node_id: &NodeId, shift: IVec2, shifted_nodes: &mut HashSet<NodeId>, network_path: &[NodeId]) {
		if !shifted_nodes.contains(node_id) {
			self.shift_node(node_id, shift, network_path);
			shifted_nodes.insert(*node_id);
		}
	}

	fn vertical_shift_with_push(&mut self, node_id: &NodeId, shift_sign: i32, shifted_nodes: &mut HashSet<NodeId>, network_path: &[NodeId]) {
		// Do not shift a node more than once
		if shifted_nodes.contains(node_id) {
			return;
		}
		shifted_nodes.insert(*node_id);

		let nodes_to_shift = self.check_collision_with_stack_dependents(node_id, shift_sign, network_path);

		for node_to_shift in nodes_to_shift {
			self.shift_node_or_parent(&node_to_shift.0, shift_sign, shifted_nodes, network_path);
		}

		self.shift_node(node_id, IVec2::new(0, shift_sign), network_path);

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in export_ports");
			return;
		};
		let TransientMetadata::Loaded(stack_dependents) = &mut network_metadata.transient_metadata.stack_dependents else {
			log::error!("Stack dependents should be loaded in vertical_shift_with_push");
			return;
		};

		let mut default_layer_owner = LayerOwner::None(0);
		let layer_owner = stack_dependents.get_mut(node_id).unwrap_or_else(|| {
			log::error!("Could not get layer owner in vertical_shift_with_push for node {node_id}");
			&mut default_layer_owner
		});

		match layer_owner {
			LayerOwner::None(offset) => {
				*offset += shift_sign;
				self.transaction_modified();
			}
			LayerOwner::Layer(_) => {
				log::error!("Node being shifted with a push should not be owned");
			}
		}

		// Shift the upstream layer so that it stays in the same place
		if self.is_layer(node_id, network_path) {
			let upstream_layer = {
				self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::PrimaryFlow)
					.nth(1)
					.filter(|upstream_node| self.is_stack(upstream_node, network_path))
			};
			if let Some(upstream_layer) = upstream_layer {
				self.shift_node(&upstream_layer, IVec2::new(0, -shift_sign), network_path);
			}
		}

		// Shift the nodes that are owned by the layer (if any)
		if let Some(owned_nodes) = self.owned_nodes(node_id, network_path).cloned() {
			for owned_node in owned_nodes {
				if self.is_absolute(&owned_node, network_path) {
					self.try_shift_node(&owned_node, IVec2::new(0, shift_sign), shifted_nodes, network_path);
				}
			}
		}
	}

	fn check_collision_with_stack_dependents(&mut self, node_id: &NodeId, shift_sign: i32, network_path: &[NodeId]) -> Vec<(NodeId, LayerOwner)> {
		self.try_load_all_node_click_targets(network_path);
		self.try_load_stack_dependents(network_path);
		let Some(stack_dependents) = self.try_get_stack_dependents(network_path) else {
			log::error!("Could not load stack dependents in shift_selected_nodes");
			return Vec::new();
		};
		// Check collisions and for all owned nodes and recursively shift them
		let mut nodes_to_shift = Vec::new();

		let default_hashset = HashSet::new();
		let owned_nodes = self.owned_nodes(node_id, network_path).unwrap_or(&default_hashset);

		for current_node in owned_nodes.iter().chain(std::iter::once(node_id)) {
			for node_to_check_collision in stack_dependents {
				// Do not check collision between any of the owned nodes or the shifted node
				if owned_nodes.contains(node_to_check_collision.0) || node_to_check_collision.0 == node_id {
					continue;
				}

				if node_to_check_collision.0 == current_node {
					continue;
				}
				let Some(mut current_node_bounding_box) = self.try_get_node_bounding_box(current_node, network_path) else {
					log::error!("Could not get bounding box for node {node_id} in shift_selected_nodes");
					continue;
				};

				let Some(node_bounding_box) = self.try_get_node_bounding_box(node_to_check_collision.0, network_path) else {
					log::error!("Could not get bounding box for node {node_to_check_collision:?} in shift_selected_nodes");
					continue;
				};
				// If the nodes do not intersect horizontally, then there is no collision
				if current_node_bounding_box[1].x < node_bounding_box[0].x || current_node_bounding_box[0].x > node_bounding_box[1].x {
					continue;
				}
				// Do not check collision if the nodes are currently intersecting
				if current_node_bounding_box[1].y >= node_bounding_box[0].y - 0.1 && current_node_bounding_box[0].y <= node_bounding_box[1].y + 0.1 {
					continue;
				}

				current_node_bounding_box[1].y += GRID_SIZE as f64 * shift_sign as f64;
				current_node_bounding_box[0].y += GRID_SIZE as f64 * shift_sign as f64;

				let collision = current_node_bounding_box[1].y >= node_bounding_box[0].y - 0.1 && current_node_bounding_box[0].y <= node_bounding_box[1].y + 0.1;
				if collision {
					nodes_to_shift.push((*node_to_check_collision.0, node_to_check_collision.1.clone()));
				}
			}
		}
		nodes_to_shift
	}

	fn shift_node_or_parent(&mut self, node_id: &NodeId, shift_sign: i32, shifted_nodes: &mut HashSet<NodeId>, network_path: &[NodeId]) {
		let Some(stack_dependents) = self.stack_dependents(network_path) else {
			log::error!("Could not load stack dependents in shift_selected_nodes");
			return;
		};
		let Some(layer_owner) = stack_dependents.get(node_id) else {
			log::error!("Could not get layer owner in shift_node_or_parent for node {node_id}");
			return;
		};
		match layer_owner {
			LayerOwner::Layer(layer_owner) => {
				let layer_owner = *layer_owner;
				self.shift_node_or_parent(&layer_owner, shift_sign, shifted_nodes, network_path)
			}
			LayerOwner::None(_) => self.vertical_shift_with_push(node_id, shift_sign, shifted_nodes, network_path),
		}
	}

	/// Shifts a node by a certain offset without the auto layout system. If the node is a layer in a stack, the y_offset is shifted. If the node is a node in a chain, its position gets set to absolute.
	// TODO: Check for unnecessary unloading of click targets
	pub fn shift_node(&mut self, node_id: &NodeId, shift: IVec2, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if let LayerPosition::Absolute(layer_position) = &mut layer_metadata.position {
				*layer_position += shift;
				self.transaction_modified();
			} else if let LayerPosition::Stack(y_offset) = &mut layer_metadata.position {
				let shifted_y_offset = *y_offset as i32 + shift.y;
				// A layer can only be shifted to a positive y_offset
				if shifted_y_offset < 0 {
					log::error!(
						"Space should be made above the layer before shifting it up. Layer {node_id} current y_offset: {y_offset} shift: {}",
						shift.y
					);
				}
				if shift.x != 0 {
					log::error!("Stack layer {node_id} cannot be shifted horizontally.");
				}
				let new_y_offset = shifted_y_offset.max(0) as u32;
				if *y_offset == new_y_offset {
					return;
				}
				*y_offset = new_y_offset;
				self.transaction_modified();
			}
			// Unload click targets for all upstream nodes, since they may have been derived from the node that was shifted
			self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		} else if let NodeTypePersistentMetadata::Node(node_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if let NodePosition::Absolute(node_metadata) = &mut node_metadata.position {
				*node_metadata += shift;
				self.transaction_modified();
				// Unload click targets for all upstream nodes, since they may have been derived from the node that was shifted
				self.unload_upstream_node_click_targets(vec![*node_id], network_path);
				self.try_set_node_to_chain(node_id, network_path);
			} else if let NodePosition::Chain = node_metadata.position {
				self.set_upstream_chain_to_absolute(node_id, network_path);
				self.shift_node(node_id, shift, network_path);
			}
		}
		// Unload click targets for all upstream nodes, since they may have been derived from the node that was shifted
		self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		self.unload_all_nodes_bounding_box(network_path);
	}

	// TODO: Run the auto layout system to make space for the new nodes
	/// Disconnect the layers primary output and the input to the last non layer node feeding into it through primary flow, reconnects, then moves the layer to the new layer and stack index
	pub fn move_layer_to_stack(&mut self, layer: LayerNodeIdentifier, mut parent: LayerNodeIdentifier, mut insert_index: usize, network_path: &[NodeId]) {
		// Prevent moving an artboard anywhere but to the ROOT_PARENT child stack
		if self.is_artboard(&layer.to_node(), network_path) && parent != LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Artboard can only be moved to the root parent stack");
			return;
		}

		// A layer is considered to be the height of that layer plus the height to the upstream layer sibling
		// If a non artboard layer is attempted to be connected to the exports, and there is already an artboard connected, then connect the layer to the artboard.
		if let Some(first_layer) = LayerNodeIdentifier::ROOT_PARENT.children(&self.document_metadata).next() {
			if parent == LayerNodeIdentifier::ROOT_PARENT
				&& !self.reference(&layer.to_node(), network_path).is_some_and(|reference| reference == "Artboard")
				&& self.is_artboard(&first_layer.to_node(), network_path)
			{
				parent = first_layer;
				insert_index = 0;
			}
		}

		let Some(layer_to_move_position) = self.position(&layer.to_node(), network_path) else {
			log::error!("Could not get layer_to_move_position in move_layer_to_stack");
			return;
		};

		let mut lowest_upstream_node_height = 0;
		for upstream_node in self
			.upstream_flow_back_from_nodes(vec![layer.to_node()], network_path, FlowType::LayerChildrenUpstreamFlow)
			.collect::<Vec<_>>()
		{
			let Some(upstream_node_position) = self.position(&upstream_node, network_path) else {
				log::error!("Could not get upstream node position in move_layer_to_stack");
				return;
			};
			lowest_upstream_node_height = lowest_upstream_node_height.max((upstream_node_position.y - layer_to_move_position.y).max(0) as u32);
		}

		// If the moved layer is a child of the new parent, then get its index after the disconnect
		if let Some(moved_layer_previous_index) = parent.children(&self.document_metadata).position(|child| child == layer) {
			// Adjust the insert index if the layer's previous index is less than the insert index
			if moved_layer_previous_index < insert_index {
				insert_index -= 1;
			}
		}

		// Disconnect layer to move
		self.remove_references_from_network(&layer.to_node(), network_path);

		let post_node = ModifyInputsContext::get_post_node_with_index(self, parent, insert_index);

		// Get the previous input to the post node before inserting the layer
		let Some(post_node_input) = self.input_from_connector(&post_node, network_path).cloned() else {
			log::error!("Could not get previous input in move_layer_to_stack for parent {parent:?} and insert_index {insert_index}");
			return;
		};

		let Some(previous_layer_position) = self.position(&layer.to_node(), network_path) else {
			log::error!("Could not get previous layer position in move_layer_to_stack");
			return;
		};

		let after_move_post_layer_position = if let Some(post_node_id) = post_node.node_id() {
			self.position(&post_node_id, network_path)
		} else {
			Some(IVec2::new(8, -3))
		};

		let Some(after_move_post_layer_position) = after_move_post_layer_position else {
			log::error!("Could not get post node position in move_layer_to_stack");
			return;
		};

		// Get the height of the downstream node if inserting into a stack
		let mut downstream_height = 0;
		let inserting_into_stack =
			!(post_node.input_index() == 1 || matches!(post_node, InputConnector::Export(_)) || !post_node.node_id().is_some_and(|post_node_id| self.is_layer(&post_node_id, network_path)));
		if inserting_into_stack {
			if let Some(downstream_node) = post_node.node_id() {
				let Some(downstream_node_position) = self.position(&downstream_node, network_path) else {
					log::error!("Could not get downstream node position in move_layer_to_stack");
					return;
				};
				let mut lowest_y_position = downstream_node_position.y + 3;

				for bottom_position in self.upstream_nodes_below_layer(&downstream_node, network_path).iter().filter_map(|node_id| {
					let is_layer = self.is_layer(node_id, network_path);
					self.position(node_id, network_path).map(|position| position.y + if is_layer { 3 } else { 2 })
				}) {
					lowest_y_position = lowest_y_position.max(bottom_position);
				}
				downstream_height = lowest_y_position - (downstream_node_position.y + 3);
			}
		}

		let mut highest_y_position = layer_to_move_position.y;
		let mut lowest_y_position = layer_to_move_position.y;

		for (bottom_position, top_position) in self.upstream_nodes_below_layer(&layer.to_node(), network_path).iter().filter_map(|node_id| {
			let is_layer = self.is_layer(node_id, network_path);
			let bottom_position = self.position(node_id, network_path).map(|position| position.y + if is_layer { 3 } else { 2 });
			let top_position = self.position(node_id, network_path).map(|position| if is_layer { position.y - 1 } else { position.y });
			bottom_position.zip(top_position)
		}) {
			highest_y_position = highest_y_position.min(top_position);
			lowest_y_position = lowest_y_position.max(bottom_position);
		}
		let height_above_layer = layer_to_move_position.y - highest_y_position + downstream_height;
		let height_below_layer = lowest_y_position - layer_to_move_position.y - 3;

		// If there is an upstream node in the new location for the layer, create space for the moved layer by shifting the upstream node down
		if let Some(upstream_node_id) = post_node_input.as_node() {
			// Select the layer to move to ensure the shifting works correctly
			let Some(selected_nodes) = self.selected_nodes_mut(network_path) else {
				log::error!("Could not get selected nodes in move_layer_to_stack");
				return;
			};
			let old_selected_nodes = selected_nodes.replace_with(vec![upstream_node_id]);

			// Create the minimum amount space for the moved layer
			for _ in 0..3 {
				self.vertical_shift_with_push(&upstream_node_id, 1, &mut HashSet::new(), network_path);
			}

			let Some(stack_position) = self.position(&upstream_node_id, network_path) else {
				log::error!("Could not get stack position in move_layer_to_stack");
				return;
			};

			let current_gap = stack_position.y - (after_move_post_layer_position.y + 2);
			let target_gap = 1 + height_above_layer + 2 + height_below_layer + 1;

			for _ in 0..(target_gap - current_gap).max(0) {
				self.vertical_shift_with_push(&upstream_node_id, 1, &mut HashSet::new(), network_path);
			}

			let _ = self.selected_nodes_mut(network_path).unwrap().replace_with(old_selected_nodes);
		}

		// If inserting into a stack with a parent, ensure the parent stack has enough space for the child stack
		if parent != LayerNodeIdentifier::ROOT_PARENT {
			if let Some(upstream_sibling) = parent.next_sibling(&self.document_metadata) {
				let Some(parent_position) = self.position(&parent.to_node(), network_path) else {
					log::error!("Could not get parent position in move_layer_to_stack");
					return;
				};
				let last_child = parent.last_child(&self.document_metadata).unwrap_or(parent);

				let Some(mut last_child_position) = self.position(&last_child.to_node(), network_path) else {
					log::error!("Could not get last child position in move_layer_to_stack");
					return;
				};

				if self.is_layer(&last_child.to_node(), network_path) {
					last_child_position.y += 3;
				} else {
					last_child_position.y += 2;
				}

				// If inserting below the current last child, then the last child is layer to move
				if post_node.node_id() == Some(last_child.to_node()) {
					last_child_position += height_above_layer + 3 + height_below_layer;
				}

				let Some(upstream_sibling_position) = self.position(&upstream_sibling.to_node(), network_path) else {
					log::error!("Could not get upstream sibling position in move_layer_to_stack");
					return;
				};

				let target_gap = last_child_position.y - parent_position.y + 3;
				let current_gap = upstream_sibling_position.y - parent_position.y;

				let upstream_nodes = self
					.upstream_flow_back_from_nodes(vec![upstream_sibling.to_node()], network_path, FlowType::UpstreamFlow)
					.collect::<Vec<_>>();
				let Some(selected_nodes) = self.selected_nodes_mut(network_path) else {
					log::error!("Could not get selected nodes in move_layer_to_stack");
					return;
				};
				let old_selected_nodes = selected_nodes.replace_with(upstream_nodes);

				for _ in 0..(target_gap - current_gap).max(0) {
					self.shift_selected_nodes(Direction::Down, true, network_path);
				}

				let _ = self.selected_nodes_mut(network_path).unwrap().replace_with(old_selected_nodes);
			}
		}

		// Connect the layer to a parent layer/node at the top of the stack, or a non layer node midway down the stack
		if !inserting_into_stack {
			match post_node_input {
				// Create a new stack
				NodeInput::Value { .. } | NodeInput::Scope(_) | NodeInput::Inline(_) | NodeInput::Reflection(_) => {
					self.create_wire(&OutputConnector::node(layer.to_node(), 0), &post_node, network_path);

					let final_layer_position = after_move_post_layer_position + IVec2::new(-8, 3);
					let shift = final_layer_position - previous_layer_position;
					self.shift_absolute_node_position(&layer.to_node(), shift, network_path);
				}
				// Move to the top of a stack.
				NodeInput::Node { node_id, .. } => {
					let Some(stack_top_position) = self.position(&node_id, network_path) else {
						log::error!("Could not get stack x position in move_layer_to_stack");
						return;
					};

					let final_layer_position = IVec2::new(stack_top_position.x, after_move_post_layer_position.y + 3 + height_above_layer);
					let shift = final_layer_position - previous_layer_position;
					self.shift_absolute_node_position(&layer.to_node(), shift, network_path);
					self.insert_node_between(&layer.to_node(), &post_node, 0, network_path);
				}
				NodeInput::Network { .. } => {
					log::error!("Cannot move post node to parent which connects to the imports")
				}
			}
		} else {
			match post_node_input {
				// Move to the bottom of the stack
				NodeInput::Value { .. } | NodeInput::Scope(_) | NodeInput::Inline(_) | NodeInput::Reflection(_) => {
					let offset = after_move_post_layer_position - previous_layer_position + IVec2::new(0, 3 + height_above_layer);
					self.shift_absolute_node_position(&layer.to_node(), offset, network_path);
					self.create_wire(&OutputConnector::node(layer.to_node(), 0), &post_node, network_path);
				}
				// Insert into the stack
				NodeInput::Node { .. } => {
					let final_layer_position = after_move_post_layer_position + IVec2::new(0, 3 + height_above_layer);
					let shift = final_layer_position - previous_layer_position;
					self.shift_absolute_node_position(&layer.to_node(), shift, network_path);
					self.insert_node_between(&layer.to_node(), &post_node, 0, network_path);
				}
				NodeInput::Network { .. } => {
					log::error!("Cannot move post node to parent which connects to the imports")
				}
			}
		}
		self.unload_upstream_node_click_targets(vec![layer.to_node()], network_path);
	}

	// Insert a node onto a wire. Ensure insert_node_input_index is an exposed input
	pub fn insert_node_between(&mut self, node_id: &NodeId, input_connector: &InputConnector, insert_node_input_index: usize, network_path: &[NodeId]) {
		if self.number_of_displayed_inputs(node_id, network_path) == 0 {
			log::error!("Cannot insert a node onto a wire with no exposed inputs");
			return;
		}

		let Some(upstream_output) = self.upstream_output_connector(input_connector, network_path) else {
			log::error!("Could not get upstream output in insert_node_between");
			return;
		};

		// Disconnect the previous input
		self.disconnect_input(input_connector, network_path);

		// Connect the input connector to the new node
		self.create_wire(&OutputConnector::node(*node_id, 0), input_connector, network_path);

		// Connect the new node to the previous node
		self.create_wire(&upstream_output, &InputConnector::node(*node_id, insert_node_input_index), network_path);
	}

	// Moves a node and to the start of a layer chain (feeding into the secondary input of the layer)
	pub fn move_node_to_chain_start(&mut self, node_id: &NodeId, parent: LayerNodeIdentifier, network_path: &[NodeId]) {
		let Some(current_input) = self.input_from_connector(&InputConnector::node(parent.to_node(), 1), network_path) else {
			log::error!("Could not get input for node {node_id}");
			return;
		};
		if matches!(current_input, NodeInput::Value { .. }) {
			self.create_wire(&OutputConnector::node(*node_id, 0), &InputConnector::node(parent.to_node(), 1), network_path);
			self.set_chain_position(node_id, network_path);
		} else {
			// TODO: Implement a more robust horizontal shift system when inserting a node into a chain.
			// This should be done by breaking the chain and shifting the sole dependents for each node upstream of the insertion.
			// Before inserting the node, shift the layer right 7 units so that all sole dependents are also shifted
			// let input_connector = InputConnector::node(parent.to_node(), 0);
			// let old_upstream = self.upstream_output_connector(&input_connector, network_path);
			// This also needs to disconnect from the downstream layer
			// self.disconnect_input(&input_connector, network_path);
			// let Some(selected_nodes) = self.selected_nodes_mut(network_path) else {
			// 	log::error!("Could not get selected nodes in move_layer_to_stack");
			// 	return;
			// };
			// let old_selected_nodes = selected_nodes.replace_with(vec![parent.to_node()]);

			// for _ in 0..7 {
			// 	self.shift_selected_nodes(Direction::Left, false, network_path);
			// }
			// // Grip drag it back to the right
			// for _ in 0..7 {
			// 	self.shift_selected_nodes(Direction::Right, true, network_path);
			// }
			// let _ = self.selected_nodes_mut(network_path).unwrap().replace_with(old_selected_nodes);
			// if let Some(old_upstream) = old_upstream {
			// 	self.create_wire(&old_upstream, &input_connector, network_path);
			// }

			// Insert the node in the gap and set the upstream to a chain
			self.insert_node_between(node_id, &InputConnector::node(parent.to_node(), 1), 0, network_path);
			self.force_set_upstream_to_chain(node_id, network_path);
		}
	}
}

#[derive(PartialEq)]
pub enum FlowType {
	/// Iterate over all upstream nodes (inclusive) from every input (the primary and all secondary).
	UpstreamFlow,
	/// Iterate over nodes (inclusive) connected to the primary input.
	PrimaryFlow,
	/// Iterate over the secondary input (inclusive) for layer nodes and primary input for non layer nodes.
	HorizontalFlow,
	/// Same as horizontal flow, but only iterates over connections to primary outputs
	HorizontalPrimaryOutputFlow,
	/// Upstream flow starting from the either the node (inclusive) or secondary input of the layer (not inclusive).
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
	type Item = NodeId;
	fn next(&mut self) -> Option<Self::Item> {
		loop {
			let node_id = self.stack.pop()?;

			if let (Some(document_node), Some(node_metadata)) = (self.network.nodes.get(&node_id), self.network_metadata.persistent_metadata.node_metadata.get(&node_id)) {
				let skip = if matches!(self.flow_type, FlowType::HorizontalFlow | FlowType::HorizontalPrimaryOutputFlow) && node_metadata.persistent_metadata.is_layer() {
					1
				} else {
					0
				};
				let take = if self.flow_type == FlowType::UpstreamFlow { usize::MAX } else { 1 };
				let inputs = document_node.inputs.iter().skip(skip).take(take);

				let node_ids = inputs.filter_map(|input| match input {
					NodeInput::Node { output_index, .. } if self.flow_type == FlowType::HorizontalPrimaryOutputFlow && *output_index != 0 => None,
					NodeInput::Node { node_id, .. } => Some(node_id),
					_ => None,
				});

				self.stack.extend(node_ids);

				return Some(node_id);
			}
		}
	}
}

/// Represents the source of a resolved type (for debugging)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum TypeSource {
	Compiled,
	RandomProtonodeImplementation,
	DocumentNodeDefault,
	TaggedValue,
	OuterMostExportDefault,

	Error(&'static str),
}

impl Default for TypeSource {
	fn default() -> Self {
		Self::Error("no source")
	}
}

/// Represents an input connector with index based on the [`DocumentNode::inputs`] index, not the visible input index
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum InputConnector {
	#[serde(rename = "node")]
	Node {
		#[serde(rename = "nodeId")]
		node_id: NodeId,
		#[serde(rename = "inputIndex")]
		input_index: usize,
	},
	#[serde(rename = "export")]
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
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum OutputConnector {
	#[serde(rename = "node")]
	Node {
		#[serde(rename = "nodeId")]
		node_id: NodeId,
		#[serde(rename = "outputIndex")]
		output_index: usize,
	},
	#[serde(rename = "import")]
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

	pub fn from_input(input: &NodeInput) -> Option<Self> {
		match input {
			NodeInput::Network { import_index, .. } => Some(Self::Import(*import_index)),
			NodeInput::Node { node_id, output_index, .. } => Some(Self::node(*node_id, *output_index)),
			_ => None,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Ports {
	input_ports: Vec<(usize, ClickTarget)>,
	output_ports: Vec<(usize, ClickTarget)>,
}

impl Default for Ports {
	fn default() -> Self {
		Self::new()
	}
}

impl Ports {
	pub fn new() -> Ports {
		Ports {
			input_ports: Vec::new(),
			output_ports: Vec::new(),
		}
	}

	pub fn click_targets(&self) -> impl Iterator<Item = &ClickTarget> {
		self.input_ports
			.iter()
			.map(|(_, click_target)| click_target)
			.chain(self.output_ports.iter().map(|(_, click_target)| click_target))
	}

	pub fn insert_input_port_at_center(&mut self, input_index: usize, center: DVec2) {
		let subpath = Subpath::new_ellipse(center - DVec2::new(8., 8.), center + DVec2::new(8., 8.));
		self.input_ports.push((input_index, ClickTarget::new(subpath, 0.)));
	}

	pub fn insert_output_port_at_center(&mut self, output_index: usize, center: DVec2) {
		let subpath = Subpath::new_ellipse(center - DVec2::new(8., 8.), center + DVec2::new(8., 8.));
		self.output_ports.push((output_index, ClickTarget::new(subpath, 0.)));
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
		let center = node_top_left + DVec2::new(2. * 24., -8.);
		self.insert_output_port_at_center(0, center);
	}

	pub fn clicked_input_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.input_ports.iter().find_map(|(port, click_target)| click_target.intersect_point_no_stroke(point).then_some(*port))
	}

	pub fn clicked_output_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.output_ports.iter().find_map(|(port, click_target)| click_target.intersect_point_no_stroke(point).then_some(*port))
	}

	pub fn input_port_position(&self, index: usize) -> Option<DVec2> {
		self.input_ports
			.get(index)
			.and_then(|(_, click_target)| click_target.bounding_box().map(|bounds| bounds[0] + DVec2::new(8., 8.)))
	}

	pub fn output_port_position(&self, index: usize) -> Option<DVec2> {
		self.output_ports
			.get(index)
			.and_then(|(_, click_target)| click_target.bounding_box().map(|bounds| bounds[0] + DVec2::new(8., 8.)))
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
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkMetadata {
	pub persistent_metadata: NodeNetworkPersistentMetadata,
	#[serde(skip)]
	pub transient_metadata: NodeNetworkTransientMetadata,
}

impl Clone for NodeNetworkMetadata {
	fn clone(&self) -> Self {
		NodeNetworkMetadata {
			persistent_metadata: self.persistent_metadata.clone(),
			transient_metadata: Default::default(),
		}
	}
}

impl PartialEq for NodeNetworkMetadata {
	fn eq(&self, other: &Self) -> bool {
		self.persistent_metadata == other.persistent_metadata
	}
}

impl NodeNetworkMetadata {
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
				.and_then(|network: &mut NodeNetworkMetadata| network.persistent_metadata.node_metadata.get_mut(segment))
				.and_then(|node| node.persistent_metadata.network_metadata.as_mut());
		}
		network_metadata
	}
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkPersistentMetadata {
	/// Node metadata must exist for every document node in the network
	#[serde(serialize_with = "graphene_std::vector::serialize_hashmap", deserialize_with = "graphene_std::vector::deserialize_hashmap")]
	pub node_metadata: HashMap<NodeId, DocumentNodeMetadata>,
	/// Cached metadata for each node, which is calculated when adding a node to node_metadata
	/// Indicates whether the network is currently rendered with a particular node that is previewed, and if so, which connection should be restored when the preview ends.
	pub previewing: Previewing,
	// Stores the transform and navigation state for the network
	pub navigation_metadata: NavigationMetadata,
	/// Stack of selection snapshots for previous history states.
	#[serde(default)]
	pub selection_undo_history: VecDeque<SelectedNodes>,
	/// Stack of selection snapshots for future history states.
	#[serde(default)]
	pub selection_redo_history: VecDeque<SelectedNodes>,
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
	/// Sole dependents of the top of the stacks of all selected nodes. Used to determine which nodes are checked for collision when shifting.
	/// The LayerOwner is used to determine whether the collided node should be shifted, or the layer that owns it.
	pub stack_dependents: TransientMetadata<HashMap<NodeId, LayerOwner>>,
	/// Cache for the bounding box around all nodes in node graph space.
	pub all_nodes_bounding_box: TransientMetadata<[DVec2; 2]>,
	/// Cache bounding box for all "groups of nodes", which will be used to prevent overlapping nodes
	// node_group_bounding_box: Vec<(Subpath<ManipulatorGroupId>, Vec<Nodes>)>,
	/// Cache for all outward wire connections
	pub outward_wires: TransientMetadata<HashMap<OutputConnector, Vec<InputConnector>>>,
	/// TODO: Cache all wire paths instead of calculating in Graph.svelte
	// pub wire_paths: Vec<WirePath>
	/// All export connector click targets
	pub import_export_ports: TransientMetadata<Ports>,
	/// Click targets for adding, removing, and moving import/export ports
	pub modify_import_export: TransientMetadata<ModifyImportExportClickTarget>,
	// Distance to the edges of the network, where the import/export ports are displayed. Rounded to nearest grid space when the panning ends.
	pub rounded_network_edge_distance: TransientMetadata<NetworkEdgeDistance>,
}

#[derive(Debug, Clone)]
pub struct ModifyImportExportClickTarget {
	// Plus icon that appears below all imports/exports
	pub add_import: ClickTarget,
	pub add_export: ClickTarget,
	// Subtract icon that appears when hovering over an import/export
	pub remove_imports: Vec<ClickTarget>,
	pub remove_exports: Vec<ClickTarget>,
	// Grip drag icon that appears when hovering over an import/export
	pub move_import: Vec<ClickTarget>,
	pub move_export: Vec<ClickTarget>,
}

#[derive(Debug, Clone)]
pub struct NetworkEdgeDistance {
	/// The viewport pixel distance between the left edge of the node graph and the exports.
	pub exports_to_edge_distance: DVec2,
	/// The viewport pixel distance between the left edge of the node graph and the imports.
	pub imports_to_edge_distance: DVec2,
}

#[derive(Debug, Clone)]
pub enum LayerOwner {
	// Used to get the layer that should be shifted when there is a collision.
	Layer(NodeId),
	// The vertical offset of a node from the start of its shift. Should be reset when the drag ends.
	None(i32),
}

/// Utility function for providing a default boolean value to serde.
#[inline(always)]
fn return_true() -> bool {
	true
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DocumentNodeMetadata {
	pub persistent_metadata: DocumentNodePersistentMetadata,
	#[serde(skip)]
	pub transient_metadata: DocumentNodeTransientMetadata,
}

impl Clone for DocumentNodeMetadata {
	fn clone(&self) -> Self {
		DocumentNodeMetadata {
			persistent_metadata: self.persistent_metadata.clone(),
			transient_metadata: Default::default(),
		}
	}
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
	/// Input/Output names may not be the same length as the number of inputs/outputs. They are the same as the nested networks Imports/Exports.
	/// If the string is empty/DNE, then it uses the type.
	pub input_names: Vec<String>,
	pub output_names: Vec<String>,
	/// Indicates to the UI if a primary output should be drawn for this node.
	/// True for most nodes, but the Split Channels node is an example of a node that has multiple secondary outputs but no primary output.
	#[serde(default = "return_true")]
	pub has_primary_output: bool,
	/// Represents the lock icon for locking/unlocking the node in the graph UI. When locked, a node cannot be moved in the graph UI.
	#[serde(default)]
	pub locked: bool,
	/// Indicates that the node will be shown in the Properties panel when it would otherwise be empty, letting a user easily edit its properties by just deselecting everything.
	#[serde(default)]
	pub pinned: bool,
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
			pinned: false,
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
			owned_nodes: TransientMetadata::default(),
		})
	}
}

/// All fields in LayerMetadata should automatically be updated by using the network interface API
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LayerPersistentMetadata {
	// TODO: Store click target for the preview button, which will appear when the node is a selected/(hovered?) layer node
	// preview_click_target: Option<ClickTarget>,
	/// Stores the position of a layer node, which can either be Absolute or Stack
	pub position: LayerPosition,
	/// All nodes that should be moved when the layer is moved.
	#[serde(skip)]
	pub owned_nodes: TransientMetadata<HashSet<NodeId>>,
}

impl PartialEq for LayerPersistentMetadata {
	fn eq(&self, other: &Self) -> bool {
		self.position == other.position
	}
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodePersistentMetadata {
	/// Stores the position of a non layer node, which can either be Absolute or Chain
	position: NodePosition,
}

/// A layer can either be position as Absolute or in a Stack
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LayerPosition {
	// Position of the node in grid spaces
	Absolute(IVec2),
	// A layer is in a Stack when it feeds into the bottom input of a layer. The Y position stores the vertical distance between the layer and its upstream sibling/parent.
	Stack(u32),
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
	// The click targets are stored as a single struct since it is very rare for only one to be updated, and recomputing all click targets in one function is more efficient than storing them separately.
	pub click_targets: TransientMetadata<DocumentNodeClickTargets>,
	// Metadata that is specific to either nodes or layers, which are chosen states for displaying as a left-to-right node or bottom-to-top layer.
	pub node_type_metadata: NodeTypeTransientMetadata,
}

#[derive(Debug, Clone)]
pub struct DocumentNodeClickTargets {
	/// In order to keep the displayed position of the node in sync with the click target, the displayed position of a node is derived from the top left of the click target
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
	// Should not be a performance concern to calculate when needed with chain_width.
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
	/// Cache for the grip icon, which is next to the visibility button.
	pub grip_click_target: ClickTarget,
	// TODO: Store click target for the preview button, which will appear when the node is a selected/(hovered?) layer node
	// preview_click_target: ClickTarget,
}

pub enum LayerClickTargetTypes {
	Visibility,
	Grip,
	// Preview,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NavigationMetadata {
	/// The current pan, and zoom state of the viewport's view of the node graph.
	/// Ensure `DocumentMessage::UpdateDocumentTransform` is called when the pan, zoom, or transform changes.
	pub node_graph_ptz: PTZ,
	// TODO: Remove and replace with calculate_offset_transform from the node_graph_ptz. This will be difficult since it requires both the navigation message handler and the IPP
	/// Transform from node graph space to viewport space.
	pub node_graph_to_viewport: DAffine2,
	/// Top right of the node graph in viewport space
	#[serde(default)]
	pub node_graph_top_right: DVec2,
}

impl Default for NavigationMetadata {
	fn default() -> NavigationMetadata {
		// Default PTZ and transform
		NavigationMetadata {
			node_graph_ptz: PTZ::default(),
			node_graph_to_viewport: DAffine2::IDENTITY,
			// TODO: Eventually replace with footprint
			node_graph_top_right: DVec2::ZERO,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TransactionStatus {
	Started,
	Modified,
	#[default]
	Finished,
}
