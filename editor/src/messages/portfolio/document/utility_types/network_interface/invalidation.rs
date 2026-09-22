//! What a change feeds: each rule names a kind of edit and unloads everything derived from it.
//!
//! Declared here once rather than restated at each mutation site, so two edits of the same kind cannot
//! disagree about what they invalidate.

use super::*;

impl NodeNetworkInterface {
	/// Unloads everything a node's position feeds: its own geometry, the geometry of every node placed
	/// relative to it, and the network bounds. A chain node also sets the width of the layer encapsulating
	/// its chain, so that layer reloads too.
	///
	/// Takes the changed nodes as a batch, so a run of moves walks the upstream cone once.
	pub(crate) fn invalidate_positions(&mut self, node_ids: Vec<NodeId>, network_path: &[NodeId]) {
		let encapsulating_layers = node_ids
			.iter()
			.filter(|node_id| self.is_chain(node_id, network_path))
			.filter_map(|node_id| self.downstream_layer_for_chain_node(node_id, network_path))
			.collect::<Vec<_>>();
		for downstream_layer in encapsulating_layers {
			self.unload_node_click_targets(&downstream_layer, network_path);
		}

		self.unload_upstream_node_click_targets(node_ids, network_path);
		self.unload_all_nodes_bounding_box(network_path);
	}

	/// Unloads the network's import and export strip: the ports themselves and the handles beside them
	/// for adding, removing and reordering.
	pub(crate) fn invalidate_import_export(&mut self, network_path: &[NodeId]) {
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
	}

	/// Unloads what a node's name or lock state feeds: a layer's width, and the geometry sized from it.
	pub(crate) fn invalidate_node_appearance(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		self.try_unload_layer_width(node_id, network_path);
		self.unload_node_click_targets(node_id, network_path);
	}
}
