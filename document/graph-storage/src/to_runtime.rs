use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

use core_types::memo::MemoHash;
use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NetworkId as RuntimeNetworkId, NodeInput as GraphCraftNodeInput, NodeNetwork, OwnedNetworkLocation};
use graph_craft::{ProtoNodeIdentifier, Type, concrete};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::attr::*;
use crate::metadata_source::{InputMetadataEntry, NetworkMetadataEntry, NodeMetadataEntry};
use crate::{Implementation, NetworkId, Node, NodeId, NodeInput, Position, ProtoNode, ROOT_NETWORK, Registry, ResourceId, Value, from_value};

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
	#[error("Network {0} not found")]
	NetworkNotFound(NetworkId),
	#[error("Node {0} not found")]
	NodeNotFound(NodeId),
	#[error("ProtoNode declaration {0} not found in provided declarations")]
	DeclarationNotFound(ResourceId),
	#[error("Deserialization error: {0}")]
	DeserializationError(String),
	#[error("Network {network} has two nodes mapping to runtime ID {runtime_id}")]
	DuplicateRuntimeNodeId { network: NetworkId, runtime_id: u64 },
	#[error("Network {network} references node {referenced}, which lives in a different network")]
	CrossNetworkReference { network: NetworkId, referenced: NodeId },
	#[error("Scope injection {key:?} in network {network} references node {referenced}, which is missing or in a different network")]
	DanglingScopeInjection { network: NetworkId, key: String, referenced: NodeId },
	#[error("Network {0} is reachable from itself through nested implementations, forming a cycle")]
	CyclicNetwork(NetworkId),
}

/// Resolved proto-node declarations, keyed by the `ResourceId` that `Implementation::ProtoNode`
/// references. The caller resolves these from its byte store (`ResourceId` → `ResourceHash` →
/// stored `ProtoNode` bytes) before converting, since `document-graph-storage` holds only references.
pub type Declarations = std::collections::HashMap<ResourceId, ProtoNode>;

impl Registry {
	/// Returns the network plus one metadata entry per node, since every node carries an identity to
	/// restore even when it has no `ui::*` attribute.
	pub fn to_runtime_with_metadata(&self, declarations: &Declarations) -> Result<(NodeNetwork, Vec<NodeMetadataEntry>), ConversionError> {
		let (network, node_entries, _) = self.to_runtime_with_full_metadata(declarations)?;
		Ok((network, node_entries))
	}

	/// Like `to_runtime_with_metadata` but also returns per-network entries (navigation, previewing).
	/// Used by the editor's full-rebuild path.
	///
	/// Every network the root reaches becomes an entry in the root's definitions table under its storage ID.
	/// The nodes that implement it name it by that ID.
	pub fn to_runtime_with_full_metadata(&self, declarations: &Declarations) -> Result<(NodeNetwork, Vec<NodeMetadataEntry>, Vec<NetworkMetadataEntry>), ConversionError> {
		let mut node_metadata = Vec::new();
		let mut network_metadata = Vec::new();

		let context = ConversionContext::new(self, declarations);

		// Reject cycles first because the runtime cannot inline a definition that contains itself
		detect_network_cycle(&context, ROOT_NETWORK)?;

		let mut network = convert_network(&context, ROOT_NETWORK, &mut node_metadata, &mut network_metadata)?;
		for network_id in context.reachable_definitions() {
			let definition = convert_network(&context, network_id, &mut node_metadata, &mut network_metadata)?;
			network.definitions.insert(runtime_network_id(network_id), definition);
		}

		Ok((network, node_metadata, network_metadata))
	}

	/// Rebuild the runtime [`ResourceRegistry`](graphene_resource::ResourceRegistry) from the stored
	/// `resources`. Each entry's source chain is restored in priority order (the chain is kept
	/// sorted by key) with bodies decoded from their type-erased `Value` form back to
	/// `DataSource`; the resolved hash, if any, is restored last. Inverse of `convert_resources` in
	/// `from_runtime`.
	pub fn to_resource_registry(&self) -> Result<graphene_resource::ResourceRegistry, ConversionError> {
		let mut registry = graphene_resource::ResourceRegistry::new();

		for (id, entry) in &self.resources {
			for (_, source) in entry.live_sources() {
				let decoded: graphene_resource::DataSource = from_value(&source.source).map_err(|error| ConversionError::DeserializationError(error.to_string()))?;
				registry.push_source_back(id, decoded);
			}
			if let Some(hash) = entry.hash {
				registry.resolve(id, hash);
			}
		}

		Ok(registry)
	}
}

