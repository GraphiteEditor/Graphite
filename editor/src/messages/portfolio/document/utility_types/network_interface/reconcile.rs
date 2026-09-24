//! Brings the interface into line with the working registry for the entities peers touched, so a remote
//! batch costs deltas for what changed rather than a rebuild of the whole interface.
//!
//! Each touched node is projected to its runtime form and compared with what the interface holds. The
//! difference is applied as the same `EditorDelta`s a local edit records, through `apply`, which mirrors
//! without recording, so nothing here is offered back to the peer that sent it. A node held on one side
//! only is added or removed whole. What a comparison cannot express as a field write, a changed
//! implementation or scope injection, replaces the node.
//!
//! The touched set is an over-approximation: a late-writer-wins loser or an op that failed to apply
//! names an entity that did not change, and the comparison finds nothing to emit for it.

use std::collections::{BTreeSet, HashMap};

use document_graph_storage::to_runtime::ConversionError;
use document_graph_storage::{Declarations, NodeId as StorageNodeId, ROOT_NETWORK, Registry, RuntimeProjection, Touched};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput};
use graph_craft::runtime_delta::RuntimeDelta;

use super::storage_metadata::{InterfaceRebuildError, node_metadata_from_projection};
use super::{DocumentNodePersistentMetadata, EditorDelta, NetworkMetadataChange, NodeMetadataChange, NodeNetworkInterface};

#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
	/// A declaration the touched nodes need is not cached yet. Nothing was applied; retry once it lands.
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

/// What a reconcile changed, for the caller to unload what those changes feed.
#[derive(Debug, Default)]
pub struct Reconciled {
	/// The networks a delta was applied in.
	pub networks: BTreeSet<Vec<NodeId>>,
}

/// Where the interface holds each node storage knows, keyed by the identity storage addresses it by.
type StorageIndex = HashMap<StorageNodeId, (Vec<NodeId>, NodeId)>;

