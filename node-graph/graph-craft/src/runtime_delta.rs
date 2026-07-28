use crate::document::{DocumentNode, NodeId, NodeInput};

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeDelta {
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
