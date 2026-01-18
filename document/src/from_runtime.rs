use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput as GraphCraftNodeInput, NodeNetwork};

use crate::{DeclarationId, Implementation, Network, NetworkId, Node, NodeId, NodeInput, ProtoNode, Registry, ATTR_CALL_ARGUMENT, ATTR_CONTEXT_FEATURES, ATTR_IMPORT_TYPE, ATTR_VISIBLE, ATTR_SKIP_DEDUPLICATION, ATTR_REFLECTION_METADATA, ATTR_ORIGINAL_NODE_ID};

/// Represents a path to a node in the document structure for generating stable IDs
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct NodePath {
	/// Sequence of (node_id, network_id) pairs from root to this node
	/// For root network nodes, this is empty and we use the original ID
	path: Vec<(NodeId, NetworkId)>,
	/// The local ID of this node within its network
	local_id: NodeId,
}

impl NodePath {
	/// Create a path for a root network node
	fn root(node_id: NodeId) -> Self {
		Self {
			path: vec![],
			local_id: node_id,
		}
	}

	/// Create a path for a nested network node
	fn nested(parent_path: &NodePath, parent_node_id: NodeId, network_id: NetworkId, local_id: NodeId) -> Self {
		let mut path = parent_path.path.clone();
		path.push((parent_node_id, network_id));
		Self { path, local_id }
	}

	/// Generate a stable, globally unique ID by hashing this path
	fn to_global_id(&self) -> NodeId {
		// For root network nodes (empty path), use the original ID
		if self.path.is_empty() {
			return self.local_id;
		}

		// For nested nodes, hash the entire path
		let mut hasher = std::collections::hash_map::DefaultHasher::new();
		self.hash(&mut hasher);
		hasher.finish()
	}
}

/// Errors that can occur during conversion from NodeNetwork to Registry
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
	#[error("Failed to serialize value: {0}")]
	SerializationError(String),
	#[error("Unsupported node implementation type")]
	UnsupportedImplementation,
	#[error("Invalid network structure: {0}")]
	InvalidNetwork(String),
}

/// Converts a NodeNetwork back to a Registry.
///
/// ## Identity Node Creation
///
/// Since Registry uses identity nodes for network exports, this conversion:
/// 1. Creates identity nodes for each export in the network
/// 2. Adds these identity nodes to the network's nodes
/// 3. Updates the network exports to reference these identity nodes
///
/// ## Nested Structure Flattening
///
/// The conversion flattens nested networks:
/// - Each nested NodeNetwork is assigned a unique NetworkId
/// - All nodes (including from nested networks) are added to the flat registry.node_instances map
/// - Each node records which network it belongs to via the `network` field
impl TryFrom<&NodeNetwork> for Registry {
	type Error = ConversionError;

	fn try_from(node_network: &NodeNetwork) -> Result<Self, Self::Error> {
		convert_node_network(node_network)
	}
}

/// Converts a NodeNetwork to a Registry by flattening the nested structure
fn convert_node_network(node_network: &NodeNetwork) -> Result<Registry, ConversionError> {
	let mut registry = Registry {
		node_declarations: HashMap::new(),
		node_instances: HashMap::new(),
		networks: HashMap::new(),
		exported_nodes: vec![],
	};

	// Start with network ID 0 for the root network
	let root_network_id = 0;

	// Track the next available IDs
	let mut next_network_id = 1;
	let mut next_decl_id = 0;

	// Track proto node identifiers to declaration IDs
	let mut proto_node_map: HashMap<String, DeclarationId> = HashMap::new();

	// Root network has no parent path
	let root_parent_path = None;

	// Convert the root network
	let identity_node_ids = convert_network(
		node_network,
		root_network_id,
		root_parent_path,
		&mut registry,
		&mut next_network_id,
		&mut next_decl_id,
		&mut proto_node_map,
	)?;

	// Set the exported_nodes to the identity nodes
	registry.exported_nodes = identity_node_ids;

	Ok(registry)
}