/// Immutable shared context threaded through the recursive conversion. `nodes_by_network` is the
/// one-pass grouping of `registry.node_instances` by owning network, so each network's nodes are an
/// O(1) lookup rather than a full rescan.
struct ConversionContext<'a> {
	registry: &'a Registry,
	declarations: &'a Declarations,
	nodes_by_network: FxHashMap<NetworkId, Vec<(NodeId, &'a Node)>>,
}

impl<'a> ConversionContext<'a> {
	fn new(registry: &'a Registry, declarations: &'a Declarations) -> Self {
		let mut nodes_by_network: FxHashMap<NetworkId, Vec<(NodeId, &Node)>> = FxHashMap::default();
		for (&global_id, node) in &registry.node_instances {
			nodes_by_network.entry(node.network).or_default().push((global_id, node));
		}
		Self {
			registry,
			declarations,
			nodes_by_network,
		}
	}

	/// The networks other than the root that the root reaches through node implementations.
	fn reachable_definitions(&self) -> FxHashSet<NetworkId> {
		let mut reached = FxHashSet::default();
		let mut pending = vec![ROOT_NETWORK];

		while let Some(network_id) = pending.pop() {
			for &(_, node) in self.nodes_by_network.get(&network_id).map(Vec::as_slice).unwrap_or_default() {
				if let Implementation::Network(child) = node.implementation
					&& child != ROOT_NETWORK
					&& reached.insert(child)
				{
					pending.push(child);
				}
			}
		}

		reached
	}
}

fn runtime_network_id(id: NetworkId) -> RuntimeNetworkId {
	RuntimeNetworkId(id.0)
}

/// Where the network stored as `id` sits in the runtime. This is the root or an entry in the root's table.
fn runtime_location(id: NetworkId) -> OwnedNetworkLocation {
	match id {
		ROOT_NETWORK => OwnedNetworkLocation::default(),
		id => OwnedNetworkLocation::definition(runtime_network_id(id)),
	}
}

/// The id the node had in the runtime it was converted from, kept as an attribute so a conversion back
/// lands it under the id the editor's metadata is keyed by.
fn runtime_node_id(global_id: NodeId, node: &Node) -> RuntimeNodeId {
	RuntimeNodeId(node.attributes.get_typed(node::ORIGINAL_NODE_ID).unwrap_or(global_id.0))
}

/// A node in runtime form with its metadata entry.
pub struct ProjectedNode {
	/// The runtime location of the network the node sits in.
	pub network: OwnedNetworkLocation,
	pub local_id: RuntimeNodeId,
	pub node: DocumentNode,
	pub metadata: NodeMetadataEntry,
}

/// A network's own runtime form without its nodes, as [`RuntimeProjection::network_entry`] produces it.
pub struct ProjectedNetwork {
	pub network: OwnedNetworkLocation,
	pub exports: Vec<GraphCraftNodeInput>,
	pub scope_injections: FxHashMap<String, (RuntimeNodeId, Type)>,
	pub metadata: NetworkMetadataEntry,
}

/// Converts single registry entities to runtime form, for keeping a runtime mirror in step without
/// rebuilding it.
pub struct RuntimeProjection<'a> {
	context: ConversionContext<'a>,
	reachable: FxHashSet<NetworkId>,
}

impl Registry {
	pub fn runtime_projection<'a>(&'a self, declarations: &'a Declarations) -> RuntimeProjection<'a> {
		let context = ConversionContext::new(self, declarations);
		RuntimeProjection {
			reachable: context.reachable_definitions(),
			context,
		}
	}
}

