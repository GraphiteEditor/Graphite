use crate::messages::prelude::*;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graph_craft::proto::GraphErrors;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypes;

#[impl_message(Message, DocumentMessage, NodeGraph)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NodeGraphMessage {
	// Messages
	Init,
	SelectedNodesUpdated,
	ConnectNodesByLink {
		output_node: NodeId,
		output_node_connector_index: usize,
		input_node: NodeId,
		input_node_connector_index: usize,
	},
	Copy,
	CreateNode {
		node_id: Option<NodeId>,
		node_type: String,
		x: i32,
		y: i32,
	},
	Cut,
	DeleteNode {
		node_id: NodeId,
		reconnect: bool,
	},
	DeleteSelectedNodes {
		reconnect: bool,
	},
	DisconnectNodes {
		node_id: NodeId,
		input_index: usize,
	},
	EnterNestedNetwork {
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
	PasteNodes {
		serialized_nodes: String,
	},
	RunDocumentGraph,
	SelectedNodesAdd {
		nodes: Vec<NodeId>,
	},
	SelectedNodesRemove {
		nodes: Vec<NodeId>,
	},
	SelectedNodesSet {
		nodes: Vec<NodeId>,
	},
	SendGraph,
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
		node_path: Vec<NodeId>,
		input_index: usize,
		value: TaggedValue,
	},
	/// Move all the downstream nodes to the right in the graph to allow space for a newly inserted node
	ShiftNode {
		node_id: NodeId,
	},
	ToggleSelectedHidden,
	ToggleHidden {
		node_id: NodeId,
	},
	SetHidden {
		node_id: NodeId,
		hidden: bool,
	},
	SetName {
		node_id: NodeId,
		name: String,
	},
	SetNameImpl {
		node_id: NodeId,
		name: String,
	},
	TogglePreview {
		node_id: NodeId,
	},
	TogglePreviewImpl {
		node_id: NodeId,
	},
	UpdateNewNodeGraph,
	UpdateTypes {
		#[serde(skip)]
		resolved_types: ResolvedDocumentNodeTypes,
		#[serde(skip)]
		node_graph_errors: GraphErrors,
	},
}
