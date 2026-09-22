use super::{DocumentNodeMetadata, DocumentNodePersistentMetadata, LayerPosition, NodePosition, NodeTypePersistentMetadata};
use document_graph_storage::attr::node as node_attr;
use document_graph_storage::from_runtime::{ConversionError, DeclarationBytes};
use document_graph_storage::{AttributeDelta, Attributes, Implementation, NoMetadata, PathResolver, Position, Registry, RegistryDelta, ScopedConversion, TimeStamp};
use document_graph_storage::{convert_resource_entry, encode_input_ui_attributes, encode_node_ui_attributes, node_value_resource_refs, value_resource_ref};
use graph_craft::application_io::resource::{ResourceId, ResourceRegistry};
use graph_craft::document::NodeId;
use graph_craft::runtime_delta::RuntimeDelta;
use std::collections::HashSet;

/// A [`RuntimeDelta`] extended with the editor-only change kind: a wholesale copy of a node's
/// persistent metadata, which storage diffs against the working registry so minimal attribute ops
/// fall out. The compiler consumes only the `Graph` variant.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorDelta {
	Graph(RuntimeDelta),
	/// The copy includes the metadata of everything nested under the node, so one delta covers a
	/// group and its contents.
	NodeMetadata {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		metadata: Box<DocumentNodePersistentMetadata>,
	},
}

pub struct ConstructedOps {
	pub ops: Vec<RegistryDelta>,
	pub declaration_bytes: DeclarationBytes,
}

/// Constructs the storage ops for one gesture's deltas, in delta order. Removal closures and
/// resource liveness are computed against the whole batch, since several removals in one gesture
/// can jointly orphan a resource that each alone would not. Op timestamps are placeholders,
/// re-stamped by the staging clock.
pub fn construct_batch(deltas: &[EditorDelta], working: &Registry, resources: &ResourceRegistry, peer: document_graph_storage::PeerId) -> Result<ConstructedOps, ConversionError> {
	let resolver = PathResolver::new(peer);
	let mut ops = Vec::new();
	let mut declaration_bytes = DeclarationBytes::new();
	let mut batch_removed_nodes = Vec::new();
	let mut batch_removed_networks = Vec::new();
	let mut batch_added_resources = HashSet::new();

	for delta in deltas {
		if let EditorDelta::Graph(RuntimeDelta::RemoveNode { network_path, node_id } | RuntimeDelta::ReplaceNode { network_path, node_id, .. }) = delta {
			collect_removal_closure(resolver.node_id(network_path, *node_id), working, &mut batch_removed_nodes, &mut batch_removed_networks);
		}
	}
	batch_removed_nodes.sort();
	batch_removed_nodes.dedup();
	batch_removed_networks.sort();
	batch_removed_networks.dedup();

	for delta in deltas {
		delta.construct(working, resources, peer, &resolver, &batch_removed_nodes, &mut batch_added_resources, &mut ops, &mut declaration_bytes)?;
	}

	construct_resource_removals(&batch_removed_nodes, working, &mut ops);
	Ok(ConstructedOps { ops, declaration_bytes })
}