impl RuntimeProjection<'_> {
	/// The runtime location of `network`, or `None` if the root does not reach it.
	pub fn network_location(&self, network: NetworkId) -> Option<OwnedNetworkLocation> {
		(network == ROOT_NETWORK || self.reachable.contains(&network)).then(|| runtime_location(network))
	}

	/// The location of the node's network in the runtime, and the node's ID in that network.
	pub fn node_address(&self, id: NodeId) -> Option<(OwnedNetworkLocation, RuntimeNodeId)> {
		let node = self.context.registry.node_instances.get(&id)?;
		Some((self.network_location(node.network)?, runtime_node_id(id, node)))
	}

	/// The node converted as a whole-document conversion would.
	pub fn node(&self, id: NodeId) -> Result<ProjectedNode, ConversionError> {
		let node = self.context.registry.node_instances.get(&id).ok_or(ConversionError::NodeNotFound(id))?;
		let network = self.network_location(node.network).ok_or(ConversionError::NetworkNotFound(node.network))?;
		let local_id = runtime_node_id(id, node);

		// Reject a missing or cyclic network, like the whole-document conversion does
		if let Implementation::Network(nested) = node.implementation {
			self.context.registry.network_or_removed(nested).ok_or(ConversionError::NetworkNotFound(nested))?;
			detect_network_cycle(&self.context, nested)?;
		}

		Ok(ProjectedNode {
			node: convert_node(&self.context, node)?,
			metadata: extract_ui_metadata(node, id, network.clone(), local_id),
			network,
			local_id,
		})
	}

	/// The network's exports, scope injections and metadata, without its nodes.
	pub fn network_entry(&self, id: NetworkId) -> Result<ProjectedNetwork, ConversionError> {
		let registry = self.context.registry;
		let network = registry.network_or_removed(id).ok_or(ConversionError::NetworkNotFound(id))?;
		let location = self.network_location(id).ok_or(ConversionError::NetworkNotFound(id))?;

		Ok(ProjectedNetwork {
			metadata: extract_network_metadata(registry, &network.attributes, location.clone(), id),
			scope_injections: read_scope_injections(registry, id, &network.attributes)?,
			exports: convert_exports(registry, id, network)?,
			network: location,
		})
	}
}

/// Rejects any cycle in the graph of `Implementation::Network` references from a network to the networks its nodes implement.
/// The runtime cannot inline a cycle. This is an iterative DFS, and a child already on the active path is a back edge.
fn detect_network_cycle(context: &ConversionContext, root: NetworkId) -> Result<(), ConversionError> {
	// Networks reachable from `root` that referenced networks, used by an embedded node, are pushed in
	// reverse so the natural processing order matches a recursive walk. `Enter`/`Leave` frames let us
	// maintain the gray (active-path) set with an explicit stack.
	enum Frame {
		Enter(NetworkId),
		Leave(NetworkId),
	}

	let mut stack = vec![Frame::Enter(root)];
	let mut on_path: FxHashSet<NetworkId> = FxHashSet::default();
	let mut fully_explored: FxHashSet<NetworkId> = FxHashSet::default();

	while let Some(frame) = stack.pop() {
		match frame {
			Frame::Leave(network_id) => {
				on_path.remove(&network_id);
				fully_explored.insert(network_id);
			}
			Frame::Enter(network_id) => {
				if fully_explored.contains(&network_id) {
					continue;
				}
				if !on_path.insert(network_id) {
					return Err(ConversionError::CyclicNetwork(network_id));
				}

				stack.push(Frame::Leave(network_id));

				for &(_, node) in context.nodes_by_network.get(&network_id).map(Vec::as_slice).unwrap_or_default() {
					if let Implementation::Network(child) = node.implementation {
						stack.push(Frame::Enter(child));
					}
				}
			}
		}
	}

	Ok(())
}

/// Converts a single network, naming the networks its nodes implement by their table entries.
///
/// The registry uses globally hashed IDs, and runtime networks need local IDs.
/// Each node and each `NodeInput::Node` reference gets its local ID back from `attr::ORIGINAL_NODE_ID`.
/// References only point within the same network, so a lookup per network is enough.
///
/// Storage keeps exports in a sparse `Vec<ExportSlot>` where `None` slots are valid.
/// They are compacted here into the runtime's dense `Vec<NodeInput>`, since slot stability only matters in storage.
fn convert_network(
	context: &ConversionContext,
	network_id: NetworkId,
	node_metadata: &mut Vec<NodeMetadataEntry>,
	network_metadata: &mut Vec<NetworkMetadataEntry>,
) -> Result<NodeNetwork, ConversionError> {
	let network = context.registry.network_or_removed(network_id).ok_or(ConversionError::NetworkNotFound(network_id))?;
	let location = runtime_location(network_id);

	network_metadata.push(extract_network_metadata(context.registry, &network.attributes, location.clone(), network_id));

	let mut nodes: FxHashMap<RuntimeNodeId, DocumentNode> = FxHashMap::default();
	for &(global_id, node) in context.nodes_by_network.get(&network_id).map(Vec::as_slice).unwrap_or_default() {
		let runtime_id = runtime_node_id(global_id, node);

		node_metadata.push(extract_ui_metadata(node, global_id, location.clone(), runtime_id));

		// Two storage nodes resolving to the same runtime ID would silently collapse into one on
		// insert, dropping a node from the reconstructed graph.
		if nodes.insert(runtime_id, convert_node(context, node)?).is_some() {
			return Err(ConversionError::DuplicateRuntimeNodeId {
				network: network_id,
				runtime_id: runtime_id.0,
			});
		}
	}

	let exports = convert_exports(context.registry, network_id, network)?;
	let scope_injections = read_scope_injections(context.registry, network_id, &network.attributes)?;

	Ok(NodeNetwork {
		exports,
		nodes,
		scope_injections,
		definitions: FxHashMap::default(),
		generated: false,
	})
}

