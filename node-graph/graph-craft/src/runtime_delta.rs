use crate::document::{DocumentNode, NodeId, NodeInput};

/// One mutation's worth of structural graph change, carrying its post-change data as plain runtime
/// types. Constructed by the mutation itself; a compound mutation emits several. Consumed typed and
/// unserialized by the compiler, and paired with `EditorDelta` metadata for storage staging.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeDelta {
	/// The node's nested network, if it implements one, rides inside the `DocumentNode`.
	AddNode {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		node: Box<DocumentNode>,
	},
	ReplaceNode {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		node: Box<DocumentNode>,
	},
	/// Address-only: removal snapshots come from the storage layer's working registry.
	RemoveNode {
		network_path: Vec<NodeId>,
		node_id: NodeId,
	},
	SetInput {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		input_index: usize,
		input: NodeInput,
	},
	SetExport {
		network_path: Vec<NodeId>,
		export_index: usize,
		input: NodeInput,
	},
	SetVisibility {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		visible: bool,
	},
}