impl EditorDelta {
	#[allow(clippy::too_many_arguments)]
	fn construct(
		&self,
		working: &Registry,
		resources: &ResourceRegistry,
		peer: document_graph_storage::PeerId,
		resolver: &PathResolver,
		batch_removed_nodes: &[document_graph_storage::NodeId],
		batch_added_resources: &mut HashSet<ResourceId>,
		ops: &mut Vec<RegistryDelta>,
		declaration_bytes: &mut DeclarationBytes,
	) -> Result<(), ConversionError> {
		match self {
			EditorDelta::Graph(RuntimeDelta::AddNode { network_path, node_id, node }) => {
				construct_structural_additions(network_path, *node_id, node, working, resources, peer, batch_added_resources, ops, declaration_bytes)?;
			}

			EditorDelta::Graph(RuntimeDelta::ReplaceNode { network_path, node_id, node }) => {
				construct_removals(resolver.node_id(network_path, *node_id), working, ops);
				construct_structural_additions(network_path, *node_id, node, working, resources, peer, batch_added_resources, ops, declaration_bytes)?;
			}

			EditorDelta::Graph(RuntimeDelta::RemoveNode { network_path, node_id }) => {
				construct_removals(resolver.node_id(network_path, *node_id), working, ops);
			}

			EditorDelta::Graph(RuntimeDelta::SetVisibility { network_path, node_id, visible }) => {
				ops.push(RegistryDelta::ChangeNodeAttribute {
					id: resolver.node_id(network_path, *node_id),
					delta: AttributeDelta {
						key: node_attr::VISIBLE.to_string(),
						value: (!visible).then_some(serde_json::Value::Bool(false)),
					},
				});
			}

			EditorDelta::Graph(RuntimeDelta::SetInput {
				network_path,
				node_id,
				input_index,
				input,
			}) => {
				ops.push(RegistryDelta::ChangeNodeInput {
					id: resolver.node_id(network_path, *node_id),
					index: (*input_index).try_into().map_err(|_| ConversionError::IndexOverflow(*input_index))?,
					new_input: resolver.convert_input_at(input, network_path)?,
				});
			}

			EditorDelta::Graph(RuntimeDelta::SetExport { network_path, export_index, input }) => {
				ops.push(RegistryDelta::SetNetworkExport {
					id: resolver.network_id(network_path),
					index: (*export_index).try_into().map_err(|_| ConversionError::IndexOverflow(*export_index))?,
					export: Some(resolver.convert_input_at(input, network_path)?),
				});
			}

			EditorDelta::NodeMetadata { network_path, node_id, metadata } => {
				construct_metadata_changes(network_path, *node_id, metadata, working, resolver, ops)?;
			}
		}

		let _ = batch_removed_nodes;
		Ok(())
	}
}

/// Converts through the same encoders as a whole-document conversion, with `NoMetadata` as the
/// source: ui attributes arrive via the gesture's paired `NodeMetadata` delta.
#[allow(clippy::too_many_arguments)]
fn construct_structural_additions(
	network_path: &[NodeId],
	node_id: NodeId,
	node: &graph_craft::document::DocumentNode,
	working: &Registry,
	resources: &ResourceRegistry,
	peer: document_graph_storage::PeerId,
	batch_added_resources: &mut HashSet<ResourceId>,
	ops: &mut Vec<RegistryDelta>,
	declaration_bytes: &mut DeclarationBytes,
) -> Result<(), ConversionError> {
	let mut scoped = ScopedConversion::new(&NoMetadata, peer);
	let mut scratch = Registry::default();
	scoped.convert_node_at(&mut scratch, network_path, node_id, node, true)?;
	declaration_bytes.extend(scoped.finish());

	let mut networks: Vec<_> = scratch.networks.iter().collect();
	networks.sort_by_key(|(id, _)| **id);
	for (id, network) in networks {
		ops.push(RegistryDelta::AddNetwork { id: *id, network: network.clone() });
	}

	let mut nodes: Vec<_> = scratch.node_instances.iter().collect();
	nodes.sort_by_key(|(id, _)| **id);
	for (id, node) in nodes {
		ops.push(RegistryDelta::AddNode { id: *id, node: node.clone() });
	}

	let mut new_resources: Vec<_> = scratch.resources.iter().filter(|(id, _)| !working.resources.contains_key(id)).collect();
	new_resources.sort_by_key(|(id, _)| **id);
	for (id, entry) in new_resources {
		if batch_added_resources.insert(*id) {
			ops.push(RegistryDelta::AddResource { id: *id, entry: entry.clone() });
		}
	}
	let mut tagged: Vec<ResourceId> = scratch.node_instances.values().flat_map(node_value_resource_refs).collect();
	tagged.sort();
	tagged.dedup();
	for id in tagged {
		if !working.resources.contains_key(&id)
			&& !scratch.resources.contains_key(&id)
			&& !batch_added_resources.contains(&id)
			&& let Some(entry) = convert_resource_entry(resources, id, peer)?
		{
			batch_added_resources.insert(id);
			ops.push(RegistryDelta::AddResource { id, entry });
		}
	}

	Ok(())
}

