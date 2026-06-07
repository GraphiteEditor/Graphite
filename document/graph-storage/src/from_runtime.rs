use std::collections::HashMap;

use core_types::Context;
use core_types::context::ContextDependencies;
use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput as GraphCraftNodeInput, NodeNetwork};
use serde::Serialize;

use crate::attr::*;
use crate::metadata_source::{NoMetadata, NodeMetadataSource};
use crate::{AttributesExt, ExportSlot, Implementation, InputSlot, Network, NetworkId, Node, NodeId, NodeInput, PeerId, ProtoNode, ROOT_NETWORK, Registry, ResourceHash, ResourceId, TimeStamp};

fn map_serialization_error(key: &str) -> impl FnOnce(serde_json::Error) -> ConversionError + '_ {
	move |e| ConversionError::SerializationError(format!("{key}: {e:?}"))
}

/// Path to a node, used to mint stable global IDs by hashing.
///
/// Hashing uses blake3 truncated to 64 bits with the document's `PeerId` mixed in, so two peers
/// converting runtime states that happen to share local IDs (e.g. both editors seeded the same
/// UUID RNG) still produce distinct global IDs. Determinism: same `(peer, path, local_id)` always
/// yields the same global ID, so a peer re-converting its own runtime state preserves IDs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct NodePath {
	path: Vec<(NodeId, NetworkId)>,
	local_id: NodeId,
}

impl NodePath {
	fn root(node_id: NodeId) -> Self {
		Self { path: vec![], local_id: node_id }
	}

	fn nested(parent_path: &NodePath, parent_node_id: NodeId, network_id: NetworkId, local_id: NodeId) -> Self {
		let mut path = parent_path.path.clone();
		path.push((parent_node_id, network_id));
		Self { path, local_id }
	}

	fn to_global_id(&self, peer: PeerId) -> NodeId {
		let bytes = rmp_serde::to_vec(&(peer, self)).expect("NodePath must serialize");
		let digest = blake3::hash(&bytes);
		let mut truncated = [0u8; 8];
		truncated.copy_from_slice(&digest.as_bytes()[..8]);
		NodeId::from_le_bytes(truncated)
	}

	/// Stable id of the network owned by the node at this path, derived purely from the (structural)
	/// path and peer so it reproduces across `to_runtime` -> `from_runtime` round trips rather than
	/// depending on traversal order. A domain tag keeps it from colliding with this node's own
	/// `to_global_id`. The root network is `ROOT_NETWORK` and never goes through here.
	fn owned_network_id(&self, peer: PeerId) -> NetworkId {
		let bytes = rmp_serde::to_vec(&("network", peer, self)).expect("NodePath must serialize");
		let digest = blake3::hash(&bytes);
		let mut truncated = [0u8; 8];
		truncated.copy_from_slice(&digest.as_bytes()[..8]);
		NetworkId::from_le_bytes(truncated)
	}
}

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
	#[error("Failed to serialize value: {0}")]
	SerializationError(String),
	#[error("Unsupported node implementation type")]
	UnsupportedImplementation,
	#[error("Invalid network structure: {0}")]
	InvalidNetwork(String),
}

/// Graph-only conversion (no editor metadata). Use [`Registry::from_runtime_with_metadata`] for
/// editor round-trips.
impl TryFrom<&NodeNetwork> for Registry {
	type Error = ConversionError;

	/// Test/utility entry point: scopes IDs under `PeerId(0)`. Real editor conversions go through
	/// `from_runtime_with_metadata` and pass the document's actual peer.
	fn try_from(node_network: &NodeNetwork) -> Result<Self, Self::Error> {
		Registry::from_runtime_with_metadata(node_network, &NoMetadata, &graphene_resource::ResourceRegistry::new(), PeerId(0))
	}
}

/// Proto-node declaration bytes extracted during conversion, keyed by content hash, for the caller
/// to persist into its byte store.
pub type DeclarationBytes = HashMap<ResourceHash, Vec<u8>>;

