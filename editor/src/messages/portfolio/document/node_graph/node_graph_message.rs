use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::utility_types::network_metadata::{Connector, InputConnector, OutputConnector};
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
	Copy,
	CloseCreateNodeMenu,
	CreateNode {
		node_id: Option<NodeId>,
		node_type: String,
		x: i32,
		y: i32,
	},
	CreateWire {
		output_connector: OutputConnector,
		input_connector: InputConnector,
		use_document_network: bool,
	},
	Cut,
	DeleteNodes {
		node_ids: Vec<NodeId>,
		reconnect: bool,
		use_document_network: bool,
	},
	DeleteSelectedNodes {
		reconnect: bool,
	},
	DisconnectInput {
		input: InputConnector,
		use_document_network: bool,
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
		node_template: NodeTemplate,
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
		input_connector: InputConnector,
		input: NodeInput,
		use_document_network: bool,
	},
	SetQualifiedInputValue {
		node_id: NodeId,
		input_index: usize,
		value: TaggedValue,
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
	ShiftNodes {
		node_ids: Vec<NodeId>,
		displacement_x: i32,
		displacement_y: i32,
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
