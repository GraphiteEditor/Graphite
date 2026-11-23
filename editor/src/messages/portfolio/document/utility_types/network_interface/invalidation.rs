//! What a change feeds: each rule names a kind of edit and unloads everything derived from it.
//!
//! Declared here once rather than restated at each mutation site, so two edits of the same kind cannot
//! disagree about what they invalidate.

use super::*;

impl NodeNetworkInterface {
	/// Unloads what node positions feed beyond the layout itself, which is the network bounds.
	pub(crate) fn invalidate_positions(&mut self, network_path: &[NodeId]) {
		self.unload_all_nodes_bounding_box(network_path);
	}

	/// Unloads the network's import and export ports.
	pub(crate) fn invalidate_import_export(&mut self, network_path: &[NodeId]) {
		self.unload_import_export_ports(network_path);
	}

	/// Unloads what a node's name or lock state feeds: a layer's width and the network bounds it contributes to.
	pub(crate) fn invalidate_node_appearance(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		self.try_unload_layer_width(node_id, network_path);
		self.unload_all_nodes_bounding_box(network_path);
	}
}
