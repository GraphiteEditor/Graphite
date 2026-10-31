use std::collections::HashMap;

use core_types::Context;
use core_types::registry::NODE_METADATA;
use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NetworkId as RuntimeNetworkId, NetworkLocation, NodeInput as GraphCraftNodeInput, NodeNetwork, OwnedNetworkLocation};
use serde::Serialize;

use crate::attr::*;
use crate::metadata_source::{NoMetadata, NodeMetadataSource};
use crate::{
	ExportSlot, Implementation, InputSlot, Network, NetworkId, Node, NodeId, NodeInput, PeerId, ProtoNode, ROOT_NETWORK, Registry, ResourceHash, ResourceId, TimeStamp, Value, ValueError, from_value,
	to_value,
};

fn map_serialization_error(key: &str) -> impl FnOnce(ValueError) -> ConversionError + '_ {
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
	#[serde(skip_serializing_if = "Option::is_none")]
	definition: Option<NetworkId>,
	path: Vec<(RuntimeNodeId, NetworkId)>,
	local_id: RuntimeNodeId,
}

impl NodePath {
	fn nested(parent_path: &NodePath, parent_node_id: RuntimeNodeId, network_id: NetworkId, local_id: RuntimeNodeId) -> Self {
		let mut path = parent_path.path.clone();
		path.push((parent_node_id, network_id));
		Self {
			definition: parent_path.definition,
			path,
			local_id,
		}
	}

	fn to_global_id(&self, peer: PeerId) -> NodeId {
		let bytes = postcard::to_stdvec(&(peer, self)).expect("NodePath must serialize");
		let digest = blake3::hash(&bytes);
		let mut truncated = [0u8; 8];
		truncated.copy_from_slice(&digest.as_bytes()[..8]);
		NodeId(u64::from_le_bytes(truncated))
	}

	/// Stable id of the network owned by the node at this path, derived purely from the (structural)
	/// path and peer so it reproduces across `to_runtime` -> `from_runtime` round trips rather than
	/// depending on traversal order. A domain tag keeps it from colliding with this node's own
	/// `to_global_id`. The root network is `ROOT_NETWORK` and never goes through here.
	fn owned_network_id(&self, peer: PeerId) -> NetworkId {
		let bytes = postcard::to_stdvec(&("network", peer, self)).expect("NodePath must serialize");
		let digest = blake3::hash(&bytes);
		let mut truncated = [0u8; 8];
		truncated.copy_from_slice(&digest.as_bytes()[..8]);
		NetworkId(u64::from_le_bytes(truncated))
	}
}

#[derive(Clone, Debug)]
enum NetworkOwner {
	Table(Option<NetworkId>),
	Node(NodePath),
}

impl NetworkOwner {
	fn of(location: NetworkLocation<'_>, peer: PeerId) -> Self {
		let mut owner = Self::Table(location.definition.map(storage_network_id));
		for &local_id in location.inline_path {
			let network_id = owner.network_id(peer);
			owner = Self::Node(owner.child_path(network_id, local_id));
		}
		owner
	}

	fn network_id(&self, peer: PeerId) -> NetworkId {
		match self {
			Self::Table(None) => ROOT_NETWORK,
			Self::Table(Some(id)) => *id,
			Self::Node(path) => path.owned_network_id(peer),
		}
	}

	fn child_path(&self, network_id: NetworkId, local_id: RuntimeNodeId) -> NodePath {
		match self {
			Self::Table(definition) => NodePath {
				definition: *definition,
				path: Vec::new(),
				local_id,
			},
			Self::Node(parent) => NodePath::nested(parent, parent.local_id, network_id, local_id),
		}
	}
}

fn storage_network_id(id: RuntimeNetworkId) -> NetworkId {
	NetworkId(id.0)
}

/// Resolves a node's storage identity: the identity the metadata source holds for it where there is
/// one, and a hash of its location otherwise. Every place a node is named goes through here, so a
/// node's own registry key and every reference to it agree.
///
/// A held identity travels with the node, so moving it between networks keeps it. A hashed one is
/// derived from the node's place in the tree, so it changes when the node moves; it stands in for a
/// node no source names, such as one in a network that has never been stored.
pub(crate) struct NodeIds<'a, M: NodeMetadataSource + ?Sized> {
	pub(crate) metadata: Option<&'a M>,
	pub(crate) location: NetworkLocation<'a>,
	pub(crate) peer: PeerId,
}

impl<M: NodeMetadataSource + ?Sized> Clone for NodeIds<'_, M> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<M: NodeMetadataSource + ?Sized> Copy for NodeIds<'_, M> {}

