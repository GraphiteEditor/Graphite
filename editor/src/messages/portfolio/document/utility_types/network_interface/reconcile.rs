//! Brings the interface into line with the working registry for the entities peers touched, applying the
//! difference as the `EditorDelta`s a local edit records, mirrored without recording them.
//!
//! The touched set over-approximates. An entity that lost a late-writer-wins race or a failed op names is
//! unchanged, and its comparison emits nothing.

use std::collections::{BTreeSet, HashMap};

use document_graph_storage::to_runtime::ConversionError;
use document_graph_storage::{Declarations, NetworkId as StorageNetworkId, NodeId as StorageNodeId, Registry, RuntimeProjection, Touched};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NetworkId, NodeId, NodeInput, NodeNetwork, OwnedNetworkLocation};
use graph_craft::runtime_delta::RuntimeDelta;

use super::storage_metadata::{InterfaceRebuildError, input_metadata_entry_to_runtime, node_metadata_from_projection};
use super::{DocumentNodePersistentMetadata, EditorDelta, NetworkMetadataChange, NodeMetadataChange, NodeNetworkInterface};

#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
	/// A declaration the touched nodes need is not cached yet.
	#[error("waiting for proto-node declaration {0}")]
	DeclarationMissing(document_graph_storage::ResourceId),
	#[error("the interface lacks the network at {0:?} that the registry places a node in")]
	NetworkMissing(OwnedNetworkLocation),
	/// No delta writes a held network's scope injections.
	#[error("the scope injections of the network at {0:?} changed")]
	ScopeInjectionsChanged(OwnedNetworkLocation),
	#[error(transparent)]
	Conversion(ConversionError),
	#[error(transparent)]
	Metadata(#[from] InterfaceRebuildError),
}

impl From<ConversionError> for ReconcileError {
	fn from(error: ConversionError) -> Self {
		match error {
			ConversionError::DeclarationNotFound(id) => Self::DeclarationMissing(id),
			other => Self::Conversion(other),
		}
	}
}

/// What a reconcile changed.
#[derive(Debug, Default)]
pub struct Reconciled {
	/// The networks a delta was applied in.
	pub networks: BTreeSet<OwnedNetworkLocation>,
}

/// Where the interface holds each node, keyed by its storage identity.
type StorageIndex = HashMap<StorageNodeId, (OwnedNetworkLocation, NodeId)>;

impl NodeNetworkInterface {
	/// Applies to the interface whatever the registry holds differently for the touched entities.
	///
	/// Can fail part-way through. Retrying with the same touched set is safe, since a reconciled entity compares equal.
	pub(crate) fn reconcile_remote(&mut self, registry: &Registry, declarations: &Declarations, touched: &Touched, peer: document_graph_storage::PeerId) -> Result<Reconciled, ReconcileError> {
		let projection = registry.runtime_projection(declarations);
		let index = self.pin_storage_identities(peer);
		let mut reconciled = Reconciled::default();

		let mut nodes = touched.nodes.clone();
		let mut networks = touched.networks.clone();
		networks.extend(nodes.iter().filter_map(|&node| projection.implemented_network(node)));
		self.add_newly_reached_networks(&projection, &mut nodes, &mut networks, &mut reconciled)?;

		let mut items: Vec<(OwnedNetworkLocation, NodeId, StorageNodeId)> = nodes
			.into_iter()
			.filter_map(|id| {
				let (location, local_id) = projection.node_address(id).or_else(|| index.get(&id).cloned())?;
				Some((location, local_id, id))
			})
			.collect();
		items.sort();
		for (location, local_id, id) in items {
			self.reconcile_node(&projection, &index, &location, local_id, id, &mut reconciled)?;
		}

		for &network in &networks {
			if projection.network_location(network).is_some() {
				self.reconcile_network(&projection, network, &mut reconciled)?;
			}
		}
		self.remove_unreached_networks(&projection, &mut reconciled);

		Ok(reconciled)
	}

