use crate::messages::prelude::*;

use glam::IVec2;
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
	DeleteNodes {
		node_ids: Vec<NodeId>,
		reconnect: bool,
	},
	DeleteSelectedNodes {
		reconnect: bool,
	},
	DisconnectNodes {
		node_id: NodeId,
		input_index: usize,
	},
	DisconnectLayerFromStack {
		node_id: NodeId,
		reconnect_to_sibling: bool,
	},
	EnterNestedNetwork {
		node: NodeId,
	},
	DuplicateSelectedNodes,
	EnforceLayerHasNoMultiParams {
		node_id: NodeId,
	},
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
	InsertNodeBetween {
		post_node_id: NodeId,
		post_node_input_index: usize,
		insert_node_output_index: usize,
		insert_node_id: NodeId,
		insert_node_input_index: usize,
		pre_node_output_index: usize,
		pre_node_id: NodeId,
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
	SetNodePosition {
		node_id: NodeId,
		position: IVec2,
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
	ShiftUpstream {
		node_id: NodeId,
		shift: IVec2,
		shift_self: bool,
	},
	SetVisibility {
		node_id: NodeId,
		visible: bool,
	},
	SetLocked {
		node_id: NodeId,
		locked: bool,
	},
	SetName {
		node_id: NodeId,
		name: String,
	},
	SetNameImpl {
		node_id: NodeId,
		name: String,
	},
	SetToNodeOrLayer {
		node_id: NodeId,
		is_layer: bool,
	},
	ToggleLocked {
		node_id: NodeId,
	},
	TogglePreview {
		node_id: NodeId,
	},
	TogglePreviewImpl {
		node_id: NodeId,
	},
	ToggleSelectedAsLayersOrNodes,
	ToggleSelectedLocked,
	ToggleSelectedVisibility,
	ToggleVisibility {
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
