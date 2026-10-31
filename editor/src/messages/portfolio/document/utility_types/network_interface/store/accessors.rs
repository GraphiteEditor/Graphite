//! Every route to a mutable reference inside the two trees, gathered here so the whole mutable
//! surface can be read in one place.
//!
//! Two tiers of visibility carry the rule. `pub(super)` reaches only the rest of `store` and hands
//! out the raw trees, so an unrecorded write to the document cannot be spelled outside this module.
//! `pub(in super::super)` reaches the rest of the network interface, and hands out only state the
//! registry never holds: transient caches, and this peer's view state.

use super::*;

impl NodeNetworkInterface {
	// ===== Raw trees, reachable only from the store =====

	/// The root of the graph tree.
	fn document_network_mut(&mut self) -> &mut NodeNetwork {
		self.network.get_mut().network_mut()
	}

	/// The graph of the network at `network_path`, written only by the store's primitives.
	pub(super) fn network_graph_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetwork> {
		self.document_network_mut().nested_network_mut(network_path)
	}

	/// The metadata tree at `network_path`.
	fn network_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkMetadata> {
		let location = self.locate(network_path)?;
		self.network_metadata.get_mut().nested_metadata_mut(location)
	}

	/// The persistent metadata of the network at `network_path`, or `None` for the document network, which
	/// no node implements.
	pub(super) fn subnetwork_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkPersistentMetadata> {
		if network_path.is_empty() {
			return None;
		}
		Some(&mut self.network_metadata_mut(network_path)?.persistent_metadata)
	}

	// TODO: Eventually remove this document upgrade code, along with the migrations that call it
	/// Rewrites the whole graph in place, for a document arriving in an older shape than the current one.
	///
	/// Nothing is recorded: a migration produces the document the peer should have had rather than
	/// changing the one they have.
	pub(in super::super) fn migrate_graph(&mut self, migrate: impl FnOnce(&mut NodeNetwork)) {
		migrate(self.document_network_mut());
	}

	// TODO: Eventually remove this document upgrade code
	/// Rewrites the whole metadata tree in place against the graph, for a document arriving in an older shape. Unrecorded, like [`Self::migrate_graph`].
	pub(in super::super) fn migrate_metadata(&mut self, migrate: impl FnOnce(&mut NodeNetworkMetadata, &NodeNetwork)) {
		migrate(self.network_metadata.get_mut(), self.network.network());
	}

	/// Updates the definitions table after a transaction.
	/// It lifts the inline networks the transaction added and drops the entries no node reaches.
	///
	/// Only the lifting is recorded. The removals that left an entry unreachable were recorded already.
	pub(crate) fn settle_definitions(&mut self) {
		let lifted = self.document_network_mut().lift_nested_networks();
		if !lifted.is_empty() {
			self.network_metadata.get_mut().lift_nested_metadata(&lifted);
			self.deltas.push(EditorDelta::DefinitionsChanged);
		}

		if self.document_network_mut().retain_reachable_definitions() {
			let network = self.network.network();
			self.network_metadata.get_mut().retain_definitions_of(network);
		}
	}

	// ===== Recorded writes =====

	/// The persistent metadata of a network, or `None` if the network is missing.
	pub(crate) fn network_mut<'a>(&'a mut self, network_path: &'a [NodeId]) -> Option<NetworkMut<'a>> {
		let location = self.locate(network_path)?;
		let metadata = &mut self.network_metadata.get_mut().nested_metadata_mut(location)?.persistent_metadata;
		Some(NetworkMut::new(metadata, &mut self.deltas, network_path))
	}

	/// Both halves of a node, or `None` if either is missing.
	pub(crate) fn node_mut<'a>(&'a mut self, locator: NodeLocator<'a>) -> Option<NodeMut<'a>> {
		let location = self.locate(locator.network_path)?;
		let node = self.network.get_mut().network_mut().located_network_mut(location)?.nodes.get_mut(&locator.node_id)?;
		let instantiated = match node.implementation {
			DocumentNodeImplementation::Definition(id) => Some(id),
			_ => None,
		};
		let (metadata, definition_metadata) = self.network_metadata.get_mut().node_metadata_with_definition_mut(location, &locator.node_id, instantiated)?;

		Some(NodeMut::new(node, &mut metadata.persistent_metadata, definition_metadata, &mut self.deltas, locator))
	}

	// ===== Unrecorded state, reachable from the whole network interface =====

	/// The transient caches of a network, which are not part of the document and so record nothing.
	pub(in super::super) fn network_transient_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkTransientMetadata> {
		Some(&mut self.network_metadata_mut(network_path)?.transient_metadata)
	}

	/// The transient caches of one node.
	pub(in super::super) fn node_transient_mut(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<&mut DocumentNodeTransientMetadata> {
		let network_metadata = self.network_metadata_mut(network_path)?;
		Some(&mut network_metadata.persistent_metadata.node_metadata.get_mut(node_id)?.transient_metadata)
	}

	/// The selection undo and redo stacks. Per-peer state that is not recorded and does not reach storage.
	pub(in super::super) fn selection_history_mut(&mut self, network_path: &[NodeId]) -> Option<SelectionHistoryMut<'_>> {
		let persistent = &mut self.network_metadata_mut(network_path)?.persistent_metadata;
		Some(SelectionHistoryMut {
			undo: &mut persistent.selection_undo_history,
			redo: &mut persistent.selection_redo_history,
		})
	}

	/// Where the node graph is panned and zoomed to. This write is not recorded.
	pub(in super::super) fn navigation_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NavigationMetadata> {
		Some(&mut self.network_metadata_mut(network_path)?.persistent_metadata.navigation_metadata)
	}

	/// Visits every network at every nesting level with its view state, for restoring what the peer was
	/// looking at onto an interface rebuilt from storage.
	pub(in super::super) fn for_each_network_view_state_mut(&mut self, mut visit: impl FnMut(&[NodeId], NetworkViewStateMut)) {
		let mut network_paths = Vec::new();
		self.for_each_network(&mut |network_path, _| network_paths.push(network_path.to_vec()));

		for network_path in network_paths {
			let Some(network_metadata) = self.network_metadata_mut(&network_path) else { continue };
			visit(&network_path, NetworkViewStateMut::of(&mut network_metadata.persistent_metadata));
		}
	}

	/// The view state of the network at `location`, which every instance of the network shares.
	pub(in super::super) fn located_view_state_mut(&mut self, location: NetworkLocation) -> Option<NetworkViewStateMut<'_>> {
		let network_metadata = self.network_metadata.get_mut().nested_metadata_mut(location)?;
		Some(NetworkViewStateMut::of(&mut network_metadata.persistent_metadata))
	}
}

impl<'a> NetworkViewStateMut<'a> {
	fn of(persistent: &'a mut NodeNetworkPersistentMetadata) -> Self {
		Self {
			navigation: &mut persistent.navigation_metadata,
			previewing: &mut persistent.previewing,
			selection: SelectionHistoryMut {
				undo: &mut persistent.selection_undo_history,
				redo: &mut persistent.selection_redo_history,
			},
			nodes: &persistent.node_metadata,
		}
	}
}

/// One network's per-peer view state: what a rebuild restores, and what the registry never records.
pub(crate) struct NetworkViewStateMut<'a> {
	pub(crate) navigation: &'a mut NavigationMetadata,
	pub(crate) previewing: &'a mut Previewing,
	pub(crate) selection: SelectionHistoryMut<'a>,
	/// The network's nodes, for a view field that may only name one that still exists.
	pub(crate) nodes: &'a HashMap<NodeId, DocumentNodeMetadata>,
}