	/// Adds each table entry the touched entities newly reach, with the entries and nodes under it queued as if touched, since
	/// a network the root did not reach before may hold nothing peers just wrote.
	fn add_newly_reached_networks(
		&mut self,
		projection: &RuntimeProjection,
		nodes: &mut BTreeSet<StorageNodeId>,
		networks: &mut BTreeSet<StorageNetworkId>,
		reconciled: &mut Reconciled,
	) -> Result<(), ReconcileError> {
		let mut pending: Vec<StorageNetworkId> = networks.iter().copied().collect();
		while let Some(network) = pending.pop() {
			let Some(location) = projection.network_location(network) else { continue };
			let Some(id) = location.definition else { continue };
			if self.holds_network(&location) {
				continue;
			}

			let target = projection.network_entry(network)?;
			let added = NodeNetwork {
				exports: target.exports,
				scope_injections: target.scope_injections,
				..Default::default()
			};
			self.apply_reconciled(&location, vec![EditorDelta::Graph(RuntimeDelta::AddNetwork { id, network: Box::new(added) })], reconciled);

			networks.insert(network);
			for node in projection.network_nodes(network) {
				nodes.insert(node);
				pending.extend(projection.implemented_network(node));
			}
		}
		Ok(())
	}

	/// Drops the table entries the root no longer reaches, which a whole rebuild would not hold.
	fn remove_unreached_networks(&mut self, projection: &RuntimeProjection, reconciled: &mut Reconciled) {
		let reached: BTreeSet<NetworkId> = projection.definitions().collect();
		let unreached: Vec<NetworkId> = self.document_network().definitions.keys().copied().filter(|id| !reached.contains(id)).collect();
		for id in unreached {
			self.apply_reconciled(&OwnedNetworkLocation::definition(id), vec![EditorDelta::Graph(RuntimeDelta::RemoveNetwork { id })], reconciled);
		}
	}

	fn reconcile_network(&mut self, projection: &RuntimeProjection, network: StorageNetworkId, reconciled: &mut Reconciled) -> Result<(), ReconcileError> {
		let target = projection.network_entry(network)?;
		let location = target.network;
		let (Some(current), Some(current_metadata)) = (
			self.document_network().located_network(location.as_location()),
			self.network_metadata.nested_metadata(location.as_location()).map(|metadata| &metadata.persistent_metadata),
		) else {
			return Err(ReconcileError::NetworkMissing(location));
		};
		if current.scope_injections != target.scope_injections {
			return Err(ReconcileError::ScopeInjectionsChanged(location));
		}

		let mut deltas = Vec::new();
		export_deltas(&location, &current.exports, &target.exports, &mut deltas);
		// An export slot write resizes the names and descriptions, so a change of slot count restates them
		let resized = current.exports.len() != target.exports.len();

		let change = |change| EditorDelta::NetworkMetadata { network: location.clone(), change };
		if current_metadata.reference != target.metadata.reference {
			deltas.push(change(NetworkMetadataChange::Reference(target.metadata.reference.clone())));
		}
		if current_metadata.pinned_node_order != target.metadata.pinned_order {
			deltas.push(change(NetworkMetadataChange::PinnedOrder(target.metadata.pinned_order.clone())));
		}
		// An empty list states nothing, since an older document keeps its interface on the instances
		let import_metadata: Vec<_> = target.metadata.import_metadata.iter().cloned().map(input_metadata_entry_to_runtime).collect();
		if !import_metadata.is_empty() && current_metadata.import_metadata != import_metadata {
			deltas.push(change(NetworkMetadataChange::ImportMetadata(import_metadata)));
		}
		if !target.metadata.export_names.is_empty() && (resized || current_metadata.export_names != target.metadata.export_names) {
			deltas.push(change(NetworkMetadataChange::ExportNames(target.metadata.export_names.clone())));
		}
		if !target.metadata.export_names.is_empty() && (resized || current_metadata.export_descriptions != target.metadata.export_descriptions) {
			deltas.push(change(NetworkMetadataChange::ExportDescriptions(target.metadata.export_descriptions.clone())));
		}

		self.apply_reconciled(&location, deltas, reconciled);
		Ok(())
	}