impl<M: NodeMetadataSource + ?Sized> NodeIds<'_, M> {
	fn resolve(&self, path: &NodePath, local_id: RuntimeNodeId) -> NodeId {
		self.metadata
			.and_then(|metadata| metadata.storage_node_id(self.location, local_id))
			.unwrap_or_else(|| path.to_global_id(self.peer))
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
	#[error("Index {0} exceeds the storage format's u32 range")]
	IndexOverflow(usize),
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
/// declarations it extracted, as bytes keyed by content hash (for the caller's byte store) and as
/// decoded [`ProtoNode`]s keyed by id (for the caller's declaration cache).
pub struct RuntimeConversion {
	pub registry: Registry,
	pub declaration_bytes: DeclarationBytes,
	pub declarations: crate::Declarations,
	/// The storage `NetworkId` of each runtime network location, to key per-network view state in `session.json`.
	pub network_ids: HashMap<OwnedNetworkLocation, NetworkId>,
}

/// Encode a [`ProtoNode`] declaration to its content-addressed bytes: through the self-describing
/// [`Value`] (so serde aliases keep working and the on-disk shape stays migratable), then postcard.
/// Paired with [`decode_declaration`].
pub fn encode_declaration(proto: &ProtoNode) -> Result<Vec<u8>, String> {
	let value = to_value(proto).map_err(|error| error.to_string())?;
	postcard::to_stdvec(&value).map_err(|error| error.to_string())
}

/// A declaration with no attributes, as runtime conversion stores `identifier@version` and node migrations switch nodes to it.
pub struct EncodedDeclaration {
	pub id: ResourceId,
	pub hash: ResourceHash,
	pub bytes: Vec<u8>,
	pub proto: ProtoNode,
}

impl EncodedDeclaration {
	pub fn new(identifier: String, version: u32) -> Result<Self, String> {
		let proto = ProtoNode {
			identifier,
			version,
			attributes: Default::default(),
		};
		let bytes = encode_declaration(&proto)?;
		let hash = ResourceHash::from(bytes.as_slice());
		Ok(Self {
			id: ResourceId::from_hash(&hash),
			hash,
			bytes,
			proto,
		})
	}
}

/// Decode a [`ProtoNode`] declaration from the bytes [`encode_declaration`] produced.
pub fn decode_declaration(bytes: &[u8]) -> Result<ProtoNode, String> {
	let value: Value = postcard::from_bytes(bytes).map_err(|error| error.to_string())?;
	from_value(&value).map_err(|error| error.to_string())
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
			declarations: HashMap::new(),
			network_ids: HashMap::new(),
			metadata,
			peer,
		};

		convert_network(node_network, &NetworkOwner::Table(None), NetworkLocation::ROOT, &mut registry, &mut ctx)?;

		let reachable = node_network.reachable_definitions();
		if let Some(missing) = reachable.iter().find(|id| !node_network.definitions.contains_key(id)) {
			return Err(ConversionError::InvalidNetwork(format!("Definition {missing:?} is referenced but not defined")));
		}
		for (&id, definition) in node_network.definitions.iter().filter(|(id, _)| reachable.contains(id)) {
			if storage_network_id(id) == ROOT_NETWORK {
				return Err(ConversionError::InvalidNetwork(format!("Definition {id:?} collides with the root network ID")));
			}

			let location = NetworkLocation {
				definition: Some(id),
				inline_path: &[],
			};
			convert_network(definition, &NetworkOwner::Table(Some(storage_network_id(id))), location, &mut registry, &mut ctx)?;
		}

		// Only snapshot resources the network actually references. The runtime resource cache also keeps
		// resources alive across undo (so legacy redo can restore them), so it can contain orphans whose
		// node was removed by an undo. Snapshotting those would re-introduce an `AddResource` on the next
		// diff and let an undone resource resurface as a phantom edit. Declaration resources are added
		// separately by `convert_network` and are always referenced, so they're unaffected by this filter.
		let referenced = collect_referenced_resources(node_network, &reachable);
		convert_resources(resources, &referenced, peer, &mut registry)?;

		Ok(RuntimeConversion {
			registry,
			declaration_bytes: ctx.declaration_bytes,
			declarations: ctx.declarations,
			network_ids: ctx.network_ids,
		})
	}
}

