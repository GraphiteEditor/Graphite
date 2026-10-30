//! Brings the interface into line with the working registry for the entities peers touched, applying the
//! difference as the `EditorDelta`s a local edit records, mirrored without recording them.
//!
//! The touched set over-approximates: an entity that lost a late-writer-wins race or a failed op names is
//! unchanged, and its comparison emits nothing.

use std::collections::{BTreeSet, HashMap};

use document_graph_storage::to_runtime::ConversionError;
use document_graph_storage::{Declarations, NodeId as StorageNodeId, ROOT_NETWORK, Registry, RuntimeProjection, Touched};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput};
use graph_craft::runtime_delta::RuntimeDelta;

use super::storage_metadata::{InterfaceRebuildError, node_metadata_from_projection};
use super::{DocumentNodePersistentMetadata, EditorDelta, NetworkMetadataChange, NodeMetadataChange, NodeNetworkInterface};

#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
	/// A declaration the touched nodes need is not cached yet.
	#[error("waiting for proto-node declaration {0}")]
	DeclarationMissing(document_graph_storage::ResourceId),
	#[error("the interface lacks the network at {0:?} that the registry places a node in")]
	NetworkMissing(Vec<NodeId>),
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
	pub networks: BTreeSet<Vec<NodeId>>,
}

/// Where the interface holds each node, keyed by its storage identity.
type StorageIndex = HashMap<StorageNodeId, (Vec<NodeId>, NodeId)>;

impl NodeNetworkInterface {
	/// Applies to the interface whatever the registry holds differently for the touched entities.
	///
	/// Can fail part-way through. Retrying with the same touched set is safe, since a reconciled node compares equal.
	pub(crate) fn reconcile_remote(&mut self, registry: &Registry, declarations: &Declarations, touched: &Touched, peer: document_graph_storage::PeerId) -> Result<Reconciled, ReconcileError> {
		let projection = registry.runtime_projection(declarations);
		let index = self.pin_storage_identities(peer);

		// A network is compared through the node implementing it; the root has none
		let mut ids = touched.nodes.clone();
		ids.extend(touched.networks.iter().filter_map(|&network| projection.owner(network)));

		let mut items: Vec<(Vec<NodeId>, NodeId, StorageNodeId)> = ids
			.into_iter()
			.filter_map(|id| {
				let (path, local_id) = projection.node_address(id).or_else(|| index.get(&id).cloned())?;
				Some((path, local_id, id))
			})
			.collect();
		// Sorting by path reconciles each network node before the nodes inside it
		items.sort();

		let mut reconciled = Reconciled::default();
		for (path, local_id, id) in items {
			self.reconcile_node(&projection, &index, &path, local_id, id, &mut reconciled)?;
		}
		if touched.networks.contains(&ROOT_NETWORK) {
			self.reconcile_root_network(&projection, &mut reconciled)?;
		}

		Ok(reconciled)
	}

	fn reconcile_root_network(&mut self, projection: &RuntimeProjection, reconciled: &mut Reconciled) -> Result<(), ReconcileError> {
		let target = projection.network_entry(ROOT_NETWORK)?;
		let current = &self.document_network_metadata().persistent_metadata;

		let mut deltas = Vec::new();
		export_deltas(&[], &self.document_network().exports, &target.exports, &mut deltas);
		network_metadata_deltas(
			&[],
			(current.reference.as_deref(), &current.pinned_node_order),
			(target.metadata.reference.as_deref(), &target.metadata.pinned_order),
			&mut deltas,
		);
		self.apply_reconciled(&[], deltas, reconciled);
		Ok(())
	}

	fn reconcile_node(
		&mut self,
		projection: &RuntimeProjection,
		index: &StorageIndex,
		path: &[NodeId],
		local_id: NodeId,
		id: StorageNodeId,
		reconciled: &mut Reconciled,
	) -> Result<(), ReconcileError> {
		// A copy held under another id or in another network goes first
		if let Some((held_path, held_id)) = index.get(&id)
			&& (held_path.as_slice() != path || *held_id != local_id)
			&& self.holds_node(held_path, *held_id)
		{
			self.apply_reconciled(held_path, vec![remove_node(held_path, *held_id)], reconciled);
		}

		let stored = projection.node_address(id).is_some();
		match (stored, self.holds_node(path, local_id)) {
			(false, false) => {}
			(false, true) => self.apply_reconciled(path, vec![remove_node(path, local_id)], reconciled),
			(true, false) => {
				if !self.holds_network(path) {
					return Err(ReconcileError::NetworkMissing(path.to_vec()));
				}
				let (node, metadata) = node_metadata_from_projection(projection.node(id)?)?;
				let nested = matches!(node.implementation, DocumentNodeImplementation::Network(_));
				let deltas = vec![
					EditorDelta::Graph(RuntimeDelta::AddNode {
						network_path: path.to_vec(),
						node_id: local_id,
						node: Box::new(node),
					}),
					EditorDelta::NodeMetadataSnapshot {
						network_path: path.to_vec(),
						node_id: local_id,
						metadata: Box::new(metadata),
					},
				];
				self.apply_reconciled(path, deltas, reconciled);
				if nested {
					reconciled.networks.insert(nested_path(path, local_id));
				}
			}
			(true, true) => {
				let (node, metadata) = node_metadata_from_projection(projection.node(id)?)?;
				self.diff_node(path, local_id, node, metadata, reconciled);
			}
		}
		Ok(())
	}