impl NodeNetworkInterface {
	/// Applies to the interface whatever the registry holds differently for the touched entities.
	///
	/// Can fail part-way through, leaving some of them reconciled. That is safe to retry with the same
	/// touched set, since a reconciled node compares equal and emits nothing, which is what the caller
	/// does when a declaration is still on its way; on any other error it rebuilds from the whole registry.
	pub(crate) fn reconcile_remote(&mut self, registry: &Registry, declarations: &Declarations, touched: &Touched, peer: document_graph_storage::PeerId) -> Result<Reconciled, ReconcileError> {
		let projection = registry.runtime_projection(declarations);
		let index = self.pin_storage_identities(peer);

		// A touched network is reconciled through the node implementing it, whose comparison covers the
		// network's exports and metadata. The root has no such node.
		let mut ids = touched.nodes.clone();
		let mut root = false;
		for &network in &touched.networks {
			match projection.owner(network) {
				Some(owner) => {
					ids.insert(owner);
				}
				None => root |= network == ROOT_NETWORK,
			}
		}

		// A node the runtime cannot reach and the interface does not hold has nothing to reconcile.
		let mut items: Vec<(Vec<NodeId>, NodeId, StorageNodeId)> = ids
			.into_iter()
			.filter_map(|id| {
				let (path, local_id) = projection.node_address(id).or_else(|| index.get(&id).cloned())?;
				Some((path, local_id, id))
			})
			.collect();
		// A network's path is a prefix of the paths inside it, so this puts each node after the one
		// implementing its network, and a nested node finds its network in place.
		items.sort();

		let mut reconciled = Reconciled::default();
		for (path, local_id, id) in items {
			self.reconcile_node(&projection, &index, &path, local_id, id, &mut reconciled)?;
		}
		if root {
			let target = projection.network_entry(ROOT_NETWORK)?;
			let current_exports = self.document_network().exports.clone();
			let current = &self.document_network_metadata().persistent_metadata;
			let mut deltas = Vec::new();
			export_deltas(&[], &current_exports, &target.exports, &mut deltas);
			network_metadata_deltas(
				&[],
				current.reference.as_deref(),
				&current.pinned_node_order,
				target.metadata.reference.as_deref(),
				&target.metadata.pinned_order,
				&mut deltas,
			);
			self.apply_reconciled(&[], deltas, &mut reconciled);
		}

		Ok(reconciled)
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
		// The interface holds the node somewhere else: under another id, or in a network it has since
		// left. That copy goes before the one the registry describes is put in place.
		if let Some((held_path, held_id)) = index.get(&id)
			&& (held_path.as_slice() != path || *held_id != local_id)
			&& self.holds_node(held_path, *held_id)
		{
			self.apply_reconciled(held_path, vec![remove_node(held_path, *held_id)], reconciled);
		}

		let stored = projection.node_address(id).is_some();
		let held = self.holds_node(path, local_id);
		match (stored, held) {
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

	/// Emits what differs between the node the interface holds at `path` / `local_id` and `target`, which
	/// the registry describes. A network node's contents are left to their own reconciliation: only its
	/// exports and its network's metadata are compared here.
	fn diff_node(&mut self, path: &[NodeId], local_id: NodeId, target: DocumentNode, target_metadata: DocumentNodePersistentMetadata, reconciled: &mut Reconciled) {
		let Some(current) = self.document_network().nested_network(path).and_then(|network| network.nodes.get(&local_id)) else {
			return;
		};
		let Some(current_metadata) = self
			.document_network_metadata()
			.nested_metadata(path)
			.and_then(|network| network.persistent_metadata.node_metadata.get(&local_id))
			.map(|node| &node.persistent_metadata)
		else {
			return;
		};

		let mut deltas = Vec::new();
		let mut nested_changed = false;
		let at = |change: NodeMetadataChange| EditorDelta::NodeMetadata {
			network_path: path.to_vec(),
			node_id: local_id,
			change,
		};

		// What no field write expresses replaces the node whole, and its metadata with it.
		if !same_implementation(&current.implementation, &target.implementation) || current.skip_deduplication != target.skip_deduplication {
			nested_changed = matches!(target.implementation, DocumentNodeImplementation::Network(_)) || matches!(current.implementation, DocumentNodeImplementation::Network(_));
			deltas.push(EditorDelta::Graph(RuntimeDelta::ReplaceNode {
				network_path: path.to_vec(),
				node_id: local_id,
				node: Box::new(target),
			}));
			deltas.push(EditorDelta::NodeMetadataSnapshot {
				network_path: path.to_vec(),
				node_id: local_id,
				metadata: Box::new(target_metadata),
			});
		} else {
			// The slots first, so the metadata array that indexes them lands on a list of the right length.
			if current.inputs.len() != target.inputs.len() {
				deltas.push(EditorDelta::Graph(RuntimeDelta::SetInputs {
					network_path: path.to_vec(),
					node_id: local_id,
					inputs: target.inputs.clone(),
				}));
				deltas.push(EditorDelta::NodeInputMetadata {
					network_path: path.to_vec(),
					node_id: local_id,
					input_metadata: target_metadata.input_metadata.clone(),
				});
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
					deltas.push(EditorDelta::NodeInputMetadata {
						network_path: path.to_vec(),
						node_id: local_id,
						input_metadata: target_metadata.input_metadata.clone(),
					});
				}
			}

			// Export slot writes rename the encapsulating node's outputs as a side effect, so the names are
			// restated after them whenever the slot count moved.
			let mut restate_output_names = false;
			if let (DocumentNodeImplementation::Network(held), DocumentNodeImplementation::Network(wanted)) = (&current.implementation, &target.implementation) {
				let nested = nested_path(path, local_id);
				let before = deltas.len();
				export_deltas(&nested, &held.exports, &wanted.exports, &mut deltas);
				restate_output_names = held.exports.len() != wanted.exports.len();

				let (held_reference, held_order) = current_metadata
					.network_metadata
					.as_ref()
					.map(|network| (network.persistent_metadata.reference.as_deref(), network.persistent_metadata.pinned_node_order.as_slice()))
					.unwrap_or((None, &[]));
				let (wanted_reference, wanted_order) = target_metadata
					.network_metadata
					.as_ref()
					.map(|network| (network.persistent_metadata.reference.as_deref(), network.persistent_metadata.pinned_node_order.as_slice()))
					.unwrap_or((None, &[]));
				network_metadata_deltas(&nested, held_reference, held_order, wanted_reference, wanted_order, &mut deltas);
				nested_changed = deltas.len() != before;
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
			if current.context_features != target.context_features {
				deltas.push(EditorDelta::Graph(RuntimeDelta::SetContextFeatures {
					network_path: path.to_vec(),
					node_id: local_id,
					context_features: target.context_features,
				}));
			}

			if current_metadata.node_type_metadata != target_metadata.node_type_metadata {
				deltas.push(at(NodeMetadataChange::NodeType(target_metadata.node_type_metadata.clone())));
			}
			if current_metadata.display_name != target_metadata.display_name {
				deltas.push(at(NodeMetadataChange::DisplayName(target_metadata.display_name.clone())));
			}
			if current_metadata.locked != target_metadata.locked {
				deltas.push(at(NodeMetadataChange::Locked(target_metadata.locked)));
			}
			if current_metadata.pinned != target_metadata.pinned {
				deltas.push(at(NodeMetadataChange::Pinned(target_metadata.pinned)));
			}
			if restate_output_names || current_metadata.output_names != target_metadata.output_names {
				deltas.push(at(NodeMetadataChange::OutputNames(target_metadata.output_names.clone())));
			}
		}

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
			&& self
				.document_network_metadata()
				.nested_metadata(path)
				.is_some_and(|network| network.persistent_metadata.node_metadata.contains_key(&local_id))
	}

	fn holds_network(&self, path: &[NodeId]) -> bool {
		self.document_network().nested_network(path).is_some() && self.document_network_metadata().nested_metadata(path).is_some()
	}
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

/// Whether a field-by-field comparison can bring `current` to `target`, which it cannot across a
/// change of what the node computes.
fn same_implementation(current: &DocumentNodeImplementation, target: &DocumentNodeImplementation) -> bool {
	match (current, target) {
		(DocumentNodeImplementation::ProtoNode(held), DocumentNodeImplementation::ProtoNode(wanted)) => held == wanted,
		(DocumentNodeImplementation::Network(held), DocumentNodeImplementation::Network(wanted)) => held.scope_injections == wanted.scope_injections,
		_ => false,
	}
}

/// The slot writes taking `current` to `target`: a write per slot that differs, then a removal at the
/// end for each slot too many, which the mirror applies as removals of that index in turn.
fn export_deltas(network_path: &[NodeId], current: &[NodeInput], target: &[NodeInput], deltas: &mut Vec<EditorDelta>) {
	for (export_index, input) in target.iter().enumerate() {
		if current.get(export_index) != Some(input) {
			deltas.push(EditorDelta::Graph(RuntimeDelta::SetExport {
				network_path: network_path.to_vec(),
				export_index,
				input: Some(input.clone()),
			}));
		}
	}
	for _ in target.len()..current.len() {
		deltas.push(EditorDelta::Graph(RuntimeDelta::SetExport {
			network_path: network_path.to_vec(),
			export_index: target.len(),
			input: None,
		}));
	}
}

fn network_metadata_deltas(network_path: &[NodeId], current_reference: Option<&str>, current_order: &[NodeId], target_reference: Option<&str>, target_order: &[NodeId], deltas: &mut Vec<EditorDelta>) {
	if current_reference != target_reference {
		deltas.push(EditorDelta::NetworkMetadata {
			network_path: network_path.to_vec(),
			change: NetworkMetadataChange::Reference(target_reference.map(str::to_string)),
		});
	}
	if current_order != target_order {
		deltas.push(EditorDelta::NetworkMetadata {
			network_path: network_path.to_vec(),
			change: NetworkMetadataChange::PinnedOrder(target_order.to_vec()),
		});
	}
}