/// Snapshot the runtime [`ResourceRegistry`](graphene_resource::ResourceRegistry) into the storage
/// [`ResourceStore`](crate::ResourceStore). Each source's chain position becomes a fractional
/// [`Priority`](crate::Priority) (index-as-priority preserves order); the `DataSource` body is
/// stored type-erased as [`Value`] so its on-disk shape can migrate freely. All
/// timestamps are `ORIGIN`, since this is a bootstrap snapshot, not an edit.
fn convert_resources(resources: &graphene_resource::ResourceRegistry, referenced: &std::collections::HashSet<ResourceId>, peer: PeerId, registry: &mut Registry) -> Result<(), ConversionError> {
	for id in resources.ids() {
		if !referenced.contains(&id) {
			continue;
		}
		if let Some(entry) = convert_resource_entry(resources, id, peer)? {
			registry.resources.insert(id, entry);
		}
	}
	Ok(())
}

/// Snapshot one runtime resource into a storage entry, or `None` if the runtime has no such resource.
pub fn convert_resource_entry(resources: &graphene_resource::ResourceRegistry, id: ResourceId, peer: PeerId) -> Result<Option<crate::ResourceEntry>, ConversionError> {
	let Some(info) = resources.info(&id) else { return Ok(None) };

	let mut entry = crate::ResourceEntry {
		hash: info.hash.copied(),
		hash_timestamp: TimeStamp::ORIGIN,
		..Default::default()
	};
	for (position, source) in info.sources.iter().enumerate() {
		let key = crate::SourceKey {
			priority: crate::Priority::new(position as f64).expect("enumerate index is finite"),
			peer,
		};
		let body = to_value(source).map_err(|error| ConversionError::SerializationError(error.to_string()))?;
		// Built rather than written: the chain is put in place as converted, floor and all at the origin.
		entry.force_set_source(
			key,
			crate::SourceValue {
				source: body,
				timestamp: TimeStamp::ORIGIN,
				deleted: false,
			},
		);
	}

	Ok(Some(entry))
}

/// Collect the `ResourceId`s referenced by `TaggedValue::Resource` inputs anywhere in the network
/// (recursively through nested networks). These are the resources the document actually uses; the
/// runtime cache may hold more (history-retained orphans) that shouldn't be snapshotted into storage.
fn collect_referenced_resources(network: &NodeNetwork, reachable: &rustc_hash::FxHashSet<RuntimeNetworkId>) -> std::collections::HashSet<ResourceId> {
	let mut referenced = std::collections::HashSet::new();
	collect_referenced_resources_inner(network, &mut referenced);
	for definition in reachable.iter().filter_map(|id| network.definitions.get(id)) {
		collect_referenced_resources_inner(definition, &mut referenced);
	}
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
	/// The same declarations decoded, keyed by id, handed back for the caller's declaration cache.
	declarations: crate::Declarations,
	/// The storage `NetworkId` of each runtime network location, to key per-network view state in `session.json`.
	network_ids: HashMap<OwnedNetworkLocation, NetworkId>,
	metadata: &'m M,
	peer: PeerId,
}

impl<'m, M: NodeMetadataSource + ?Sized> ConversionContext<'m, M> {
	/// The identity resolver for nodes addressed inside the network at `location`.
	fn ids<'a>(&self, location: NetworkLocation<'a>) -> NodeIds<'a, M>
	where
		'm: 'a,
	{
		NodeIds {
			metadata: Some(self.metadata),
			location,
			peer: self.peer,
		}
	}
}

fn convert_network<M: NodeMetadataSource + ?Sized>(
	node_network: &NodeNetwork,
	owner: &NetworkOwner,
	location: NetworkLocation<'_>,
	registry: &mut Registry,
	ctx: &mut ConversionContext<'_, M>,
) -> Result<(), ConversionError> {
	let network_id = owner.network_id(ctx.peer);

	for (runtime_node_id, doc_node) in &node_network.nodes {
		let global_id = ctx.ids(location).resolve(&owner.child_path(network_id, *runtime_node_id), *runtime_node_id);

		let node_location = NodeLocation {
			owner,
			network_id,
			network: location,
			runtime_node_id: *runtime_node_id,
		};
		let mut node = convert_node(doc_node, node_location, registry, ctx, true)?;
		node.attributes.set(node::ORIGINAL_NODE_ID, Value::Int(runtime_node_id.0.into()), TimeStamp::ORIGIN);
		registry.node_instances.insert(global_id, node);
	}

	let network = convert_network_entry(node_network, owner, network_id, location, ctx)?;
	registry.networks.insert(network_id, network);
	ctx.network_ids.insert(location.to_owned_location(), network_id);

	Ok(())
}

