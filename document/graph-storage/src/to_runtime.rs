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
use crate::{AttributesRead, Implementation, NetworkId, NodeId, NodeInput, Position, ProtoNode, ROOT_NETWORK, Registry, ResourceId};

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
	#[error("Node {node:?} has {inputs} inputs but {attributes} input-attribute entries")]
	InputAttributeCountMismatch { node: NodeId, inputs: usize, attributes: usize },
	#[error("Network {network} has two nodes mapping to runtime ID {runtime_id}")]
	DuplicateRuntimeNodeId { network: NetworkId, runtime_id: u64 },
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
		let network = convert_network(self, declarations, ROOT_NETWORK, &[], &mut node_metadata, &mut network_metadata)?;
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
	registry: &Registry,
	declarations: &Declarations,
	network_id: NetworkId,
	metadata_path: &[RuntimeNodeId],
	node_collector: &mut Option<Vec<NodeMetadataEntry>>,
	network_collector: &mut Option<Vec<NetworkMetadataEntry>>,
) -> Result<NodeNetwork, ConversionError> {
	let network = registry.networks.get(&network_id).ok_or(ConversionError::NetworkNotFound(network_id))?;

	if let Some(collector) = network_collector.as_mut() {
		collector.push(extract_network_metadata(&network.attributes, metadata_path, network_id));
	}

	let mut nodes: FxHashMap<RuntimeNodeId, DocumentNode> = FxHashMap::default();
	for (&global_id, node) in registry.node_instances.iter().filter(|(_, node)| node.network == network_id) {
		let local_id = node.attributes.get(ORIGINAL_NODE_ID).and_then(|v| v.value.as_u64()).unwrap_or(global_id);
		let runtime_id = RuntimeNodeId(local_id);

		if node.inputs.len() != node.inputs_attributes.len() {
			return Err(ConversionError::InputAttributeCountMismatch {
				node: global_id,
				inputs: node.inputs.len(),
				attributes: node.inputs_attributes.len(),
			});
		}

		if let Some(collector) = node_collector.as_mut()
			&& let Some(entry) = extract_ui_metadata(node, metadata_path, runtime_id)
		{
			collector.push(entry);
		}

		let doc_node = convert_node(registry, declarations, node, metadata_path, runtime_id, node_collector, network_collector)?;

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
		.map(|input| convert_input(registry, input, &empty_attrs))
		.collect::<Result<Vec<_>, _>>()?;

	Ok(NodeNetwork {
		exports,
		nodes,
		// TODO: Support scope injections
		scope_injections: FxHashMap::default(),
		generated: false,
	})
}

/// Returns `None` when the node has no `ui::*` attributes at all so callers don't end up with
/// empty entries for unconverted-from-runtime nodes. `input_metadata` is always sized to match
/// `node.inputs.len()` for a strict slot-by-slot rebuild; empty slots use `InputMetadataEntry::default()`.
fn extract_ui_metadata(node: &crate::Node, network_path: &[RuntimeNodeId], local_id: RuntimeNodeId) -> Option<NodeMetadataEntry> {
	let position: Option<Position> = node.attributes.get_typed(UI_POSITION);
	let is_layer = node.attributes.get_or(UI_IS_LAYER, false);
	let display_name: Option<String> = node.attributes.get_typed(UI_DISPLAY_NAME);
	let locked = node.attributes.get_or(UI_LOCKED, false);
	let pinned = node.attributes.get_or(UI_PINNED, false);
	let output_names: Vec<String> = node.attributes.get_or_default(UI_OUTPUT_NAMES);

	let input_metadata: Vec<InputMetadataEntry> = node.inputs_attributes.iter().map(extract_input_metadata).collect();

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
		reference: attributes.get_typed(UI_REFERENCE),
	}
}

