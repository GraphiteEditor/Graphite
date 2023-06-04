use crate::messages::prelude::*;

use document_legacy::LayerId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, NodeGraph)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NodeGraphMessage {
	// Messages
	CloseNodeGraph,
	ConnectNodesByLink {
		output_node: u64,
		output_node_connector_index: usize,
		input_node: u64,
		input_node_connector_index: usize,
	},
	Copy,
	CreateNode {
		// Having the caller generate the id means that we don't have to return it. This can be a random u64.
		node_id: Option<NodeId>,
		node_type: String,
		x: i32,
		y: i32,
	},
	Cut,
	DeleteNode {
		node_id: NodeId,
	},
	DeleteSelectedNodes,
	DisconnectNodes {
		node_id: NodeId,
		input_index: usize,
	},
	DoubleClickNode {
		node: NodeId,
	},
	DuplicateSelectedNodes,
	ExitNestedNetwork {
		depth_of_nesting: usize,
	},
	ExposeInput {
		node_id: NodeId,
		input_index: usize,
		new_exposed: bool,
	},
	InsertNode {
		node_id: NodeId,
		document_node: DocumentNode,
	},
	MoveSelectedNodes {
		displacement_x: i32,
		displacement_y: i32,
	},
	OpenNodeGraph {
		layer_path: Vec<document_legacy::LayerId>,
	},
	PasteNodes {
		serialized_nodes: String,
	},
	RunDocumentGraph,
	SelectNodes {
		nodes: Vec<NodeId>,
	},
	SendGraph {
		should_rerender: bool,
	},
	SetInputValue {
		node_id: NodeId,
		input_index: usize,
		value: TaggedValue,
	},
	SetNodeInput {
		node_id: NodeId,
		input_index: usize,
		input: NodeInput,
	},
	SetQualifiedInputValue {
		layer_path: Vec<LayerId>,
		node_path: Vec<NodeId>,
		input_index: usize,
		value: TaggedValue,
	},
	ShiftNode {
		node_id: NodeId,
	},
	ToggleHidden,
	ToggleHiddenImpl,
	TogglePreview {
		node_id: NodeId,
	},
	TogglePreviewImpl {
		node_id: NodeId,
	},
	UpdateNewNodeGraph,
}