/// A network's export slots and network-level attributes, without its nodes.
fn convert_network_entry<M: NodeMetadataSource + ?Sized>(
	node_network: &NodeNetwork,
	owner: &NetworkOwner,
	network_id: NetworkId,
	location: NetworkLocation<'_>,
	ctx: &ConversionContext<'_, M>,
) -> Result<Network, ConversionError> {
	let ids = ctx.ids(location);

	let exports = node_network
		.exports
		.iter()
		.map(|export| {
			Ok(ExportSlot {
				target: Some(convert_input(export, owner, network_id, ids)?),
				timestamp: TimeStamp::ORIGIN,
			})
		})
		.collect::<Result<Vec<_>, ConversionError>>()?;

	let mut attributes = crate::Attributes::new();
	write_ui_network_attributes(&mut attributes, node_network, owner, network_id, ids, TimeStamp::ORIGIN)?;
	write_scope_injections(&mut attributes, node_network, owner, network_id, ids, TimeStamp::ORIGIN)?;

	Ok(Network {
		presence: TimeStamp::ORIGIN,
		exports,
		exports_timestamp: TimeStamp::ORIGIN,
		attributes,
	})
}

/// Serialize a network's `scope_injections` onto its attributes as one whole-map LWW blob, remapping
/// each runtime-local node reference to its stable storage global ID so the reference survives a
/// round trip even if runtime IDs are later reshuffled.
fn write_scope_injections<M: NodeMetadataSource + ?Sized>(
	attributes: &mut crate::Attributes,
	node_network: &NodeNetwork,
	owner: &NetworkOwner,
	network_id: NetworkId,
	ids: NodeIds<'_, M>,
	timestamp: TimeStamp,
) -> Result<(), ConversionError> {
	if node_network.scope_injections.is_empty() {
		return Ok(());
	}

	let stored: HashMap<String, (NodeId, core_types::Type)> = node_network
		.scope_injections
		.iter()
		.map(|(key, (runtime_id, ty))| {
			let storage_id = ids.resolve(&owner.child_path(network_id, *runtime_id), *runtime_id);
			(key.clone(), (storage_id, ty.clone()))
		})
		.collect();

	attributes
		.set_serialized(network::SCOPE_INJECTIONS, &stored, timestamp)
		.map_err(map_serialization_error(network::SCOPE_INJECTIONS))
}

/// Where a node sits in both the storage tree (`owner`, `network_id`) and the runtime tree (`network`,
/// `runtime_node_id`).
struct NodeLocation<'a> {
	owner: &'a NetworkOwner,
	network_id: NetworkId,
	network: NetworkLocation<'a>,
	runtime_node_id: RuntimeNodeId,
}

fn convert_node<M: NodeMetadataSource + ?Sized>(
	doc_node: &DocumentNode,
	location: NodeLocation<'_>,
	registry: &mut Registry,
	ctx: &mut ConversionContext<'_, M>,
	recurse: bool,
) -> Result<Node, ConversionError> {
	let NodeLocation {
		owner,
		network_id,
		network,
		runtime_node_id,
	} = location;

	let node_path = owner.child_path(network_id, runtime_node_id);
	let timestamp = TimeStamp::ORIGIN;

	let mut inputs = Vec::with_capacity(doc_node.inputs.len());
	for (input_index, input) in doc_node.inputs.iter().enumerate() {
		let mut input_attrs = convert_input_attributes(input)?;
		input_attrs.set_floor(timestamp);
		write_ui_input_attributes(&mut input_attrs, ctx.metadata, network, runtime_node_id, input_index, timestamp)?;

		inputs.push(InputSlot {
			input: convert_input(input, owner, network_id, ctx.ids(network))?,
			timestamp,
			attributes: input_attrs,
		});
	}

	let implementation = convert_implementation(&doc_node.implementation, &node_path, network, registry, ctx, recurse)?;

	// Defaults match `DocumentNode::default()`; `to_runtime` rehydrates absent keys from the same defaults.
	let mut attributes = crate::Attributes::empty_at(timestamp);
	attributes
		.set_if_not_default(node::CALL_ARGUMENT, &doc_node.call_argument, &concrete!(Context), timestamp)
		.map_err(map_serialization_error(node::CALL_ARGUMENT))?;
	attributes
		.set_if_not_default(node::VISIBLE, &doc_node.visible, &true, timestamp)
		.map_err(map_serialization_error(node::VISIBLE))?;
	attributes
		.set_if_not_default(node::SKIP_DEDUPLICATION, &doc_node.skip_deduplication, &false, timestamp)
		.map_err(map_serialization_error(node::SKIP_DEDUPLICATION))?;

	write_ui_attributes(&mut attributes, ctx.metadata, network, runtime_node_id, timestamp)?;

	Ok(Node {
		presence: timestamp,
		network_timestamp: timestamp,
		inputs_timestamp: timestamp,
		implementation,
		implementation_timestamp: timestamp,
		inputs,
		attributes,
		network: network_id,
	})
}

