use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use core_types::memo::MemoHash;
use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput as GraphCraftNodeInput, NodeNetwork};
use graph_craft::{ProtoNodeIdentifier, Type, concrete};
use rustc_hash::FxHashMap;

use crate::{DeclarationId, Implementation, NetworkId, NodeId, NodeInput, Registry, ATTR_CALL_ARGUMENT, ATTR_CONTEXT_FEATURES, ATTR_IMPORT_TYPE, ATTR_VISIBLE, ATTR_SKIP_DEDUPLICATION, ATTR_REFLECTION_METADATA, ATTR_ORIGINAL_NODE_ID};

/// Errors that can occur during conversion from Registry to NodeNetwork
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
	#[error("Network {0} not found")]
	NetworkNotFound(NetworkId),
	#[error("Node {0} not found")]
	NodeNotFound(NodeId),
	#[error("ProtoNode declaration {0} not found")]
	DeclarationNotFound(DeclarationId),
	#[error("Deserialization error: {0}")]
	DeserializationError(String),
	#[error("Cannot find root network")]
	MissingRootNetwork,
	#[error("Identity node {0} has no inputs")]
	InvalidIdentityNode(NodeId),
}

/// Converts a Registry to a NodeNetwork.
///
/// ## Identity Node Pattern
///
/// The Registry uses dummy identity nodes for network exports to reuse CRDT logic.
/// `Network.exports: Vec<NodeId>` contains IDs of identity nodes (not the actual nodes to export).
/// These identity nodes have a single input pointing to the actual node to export.
///
/// During conversion:
/// 1. Identity nodes are resolved by following their first input
/// 2. Identity nodes are excluded from the converted network's nodes map
/// 3. The resolved inputs become the network's exports
///
/// ## Nested Structure Preservation
///
/// The conversion maintains full nesting:
/// - Each `NodeNetwork.nodes` contains only the nodes at that specific network level
/// - Nested networks are recursively converted and embedded in `DocumentNodeImplementation::Network`
/// - Registry uses indirection (NetworkId references), NodeNetwork uses direct embedding
impl TryFrom<&Registry> for NodeNetwork {
	type Error = ConversionError;

	fn try_from(registry: &Registry) -> Result<Self, Self::Error> {
		convert_registry(registry)
	}
}

/// Converts the Registry to a NodeNetwork by finding and converting the root network
fn convert_registry(registry: &Registry) -> Result<NodeNetwork, ConversionError> {
	let root_network_id = find_root_network_id(registry)?;
	convert_network(registry, root_network_id)
}

/// Finds the root network ID by looking at exported_nodes.
///
/// Note: exported_nodes points to identity nodes, but their `network` field
/// tells us which network they belong to (the root network).
fn find_root_network_id(registry: &Registry) -> Result<NetworkId, ConversionError> {
	registry
		.exported_nodes
		.first()
		.and_then(|&node_id| registry.node_instances.get(&node_id))
		.map(|node| node.network)
		.ok_or(ConversionError::MissingRootNetwork)
}

/// Converts a specific network by ID, recursively converting any nested networks.
///
/// ## Identity Node Handling
///
/// Identity nodes (used for exports) are:
/// 1. Identified by checking if they're in `network.exports`
/// 2. Excluded from the converted network's `nodes` map
/// 3. Resolved to their first input, which becomes the actual export
///
/// ## ID Remapping
///
/// The Registry uses globally unique hashed IDs, but each NodeNetwork needs local IDs (0, 1, 2...).
/// We extract the original local IDs from ATTR_ORIGINAL_NODE_ID on-demand when converting nodes
/// and their references. Since references only point to nodes in the same network, we can
/// deterministically look up the original ID without building an upfront mapping.
fn convert_network(registry: &Registry, network_id: NetworkId) -> Result<NodeNetwork, ConversionError> {
	let network = registry.networks.get(&network_id).ok_or(ConversionError::NetworkNotFound(network_id))?;

	// Identify identity nodes used for exports so we can exclude them
	let export_identity_node_ids: HashSet<NodeId> = network.exports.iter().copied().collect();

	// Filter nodes belonging to this specific network level only.
	// Nested network nodes are not included here - they'll be recursively
	// converted when we encounter Implementation::Network references.
	let nodes: FxHashMap<_, DocumentNode> = registry
		.node_instances
		.iter()
		.filter(|(_, node)| node.network == network_id)
		.filter(|(node_id, _)| !export_identity_node_ids.contains(&node_id)) // Exclude identity nodes
		.map(|(&global_id, node)| {
			// Extract the original local ID from attributes
			let local_id = node
				.attributes
				.get(ATTR_ORIGINAL_NODE_ID)
				.and_then(|(value, _)| value.as_u64())
				.unwrap_or(global_id); // Fallback to global ID if not found (for backward compatibility)

			convert_node(registry, node).map(|doc_node| (RuntimeNodeId(local_id), doc_node))
		})
		.collect::<Result<FxHashMap<_, _>, _>>()?;

	// Convert exports by resolving identity nodes.
	// Identity nodes have a single input that points to the actual node to export.
	let exports: Vec<GraphCraftNodeInput> = network
		.exports
		.iter()
		.map(|&identity_node_id| {
			let identity_node = registry.node_instances.get(&identity_node_id).ok_or(ConversionError::NodeNotFound(identity_node_id))?;

			// Identity node should have exactly one input
			let input = identity_node.inputs.first().ok_or(ConversionError::InvalidIdentityNode(identity_node_id))?;
			let empty_attrs = HashMap::new();
			let input_attrs = identity_node.inputs_attributes.first().unwrap_or(&empty_attrs);

			convert_input(registry, input, input_attrs)
		})
		.collect::<Result<Vec<_>, _>>()?;

	// Construct NodeNetwork with ONLY this level's nodes.
	// Any nested networks are embedded in the DocumentNode.implementation.
	Ok(NodeNetwork {
		exports,
		nodes, // Only nodes at this network level (excluding identity nodes)
		// TODO: Support scope injections
		scope_injections: FxHashMap::default(),
		generated: false,
	})
}