fn construct_metadata_changes(
	network_path: &[NodeId],
	node_id: NodeId,
	metadata: &DocumentNodePersistentMetadata,
	working: &Registry,
	resolver: &PathResolver,
	ops: &mut Vec<RegistryDelta>,
) -> Result<(), ConversionError> {
	let source = MetadataCopySource {
		anchor_path: network_path,
		anchor_id: node_id,
		metadata,
	};

	let mut pending = vec![(network_path.to_vec(), node_id, metadata)];
	while let Some((path, id, node_metadata)) = pending.pop() {
		let global_id = resolver.node_id(&path, id);
		let working_node = working.node_instances.get(&global_id);

		let mut encoded = Attributes::new();
		encode_node_ui_attributes(&mut encoded, &source, &path, id, TimeStamp::ORIGIN)?;
		for delta in ui_attribute_deltas(working_node.map(|node| node.attributes()), &encoded) {
			ops.push(RegistryDelta::ChangeNodeAttribute { id: global_id, delta });
		}

		for input_index in 0..node_metadata.input_metadata.len() {
			let mut encoded = Attributes::new();
			encode_input_ui_attributes(&mut encoded, &source, &path, id, input_index, TimeStamp::ORIGIN)?;
			let current = working_node.and_then(|node| node.inputs().get(input_index)).map(|slot| &slot.attributes);
			for delta in ui_attribute_deltas(current, &encoded) {
				ops.push(RegistryDelta::ChangeNodeInputAttribute {
					id: global_id,
					index: input_index.try_into().map_err(|_| ConversionError::IndexOverflow(input_index))?,
					delta,
				});
			}
		}

		if let Some(network_metadata) = &node_metadata.network_metadata {
			let mut nested_path = path.clone();
			nested_path.push(id);
			let network_id = resolver.network_id(&nested_path);

			let target = network_metadata.persistent_metadata.reference.clone().map(serde_json::Value::String);
			let current = working
				.networks
				.get(&network_id)
				.and_then(|network| network.attributes.get(node_attr::ui::REFERENCE))
				.map(|value| value.value.clone());
			if current != target {
				ops.push(RegistryDelta::ChangeNetworkAttribute {
					id: network_id,
					delta: AttributeDelta {
						key: node_attr::ui::REFERENCE.to_string(),
						value: target,
					},
				});
			}

			for (child_id, child) in &network_metadata.persistent_metadata.node_metadata {
				pending.push((nested_path.clone(), *child_id, &child.persistent_metadata));
			}
		}
	}

	Ok(())
}

/// Minimal ops transforming the `ui::`-prefixed subset of `current` into `encoded`, comparing
/// values only, since timestamps are re-stamped at staging.
fn ui_attribute_deltas(current: Option<&Attributes>, encoded: &Attributes) -> Vec<AttributeDelta> {
	let owned = |key: &str| key.starts_with("ui::");
	let mut deltas = Vec::new();

	if let Some(current) = current {
		for key in current.keys() {
			if owned(key) && !encoded.contains_key(key) {
				deltas.push(AttributeDelta { key: key.clone(), value: None });
			}
		}
	}
	for (key, value) in encoded {
		let unchanged = current.and_then(|current| current.get(key)).is_some_and(|existing| existing.value == value.value);
		if !unchanged {
			deltas.push(AttributeDelta {
				key: key.clone(),
				value: Some(value.value.clone()),
			});
		}
	}

	deltas.sort_by(|a, b| a.key.cmp(&b.key));
	deltas
}

