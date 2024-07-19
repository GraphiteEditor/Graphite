use crate::messages::input_mapper::utility_types::input_keyboard::Key;
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
	ConnectNodesByWire {
		output_node: NodeId,
		output_node_connector_index: usize,
		input_node: NodeId,
		input_node_connector_index: usize,
	},
	Copy,
	CloseCreateNodeMenu,
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
	DisconnectInput {
		node_id: NodeId,
		input_index: usize,
	},
	EnterNestedNetwork,
	DuplicateSelectedNodes,
	EnforceLayerHasNoMultiParams {
		node_id: NodeId,
	},
	ExitNestedNetwork {
		steps_back: usize,
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
		move_upstream: bool,
	},
	PasteNodes {
		serialized_nodes: String,
	},
	PointerDown {
		shift_click: bool,
		control_click: bool,
		alt_click: bool,
		right_click: bool,
	},
	PointerMove {
		shift: Key,
	},
	PointerUp,
	PointerOutsideViewport {
		shift: Key,
	},
	PrintSelectedNodeCoordinates,
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
		node_id: NodeId,
		input_index: usize,
		value: TaggedValue,
	},
	/// Move all the downstream nodes to the right in the graph to allow space for a newly inserted node
	ShiftNode {
		node_id: NodeId,
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
	StartPreviewingWithoutRestore {
		node_id: NodeId,
	},
	TogglePreview {
		node_id: NodeId,
	},
	TogglePreviewImpl {
		node_id: NodeId,
	},
	ToggleSelectedAsLayersOrNodes,
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