	/// Applies what differs between the node the interface holds and `target`. A network node's contents are left to their
	/// own reconciliation: only its exports and its network's metadata are compared here.
	fn diff_node(&mut self, path: &[NodeId], local_id: NodeId, target: DocumentNode, target_metadata: DocumentNodePersistentMetadata, reconciled: &mut Reconciled) {
		let Some(current) = self.document_network().nested_network(path).and_then(|network| network.nodes.get(&local_id)) else {
			return;
		};
		let Some(current_metadata) = self
			.nested_metadata(path)
			.and_then(|network| network.persistent_metadata.node_metadata.get(&local_id))
			.map(|node| &node.persistent_metadata)
		else {
			return;
		};

		let (deltas, nested_changed) = if same_implementation(&current.implementation, &target.implementation) && current.skip_deduplication == target.skip_deduplication {
			field_deltas(path, local_id, (current, current_metadata), (&target, &target_metadata))
		} else {
			let nested_changed = is_network(&current.implementation) || is_network(&target.implementation);
			let deltas = vec![
				EditorDelta::Graph(RuntimeDelta::ReplaceNode {
					network_path: path.to_vec(),
					node_id: local_id,
					node: Box::new(target),
				}),
				EditorDelta::NodeMetadataSnapshot {
					network_path: path.to_vec(),
					node_id: local_id,
					metadata: Box::new(target_metadata),
				},
			];
			(deltas, nested_changed)
		};

		if deltas.is_empty() {
			return;
		}
		self.apply_reconciled(path, deltas, reconciled);
		if nested_changed {
			reconciled.networks.insert(nested_path(path, local_id));
		}
	}

	fn apply_reconciled(&mut self, path: &[NodeId], deltas: Vec<EditorDelta>, reconciled: &mut Reconciled) {
		if deltas.is_empty() {
			return;
		}
		for delta in &deltas {
			self.apply(delta);
		}
		reconciled.networks.insert(path.to_vec());
	}

	fn holds_node(&self, path: &[NodeId], local_id: NodeId) -> bool {
		self.document_network().nested_network(path).is_some_and(|network| network.nodes.contains_key(&local_id))
			&& self.nested_metadata(path).is_some_and(|network| network.persistent_metadata.node_metadata.contains_key(&local_id))
	}

	fn holds_network(&self, path: &[NodeId]) -> bool {
		self.document_network().nested_network(path).is_some() && self.nested_metadata(path).is_some()
	}
}