/// A `from_runtime` conversion result: the reference-only [`Registry`] plus the proto-node
/// declaration *bytes* it extracted, keyed by content hash. `graph-storage` doesn't own a byte
/// store, so the caller (the `Gdd`) persists these into its content store; the registry only holds
/// the `ResourceId`/`ResourceHash` references.
pub struct RuntimeConversion {
	pub registry: Registry,
	pub declaration_bytes: DeclarationBytes,
	/// Each network's runtime `metadata_path` mapped to its stable storage `NetworkId`, for associating
	/// per-network, per-peer view state (`session.json`) without re-deriving ids.
	pub network_ids: HashMap<Vec<RuntimeNodeId>, NetworkId>,
}

impl RuntimeConversion {
	/// Rebuild the [`Declarations`](crate::Declarations) map (`ResourceId` → [`ProtoNode`]) from the
	/// extracted bytes, for callers that keep the bytes in hand instead of routing them through a
	/// byte store (tests, the round-trip CLI). Editor/`Gdd` paths persist the bytes and resolve via
	/// their byte store instead.
	pub fn declarations(&self) -> Result<crate::Declarations, ConversionError> {
		self.declaration_bytes
			.iter()
			.map(|(hash, bytes)| {
				let proto = decode_declaration(bytes).map_err(|error| ConversionError::SerializationError(format!("declaration {hash}: {error}")))?;
				Ok((ResourceId::from_hash(hash), proto))
			})
			.collect()
	}
}

/// Encode a [`ProtoNode`] declaration to its content-addressed bytes: through a self-describing
/// `serde_json::Value` (so serde aliases keep working and the on-disk shape stays migratable), then
/// rmp-serialized (which encodes the intermediate `Value` compactly). Paired with [`decode_declaration`].
pub fn encode_declaration(proto: &ProtoNode) -> Result<Vec<u8>, String> {
	let value = serde_json::to_value(proto).map_err(|error| error.to_string())?;
	rmp_serde::to_vec(&value).map_err(|error| error.to_string())
}

/// Decode a [`ProtoNode`] declaration from the bytes [`encode_declaration`] produced.
pub fn decode_declaration(bytes: &[u8]) -> Result<ProtoNode, String> {
	let value: serde_json::Value = rmp_serde::from_slice(bytes).map_err(|error| error.to_string())?;
	serde_json::from_value(value).map_err(|error| error.to_string())
}

impl Registry {
	/// Convenience wrapper returning only the registry (declaration bytes discarded). For callers
	/// that don't persist a byte store — e.g. the graph-only `TryFrom` and value-comparison tests.
	pub fn from_runtime_with_metadata<M: NodeMetadataSource>(node_network: &NodeNetwork, metadata: &M, resources: &graphene_resource::ResourceRegistry, peer: PeerId) -> Result<Self, ConversionError> {
		Ok(Self::convert_from_runtime(node_network, metadata, resources, peer)?.registry)
	}

	/// Full conversion: returns the registry and the extracted declaration bytes for the caller to
	/// persist. See [`RuntimeConversion`].
	pub fn convert_from_runtime<M: NodeMetadataSource>(
		node_network: &NodeNetwork,
		metadata: &M,
		resources: &graphene_resource::ResourceRegistry,
		peer: PeerId,
	) -> Result<RuntimeConversion, ConversionError> {
		let mut registry = Registry::default();
		let mut ctx = ConversionContext {
			declaration_ids: HashMap::new(),
			declaration_bytes: HashMap::new(),
			network_ids: HashMap::new(),
			metadata,
			peer,
		};

		convert_network(node_network, ROOT_NETWORK, None, &[], &mut registry, &mut ctx)?;

		// Only snapshot resources the network actually references. The runtime resource cache also keeps
		// resources alive across undo (so legacy redo can restore them), so it can contain orphans whose
		// node was removed by an undo. Snapshotting those would re-introduce an `AddResource` on the next
		// diff and let an undone resource resurface as a phantom edit. Declaration resources are added
		// separately by `convert_network` and are always referenced, so they're unaffected by this filter.
		let referenced = collect_referenced_resources(node_network);
		convert_resources(resources, &referenced, peer, &mut registry)?;

		Ok(RuntimeConversion {
			registry,
			declaration_bytes: ctx.declaration_bytes,
			network_ids: ctx.network_ids,
		})
	}
}

