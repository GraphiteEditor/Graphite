use std::borrow::Cow;
use std::collections::HashMap;

use core_types::memo::MemoHash;
use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput as GraphCraftNodeInput, NodeNetwork};
use graph_craft::{ProtoNodeIdentifier, Type, concrete};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::attr::*;
use crate::metadata_source::{InputMetadataEntry, NetworkMetadataEntry, NodeMetadataEntry};
use crate::{AttributesRead, Implementation, NetworkId, Node, NodeId, NodeInput, Position, ProtoNode, ROOT_NETWORK, Registry, ResourceId};

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
	pub fn to_runtime_with_full_metadata(&self, declarations: &Declarations) -> Result<(NodeNetwork, Vec<NodeMetadataEntry>, Vec<NetworkMetadataEntry>), ConversionError> {
		let mut node_metadata = Some(Vec::new());
		let mut network_metadata = Some(Vec::new());

		// Nodes are grouped by their owning network in one pass, so each `convert_network` call (one per
		// network, including nested ones) takes its node list by lookup instead of rescanning the whole
		// flat `node_instances` map, which would be quadratic on graphs with many networks.
		let context = ConversionContext::new(self, declarations);

		// Reject cycles up front so the recursive conversion below can assume the network reference
		// graph is acyclic and never blow the stack on a self-referential `Implementation::Network`.
		detect_network_cycle(&context, ROOT_NETWORK)?;

		let network = convert_network(&context, ROOT_NETWORK, &[], &mut node_metadata, &mut network_metadata)?;
		Ok((network, node_metadata.expect("seeded above"), network_metadata.expect("seeded above")))
	}

	/// Rebuild the runtime [`ResourceRegistry`](graphene_resource::ResourceRegistry) from the stored
	/// `resources`. Each entry's source chain is restored in priority order (the chain is kept
	/// sorted by key) with bodies decoded from their type-erased `serde_json::Value` form back to
	/// `DataSource`; the resolved hash, if any, is restored last. Inverse of `convert_resources` in
	/// `from_runtime`.
	pub fn to_resource_registry(&self) -> Result<graphene_resource::ResourceRegistry, ConversionError> {
		let mut registry = graphene_resource::ResourceRegistry::new();

		for (id, entry) in &self.resources {
			for (_, source) in &entry.sources {
				let decoded: graphene_resource::DataSource = serde_json::from_value(source.source.clone()).map_err(|error| ConversionError::DeserializationError(error.to_string()))?;
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
}

/// The id the node had in the runtime it was converted from, which storage keeps as an attribute so a
/// conversion back lands the node under the id the editor's metadata is keyed by.
fn runtime_node_id(global_id: NodeId, node: &Node) -> RuntimeNodeId {
	RuntimeNodeId(node.attributes.get(node::ORIGINAL_NODE_ID).and_then(|value| value.value.as_u64()).unwrap_or(global_id.0))
}

/// One node in runtime form together with the metadata for it and everything nested under it, as
/// [`RuntimeProjection::node`] produces it. Entry paths are absolute, the way a whole-document
/// conversion writes them.
pub struct ProjectedNode {
	/// The runtime path of the network the node sits in.
	pub network_path: Vec<RuntimeNodeId>,
	pub local_id: RuntimeNodeId,
	pub node: DocumentNode,
	/// The node's own entry first, then one per nested node.
	pub node_entries: Vec<NodeMetadataEntry>,
	/// One per network the node's implementation nests, outermost first.
	pub network_entries: Vec<NetworkMetadataEntry>,
}

/// A network's own runtime form without its nodes, as [`RuntimeProjection::network_entry`] produces it.
pub struct ProjectedNetwork {
	pub network_path: Vec<RuntimeNodeId>,
	/// Dense, the way the runtime holds them: a storage slot holding nothing is skipped.
	pub exports: Vec<GraphCraftNodeInput>,
	pub scope_injections: FxHashMap<String, (RuntimeNodeId, Type)>,
	pub metadata: NetworkMetadataEntry,
}

/// Converts single entities of the registry to their runtime form, for a caller keeping a runtime
/// mirror in step with the registry rather than rebuilding it. Owner relationships are gathered once at
/// construction so each address lookup is a probe.
pub struct RuntimeProjection<'a> {
	context: ConversionContext<'a>,
	/// The node whose implementation each nested network is.
	owners: FxHashMap<NetworkId, NodeId>,
}

impl Registry {
	pub fn runtime_projection<'a>(&'a self, declarations: &'a Declarations) -> RuntimeProjection<'a> {
		let owners = self
			.node_instances
			.iter()
			.filter_map(|(&id, node)| match node.implementation {
				Implementation::Network(network) => Some((network, id)),
				Implementation::ProtoNode(_) => None,
			})
			.collect();
		RuntimeProjection {
			context: ConversionContext::new(self, declarations),
			owners,
		}
	}
}

impl RuntimeProjection<'_> {
	/// The node implementing `network`, or `None` for the root and for a network no node implements.
	pub fn owner(&self, network: NetworkId) -> Option<NodeId> {
		self.owners.get(&network).copied()
	}

	/// The runtime path of `network`: the nodes implementing it and its ancestors, outermost first.
	/// `None` for a network the root does not reach, which the runtime does not hold.
	pub fn network_path(&self, network: NetworkId) -> Option<Vec<RuntimeNodeId>> {
		let mut chain = Vec::new();
		let mut current = network;
		while current != ROOT_NETWORK {
			let owner = self.owner(current)?;
			let node = self.context.registry.node_instances.get(&owner)?;
			// Owners can form a cycle through concurrent implementation swaps, which the runtime rejects.
			if chain.contains(&owner) {
				return None;
			}
			chain.push(owner);
			current = node.network;
		}

		Some(chain.into_iter().rev().map(|owner| runtime_node_id(owner, &self.context.registry.node_instances[&owner])).collect())
	}

	/// Where the node sits in the runtime: the path of its network, and its id within that network.
	pub fn node_address(&self, id: NodeId) -> Option<(Vec<RuntimeNodeId>, RuntimeNodeId)> {
		let node = self.context.registry.node_instances.get(&id)?;
		Some((self.network_path(node.network)?, runtime_node_id(id, node)))
	}

	/// The node and everything nested under it, converted the way a whole-document conversion converts
	/// them, so a mirror patched with the result matches a rebuild.
	pub fn node(&self, id: NodeId) -> Result<ProjectedNode, ConversionError> {
		let node = self.context.registry.node_instances.get(&id).ok_or(ConversionError::NodeNotFound(id))?;
		let network_path = self.network_path(node.network).ok_or(ConversionError::NetworkNotFound(node.network))?;
		let local_id = runtime_node_id(id, node);

		if let Implementation::Network(nested) = node.implementation {
			detect_network_cycle(&self.context, nested)?;
		}

		let mut node_entries = Some(vec![extract_ui_metadata(node, id, &network_path, local_id)]);
		let mut network_entries = Some(Vec::new());
		let document_node = convert_node(&self.context, node, &network_path, local_id, &mut node_entries, &mut network_entries)?;

		Ok(ProjectedNode {
			network_path,
			local_id,
			node: document_node,
			node_entries: node_entries.expect("seeded above"),
			network_entries: network_entries.expect("seeded above"),
		})
	}

	/// The network's exports, scope injections and metadata, without its nodes.
	pub fn network_entry(&self, id: NetworkId) -> Result<ProjectedNetwork, ConversionError> {
		let registry = self.context.registry;
		let network = registry.networks.get(&id).ok_or(ConversionError::NetworkNotFound(id))?;
		let network_path = self.network_path(id).ok_or(ConversionError::NetworkNotFound(id))?;

		let empty_attrs = crate::Attributes::new();
		let exports = network
			.exports
			.iter()
			.filter_map(|slot| slot.target.as_ref())
			.map(|input| convert_input(registry, id, input, &empty_attrs))
			.collect::<Result<Vec<_>, _>>()?;

		Ok(ProjectedNetwork {
			metadata: extract_network_metadata(registry, &network.attributes, &network_path, id),
			scope_injections: read_scope_injections(registry, id, &network.attributes)?,
			exports,
			network_path,
		})
	}
}