/// The network's exports, compacted into the runtime's dense list.
fn convert_exports(registry: &Registry, network_id: NetworkId, network: &crate::Network) -> Result<Vec<GraphCraftNodeInput>, ConversionError> {
	// Input attributes aren't round-tripped for exports — Reflection/Import inputs don't appear there.
	let empty_attrs = crate::Attributes::new();
	network
		.exports
		.iter()
		.filter_map(|slot| slot.target.as_ref())
		.map(|input| convert_input(registry, network_id, input, &empty_attrs))
		.collect()
}

/// Rebuild a network's `scope_injections` from its serialized attribute blob, resolving each stored
/// storage node ID back to its runtime-local ID. Mirrors `from_runtime::write_scope_injections`.
fn read_scope_injections(registry: &Registry, network_id: NetworkId, attributes: &crate::Attributes) -> Result<FxHashMap<String, (RuntimeNodeId, Type)>, ConversionError> {
	let Some(stored) = attributes.get_typed::<HashMap<String, (NodeId, Type)>>(network::SCOPE_INJECTIONS) else {
		return Ok(FxHashMap::default());
	};

	stored
		.into_iter()
		.map(|(key, (storage_id, ty))| {
			// The injection must point at a node in this same network, like any `NodeInput::Node`.
			let referenced = registry.node_instances.get(&storage_id).filter(|node| node.network == network_id);
			let Some(referenced) = referenced else {
				return Err(ConversionError::DanglingScopeInjection {
					network: network_id,
					key,
					referenced: storage_id,
				});
			};

			Ok((key, (runtime_node_id(storage_id, referenced), ty)))
		})
		.collect()
}

/// Emitted for every node, since `storage_id` is worth restoring even where no `ui::*` attribute is.
/// `input_metadata` is always sized to match `node.inputs.len()` for a strict slot-by-slot rebuild;
/// empty slots use `InputMetadataEntry::default()`.
fn extract_ui_metadata(node: &crate::Node, storage_id: NodeId, network: OwnedNetworkLocation, local_id: RuntimeNodeId) -> NodeMetadataEntry {
	let position: Option<Position> = node.attributes.get_typed(node::ui::POSITION);
	let is_layer = node.attributes.get_or(node::ui::IS_LAYER, false);
	let display_name: Option<String> = node.attributes.get_typed(node::ui::DISPLAY_NAME);
	let locked = node.attributes.get_or(node::ui::LOCKED, false);
	let pinned = node.attributes.get_or(node::ui::PINNED, false);
	let output_names: Vec<String> = node.attributes.get_or_default(node::ui::OUTPUT_NAMES);
	let output_descriptions: Vec<String> = node.attributes.get_or_default(node::ui::OUTPUT_DESCRIPTIONS);

	let input_metadata: Vec<InputMetadataEntry> = node.inputs.iter().map(|slot| &slot.attributes).map(extract_input_metadata).collect();

	NodeMetadataEntry {
		network,
		local_id,
		storage_id,
		position,
		is_layer,
		display_name,
		locked,
		pinned,
		input_metadata,
		output_names,
		output_descriptions,
	}
}

/// Node references stored here are storage IDs, resolved back to runtime-local IDs the way
/// `read_scope_injections` does. A reference to a node that is no longer in this network is dropped
/// rather than failing the conversion: a pinned node can have been deleted.
fn extract_network_metadata(registry: &Registry, attributes: &crate::Attributes, network: OwnedNetworkLocation, network_id: NetworkId) -> NetworkMetadataEntry {
	let to_runtime_id = |storage_id: NodeId| {
		let node = registry.node_instances.get(&storage_id).filter(|node| node.network == network_id)?;
		Some(runtime_node_id(storage_id, node))
	};

	let pinned_order = attributes
		.get_typed::<Vec<NodeId>>(network::PINNED_ORDER)
		.unwrap_or_default()
		.into_iter()
		.filter_map(to_runtime_id)
		.collect();

	NetworkMetadataEntry {
		network,
		network_id,
		reference: attributes.get_typed(node::ui::REFERENCE),
		pinned_order,
		import_metadata: attributes.get_or_default(network::IMPORT_METADATA),
		export_names: attributes.get_or_default(network::EXPORT_NAMES),
		export_descriptions: attributes.get_or_default(network::EXPORT_DESCRIPTIONS),
	}
}