	fn reconcile_node(
		&mut self,
		projection: &RuntimeProjection,
		index: &StorageIndex,
		location: &OwnedNetworkLocation,
		local_id: NodeId,
		id: StorageNodeId,
		reconciled: &mut Reconciled,
	) -> Result<(), ReconcileError> {
		// A copy held under another id or in another network goes first
		if let Some((held_location, held_id)) = index.get(&id)
			&& (held_location != location || *held_id != local_id)
			&& self.holds_node(held_location, *held_id)
		{
			self.apply_reconciled(held_location, vec![remove_node(held_location, *held_id)], reconciled);
		}

		let stored = projection.node_address(id).is_some();
		match (stored, self.holds_node(location, local_id)) {
			(false, false) => {}
			(false, true) => self.apply_reconciled(location, vec![remove_node(location, local_id)], reconciled),
			(true, false) => {
				if !self.holds_network(location) {
					return Err(ReconcileError::NetworkMissing(location.clone()));
				}
				let (node, metadata) = node_metadata_from_projection(projection.node(id)?)?;
				let deltas = vec![
					EditorDelta::Graph(RuntimeDelta::AddNode {
						network: location.clone(),
						node_id: local_id,
						node: Box::new(node),
					}),
					EditorDelta::NodeMetadataSnapshot {
						network: location.clone(),
						node_id: local_id,
						metadata: Box::new(metadata),
					},
				];
				self.apply_reconciled(location, deltas, reconciled);
			}
			(true, true) => {
				let (node, metadata) = node_metadata_from_projection(projection.node(id)?)?;
				self.diff_node(location, local_id, node, metadata, reconciled);
			}
		}
		Ok(())
	}

	/// Applies what differs between the node the interface holds and `target`.
	fn diff_node(&mut self, location: &OwnedNetworkLocation, local_id: NodeId, target: DocumentNode, target_metadata: DocumentNodePersistentMetadata, reconciled: &mut Reconciled) {
		let Some(current) = self.document_network().located_network(location.as_location()).and_then(|network| network.nodes.get(&local_id)) else {
			return;
		};
		let Some(current_metadata) = self
			.network_metadata
			.nested_metadata(location.as_location())
			.and_then(|network| network.persistent_metadata.node_metadata.get(&local_id))
			.map(|node| &node.persistent_metadata)
		else {
			return;
		};

		let deltas = if current.implementation == target.implementation && current.skip_deduplication == target.skip_deduplication {
			field_deltas(location, local_id, (current, current_metadata), (&target, &target_metadata))
		} else {
			vec![
				EditorDelta::Graph(RuntimeDelta::ReplaceNode {
					network: location.clone(),
					node_id: local_id,
					node: Box::new(target),
				}),
				EditorDelta::NodeMetadataSnapshot {
					network: location.clone(),
					node_id: local_id,
					metadata: Box::new(target_metadata),
				},
			]
		};
		self.apply_reconciled(location, deltas, reconciled);
	}

	fn apply_reconciled(&mut self, location: &OwnedNetworkLocation, deltas: Vec<EditorDelta>, reconciled: &mut Reconciled) {
		if deltas.is_empty() {
			return;
		}
		for delta in &deltas {
			self.apply(delta);
		}
		reconciled.networks.insert(location.clone());
	}

	/// Every path through the document that reaches one of `locations`. A shared network has one per instance.
	pub(crate) fn paths_reaching(&self, locations: &BTreeSet<OwnedNetworkLocation>) -> Vec<Vec<NodeId>> {
		let root = self.document_network();
		let mut paths = Vec::new();
		let mut pending = vec![(Vec::new(), root)];
		while let Some((path, network)) = pending.pop() {
			for (&node_id, node) in &network.nodes {
				let nested = match &node.implementation {
					DocumentNodeImplementation::Network(nested) => Some(nested),
					DocumentNodeImplementation::Definition(id) => root.definitions.get(id),
					_ => None,
				};
				if let Some(nested) = nested {
					pending.push(([path.as_slice(), &[node_id]].concat(), nested));
				}
			}
			if root.locate(&path).is_some_and(|location| locations.contains(&location.to_owned_location())) {
				paths.push(path);
			}
		}
		paths
	}

	fn holds_node(&self, location: &OwnedNetworkLocation, local_id: NodeId) -> bool {
		self.document_network()
			.located_network(location.as_location())
			.is_some_and(|network| network.nodes.contains_key(&local_id))
			&& self
				.network_metadata
				.nested_metadata(location.as_location())
				.is_some_and(|network| network.persistent_metadata.node_metadata.contains_key(&local_id))
	}

	fn holds_network(&self, location: &OwnedNetworkLocation) -> bool {
		self.document_network().located_network(location.as_location()).is_some() && self.network_metadata.nested_metadata(location.as_location()).is_some()
	}
}