/// Converts a single network and returns the identity node IDs for its exports
fn convert_network(
	node_network: &NodeNetwork,
	network_id: NetworkId,
	parent_path: Option<&NodePath>,
	registry: &mut Registry,
	next_network_id: &mut NetworkId,
	next_decl_id: &mut DeclarationId,
	proto_node_map: &mut HashMap<String, DeclarationId>,
) -> Result<Vec<NodeId>, ConversionError> {
	// Convert all nodes in this network
	for (runtime_node_id, doc_node) in &node_network.nodes {
		let local_id = runtime_node_id.0;

		// Generate a stable global ID based on the node's path
		let node_path = match parent_path {
			None => NodePath::root(local_id),
			Some(parent) => NodePath::nested(parent, parent.local_id, network_id, local_id),
		};
		let global_id = node_path.to_global_id();

		let mut node = convert_node(doc_node, local_id, network_id, parent_path, registry, next_network_id, next_decl_id, proto_node_map)?;

		// Store the original local ID in attributes for reconstruction
		let timestamp = 0;
		node.attributes.insert(
			ATTR_ORIGINAL_NODE_ID.to_string(),
			(serde_json::json!(local_id), timestamp)
		);

		registry.node_instances.insert(global_id, node);
	}

	// Create identity nodes for each export
	let mut identity_node_ids = Vec::new();
	for (export_idx, export) in node_network.exports.iter().enumerate() {
		// Generate a stable ID for the identity node based on its position
		// Identity nodes are synthetic, so we use a special naming scheme
		let identity_local_id = u64::MAX - export_idx as u64; // Use high IDs to avoid collisions
		let identity_path = match parent_path {
			None => NodePath::root(identity_local_id),
			Some(parent) => NodePath::nested(parent, parent.local_id, network_id, identity_local_id),
		};
		let identity_node_id = identity_path.to_global_id();

		// Get or create the identity proto node declaration
		let identity_decl_id = proto_node_map
			.entry("graphene_core::ops::identity::IdentityNode".to_string())
			.or_insert_with(|| {
				let decl_id = *next_decl_id;
				*next_decl_id += 1;
				registry.node_declarations.insert(
					decl_id,
					ProtoNode {
						identifier: "graphene_core::ops::identity::IdentityNode".to_string(),
						code: None,
						wasm: None,
						attributes: Default::default(),
					},
				);
				decl_id
			});

		// Create the identity node with the export as its input (remapping node references)
		let identity_node = Node {
			implementation: Implementation::ProtoNode(*identity_decl_id),
			inputs: vec![convert_input(export, parent_path, network_id)?],
			inputs_attributes: vec![],
			attributes: Default::default(),
			network: network_id,
		};

		registry.node_instances.insert(identity_node_id, identity_node);
		identity_node_ids.push(identity_node_id);
	}

	// Register the network with identity nodes as exports
	registry.networks.insert(
		network_id,
		Network {
			exports: identity_node_ids.clone(),
		},
	);

	Ok(identity_node_ids)
}

/// Converts a DocumentNode to a Registry Node
fn convert_node(
	doc_node: &DocumentNode,
	local_id: NodeId,
	network_id: NetworkId,
	parent_path: Option<&NodePath>,
	registry: &mut Registry,
	next_network_id: &mut NetworkId,
	next_decl_id: &mut DeclarationId,
	proto_node_map: &mut HashMap<String, DeclarationId>,
) -> Result<Node, ConversionError> {
	// Construct this node's full path
	let node_path = match parent_path {
		None => NodePath::root(local_id),
		Some(parent) => NodePath::nested(parent, parent.local_id, network_id, local_id),
	};

	// Convert inputs, tracking their attributes
	let mut inputs = Vec::new();
	let mut inputs_attributes = Vec::new();

	for input in &doc_node.inputs {
		inputs.push(convert_input(input, parent_path, network_id)?);
		inputs_attributes.push(convert_input_attributes(input)?);
	}

	// Convert implementation (pass this node's path for nested networks)
	let implementation = convert_implementation(&doc_node.implementation, &node_path, registry, next_network_id, next_decl_id, proto_node_map)?;

	// Store DocumentNode metadata in attributes for lossless conversion
	let mut attributes = HashMap::new();
	// TODO: Implement proper timestamp management for attributes.
	// For initial conversion from NodeNetwork, we use timestamp 0.
	// The CRDT system will manage timestamps when applying deltas.
	let timestamp = 0;

	// Store call_argument
	let serialized_call_arg = serde_json::to_value(&doc_node.call_argument)
		.map_err(|e| ConversionError::SerializationError(format!("call_argument: {:?}", e)))?;
	attributes.insert(ATTR_CALL_ARGUMENT.to_string(), (serialized_call_arg, timestamp));

	// Store context_features
	let serialized_context = serde_json::to_value(&doc_node.context_features)
		.map_err(|e| ConversionError::SerializationError(format!("context_features: {:?}", e)))?;
	attributes.insert(ATTR_CONTEXT_FEATURES.to_string(), (serialized_context, timestamp));

	// Store visible
	let serialized_visible = serde_json::to_value(&doc_node.visible)
		.map_err(|e| ConversionError::SerializationError(format!("visible: {:?}", e)))?;
	attributes.insert(ATTR_VISIBLE.to_string(), (serialized_visible, timestamp));

	// Store skip_deduplication
	let serialized_skip_dedup = serde_json::to_value(&doc_node.skip_deduplication)
		.map_err(|e| ConversionError::SerializationError(format!("skip_deduplication: {:?}", e)))?;
	attributes.insert(ATTR_SKIP_DEDUPLICATION.to_string(), (serialized_skip_dedup, timestamp));

	Ok(Node {
		implementation,
		inputs,
		inputs_attributes,
		attributes,
		network: network_id,
	})
}