/// The `call_argument` attribute write for a node, cleared when the value is the default that
/// `to_runtime` rehydrates an absent key from. For staging paths that encode this field alone.
pub fn encode_call_argument(call_argument: &core_types::Type) -> Result<crate::AttributeDelta, ConversionError> {
	encode_if_not_default(node::CALL_ARGUMENT, call_argument, &concrete!(Context))
}

fn encode_if_not_default<T: Serialize + PartialEq>(key: &str, value: &T, default: &T) -> Result<crate::AttributeDelta, ConversionError> {
	let value = if value == default { None } else { Some(to_value(value).map_err(map_serialization_error(key))?) };

	Ok(crate::AttributeDelta { key: key.to_string(), value })
}

/// Public form of the node ui-attribute encoding, for staging paths that encode single nodes.
pub fn encode_node_ui_attributes<M: NodeMetadataSource + ?Sized>(
	attributes: &mut crate::Attributes,
	metadata: &M,
	network: NetworkLocation<'_>,
	runtime_node_id: RuntimeNodeId,
	timestamp: TimeStamp,
) -> Result<(), ConversionError> {
	write_ui_attributes(attributes, metadata, network, runtime_node_id, timestamp)
}

/// Public form of the per-input ui-attribute encoding, matching `encode_node_ui_attributes`.
pub fn encode_input_ui_attributes<M: NodeMetadataSource + ?Sized>(
	attributes: &mut crate::Attributes,
	metadata: &M,
	network: NetworkLocation<'_>,
	runtime_node_id: RuntimeNodeId,
	input_index: usize,
	timestamp: TimeStamp,
) -> Result<(), ConversionError> {
	write_ui_input_attributes(attributes, metadata, network, runtime_node_id, input_index, timestamp)
}

fn write_ui_attributes<M: NodeMetadataSource + ?Sized>(
	attributes: &mut crate::Attributes,
	metadata: &M,
	network: NetworkLocation<'_>,
	runtime_node_id: RuntimeNodeId,
	timestamp: TimeStamp,
) -> Result<(), ConversionError> {
	if let Some(position) = metadata.position(network, runtime_node_id) {
		attributes
			.set_serialized(node::ui::POSITION, &position, timestamp)
			.map_err(map_serialization_error(node::ui::POSITION))?;
	}

	// Bool flags are only emitted when true; absence reads as false.
	for (key, value) in [
		(node::ui::IS_LAYER, metadata.is_layer(network, runtime_node_id)),
		(node::ui::LOCKED, metadata.locked(network, runtime_node_id)),
		(node::ui::PINNED, metadata.pinned(network, runtime_node_id)),
	] {
		if value {
			attributes.set(key, Value::Bool(true), timestamp);
		}
	}

	if let Some(name) = metadata.display_name(network, runtime_node_id)
		&& !name.is_empty()
	{
		attributes.set(node::ui::DISPLAY_NAME, Value::Str(name.to_string()), timestamp);
	}

	// One whole-vec attribute; per-slot LWW would be overkill for rename-on-output.
	let output_names = metadata.output_names(network, runtime_node_id);
	if !output_names.is_empty() {
		attributes
			.set_serialized(node::ui::OUTPUT_NAMES, &output_names, timestamp)
			.map_err(map_serialization_error(node::ui::OUTPUT_NAMES))?;
	}

	let output_descriptions = metadata.output_descriptions(network, runtime_node_id);
	if output_descriptions.iter().any(|description| !description.is_empty()) {
		attributes
			.set_serialized(node::ui::OUTPUT_DESCRIPTIONS, &output_descriptions, timestamp)
			.map_err(map_serialization_error(node::ui::OUTPUT_DESCRIPTIONS))?;
	}

	Ok(())
}

/// Node references here are remapped to stable storage IDs for the same reason `write_scope_injections`
/// does it: a runtime ID is not stable across a round trip.
fn write_ui_network_attributes<M: NodeMetadataSource + ?Sized>(
	attributes: &mut crate::Attributes,
	node_network: &NodeNetwork,
	owner: &NetworkOwner,
	network_id: NetworkId,
	ids: NodeIds<'_, M>,
	timestamp: TimeStamp,
) -> Result<(), ConversionError> {
	// The same source the IDs resolve through, so a network's attributes and the nodes they name agree.
	let Some(metadata) = ids.metadata else { return Ok(()) };
	let location = ids.location;

	if let Some(reference) = metadata.reference(location) {
		attributes.set(node::ui::REFERENCE, Value::Str(reference.to_string()), timestamp);
	}
	write_network_interface_attributes(attributes, metadata, location, timestamp)?;

	let to_storage_id = |runtime_id: RuntimeNodeId| ids.resolve(&owner.child_path(network_id, runtime_id), runtime_id);

	// Only nodes the converted network still holds: the read side drops a stored ID it cannot find, so
	// persisting one for a node deleted or reparented elsewhere would leave stored state disagreeing with
	// what a peer converting the same input produces.
	let pinned_order: Vec<_> = metadata.pinned_order(location).into_iter().filter(|pinned| node_network.nodes.contains_key(pinned)).collect();
	if !pinned_order.is_empty() {
		let stored: Vec<NodeId> = pinned_order.into_iter().map(to_storage_id).collect();
		attributes
			.set_serialized(network::PINNED_ORDER, &stored, timestamp)
			.map_err(map_serialization_error(network::PINNED_ORDER))?;
	}

	Ok(())
}

