use super::{DocumentNodeMetadata, DocumentNodePersistentMetadata, LayerPosition, NodePosition, NodeTypePersistentMetadata};
use document_graph_storage::attr::node as node_attr;
use document_graph_storage::from_runtime::{ConversionError, DeclarationBytes};
use document_graph_storage::{AttributeDelta, Attributes, Implementation, NoMetadata, PathResolver, Position, Registry, RegistryDelta, ScopedConversion, TimeStamp};
use document_graph_storage::{convert_resource_entry, encode_input_ui_attributes, encode_node_ui_attributes, node_value_resource_refs, value_resource_ref};
use graph_craft::application_io::resource::{ResourceId, ResourceRegistry};
use graph_craft::document::NodeId;
use graph_craft::runtime_delta::RuntimeDelta;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum EditorDelta {
	Graph(RuntimeDelta),
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

impl EditorDelta {
	pub fn to_registry_deltas(&self, working: &Registry, resources: &ResourceRegistry, peer: document_graph_storage::PeerId) -> Result<ConstructedOps, ConversionError> {
		let resolver = PathResolver::new(peer);
		let mut ops = Vec::new();
		let mut declaration_bytes = DeclarationBytes::new();

		match self {
			EditorDelta::Graph(RuntimeDelta::AddNode { network_path, node_id, node }) => {
				declaration_bytes = construct_structural_additions(network_path, *node_id, node, working, resources, peer, &mut ops)?;
			}

			EditorDelta::Graph(RuntimeDelta::ReplaceNode { network_path, node_id, node }) => {
				construct_removals(resolver.node_id(network_path, *node_id), working, &mut ops);
				declaration_bytes = construct_structural_additions(network_path, *node_id, node, working, resources, peer, &mut ops)?;
			}

			EditorDelta::Graph(RuntimeDelta::RemoveNode { network_path, node_id }) => {
				construct_removals(resolver.node_id(network_path, *node_id), working, &mut ops);
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
				construct_metadata_changes(network_path, *node_id, metadata, working, &resolver, &mut ops)?;
			}
		}

		Ok(ConstructedOps { ops, declaration_bytes })
	}
}

fn construct_structural_additions(
	network_path: &[NodeId],
	node_id: NodeId,
	node: &graph_craft::document::DocumentNode,
	working: &Registry,
	resources: &ResourceRegistry,
	peer: document_graph_storage::PeerId,
	ops: &mut Vec<RegistryDelta>,
) -> Result<DeclarationBytes, ConversionError> {
	let mut scoped = ScopedConversion::new(&NoMetadata, peer);
	let mut scratch = Registry::default();
	scoped.convert_node_at(&mut scratch, network_path, node_id, node, true)?;
	let declaration_bytes = scoped.finish();

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
		ops.push(RegistryDelta::AddResource { id: *id, entry: entry.clone() });
	}
	let mut tagged: Vec<ResourceId> = scratch.node_instances.values().flat_map(node_value_resource_refs).collect();
	tagged.sort();
	tagged.dedup();
	for id in tagged {
		if !working.resources.contains_key(&id)
			&& !scratch.resources.contains_key(&id)
			&& let Some(entry) = convert_resource_entry(resources, id, peer)?
		{
			ops.push(RegistryDelta::AddResource { id, entry });
		}
	}

	Ok(declaration_bytes)
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

	let removed_node_set: HashSet<_> = removed_nodes.iter().copied().collect();
	let mut candidates: Vec<ResourceId> = removed_nodes
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