/// Converts a graph-craft NodeInput to a Registry NodeInput, remapping node IDs to global IDs
fn convert_input(input: &GraphCraftNodeInput, parent_path: Option<&NodePath>, network_id: NetworkId) -> Result<NodeInput, ConversionError> {
	Ok(match input {
		GraphCraftNodeInput::Node { node_id, output_index } => {
			// Remap the local node ID to its global hashed ID
			let local_id = node_id.0;
			let node_path = match parent_path {
				None => NodePath::root(local_id),
				Some(parent) => NodePath::nested(parent, parent.local_id, network_id, local_id),
			};
			let global_id = node_path.to_global_id();

			NodeInput::Node {
				node_id: global_id,
				output_index: *output_index,
			}
		},
		GraphCraftNodeInput::Value { tagged_value, exposed } => {
			// Serialize the TaggedValue using postcard
			let serialized = postcard::to_stdvec(&**tagged_value).map_err(|e| ConversionError::SerializationError(format!("{:?}", e)))?;
			NodeInput::Value {
				raw_value: Arc::from(serialized.into_boxed_slice()),
				exposed: *exposed,
			}
		}
		GraphCraftNodeInput::Scope(s) => NodeInput::Scope(s.clone()),
		GraphCraftNodeInput::Import { import_index, .. } => NodeInput::Import { import_idx: *import_index },
		GraphCraftNodeInput::Reflection(_) => {
			// The DocumentNodeMetadata is stored in input_attributes, this is just a marker
			NodeInput::Reflection
		}
		GraphCraftNodeInput::Inline(_) => {
			// Inline is not supported in the Registry format (GPU-specific)
			return Err(ConversionError::UnsupportedImplementation);
		}
	})
}

/// Extracts input metadata and stores it in attributes for lossless conversion
fn convert_input_attributes(input: &GraphCraftNodeInput) -> Result<crate::Attributes, ConversionError> {
	let mut attributes = HashMap::new();
	// TODO: Implement proper timestamp management for attributes.
	// For initial conversion from NodeNetwork, we use timestamp 0.
	let timestamp = 0;

	// Store import_type for Import inputs
	if let GraphCraftNodeInput::Import { import_type, .. } = input {
		let serialized_type = serde_json::to_value(import_type)
			.map_err(|e| ConversionError::SerializationError(format!("import_type: {:?}", e)))?;
		attributes.insert(ATTR_IMPORT_TYPE.to_string(), (serialized_type, timestamp));
	}

	// Store reflection_metadata for Reflection inputs
	if let GraphCraftNodeInput::Reflection(metadata) = input {
		let serialized_metadata = serde_json::to_value(metadata)
			.map_err(|e| ConversionError::SerializationError(format!("reflection_metadata: {:?}", e)))?;
		attributes.insert(ATTR_REFLECTION_METADATA.to_string(), (serialized_metadata, timestamp));
	}

	Ok(attributes)
}

/// Converts a DocumentNodeImplementation to a Registry Implementation
fn convert_implementation(
	implementation: &DocumentNodeImplementation,
	current_node_path: &NodePath,
	registry: &mut Registry,
	next_network_id: &mut NetworkId,
	next_decl_id: &mut DeclarationId,
	proto_node_map: &mut HashMap<String, DeclarationId>,
) -> Result<Implementation, ConversionError> {
	Ok(match implementation {
		DocumentNodeImplementation::ProtoNode(identifier) => {
			// Get or create a declaration for this proto node
			let identifier_str = identifier.as_str().to_string();
			let decl_id = proto_node_map.entry(identifier_str.clone()).or_insert_with(|| {
				let decl_id = *next_decl_id;
				*next_decl_id += 1;
				registry.node_declarations.insert(
					decl_id,
					ProtoNode {
						identifier: identifier_str,
						code: None,
						wasm: None,
						attributes: Default::default(),
					},
				);
				decl_id
			});
			Implementation::ProtoNode(*decl_id)
		}
		DocumentNodeImplementation::Network(nested_network) => {
			// Recursively convert the nested network
			// The current node becomes the parent for nodes in the nested network
			let nested_network_id = *next_network_id;
			*next_network_id += 1;

			convert_network(nested_network, nested_network_id, Some(current_node_path), registry, next_network_id, next_decl_id, proto_node_map)?;

			Implementation::Network(nested_network_id)
		}
		DocumentNodeImplementation::Extract => {
			// Extract nodes are not supported in the Registry format yet
			return Err(ConversionError::UnsupportedImplementation);
		}
	})
}