/// Snapshot the runtime [`ResourceRegistry`](graphene_resource::ResourceRegistry) into the storage
/// [`ResourceStore`](crate::ResourceStore). Each source's chain position becomes a fractional
/// [`Priority`](crate::Priority) (index-as-priority preserves order); the `DataSource` body is
/// stored type-erased as `serde_json::Value` so its on-disk shape can migrate freely. All
/// timestamps are `ORIGIN`, since this is a bootstrap snapshot, not an edit.
fn convert_resources(resources: &graphene_resource::ResourceRegistry, referenced: &std::collections::HashSet<ResourceId>, peer: PeerId, registry: &mut Registry) -> Result<(), ConversionError> {
	for id in resources.ids() {
		if !referenced.contains(&id) {
			continue;
		}
		let Some(info) = resources.info(&id) else { continue };

		let mut entry = crate::ResourceEntry {
			hash: info.hash.copied(),
			hash_timestamp: TimeStamp::ORIGIN,
			..Default::default()
		};
		for (position, source) in info.sources.iter().enumerate() {
			let key = crate::SourceKey {
				priority: crate::Priority(position as f64),
				peer,
			};
			let body = serde_json::to_value(source).map_err(|error| ConversionError::SerializationError(error.to_string()))?;
			entry.set_source(
				key,
				crate::SourceValue {
					source: body,
					timestamp: TimeStamp::ORIGIN,
				},
			);
		}

		registry.resources.insert(id, entry);
	}
	Ok(())
}

/// Collect the `ResourceId`s referenced by `TaggedValue::Resource` inputs anywhere in the network
/// (recursively through nested networks). These are the resources the document actually uses; the
/// runtime cache may hold more (history-retained orphans) that shouldn't be snapshotted into storage.
fn collect_referenced_resources(network: &NodeNetwork) -> std::collections::HashSet<ResourceId> {
	let mut referenced = std::collections::HashSet::new();
	collect_referenced_resources_inner(network, &mut referenced);
	referenced
}

fn collect_referenced_resources_inner(network: &NodeNetwork, referenced: &mut std::collections::HashSet<ResourceId>) {
	for export in &network.exports {
		collect_input_resource(export, referenced);
	}

	for node in network.nodes.values() {
		for input in &node.inputs {
			collect_input_resource(input, referenced);
		}
		if let DocumentNodeImplementation::Network(nested) = &node.implementation {
			collect_referenced_resources_inner(nested, referenced);
		}
	}
}

fn collect_input_resource(input: &GraphCraftNodeInput, referenced: &mut std::collections::HashSet<ResourceId>) {
	if let GraphCraftNodeInput::Value { tagged_value, .. } = input
		&& let TaggedValue::Resource(id) = &**tagged_value
	{
		referenced.insert(*id);
	}
}

/// Register a proto-node declaration as a content-addressed resource: a single `DataSource::Embedded`
/// source resolved to `hash`. The bytes themselves are persisted by the caller's byte store.
fn register_declaration_resource(registry: &mut Registry, id: ResourceId, hash: ResourceHash, peer: PeerId) {
	registry.resources.insert(id, crate::ResourceEntry::embedded(hash, peer, TimeStamp::ORIGIN));
}

struct ConversionContext<'m, M: NodeMetadataSource + ?Sized> {
	/// Cache from proto-node identifier to its derived `ResourceId`, so repeated proto-nodes reuse
	/// one id without re-serializing. (Identical content hashes to the same id anyway; this just
	/// skips the work.)
	declaration_ids: HashMap<String, ResourceId>,
	/// Extracted declaration content keyed by hash, handed back for the caller's byte store.
	declaration_bytes: DeclarationBytes,
	/// Maps each network's runtime `metadata_path` to its stable storage `NetworkId`, so the caller can
	/// associate per-network, per-peer view state (in `session.json`) with networks without re-deriving ids.
	network_ids: HashMap<Vec<RuntimeNodeId>, NetworkId>,
	metadata: &'m M,
	peer: PeerId,
}