fn construct_removals(node_id: document_graph_storage::NodeId, working: &Registry, ops: &mut Vec<RegistryDelta>) {
	let mut removed_nodes = Vec::new();
	let mut removed_networks = Vec::new();
	collect_removal_closure(node_id, working, &mut removed_nodes, &mut removed_networks);

	removed_nodes.sort();
	removed_networks.sort();
	for id in &removed_nodes {
		ops.push(RegistryDelta::RemoveNode {
			id: *id,
			snapshot: working.node_instances[id].clone(),
		});
	}
	for id in &removed_networks {
		ops.push(RegistryDelta::RemoveNetwork {
			id: *id,
			snapshot: working.networks[id].clone(),
		});
	}
}

/// Emits removals for resources referenced only by the batch's removed nodes, checked after every
/// removal is known.
fn construct_resource_removals(batch_removed_nodes: &[document_graph_storage::NodeId], working: &Registry, ops: &mut Vec<RegistryDelta>) {
	let removed_node_set: HashSet<_> = batch_removed_nodes.iter().copied().collect();
	let mut candidates: Vec<ResourceId> = batch_removed_nodes
		.iter()
		.flat_map(|id| {
			let node = &working.node_instances[id];
			let declaration = match node.implementation() {
				Implementation::ProtoNode(declaration) => Some(*declaration),
				Implementation::Network(_) => None,
			};
			declaration.into_iter().chain(node_value_resource_refs(node))
		})
		.collect();
	candidates.sort();
	candidates.dedup();

	for candidate in candidates {
		let still_referenced = working
			.node_instances
			.iter()
			.filter(|(id, _)| !removed_node_set.contains(id))
			.any(|(_, node)| matches!(node.implementation(), Implementation::ProtoNode(declaration) if *declaration == candidate) || node_value_resource_refs(node).any(|id| id == candidate))
			|| working
				.networks
				.values()
				.any(|network| network.exports.iter().any(|slot| slot.target.as_ref().and_then(value_resource_ref) == Some(candidate)));

		if !still_referenced && let Some(entry) = working.resources.get(&candidate) {
			ops.push(RegistryDelta::RemoveResource {
				id: candidate,
				snapshot: entry.clone(),
			});
		}
	}
}

fn collect_removal_closure(node_id: document_graph_storage::NodeId, working: &Registry, nodes: &mut Vec<document_graph_storage::NodeId>, networks: &mut Vec<document_graph_storage::NetworkId>) {
	let Some(node) = working.node_instances.get(&node_id) else { return };
	nodes.push(node_id);

	if let &Implementation::Network(network_id) = node.implementation() {
		networks.push(network_id);
		for (child_id, child) in &working.node_instances {
			if child.network() == network_id {
				collect_removal_closure(*child_id, working, nodes, networks);
			}
		}
	}
}

/// Serves a metadata copy as the [`document_graph_storage::NodeMetadataSource`] for its own
/// encoding, resolving requested paths relative to the anchor node the copy was taken from.
struct MetadataCopySource<'a> {
	anchor_path: &'a [NodeId],
	anchor_id: NodeId,
	metadata: &'a DocumentNodePersistentMetadata,
}

impl MetadataCopySource<'_> {
	fn metadata_for(&self, metadata_path: &[NodeId], node_id: NodeId) -> Option<&DocumentNodePersistentMetadata> {
		let relative = metadata_path.strip_prefix(self.anchor_path)?;

		let (mut current, rest) = match relative.split_first() {
			None => return (node_id == self.anchor_id).then_some(self.metadata),
			Some((first, rest)) if *first == self.anchor_id => (self.metadata, rest),
			Some(_) => return None,
		};

		for step in rest {
			current = child_metadata(current, *step)?;
		}
		child_metadata(current, node_id)
	}
}