/// Converts a single network. Recurses through `Implementation::Network` owning nodes.
///
/// **ID remapping:** Registry uses globally hashed IDs; runtime networks need local IDs. We pull
/// the original local ID from `attr::ORIGINAL_NODE_ID` on each node and on each `NodeInput::Node`
/// reference. References only point within the same network, so per-network lookup suffices.
///
/// **Exports:** the storage-side `Vec<ExportSlot>` is sparse (`None` slots are valid). Compacted
/// here into the runtime's dense `Vec<NodeInput>` — slot stability is a storage-side concern.
///
/// `metadata_path` is the owning-node chain naming *this* network (empty for the root).
/// Walk the network reference graph (edges are `Implementation::Network` references between a
/// network and the networks its nodes embed) and reject any cycle, so the recursive `convert_network`
/// can't recurse forever and overflow the stack. Iterative DFS with an explicit stack and a gray set
/// for the active path; a child already on the active path is a back edge, i.e. a cycle.
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

fn convert_network(
	context: &ConversionContext,
	network_id: NetworkId,
	metadata_path: &[RuntimeNodeId],
	node_collector: &mut Option<Vec<NodeMetadataEntry>>,
	network_collector: &mut Option<Vec<NetworkMetadataEntry>>,
) -> Result<NodeNetwork, ConversionError> {
	let network = context.registry.networks.get(&network_id).ok_or(ConversionError::NetworkNotFound(network_id))?;

	if let Some(collector) = network_collector.as_mut() {
		collector.push(extract_network_metadata(context.registry, &network.attributes, metadata_path, network_id));
	}

	let mut nodes: FxHashMap<RuntimeNodeId, DocumentNode> = FxHashMap::default();
	for &(global_id, node) in context.nodes_by_network.get(&network_id).map(Vec::as_slice).unwrap_or_default() {
		let runtime_id = runtime_node_id(global_id, node);
		let local_id = runtime_id.0;

		if let Some(collector) = node_collector.as_mut() {
			collector.push(extract_ui_metadata(node, global_id, metadata_path, runtime_id));
		}

		let doc_node = convert_node(context, node, metadata_path, runtime_id, node_collector, network_collector)?;

		// Two storage nodes resolving to the same runtime ID would silently collapse into one on
		// insert, dropping a node from the reconstructed graph.
		if nodes.insert(runtime_id, doc_node).is_some() {
			return Err(ConversionError::DuplicateRuntimeNodeId {
				network: network_id,
				runtime_id: local_id,
			});
		}
	}

	// Input attributes aren't round-tripped for exports — Reflection/Import inputs don't appear there.
	let empty_attrs = crate::Attributes::new();
	let exports: Vec<GraphCraftNodeInput> = network
		.exports
		.iter()
		.filter_map(|slot| slot.target.as_ref())
		.map(|input| convert_input(context.registry, network_id, input, &empty_attrs))
		.collect::<Result<Vec<_>, _>>()?;

	let scope_injections = read_scope_injections(context.registry, network_id, &network.attributes)?;

	Ok(NodeNetwork {
		exports,
		nodes,
		scope_injections,
		generated: false,
	})
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
fn extract_ui_metadata(node: &crate::Node, storage_id: NodeId, network_path: &[RuntimeNodeId], local_id: RuntimeNodeId) -> NodeMetadataEntry {
	let position: Option<Position> = node.attributes.get_typed(node::ui::POSITION);
	let is_layer = node.attributes.get_or(node::ui::IS_LAYER, false);
	let display_name: Option<String> = node.attributes.get_typed(node::ui::DISPLAY_NAME);
	let locked = node.attributes.get_or(node::ui::LOCKED, false);
	let pinned = node.attributes.get_or(node::ui::PINNED, false);
	let output_names: Vec<String> = node.attributes.get_or_default(node::ui::OUTPUT_NAMES);

	let input_metadata: Vec<InputMetadataEntry> = node.inputs.iter().map(|slot| &slot.attributes).map(extract_input_metadata).collect();

	NodeMetadataEntry {
		network_path: network_path.to_vec(),
		local_id,
		storage_id,
		position,
		is_layer,
		display_name,
		locked,
		pinned,
		input_metadata,
		output_names,
	}
}

/// Node references stored here are storage IDs, resolved back to runtime-local IDs the way
/// `read_scope_injections` does. A reference to a node that is no longer in this network is dropped
/// rather than failing the conversion: a pinned node can have been deleted.
fn extract_network_metadata(registry: &Registry, attributes: &crate::Attributes, network_path: &[RuntimeNodeId], network_id: NetworkId) -> NetworkMetadataEntry {
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
		network_path: network_path.to_vec(),
		network_id,
		reference: attributes.get_typed(node::ui::REFERENCE),
		pinned_order,
	}
}