/// Reassembles `input_data` by scanning every attribute under `ui::input_data::` and stripping the prefix.
fn extract_input_metadata(attributes: &crate::Attributes) -> InputMetadataEntry {
	let input_data: HashMap<String, serde_json::Value> = attributes
		.iter()
		.filter_map(|(key, value)| key.strip_prefix(UI_INPUT_DATA_PREFIX).map(|sub_key| (sub_key.to_owned(), value.value.clone())))
		.collect();

	InputMetadataEntry {
		input_name: attributes.get_typed(UI_INPUT_NAME),
		input_description: attributes.get_typed(UI_INPUT_DESCRIPTION),
		widget_override: attributes.get_typed(UI_WIDGET_OVERRIDE),
		input_data,
	}
}

fn convert_node(
	registry: &Registry,
	declarations: &Declarations,
	node: &crate::Node,
	metadata_path: &[RuntimeNodeId],
	runtime_node_id: RuntimeNodeId,
	node_collector: &mut Option<Vec<NodeMetadataEntry>>,
	network_collector: &mut Option<Vec<NetworkMetadataEntry>>,
) -> Result<DocumentNode, ConversionError> {
	let inputs = node
		.inputs
		.iter()
		.zip(node.inputs_attributes.iter())
		.map(|(slot, input_attrs)| convert_input(registry, &slot.input, input_attrs))
		.collect::<Result<Vec<_>, _>>()?;

	// Defaults must match `DocumentNode::default()` (and the `set_if_not_default` calls in `from_runtime`).
	Ok(DocumentNode {
		inputs,
		call_argument: node.attributes.get_or(CALL_ARGUMENT, concrete!(core_types::Context)),
		implementation: convert_implementation(registry, declarations, &node.implementation, metadata_path, runtime_node_id, node_collector, network_collector)?,
		visible: node.attributes.get_or(VISIBLE, true),
		skip_deduplication: node.attributes.get_or(SKIP_DEDUPLICATION, false),
		context_features: node.attributes.get_or_default(CONTEXT_FEATURES),
		// Regenerated during compilation; not stored.
		original_location: Default::default(),
	})
}

fn convert_input(registry: &Registry, input: &NodeInput, input_attributes: &crate::Attributes) -> Result<GraphCraftNodeInput, ConversionError> {
	Ok(match input {
		NodeInput::Node { node_id, output_index } => {
			let referenced = registry.node_instances.get(node_id).ok_or(ConversionError::NodeNotFound(*node_id))?;
			let local_id = referenced.attributes.get(ORIGINAL_NODE_ID).and_then(|v| v.value.as_u64()).unwrap_or(*node_id);
			GraphCraftNodeInput::Node {
				node_id: RuntimeNodeId(local_id),
				output_index: *output_index,
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
		NodeInput::Import { import_idx } => GraphCraftNodeInput::Import {
			import_type: input_attributes.get_or(IMPORT_TYPE, Type::Generic(Cow::Borrowed("T"))),
			import_index: *import_idx,
		},
		NodeInput::Reflection => GraphCraftNodeInput::Reflection(
			input_attributes
				.get_typed(REFLECTION_METADATA)
				.ok_or_else(|| ConversionError::DeserializationError("Missing reflection_metadata in input_attributes".to_string()))?,
		),
	})
}

fn convert_implementation(
	registry: &Registry,
	declarations: &Declarations,
	implementation: &Implementation,
	parent_metadata_path: &[RuntimeNodeId],
	owning_runtime_id: RuntimeNodeId,
	node_collector: &mut Option<Vec<NodeMetadataEntry>>,
	network_collector: &mut Option<Vec<NetworkMetadataEntry>>,
) -> Result<DocumentNodeImplementation, ConversionError> {
	Ok(match implementation {
		Implementation::ProtoNode(id) => {
			let proto = declarations.get(id).ok_or(ConversionError::DeclarationNotFound(*id))?;
			DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::with_owned_string(proto.identifier.clone()))
		}
		Implementation::Network(net_id) => {
			let mut child_path = Vec::with_capacity(parent_metadata_path.len() + 1);
			child_path.extend_from_slice(parent_metadata_path);
			child_path.push(owning_runtime_id);
			DocumentNodeImplementation::Network(convert_network(registry, declarations, *net_id, &child_path, node_collector, network_collector)?)
		}
	})
}