/// The field writes taking a node to `target` when its implementation stays, and whether they changed its nested network.
fn field_deltas(
	path: &[NodeId],
	local_id: NodeId,
	(current, current_metadata): (&DocumentNode, &DocumentNodePersistentMetadata),
	(target, target_metadata): (&DocumentNode, &DocumentNodePersistentMetadata),
) -> (Vec<EditorDelta>, bool) {
	let mut deltas = Vec::new();
	let metadata = |change: NodeMetadataChange| EditorDelta::NodeMetadata {
		network_path: path.to_vec(),
		node_id: local_id,
		change,
	};
	let input_metadata = || EditorDelta::NodeInputMetadata {
		network_path: path.to_vec(),
		node_id: local_id,
		input_metadata: target_metadata.input_metadata.clone(),
	};

	// The slots go first, so the metadata indexing them lands on a list of the right length
	if current.inputs.len() != target.inputs.len() {
		deltas.push(EditorDelta::Graph(RuntimeDelta::SetInputs {
			network_path: path.to_vec(),
			node_id: local_id,
			inputs: target.inputs.clone(),
		}));
		deltas.push(input_metadata());
	} else {
		for (input_index, (held, wanted)) in current.inputs.iter().zip(&target.inputs).enumerate() {
			if held != wanted {
				deltas.push(EditorDelta::Graph(RuntimeDelta::SetInput {
					network_path: path.to_vec(),
					node_id: local_id,
					input_index,
					input: wanted.clone(),
				}));
			}
		}
		if current_metadata.input_metadata != target_metadata.input_metadata {
			deltas.push(input_metadata());
		}
	}

	let mut nested_changed = false;
	// Export slot writes rename the encapsulating node's outputs, so the names are restated after a slot count change
	let mut restate_output_names = false;
	if let (DocumentNodeImplementation::Network(held), DocumentNodeImplementation::Network(wanted)) = (&current.implementation, &target.implementation) {
		let nested = nested_path(path, local_id);
		let before = deltas.len();
		export_deltas(&nested, &held.exports, &wanted.exports, &mut deltas);
		network_metadata_deltas(&nested, nested_network_metadata(current_metadata), nested_network_metadata(target_metadata), &mut deltas);
		nested_changed = deltas.len() != before;
		restate_output_names = held.exports.len() != wanted.exports.len();
	}

	if current.visible != target.visible {
		deltas.push(EditorDelta::Graph(RuntimeDelta::SetVisibility {
			network_path: path.to_vec(),
			node_id: local_id,
			visible: target.visible,
		}));
	}
	if current.call_argument != target.call_argument {
		deltas.push(EditorDelta::Graph(RuntimeDelta::SetCallArgument {
			network_path: path.to_vec(),
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
	if restate_output_names || current_metadata.output_names != target_metadata.output_names {
		deltas.push(metadata(NodeMetadataChange::OutputNames(target_metadata.output_names.clone())));
	}

	(deltas, nested_changed)
}

fn remove_node(path: &[NodeId], local_id: NodeId) -> EditorDelta {
	EditorDelta::Graph(RuntimeDelta::RemoveNode {
		network_path: path.to_vec(),
		node_id: local_id,
	})
}

fn nested_path(path: &[NodeId], local_id: NodeId) -> Vec<NodeId> {
	[path, &[local_id]].concat()
}

fn is_network(implementation: &DocumentNodeImplementation) -> bool {
	matches!(implementation, DocumentNodeImplementation::Network(_))
}

/// Whether field writes can take `current` to `target`, which they cannot across a change of what the node computes.
fn same_implementation(current: &DocumentNodeImplementation, target: &DocumentNodeImplementation) -> bool {
	match (current, target) {
		(DocumentNodeImplementation::ProtoNode(held), DocumentNodeImplementation::ProtoNode(wanted)) => held == wanted,
		(DocumentNodeImplementation::Network(held), DocumentNodeImplementation::Network(wanted)) => held.scope_injections == wanted.scope_injections,
		_ => false,
	}
}

/// The reference and pinned node order of a network node's nested network.
fn nested_network_metadata(metadata: &DocumentNodePersistentMetadata) -> (Option<&str>, &[NodeId]) {
	metadata
		.network_metadata
		.as_ref()
		.map(|network| (network.persistent_metadata.reference.as_deref(), network.persistent_metadata.pinned_node_order.as_slice()))
		.unwrap_or((None, &[]))
}

/// A write per export slot that differs, then a removal at the end for each slot too many.
fn export_deltas(network_path: &[NodeId], current: &[NodeInput], target: &[NodeInput], deltas: &mut Vec<EditorDelta>) {
	let set_export = |export_index, input| {
		EditorDelta::Graph(RuntimeDelta::SetExport {
			network_path: network_path.to_vec(),
			export_index,
			input,
		})
	};
	let changed = target.iter().enumerate().filter(|&(export_index, input)| current.get(export_index) != Some(input));
	deltas.extend(changed.map(|(export_index, input)| set_export(export_index, Some(input.clone()))));
	deltas.extend((target.len()..current.len()).map(|_| set_export(target.len(), None)));
}

/// Writes for a network's reference and pinned node order, each given as `(reference, pinned order)`.
fn network_metadata_deltas(network_path: &[NodeId], current: (Option<&str>, &[NodeId]), target: (Option<&str>, &[NodeId]), deltas: &mut Vec<EditorDelta>) {
	let change = |change| EditorDelta::NetworkMetadata {
		network_path: network_path.to_vec(),
		change,
	};
	if current.0 != target.0 {
		deltas.push(change(NetworkMetadataChange::Reference(target.0.map(str::to_string))));
	}
	if current.1 != target.1 {
		deltas.push(change(NetworkMetadataChange::PinnedOrder(target.1.to_vec())));
	}
}