fn convert_network<M: NodeMetadataSource + ?Sized>(
	node_network: &NodeNetwork,
	network_id: NetworkId,
	parent_path: Option<&NodePath>,
	metadata_path: &[RuntimeNodeId],
	registry: &mut Registry,
	ctx: &mut ConversionContext<'_, M>,
) -> Result<(), ConversionError> {
	for (runtime_node_id, doc_node) in &node_network.nodes {
		let local_id = runtime_node_id.0;
		let node_path = child_path(parent_path, network_id, local_id);
		let global_id = node_path.to_global_id(ctx.peer);

		let location = NodeLocation {
			local_id,
			network_id,
			parent_path,
			metadata_path,
			runtime_node_id: *runtime_node_id,
		};
		let mut node = convert_node(doc_node, location, registry, ctx)?;
		node.attributes.set(ORIGINAL_NODE_ID, serde_json::json!(local_id), TimeStamp::ORIGIN);
		registry.node_instances.insert(global_id, node);
	}

	let exports = node_network
		.exports
		.iter()
		.map(|export| {
			Ok(ExportSlot {
				target: Some(convert_input(export, parent_path, network_id, ctx.peer)?),
				timestamp: TimeStamp::ORIGIN,
			})
		})
		.collect::<Result<Vec<_>, ConversionError>>()?;

	let mut attributes = HashMap::new();
	write_ui_network_attributes(&mut attributes, ctx.metadata, metadata_path, TimeStamp::ORIGIN)?;

	registry.networks.insert(network_id, Network { exports, attributes });
	ctx.network_ids.insert(metadata_path.to_vec(), network_id);

	Ok(())
}

fn child_path(parent_path: Option<&NodePath>, network_id: NetworkId, local_id: NodeId) -> NodePath {
	match parent_path {
		None => NodePath::root(local_id),
		Some(parent) => NodePath::nested(parent, parent.local_id, network_id, local_id),
	}
}

/// Where a node sits in both the storage tree (`local_id`, `network_id`, `parent_path`) and the
/// runtime tree (`metadata_path`, `runtime_node_id`). `metadata_path` is the chain of runtime IDs
/// from the root down to (but not including) this node.
struct NodeLocation<'a> {
	local_id: NodeId,
	network_id: NetworkId,
	parent_path: Option<&'a NodePath>,
	metadata_path: &'a [RuntimeNodeId],
	runtime_node_id: RuntimeNodeId,
}

fn convert_node<M: NodeMetadataSource + ?Sized>(doc_node: &DocumentNode, location: NodeLocation<'_>, registry: &mut Registry, ctx: &mut ConversionContext<'_, M>) -> Result<Node, ConversionError> {
	let NodeLocation {
		local_id,
		network_id,
		parent_path,
		metadata_path,
		runtime_node_id,
	} = location;

	let node_path = child_path(parent_path, network_id, local_id);
	let timestamp = TimeStamp::ORIGIN;

	let mut inputs = Vec::with_capacity(doc_node.inputs.len());
	let mut inputs_attributes = Vec::with_capacity(doc_node.inputs.len());
	for (input_index, input) in doc_node.inputs.iter().enumerate() {
		inputs.push(InputSlot {
			input: convert_input(input, parent_path, network_id, ctx.peer)?,
			timestamp,
		});

		let mut input_attrs = convert_input_attributes(input)?;
		write_ui_input_attributes(&mut input_attrs, ctx.metadata, metadata_path, runtime_node_id, input_index, timestamp)?;
		inputs_attributes.push(input_attrs);
	}

	// For nested networks, append this node onto the metadata path.
	let mut extended_path = Vec::new();
	let child_metadata_path = if matches!(doc_node.implementation, DocumentNodeImplementation::Network(_)) {
		extended_path.extend_from_slice(metadata_path);
		extended_path.push(runtime_node_id);
		extended_path.as_slice()
	} else {
		metadata_path
	};
	let implementation = convert_implementation(&doc_node.implementation, &node_path, child_metadata_path, registry, ctx)?;

	// Defaults match `DocumentNode::default()`; `to_runtime` rehydrates absent keys from the same defaults.
	let mut attributes = HashMap::new();
	attributes
		.set_if_not_default(CALL_ARGUMENT, &doc_node.call_argument, &concrete!(Context), timestamp)
		.map_err(map_serialization_error("call_argument"))?;
	attributes
		.set_if_not_default(CONTEXT_FEATURES, &doc_node.context_features, &ContextDependencies::default(), timestamp)
		.map_err(map_serialization_error("context_features"))?;
	attributes
		.set_if_not_default(VISIBLE, &doc_node.visible, &true, timestamp)
		.map_err(map_serialization_error("visible"))?;
	attributes
		.set_if_not_default(SKIP_DEDUPLICATION, &doc_node.skip_deduplication, &false, timestamp)
		.map_err(map_serialization_error("skip_deduplication"))?;

	write_ui_attributes(&mut attributes, ctx.metadata, metadata_path, runtime_node_id, timestamp)?;

	Ok(Node {
		implementation,
		inputs,
		inputs_attributes,
		attributes,
		network: network_id,
	})
}