fn write_network_interface_attributes<M: NodeMetadataSource + ?Sized>(
	attributes: &mut crate::Attributes,
	metadata: &M,
	network: NetworkLocation<'_>,
	timestamp: TimeStamp,
) -> Result<(), ConversionError> {
	let import_metadata = metadata.import_metadata(network);
	if !import_metadata.is_empty() {
		attributes
			.set_serialized(network::IMPORT_METADATA, &import_metadata, timestamp)
			.map_err(map_serialization_error(network::IMPORT_METADATA))?;
	}

	let export_names = metadata.export_names(network);
	if !export_names.is_empty() {
		attributes
			.set_serialized(network::EXPORT_NAMES, &export_names, timestamp)
			.map_err(map_serialization_error(network::EXPORT_NAMES))?;
	}

	let export_descriptions = metadata.export_descriptions(network);
	if export_descriptions.iter().any(|description| !description.is_empty()) {
		attributes
			.set_serialized(network::EXPORT_DESCRIPTIONS, &export_descriptions, timestamp)
			.map_err(map_serialization_error(network::EXPORT_DESCRIPTIONS))?;
	}

	Ok(())
}

/// Empty strings (the runtime's "unset" sentinel) and absent values are both skipped.
/// `input_data` entries each get their own `ui::input_data::<sub_key>` attribute for per-key LWW.
fn write_ui_input_attributes<M: NodeMetadataSource + ?Sized>(
	attributes: &mut crate::Attributes,
	metadata: &M,
	network: NetworkLocation<'_>,
	runtime_node_id: RuntimeNodeId,
	input_index: usize,
	timestamp: TimeStamp,
) -> Result<(), ConversionError> {
	let non_empty_string = |key: &'static str, value: Option<&str>, attributes: &mut crate::Attributes| {
		if let Some(value) = value.filter(|s| !s.is_empty()) {
			attributes.set(key, Value::Str(value.to_string()), timestamp);
		}
	};

	non_empty_string(node::input::ui::NAME, metadata.input_name(network, runtime_node_id, input_index), attributes);
	non_empty_string(node::input::ui::DESCRIPTION, metadata.input_description(network, runtime_node_id, input_index), attributes);
	non_empty_string(node::input::ui::WIDGET_OVERRIDE, metadata.widget_override(network, runtime_node_id, input_index), attributes);

	for (sub_key, value) in metadata.input_data(network, runtime_node_id, input_index) {
		attributes.set(&format!("{prefix}{sub_key}", prefix = node::input::ui::DATA_PREFIX), value, timestamp);
	}

	Ok(())
}

fn convert_input<M: NodeMetadataSource + ?Sized>(input: &GraphCraftNodeInput, owner: &NetworkOwner, network_id: NetworkId, ids: NodeIds<'_, M>) -> Result<NodeInput, ConversionError> {
	Ok(match input {
		GraphCraftNodeInput::Node { node_id, output_index } => NodeInput::Node {
			id: ids.resolve(&owner.child_path(network_id, *node_id), *node_id),
			index: (*output_index).try_into().map_err(|_| ConversionError::IndexOverflow(*output_index))?,
		},
		GraphCraftNodeInput::Value { tagged_value, exposed } => {
			let value = to_value(&**tagged_value).map_err(|e| ConversionError::SerializationError(format!("{e:?}")))?;
			NodeInput::Value { value, exposed: *exposed }
		}
		GraphCraftNodeInput::Scope(s) => NodeInput::Scope(s.clone()),
		GraphCraftNodeInput::Import { import_index, .. } => NodeInput::Import {
			index: (*import_index).try_into().map_err(|_| ConversionError::IndexOverflow(*import_index))?,
		},
		GraphCraftNodeInput::Reflection(_) => NodeInput::Reflection,
	})
}