/// Converts a Registry Node to a DocumentNode, remapping global IDs to local IDs
fn convert_node(registry: &Registry, node: &crate::Node) -> Result<DocumentNode, ConversionError> {
	// Convert inputs with their associated attributes, remapping node references
	let inputs = node
		.inputs
		.iter()
		.zip(node.inputs_attributes.iter())
		.map(|(input, input_attrs)| convert_input(registry, input, input_attrs))
		.collect::<Result<Vec<_>, _>>()?;

	// Extract call_argument from attributes
	let call_argument = node
		.attributes
		.get(ATTR_CALL_ARGUMENT)
		.and_then(|(value, _timestamp)| serde_json::from_value(value.clone()).ok())
		.unwrap_or_else(|| concrete!(())); // Default to unit type if not found

	// Extract context_features from attributes
	let context_features = node
		.attributes
		.get(ATTR_CONTEXT_FEATURES)
		.and_then(|(value, _timestamp)| serde_json::from_value(value.clone()).ok())
		.unwrap_or_default(); // Default to empty context features if not found

	// Extract visible from attributes
	let visible = node
		.attributes
		.get(ATTR_VISIBLE)
		.and_then(|(value, _timestamp)| serde_json::from_value(value.clone()).ok())
		.unwrap_or(true); // Default to true if not found

	// Extract skip_deduplication from attributes
	let skip_deduplication = node
		.attributes
		.get(ATTR_SKIP_DEDUPLICATION)
		.and_then(|(value, _timestamp)| serde_json::from_value(value.clone()).ok())
		.unwrap_or(false); // Default to false if not found

	Ok(DocumentNode {
		inputs,
		call_argument,
		implementation: convert_implementation(registry, &node.implementation)?,
		visible,
		skip_deduplication,
		context_features,
		// OriginalLocation is generated during compilation, not stored
		original_location: Default::default(),
	})
}

/// Converts a Registry NodeInput to a graph-craft NodeInput, remapping global IDs to local IDs
fn convert_input(registry: &Registry, input: &NodeInput, input_attributes: &crate::Attributes) -> Result<GraphCraftNodeInput, ConversionError> {
	Ok(match input {
		NodeInput::Node { node_id, output_index } => {
			// Look up the referenced node and extract its original local ID
			let referenced_node = registry.node_instances.get(node_id).ok_or(ConversionError::NodeNotFound(*node_id))?;
			let local_id = referenced_node
				.attributes
				.get(ATTR_ORIGINAL_NODE_ID)
				.and_then(|(value, _)| value.as_u64())
				.unwrap_or(*node_id); // Fallback to global ID if not found

			GraphCraftNodeInput::Node {
				node_id: RuntimeNodeId(local_id),
				output_index: *output_index,
			}
		},
		NodeInput::Value { raw_value, exposed } => {
			// Deserialize using postcard - Arc<[u8]> derefs to &[u8]
			let tagged_value: TaggedValue = postcard::from_bytes(raw_value)
				.map_err(|e| ConversionError::DeserializationError(format!("TaggedValue: {:?}", e)))?;
			GraphCraftNodeInput::Value {
				tagged_value: MemoHash::new(tagged_value),
				exposed: *exposed,
			}
		}
		NodeInput::Scope(s) => GraphCraftNodeInput::Scope(s.clone()),
		NodeInput::Import { import_idx } => {
			// Extract import_type from input_attributes if available
			let import_type = input_attributes
				.get(ATTR_IMPORT_TYPE)
				.and_then(|(value, _timestamp)| serde_json::from_value(value.clone()).ok())
				.unwrap_or_else(|| Type::Generic(Cow::Borrowed("T"))); // Default to generic if not found

			GraphCraftNodeInput::Import {
				import_type,
				import_index: *import_idx,
			}
		}
		NodeInput::Reflection => {
			// Extract reflection_metadata from input_attributes
			let metadata = input_attributes
				.get(ATTR_REFLECTION_METADATA)
				.and_then(|(value, _timestamp)| serde_json::from_value(value.clone()).ok())
				.ok_or_else(|| ConversionError::DeserializationError("Missing reflection_metadata in input_attributes".to_string()))?;

			GraphCraftNodeInput::Reflection(metadata)
		}
	})
}

/// Converts a Registry Implementation to a DocumentNodeImplementation
fn convert_implementation(registry: &Registry, implementation: &Implementation) -> Result<DocumentNodeImplementation, ConversionError> {
	Ok(match implementation {
		Implementation::ProtoNode(decl_id) => {
			// Simple case: just convert the identifier
			let proto = registry.node_declarations.get(decl_id).ok_or(ConversionError::DeclarationNotFound(*decl_id))?;
			DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::with_owned_string(proto.identifier.clone()))
		}
		Implementation::Network(net_id) => {
			// Recursive case: convert the referenced network to a full NodeNetwork.
			// This will create a nested NodeNetwork with its own nodes map
			// containing only the nodes where node.network == net_id.
			DocumentNodeImplementation::Network(convert_network(registry, *net_id)?)
		}
	})
}