/// Reassembles `input_data` by scanning every attribute under `ui::input_data::` and stripping the prefix.
fn extract_input_metadata(attributes: &crate::Attributes) -> InputMetadataEntry {
	let input_data: HashMap<String, serde_json::Value> = attributes
		.iter()
		.filter_map(|(key, value)| key.strip_prefix(node::input::ui::DATA_PREFIX).map(|sub_key| (sub_key.to_owned(), value.value.clone())))
		.collect();

	InputMetadataEntry {
		input_name: attributes.get_typed(node::input::ui::NAME),
		input_description: attributes.get_typed(node::input::ui::DESCRIPTION),
		widget_override: attributes.get_typed(node::input::ui::WIDGET_OVERRIDE),
		input_data,
	}
}

fn convert_node(
	context: &ConversionContext,
	node: &crate::Node,
	metadata_path: &[RuntimeNodeId],
	runtime_node_id: RuntimeNodeId,
	node_collector: &mut Option<Vec<NodeMetadataEntry>>,
	network_collector: &mut Option<Vec<NetworkMetadataEntry>>,
) -> Result<DocumentNode, ConversionError> {
	let inputs = node
		.inputs
		.iter()
		.map(|slot| convert_input(context.registry, node.network, &slot.input, &slot.attributes))
		.collect::<Result<Vec<_>, _>>()?;

	// Defaults must match `DocumentNode::default()` (and the `set_if_not_default` calls in `from_runtime`).
	Ok(DocumentNode {
		inputs,
		call_argument: node.attributes.get_or(node::CALL_ARGUMENT, concrete!(core_types::Context)),
		implementation: convert_implementation(context, &node.implementation, metadata_path, runtime_node_id, node_collector, network_collector)?,
		visible: node.attributes.get_or(node::VISIBLE, true),
		skip_deduplication: node.attributes.get_or(node::SKIP_DEDUPLICATION, false),
		context_features: node.attributes.get_or_default(node::CONTEXT_FEATURES),
		// Regenerated during compilation; not stored.
		original_location: Default::default(),
	})
}

