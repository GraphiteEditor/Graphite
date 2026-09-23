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
		self.network_metadata.get_mut().nested_metadata_mut(network_path)
	}

	/// The metadata of the node that encapsulates the network at `network_path`, or `None` for the
	/// document network, which no node encapsulates.
	pub(super) fn encapsulating_node_metadata_mut(&mut self, network_path: &[NodeId]) -> Option<&mut DocumentNodeMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		let encapsulating_node_id = encapsulating_path.pop()?;
		self.network_metadata_mut(&encapsulating_path)?.persistent_metadata.node_metadata.get_mut(&encapsulating_node_id)
	}

	/// Rewrites the whole graph in place, for a document arriving in an older shape than the current one.
	///
	/// Nothing is recorded: a migration produces the document the peer should have had rather than
	/// changing the one they have.
	pub(in super::super) fn migrate_graph(&mut self, migrate: impl FnOnce(&mut NodeNetwork)) {
		migrate(self.document_network_mut());
	}

	// ===== Recorded writes =====

	/// The persistent metadata of a network, or `None` if the network is missing.
	pub(crate) fn network_mut<'a>(&'a mut self, network_path: &'a [NodeId]) -> Option<NetworkMut<'a>> {
		let metadata = &mut self.network_metadata.get_mut().nested_metadata_mut(network_path)?.persistent_metadata;
		Some(NetworkMut::new(metadata))
	}

	/// Both halves of a node, or `None` if either is missing.
	pub(crate) fn node_mut<'a>(&'a mut self, locator: NodeLocator<'a>) -> Option<NodeMut<'a>> {
		let node = self.network.get_mut().network_mut().nested_network_mut(locator.network_path)?.nodes.get_mut(&locator.node_id)?;
		let metadata = self
			.network_metadata
			.get_mut()
			.nested_metadata_mut(locator.network_path)?
			.persistent_metadata
			.node_metadata
			.get_mut(&locator.node_id)?;

		Some(NodeMut::new(node, &mut metadata.persistent_metadata))
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

	/// The transient caches of the network that the network at `network_path` sits inside.
	pub(in super::super) fn encapsulating_network_transient_mut(&mut self, network_path: &[NodeId]) -> Option<&mut NodeNetworkTransientMetadata> {
		let mut encapsulating_path = network_path.to_vec();
		encapsulating_path.pop()?;
		self.network_transient_mut(&encapsulating_path)
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
		let mut stack = vec![Vec::new()];
		while let Some(network_path) = stack.pop() {
			let Some(network_metadata) = self.network_metadata_mut(&network_path) else { continue };
			let persistent = &mut network_metadata.persistent_metadata;

			stack.extend(
				persistent
					.node_metadata
					.iter()
					.filter(|(_, node_metadata)| node_metadata.persistent_metadata.network_metadata.is_some())
					.map(|(node_id, _)| [network_path.as_slice(), &[*node_id]].concat()),
			);

			visit(
				&network_path,
				NetworkViewStateMut {
					navigation: &mut persistent.navigation_metadata,
					previewing: &mut persistent.previewing,
					selection: SelectionHistoryMut {
						undo: &mut persistent.selection_undo_history,
						redo: &mut persistent.selection_redo_history,
					},
					nodes: &persistent.node_metadata,
				},
			);
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