fn write_ui_attributes<M: NodeMetadataSource + ?Sized>(
	attributes: &mut crate::Attributes,
	metadata: &M,
	metadata_path: &[RuntimeNodeId],
	runtime_node_id: RuntimeNodeId,
	timestamp: TimeStamp,
) -> Result<(), ConversionError> {
	if let Some(position) = metadata.position(metadata_path, runtime_node_id) {
		attributes.set_serialized(UI_POSITION, &position, timestamp).map_err(map_serialization_error("ui::position"))?;
	}

	// Bool flags are only emitted when true; absence reads as false.
	for (key, value) in [
		(UI_IS_LAYER, metadata.is_layer(metadata_path, runtime_node_id)),
		(UI_LOCKED, metadata.locked(metadata_path, runtime_node_id)),
		(UI_PINNED, metadata.pinned(metadata_path, runtime_node_id)),
	] {
		if value {
			attributes.set(key, serde_json::Value::Bool(true), timestamp);
		}
	}

	if let Some(name) = metadata.display_name(metadata_path, runtime_node_id)
		&& !name.is_empty()
	{
		attributes.set(UI_DISPLAY_NAME, serde_json::Value::String(name.to_string()), timestamp);
	}

	// One whole-vec attribute; per-slot LWW would be overkill for rename-on-output.
	let output_names = metadata.output_names(metadata_path, runtime_node_id);
	if !output_names.is_empty() {
		attributes
			.set_serialized(UI_OUTPUT_NAMES, &output_names, timestamp)
			.map_err(map_serialization_error("ui::output_names"))?;
	}

	Ok(())
}

fn write_ui_network_attributes<M: NodeMetadataSource + ?Sized>(attributes: &mut crate::Attributes, metadata: &M, network_path: &[RuntimeNodeId], timestamp: TimeStamp) -> Result<(), ConversionError> {
	if let Some(reference) = metadata.reference(network_path) {
		attributes.set(UI_REFERENCE, serde_json::Value::String(reference.to_string()), timestamp);
	}

	Ok(())
}

/// Empty strings (the runtime's "unset" sentinel) and absent values are both skipped.
/// `input_data` entries each get their own `ui::input_data::<sub_key>` attribute for per-key LWW.
fn write_ui_input_attributes<M: NodeMetadataSource + ?Sized>(
	attributes: &mut crate::Attributes,
	metadata: &M,
	metadata_path: &[RuntimeNodeId],
	runtime_node_id: RuntimeNodeId,
	input_index: usize,
	timestamp: TimeStamp,
) -> Result<(), ConversionError> {
	let non_empty_string = |key: &'static str, value: Option<&str>, attributes: &mut crate::Attributes| {
		if let Some(value) = value.filter(|s| !s.is_empty()) {
			attributes.set(key, serde_json::Value::String(value.to_string()), timestamp);
		}
	};

	non_empty_string(UI_INPUT_NAME, metadata.input_name(metadata_path, runtime_node_id, input_index), attributes);
	non_empty_string(UI_INPUT_DESCRIPTION, metadata.input_description(metadata_path, runtime_node_id, input_index), attributes);
	non_empty_string(UI_WIDGET_OVERRIDE, metadata.widget_override(metadata_path, runtime_node_id, input_index), attributes);

	for (sub_key, value) in metadata.input_data(metadata_path, runtime_node_id, input_index) {
		attributes.set(&format!("{UI_INPUT_DATA_PREFIX}{sub_key}"), value, timestamp);
	}

	Ok(())
}