fn convert_input(registry: &Registry, network_id: NetworkId, input: &NodeInput, input_attributes: &crate::Attributes) -> Result<GraphCraftNodeInput, ConversionError> {
	Ok(match input {
		NodeInput::Node { id: node_id, index: output_index } => {
			let referenced = registry.node_instances.get(node_id).ok_or(ConversionError::NodeNotFound(*node_id))?;

			// Runtime references are local to one network. A cross-network reference would remap to a
			// local ID that doesn't exist in the current runtime network, so reject it.
			if referenced.network != network_id {
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
			let tagged_value: TaggedValue = serde_json::from_value(value.clone()).map_err(|e| ConversionError::DeserializationError(format!("TaggedValue: {e:?}")))?;
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

fn convert_implementation(
	context: &ConversionContext,
	implementation: &Implementation,
	parent_metadata_path: &[RuntimeNodeId],
	owning_runtime_id: RuntimeNodeId,
	node_collector: &mut Option<Vec<NodeMetadataEntry>>,
	network_collector: &mut Option<Vec<NetworkMetadataEntry>>,
) -> Result<DocumentNodeImplementation, ConversionError> {
	Ok(match implementation {
		Implementation::ProtoNode(id) => {
			let proto = context.declarations.get(id).ok_or(ConversionError::DeclarationNotFound(*id))?;
			DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::with_owned_string(proto.identifier.clone()))
		}
		Implementation::Network(net_id) => {
			let mut child_path = Vec::with_capacity(parent_metadata_path.len() + 1);
			child_path.extend_from_slice(parent_metadata_path);
			child_path.push(owning_runtime_id);
			DocumentNodeImplementation::Network(convert_network(context, *net_id, &child_path, node_collector, network_collector)?)
		}
	})
}