/// Reassembles `input_data` by scanning every attribute under `ui::input_data::` and stripping the prefix.
fn extract_input_metadata(attributes: &crate::Attributes) -> InputMetadataEntry {
	let input_data: BTreeMap<String, Value> = attributes
		.live()
		.filter_map(|(key, value)| key.strip_prefix(node::input::ui::DATA_PREFIX).map(|sub_key| (sub_key.to_owned(), value.value.clone())))
		.collect();

	InputMetadataEntry {
		input_name: attributes.get_typed(node::input::ui::NAME),
		input_description: attributes.get_typed(node::input::ui::DESCRIPTION),
		widget_override: attributes.get_typed(node::input::ui::WIDGET_OVERRIDE),
		input_data,
	}
}

fn convert_node(context: &ConversionContext, node: &crate::Node) -> Result<DocumentNode, ConversionError> {
	let inputs = node
		.inputs
		.iter()
		.map(|slot| convert_input(context.registry, node.network, &slot.input, &slot.attributes))
		.collect::<Result<Vec<_>, _>>()?;

	// Defaults must match `DocumentNode::default()` (and the `set_if_not_default` calls in `from_runtime`).
	Ok(DocumentNode {
		inputs,
		call_argument: node.attributes.get_or(node::CALL_ARGUMENT, concrete!(core_types::Context)),
		implementation: convert_implementation(context, &node.implementation)?,
		visible: node.attributes.get_or(node::VISIBLE, true),
		skip_deduplication: node.attributes.get_or(node::SKIP_DEDUPLICATION, false),
		// Regenerated during compilation; not stored.
		compile_metadata: None,
		original_location: Default::default(),
	})
}

fn convert_input(registry: &Registry, network_id: NetworkId, input: &NodeInput, input_attributes: &crate::Attributes) -> Result<GraphCraftNodeInput, ConversionError> {
	Ok(match input {
		NodeInput::Node { id: node_id, index: output_index } => {
			// A node removed concurrently keeps its id, as the editor holds a dangling input. So does one whose addition has not
			// arrived yet, though its placeholder knows nothing of its network.
			let referenced = registry.node_or_removed(*node_id).ok_or(ConversionError::NodeNotFound(*node_id))?;
			let placeholder = registry.removed_nodes.get(node_id).is_some_and(|mark| mark.placeholder);

			// Runtime references are local to one network. A cross-network reference would remap to a
			// local ID that doesn't exist in the current runtime network, so reject it.
			if !placeholder && referenced.network != network_id {
				return Err(ConversionError::CrossNetworkReference {
					network: network_id,
					referenced: *node_id,
				});
			}

			GraphCraftNodeInput::Node {
				node_id: runtime_node_id(*node_id, referenced),
				output_index: *output_index as usize,
			}
		}
		NodeInput::Value { value, exposed } => {
			let tagged_value: TaggedValue = from_value(value).map_err(|e| ConversionError::DeserializationError(format!("TaggedValue: {e:?}")))?;
			GraphCraftNodeInput::Value {
				tagged_value: MemoHash::new(tagged_value),
				exposed: *exposed,
			}
		}
		NodeInput::Scope(s) => GraphCraftNodeInput::Scope(s.clone()),
		NodeInput::Import { index: import_idx } => GraphCraftNodeInput::Import {
			import_type: input_attributes.get_or(node::input::IMPORT_TYPE, Type::Generic(Cow::Borrowed("T"))),
			import_index: *import_idx as usize,
		},
		NodeInput::Reflection => GraphCraftNodeInput::Reflection(
			input_attributes
				.get_typed(node::REFLECTION_METADATA)
				.ok_or_else(|| ConversionError::DeserializationError("Missing reflection_metadata in input_attributes".to_string()))?,
		),
		NodeInput::Other => return Err(ConversionError::DeserializationError("Cannot convert NodeInput::Other to a runtime input".to_string())),
	})
}

fn convert_implementation(context: &ConversionContext, implementation: &Implementation) -> Result<DocumentNodeImplementation, ConversionError> {
	Ok(match implementation {
		Implementation::ProtoNode(id) => {
			let proto = context.declarations.get(id).ok_or(ConversionError::DeclarationNotFound(*id))?;
			DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::with_owned_string(proto.identifier.clone()))
		}
		Implementation::Network(network_id) => DocumentNodeImplementation::Definition(runtime_network_id(*network_id)),
	})
}