fn convert_input(input: &GraphCraftNodeInput, parent_path: Option<&NodePath>, network_id: NetworkId, peer: PeerId) -> Result<NodeInput, ConversionError> {
	Ok(match input {
		GraphCraftNodeInput::Node { node_id, output_index } => NodeInput::Node {
			node_id: child_path(parent_path, network_id, node_id.0).to_global_id(peer),
			output_index: *output_index,
		},
		GraphCraftNodeInput::Value { tagged_value, exposed } => {
			let value = serde_json::to_value(&**tagged_value).map_err(|e| ConversionError::SerializationError(format!("{e:?}")))?;
			NodeInput::Value { value, exposed: *exposed }
		}
		GraphCraftNodeInput::Scope(s) => NodeInput::Scope(s.clone()),
		GraphCraftNodeInput::Import { import_index, .. } => NodeInput::Import { import_idx: *import_index },
		GraphCraftNodeInput::Reflection(_) => NodeInput::Reflection,
		// GPU-specific; not modeled in the Registry format.
		GraphCraftNodeInput::Inline(_) => return Err(ConversionError::UnsupportedImplementation),
	})
}

fn convert_input_attributes(input: &GraphCraftNodeInput) -> Result<crate::Attributes, ConversionError> {
	let mut attributes = HashMap::new();
	let timestamp = TimeStamp::ORIGIN;

	match input {
		GraphCraftNodeInput::Import { import_type, .. } => {
			attributes.set_serialized(IMPORT_TYPE, import_type, timestamp).map_err(map_serialization_error("import_type"))?;
		}
		GraphCraftNodeInput::Reflection(metadata) => {
			attributes
				.set_serialized(REFLECTION_METADATA, metadata, timestamp)
				.map_err(map_serialization_error("reflection_metadata"))?;
		}
		_ => {}
	}

	Ok(attributes)
}

fn convert_implementation<M: NodeMetadataSource + ?Sized>(
	implementation: &DocumentNodeImplementation,
	current_node_path: &NodePath,
	child_metadata_path: &[RuntimeNodeId],
	registry: &mut Registry,
	ctx: &mut ConversionContext<'_, M>,
) -> Result<Implementation, ConversionError> {
	Ok(match implementation {
		DocumentNodeImplementation::ProtoNode(identifier) => {
			let identifier_str = identifier.as_str().to_string();

			// Reuse a previously-converted proto-node's id; identical content hashes to the same id
			// anyway, so this only skips re-serializing.
			if let Some(id) = ctx.declaration_ids.get(&identifier_str) {
				return Ok(Implementation::ProtoNode(*id));
			}

			let proto = ProtoNode {
				identifier: identifier_str.clone(),
				code: None,
				wasm: None,
				attributes: Default::default(),
			};
			// Content-address the declaration: serialize, hash, derive a deterministic id.
			let bytes = encode_declaration(&proto).map_err(|error| ConversionError::SerializationError(format!("proto-node {identifier_str}: {error}")))?;
			let hash = ResourceHash::from(bytes.as_slice());
			let id = ResourceId::from_hash(&hash);

			register_declaration_resource(registry, id, hash, ctx.peer);
			ctx.declaration_bytes.insert(hash, bytes);
			ctx.declaration_ids.insert(identifier_str, id);

			Implementation::ProtoNode(id)
		}
		DocumentNodeImplementation::Network(nested_network) => {
			// Stable, traversal-order-independent id derived from the owning node's path, so a
			// `to_runtime` -> `from_runtime` round trip reproduces the same `NetworkId` (and thus the
			// same node-path hashes underneath it).
			let nested_network_id = current_node_path.owned_network_id(ctx.peer);
			convert_network(nested_network, nested_network_id, Some(current_node_path), child_metadata_path, registry, ctx)?;
			Implementation::Network(nested_network_id)
		}
		// TODO: Support Extract in the Registry format.
		DocumentNodeImplementation::Extract => return Err(ConversionError::UnsupportedImplementation),
	})
}