/// The non-ui attributes a slot carries, derived from the input itself: the declared type of an
/// import, and a reflection node's metadata.
pub fn convert_input_attributes(input: &GraphCraftNodeInput) -> Result<crate::Attributes, ConversionError> {
	let mut attributes = crate::Attributes::new();
	let timestamp = TimeStamp::ORIGIN;

	match input {
		GraphCraftNodeInput::Import { import_type, .. } => {
			attributes
				.set_serialized(node::input::IMPORT_TYPE, import_type, timestamp)
				.map_err(map_serialization_error(node::input::IMPORT_TYPE))?;
		}
		GraphCraftNodeInput::Reflection(metadata) => {
			attributes
				.set_serialized(node::REFLECTION_METADATA, metadata, timestamp)
				.map_err(map_serialization_error(node::REFLECTION_METADATA))?;
		}
		_ => {}
	}

	Ok(attributes)
}

fn convert_implementation<M: NodeMetadataSource + ?Sized>(
	implementation: &DocumentNodeImplementation,
	node_path: &NodePath,
	network: NetworkLocation<'_>,
	registry: &mut Registry,
	ctx: &mut ConversionContext<'_, M>,
	recurse: bool,
) -> Result<Implementation, ConversionError> {
	Ok(match implementation {
		DocumentNodeImplementation::ProtoNode(identifier) => {
			let identifier_str = identifier.as_str().to_string();

			// Reuse a previously-converted proto-node's id; identical content hashes to the same id
			// anyway, so this only skips re-serializing.
			if let Some(id) = ctx.declaration_ids.get(&identifier_str) {
				return Ok(Implementation::ProtoNode(*id));
			}

			// Nodes the binary doesn't register, such as in tests, are at their first version
			let version = NODE_METADATA.lock().ok().and_then(|metadata| metadata.get(identifier).map(|metadata| metadata.version)).unwrap_or(1);
			let EncodedDeclaration { id, hash, bytes, proto } =
				EncodedDeclaration::new(identifier_str.clone(), version).map_err(|error| ConversionError::SerializationError(format!("proto-node {identifier_str}: {error}")))?;

			register_declaration_resource(registry, id, hash, ctx.peer);
			ctx.declaration_bytes.insert(hash, bytes);
			ctx.declarations.insert(id, proto);
			ctx.declaration_ids.insert(identifier_str, id);

			Implementation::ProtoNode(id)
		}
		DocumentNodeImplementation::Network(nested_network) => {
			// Stable, traversal-order-independent id derived from the owning node's path, so a
			// `to_runtime` -> `from_runtime` round trip reproduces the same `NetworkId` (and thus the
			// same node-path hashes underneath it).
			let owner = NetworkOwner::Node(node_path.clone());
			let nested_network_id = owner.network_id(ctx.peer);
			if recurse {
				let nested_location = network.child(node_path.local_id);
				convert_network(nested_network, &owner, nested_location.as_location(), registry, ctx)?;
			}
			Implementation::Network(nested_network_id)
		}
		DocumentNodeImplementation::Definition(id) => Implementation::Network(storage_network_id(*id)),
		// TODO: Support Extract in the Registry format.
		DocumentNodeImplementation::Extract => return Err(ConversionError::UnsupportedImplementation),
	})
}

/// Derives storage IDs from runtime network locations.
/// It uses the same [`NodeIds`] as a whole-document conversion, so both give the same IDs.
pub struct PathResolver<'m> {
	metadata: Option<&'m dyn NodeMetadataSource>,
	peer: PeerId,
}

impl<'m> PathResolver<'m> {
	/// Without a metadata source every node falls back to a hash of its location, which only matches a
	/// whole-document conversion while no node has a stored identity.
	pub fn new(metadata: Option<&'m dyn NodeMetadataSource>, peer: PeerId) -> Self {
		Self { metadata, peer }
	}

	/// The storage `NetworkId` of the network at `location` (`ROOT_NETWORK` for the document network).
	pub fn network_id(&self, location: NetworkLocation<'_>) -> NetworkId {
		NetworkOwner::of(location, self.peer).network_id(self.peer)
	}

	/// The storage `NodeId` of the node with `local_id` inside the network at `location`.
	pub fn node_id(&self, location: NetworkLocation<'_>, local_id: RuntimeNodeId) -> NodeId {
		let owner = NetworkOwner::of(location, self.peer);
		let network_id = owner.network_id(self.peer);
		self.ids(location).resolve(&owner.child_path(network_id, local_id), local_id)
	}

	/// Converts one runtime input in the network at `location` to storage form, with node references mapped to storage IDs.
	pub fn convert_input_at(&self, input: &GraphCraftNodeInput, location: NetworkLocation<'_>) -> Result<NodeInput, ConversionError> {
		let owner = NetworkOwner::of(location, self.peer);
		convert_input(input, &owner, owner.network_id(self.peer), self.ids(location))
	}