fn child_metadata(metadata: &DocumentNodePersistentMetadata, child_id: NodeId) -> Option<&DocumentNodePersistentMetadata> {
	metadata
		.network_metadata
		.as_ref()
		.and_then(|network| network.persistent_metadata.node_metadata.get(&child_id))
		.map(|child: &DocumentNodeMetadata| &child.persistent_metadata)
}

impl document_graph_storage::NodeMetadataSource for MetadataCopySource<'_> {
	fn position(&self, metadata_path: &[NodeId], node_id: NodeId) -> Option<Position> {
		match &self.metadata_for(metadata_path, node_id)?.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer) => Some(match layer.position {
				LayerPosition::Absolute(offset) => Position::Absolute([offset.x, offset.y]),
				LayerPosition::Stack(offset) => Position::Stack(offset),
			}),
			NodeTypePersistentMetadata::Node(node) => Some(match *node.position() {
				NodePosition::Absolute(offset) => Position::Absolute([offset.x, offset.y]),
				NodePosition::Chain => Position::Chain,
			}),
		}
	}

	fn is_layer(&self, metadata_path: &[NodeId], node_id: NodeId) -> bool {
		self.metadata_for(metadata_path, node_id)
			.is_some_and(|metadata| matches!(metadata.node_type_metadata, NodeTypePersistentMetadata::Layer(_)))
	}

	fn display_name(&self, metadata_path: &[NodeId], node_id: NodeId) -> Option<&str> {
		self.metadata_for(metadata_path, node_id).map(|metadata| metadata.display_name.as_str())
	}

	fn locked(&self, metadata_path: &[NodeId], node_id: NodeId) -> bool {
		self.metadata_for(metadata_path, node_id).is_some_and(|metadata| metadata.locked)
	}

	fn pinned(&self, metadata_path: &[NodeId], node_id: NodeId) -> bool {
		self.metadata_for(metadata_path, node_id).is_some_and(|metadata| metadata.pinned)
	}

	fn output_names(&self, metadata_path: &[NodeId], node_id: NodeId) -> Vec<String> {
		self.metadata_for(metadata_path, node_id).map(|metadata| metadata.output_names.clone()).unwrap_or_default()
	}

	fn input_name(&self, metadata_path: &[NodeId], node_id: NodeId, input_index: usize) -> Option<&str> {
		self.metadata_for(metadata_path, node_id)
			.and_then(|metadata| metadata.input_metadata.get(input_index))
			.map(|input| input.persistent_metadata.input_name.as_str())
	}

	fn input_description(&self, metadata_path: &[NodeId], node_id: NodeId, input_index: usize) -> Option<&str> {
		self.metadata_for(metadata_path, node_id)
			.and_then(|metadata| metadata.input_metadata.get(input_index))
			.map(|input| input.persistent_metadata.input_description.as_str())
	}

	fn widget_override(&self, metadata_path: &[NodeId], node_id: NodeId, input_index: usize) -> Option<&str> {
		self.metadata_for(metadata_path, node_id)
			.and_then(|metadata| metadata.input_metadata.get(input_index))
			.and_then(|input| input.persistent_metadata.widget_override.as_deref())
	}

	fn input_data(&self, metadata_path: &[NodeId], node_id: NodeId, input_index: usize) -> std::collections::HashMap<String, serde_json::Value> {
		self.metadata_for(metadata_path, node_id)
			.and_then(|metadata| metadata.input_metadata.get(input_index))
			.map(|input| input.persistent_metadata.input_data.clone())
			.unwrap_or_default()
	}

	fn reference(&self, network_path: &[NodeId]) -> Option<&str> {
		let (owner_path, owner_id) = network_path.split_last().map(|(last, rest)| (rest, *last))?;
		self.metadata_for(owner_path, owner_id)?
			.network_metadata
			.as_ref()
			.and_then(|network| network.persistent_metadata.reference.as_deref())
	}
}