/// The field writes taking a node to `target` when its implementation stays.
fn field_deltas(
	location: &OwnedNetworkLocation,
	local_id: NodeId,
	(current, current_metadata): (&DocumentNode, &DocumentNodePersistentMetadata),
	(target, target_metadata): (&DocumentNode, &DocumentNodePersistentMetadata),
) -> Vec<EditorDelta> {
	let mut deltas = Vec::new();
	let metadata = |change: NodeMetadataChange| EditorDelta::NodeMetadata {
		network: location.clone(),
		node_id: local_id,
		change,
	};
	let input_metadata = || EditorDelta::NodeInputMetadata {
		network: location.clone(),
		node_id: local_id,
		input_metadata: target_metadata.input_metadata.clone(),
	};

	// An instance's input metadata belongs to the network implementing it, which is compared on its own
	let instance = matches!(target.implementation, DocumentNodeImplementation::Definition(_));

	// The slots go first, so the metadata indexing them lands on a list of the right length
	if current.inputs.len() != target.inputs.len() {
		deltas.push(EditorDelta::Graph(RuntimeDelta::SetInputs {
			network: location.clone(),
			node_id: local_id,
			inputs: target.inputs.clone(),
		}));
		if !instance {
			deltas.push(input_metadata());
		}
	} else {
		for (input_index, (held, wanted)) in current.inputs.iter().zip(&target.inputs).enumerate() {
			if held != wanted {
				deltas.push(EditorDelta::Graph(RuntimeDelta::SetInput {
					network: location.clone(),
					node_id: local_id,
					input_index,
					input: wanted.clone(),
				}));
			}
		}
		if !instance && current_metadata.input_metadata != target_metadata.input_metadata {
			deltas.push(input_metadata());
		}
	}

	if current.visible != target.visible {
		deltas.push(EditorDelta::Graph(RuntimeDelta::SetVisibility {
			network: location.clone(),
			node_id: local_id,
			visible: target.visible,
		}));
	}
	if current.call_argument != target.call_argument {
		deltas.push(EditorDelta::Graph(RuntimeDelta::SetCallArgument {
			network: location.clone(),
			node_id: local_id,
			call_argument: target.call_argument.clone(),
		}));
	}

	if current_metadata.node_type_metadata != target_metadata.node_type_metadata {
		deltas.push(metadata(NodeMetadataChange::NodeType(target_metadata.node_type_metadata.clone())));
	}
	if current_metadata.display_name != target_metadata.display_name {
		deltas.push(metadata(NodeMetadataChange::DisplayName(target_metadata.display_name.clone())));
	}
	if current_metadata.locked != target_metadata.locked {
		deltas.push(metadata(NodeMetadataChange::Locked(target_metadata.locked)));
	}
	if current_metadata.pinned != target_metadata.pinned {
		deltas.push(metadata(NodeMetadataChange::Pinned(target_metadata.pinned)));
	}
	if current_metadata.output_names != target_metadata.output_names {
		deltas.push(metadata(NodeMetadataChange::OutputNames(target_metadata.output_names.clone())));
	}
	if current_metadata.output_descriptions != target_metadata.output_descriptions {
		deltas.push(metadata(NodeMetadataChange::OutputDescriptions(target_metadata.output_descriptions.clone())));
	}

	deltas
}

fn remove_node(location: &OwnedNetworkLocation, local_id: NodeId) -> EditorDelta {
	EditorDelta::Graph(RuntimeDelta::RemoveNode {
		network: location.clone(),
		node_id: local_id,
	})
}

/// A write per export slot that differs, then a removal at the end for each slot too many.
fn export_deltas(location: &OwnedNetworkLocation, current: &[NodeInput], target: &[NodeInput], deltas: &mut Vec<EditorDelta>) {
	let set_export = |export_index, input| {
		EditorDelta::Graph(RuntimeDelta::SetExport {
			network: location.clone(),
			export_index,
			input,
		})
	};
	let changed = target.iter().enumerate().filter(|&(export_index, input)| current.get(export_index) != Some(input));
	deltas.extend(changed.map(|(export_index, input)| set_export(export_index, Some(input.clone()))));
	deltas.extend((target.len()..current.len()).map(|_| set_export(target.len(), None)));
}
