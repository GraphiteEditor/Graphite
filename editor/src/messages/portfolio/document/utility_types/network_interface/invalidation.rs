//! What a change feeds: each rule names a kind of edit and unloads everything derived from it.
//!
//! Declared here once rather than restated at each mutation site, so two edits of the same kind cannot
//! disagree about what they invalidate.

use super::*;

impl NodeNetworkInterface {
	/// Unloads everything a node's position feeds: its own geometry, the geometry of every node placed
	/// relative to it, and the network bounds. A chain node also sets the width of the layer encapsulating
	/// its chain, so that layer reloads too.
	pub(crate) fn invalidate_position(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		if self.is_chain(node_id, network_path)
			&& let Some(downstream_layer) = self.downstream_layer_for_chain_node(node_id, network_path)
		{
			self.unload_node_click_targets(&downstream_layer, network_path);
		}

		self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		// This unloads the import and export ports too, since they are placed from the bounds it clears.
		self.unload_all_nodes_bounding_box(network_path);
		// The handles beside those ports are placed the same way, and nothing else unloads them.
		self.unload_modify_import_export(network_path);
	}

	/// Unloads the network's import and export strip: the ports themselves and the handles beside them
	/// for adding, removing and reordering.
	pub(crate) fn invalidate_import_export(&mut self, network_path: &[NodeId]) {
		self.unload_import_export_ports(network_path);
		self.unload_modify_import_export(network_path);
	}

	/// Unloads what a node's name or lock state feeds: a layer's width, the geometry sized from it, and
	/// the network bounds that geometry contributes to.
	pub(crate) fn invalidate_node_appearance(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		self.try_unload_layer_width(node_id, network_path);
		self.unload_node_click_targets(node_id, network_path);
		self.unload_all_nodes_bounding_box(network_path);
		self.unload_modify_import_export(network_path);
	}
}