	/// The identity resolver for nodes addressed inside the network at `location`.
	fn ids<'a>(&self, location: NetworkLocation<'a>) -> NodeIds<'a, dyn NodeMetadataSource + 'm>
	where
		'm: 'a,
	{
		NodeIds {
			metadata: self.metadata,
			location,
			peer: self.peer,
		}
	}
}

/// Converts a chosen subset of runtime entities into a scratch [`Registry`] through the same
/// encoders as a whole-document conversion, so staging can derive deltas for just those entities.
pub struct ScopedConversion<'m> {
	ctx: ConversionContext<'m, dyn NodeMetadataSource + 'm>,
	resolver: PathResolver<'m>,
}

impl<'m> ScopedConversion<'m> {
	pub fn new(metadata: &'m dyn NodeMetadataSource, peer: PeerId) -> Self {
		Self {
			ctx: ConversionContext {
				declaration_ids: HashMap::new(),
				declaration_bytes: HashMap::new(),
				declarations: HashMap::new(),
				network_ids: HashMap::new(),
				metadata,
				peer,
			},
			resolver: PathResolver::new(Some(metadata), peer),
		}
	}

	/// Converts the node `local_id` in the network at `location` into `registry`.
	/// With `recurse`, it also converts the inline network the node implements. Without it, that network is only referenced by ID.
	pub fn convert_node_at(&mut self, registry: &mut Registry, location: NetworkLocation<'_>, local_id: RuntimeNodeId, doc_node: &DocumentNode, recurse: bool) -> Result<(), ConversionError> {
		let owner = NetworkOwner::of(location, self.ctx.peer);
		let node_location = NodeLocation {
			owner: &owner,
			network_id: owner.network_id(self.ctx.peer),
			network: location,
			runtime_node_id: local_id,
		};

		let mut node = convert_node(doc_node, node_location, registry, &mut self.ctx, recurse)?;
		node.attributes.set(node::ORIGINAL_NODE_ID, Value::Int(local_id.0.into()), TimeStamp::ORIGIN);
		registry.node_instances.insert(self.resolver.node_id(location, local_id), node);
		Ok(())
	}

	/// Converts the export slots and attributes of the network at `location` into `registry`, without its nodes.
	pub fn convert_network_entry_at(&mut self, registry: &mut Registry, location: NetworkLocation<'_>, node_network: &NodeNetwork) -> Result<(), ConversionError> {
		let owner = NetworkOwner::of(location, self.ctx.peer);
		let network_id = owner.network_id(self.ctx.peer);

		let network = convert_network_entry(node_network, &owner, network_id, location, &self.ctx)?;
		registry.networks.insert(network_id, network);
		Ok(())
	}

	/// The extracted proto-node declarations: the bytes for the caller's byte store, and the same
	/// declarations decoded for its declaration cache.
	pub fn finish(self) -> (DeclarationBytes, crate::Declarations) {
		(self.ctx.declaration_bytes, self.ctx.declarations)
	}
}

/// The `TaggedValue::Resource` IDs referenced by a stored node's value inputs.
///
/// Peeks at the externally-tagged serde shape (`{"Resource": <id>}`) rather than deserializing the
/// whole `TaggedValue`, which can be arbitrarily large. `resource_ref_shape_matches_serde` asserts
/// this stays in lockstep with the real serialization.
pub fn node_value_resource_refs(node: &Node) -> impl Iterator<Item = ResourceId> + '_ {
	node.inputs.iter().filter_map(|slot| value_resource_ref(&slot.input))
}

/// The `TaggedValue::Resource` ID referenced by a stored value input, if any.
pub fn value_resource_ref(input: &NodeInput) -> Option<ResourceId> {
	match input {
		NodeInput::Value { value: Value::Object(fields), .. } => match fields.as_slice() {
			[(key, id)] if key == "Resource" => from_value(id).ok(),
			_ => None,
		},
		_ => None,
	}
}

#[cfg(test)]
mod resource_ref_shape {
	use super::*;

	#[test]
	fn resource_ref_shape_matches_serde() {
		let id = ResourceId::from_hash(&ResourceHash::from(b"shape test".as_slice()));
		let value = to_value(&TaggedValue::Resource(id)).expect("TaggedValue::Resource serializes");
		let parsed = value_resource_ref(&NodeInput::Value { value, exposed: false });
		assert_eq!(parsed, Some(id), "The shape peek in node_value_resource_refs must match TaggedValue's serde form");
	}
}
