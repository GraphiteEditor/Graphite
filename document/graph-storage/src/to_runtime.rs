use std::borrow::Cow;
use std::collections::HashMap;

use core_types::memo::MemoHash;
use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput as GraphCraftNodeInput, NodeNetwork};
use graph_craft::{ProtoNodeIdentifier, Type, concrete};
use rustc_hash::FxHashMap;

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
}

/// Resolved proto-node declarations, keyed by the `ResourceId` that `Implementation::ProtoNode`
/// references. The caller resolves these from its byte store (`ResourceId` → `ResourceHash` →
/// stored `ProtoNode` bytes) before converting, since `graph-storage` holds only references.
pub type Declarations = std::collections::HashMap<ResourceId, ProtoNode>;

impl Registry {
	/// Returns the network plus per-node metadata entries (one per node carrying any `ui::*` attribute).
	pub fn to_runtime_with_metadata(&self, declarations: &Declarations) -> Result<(NodeNetwork, Vec<NodeMetadataEntry>), ConversionError> {
		let (network, node_entries, _) = self.to_runtime_with_full_metadata(declarations)?;
		Ok((network, node_entries))
	}

	/// Like `to_runtime_with_metadata` but also returns per-network entries (navigation, previewing).
	/// Used by the editor's full-rebuild path.
	pub fn to_runtime_with_full_metadata(&self, declarations: &Declarations) -> Result<(NodeNetwork, Vec<NodeMetadataEntry>, Vec<NetworkMetadataEntry>), ConversionError> {
		let mut node_metadata = Some(Vec::new());
		let mut network_metadata = Some(Vec::new());

		// Group nodes by their owning network in one pass, so each `convert_network` call (one per
		// network, including nested ones) takes its node list by lookup instead of rescanning the whole
		// flat `node_instances` map, which would be quadratic on graphs with many networks.
		let mut nodes_by_network: FxHashMap<NetworkId, Vec<(NodeId, &Node)>> = FxHashMap::default();
		for (&global_id, node) in &self.node_instances {
			nodes_by_network.entry(node.network).or_default().push((global_id, node));
		}

		let context = ConversionContext {
			registry: self,
			declarations,
			nodes_by_network,
		};
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
fn convert_network(
	context: &ConversionContext,
	network_id: NetworkId,
	metadata_path: &[RuntimeNodeId],
	node_collector: &mut Option<Vec<NodeMetadataEntry>>,
	network_collector: &mut Option<Vec<NetworkMetadataEntry>>,
) -> Result<NodeNetwork, ConversionError> {
	let network = context.registry.networks.get(&network_id).ok_or(ConversionError::NetworkNotFound(network_id))?;

	if let Some(collector) = network_collector.as_mut() {
		collector.push(extract_network_metadata(&network.attributes, metadata_path, network_id));
	}

	let mut nodes: FxHashMap<RuntimeNodeId, DocumentNode> = FxHashMap::default();
	for &(global_id, node) in context.nodes_by_network.get(&network_id).map(Vec::as_slice).unwrap_or_default() {
		let local_id = node.attributes.get(node::ORIGINAL_NODE_ID).and_then(|v| v.value.as_u64()).unwrap_or(global_id);
		let runtime_id = RuntimeNodeId(local_id);

		if let Some(collector) = node_collector.as_mut()
			&& let Some(entry) = extract_ui_metadata(node, metadata_path, runtime_id)
		{
			collector.push(entry);
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

			let local_id = referenced.attributes.get(node::ORIGINAL_NODE_ID).and_then(|v| v.value.as_u64()).unwrap_or(storage_id);
			Ok((key, (RuntimeNodeId(local_id), ty)))
		})
		.collect()
}

/// Returns `None` when the node has no `ui::*` attributes at all so callers don't end up with
/// empty entries for unconverted-from-runtime nodes. `input_metadata` is always sized to match
/// `node.inputs.len()` for a strict slot-by-slot rebuild; empty slots use `InputMetadataEntry::default()`.
fn extract_ui_metadata(node: &crate::Node, network_path: &[RuntimeNodeId], local_id: RuntimeNodeId) -> Option<NodeMetadataEntry> {
	let position: Option<Position> = node.attributes.get_typed(node::ui::POSITION);
	let is_layer = node.attributes.get_or(node::ui::IS_LAYER, false);
	let display_name: Option<String> = node.attributes.get_typed(node::ui::DISPLAY_NAME);
	let locked = node.attributes.get_or(node::ui::LOCKED, false);
	let pinned = node.attributes.get_or(node::ui::PINNED, false);
	let output_names: Vec<String> = node.attributes.get_or_default(node::ui::OUTPUT_NAMES);

	let input_metadata: Vec<InputMetadataEntry> = node.inputs.iter().map(|slot| &slot.attributes).map(extract_input_metadata).collect();

	let entry = NodeMetadataEntry {
		network_path: network_path.to_vec(),
		local_id,
		position,
		is_layer,
		display_name,
		locked,
		pinned,
		input_metadata,
		output_names,
	};
	(!entry.is_empty()).then_some(entry)
}

fn extract_network_metadata(attributes: &crate::Attributes, network_path: &[RuntimeNodeId], network_id: NetworkId) -> NetworkMetadataEntry {
	NetworkMetadataEntry {
		network_path: network_path.to_vec(),
		network_id,
		reference: attributes.get_typed(node::ui::REFERENCE),
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

			let local_id = referenced.attributes.get(node::ORIGINAL_NODE_ID).and_then(|v| v.value.as_u64()).unwrap_or(*node_id);
			GraphCraftNodeInput::Node {
				node_id: RuntimeNodeId(local_id),
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
		NodeInput::Other => unreachable!(),
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
