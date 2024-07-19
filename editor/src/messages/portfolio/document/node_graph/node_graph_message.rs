use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate, OutputConnector};
use crate::messages::prelude::*;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graph_craft::proto::GraphErrors;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypes;

#[impl_message(Message, DocumentMessage, NodeGraph)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NodeGraphMessage {
	AddNodes {
		nodes: HashMap<NodeId, NodeTemplate>,
		new_ids: HashMap<NodeId, NodeId>,
	},
	Init,
	SelectedNodesUpdated,
	Copy,
	CloseCreateNodeMenu,
	CreateNodeFromContextMenu {
		node_id: Option<NodeId>,
		node_type: String,
	},
	CreateWire {
		output_connector: OutputConnector,
		input_connector: InputConnector,
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
		input_connector: InputConnector,
	},

	EnterNestedNetwork,
	DuplicateSelectedNodes,
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
		// Post node
		post_node_id: NodeId,
		post_node_input_index: usize,
		// Inserted node
		insert_node_id: NodeId,
		insert_node_output_index: usize,
		insert_node_input_index: usize,
		// Pre node
		pre_node_id: NodeId,
		pre_node_output_index: usize,
	},
	MoveLayerToStack {
		layer: LayerNodeIdentifier,
		parent: LayerNodeIdentifier,
		insert_index: usize,
		skip_rerender: bool,
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
	SendClickTargets,
	EndSendClickTargets,
	SendGraph,
	SetInputValue {
		node_id: NodeId,
		input_index: usize,
		value: TaggedValue,
	},
	SetInput {
		input_connector: InputConnector,
		input: NodeInput,
	},
	SetDisplayName {
		node_id: NodeId,
		alias: String,
	},
	SetDisplayNameImpl {
		node_id: NodeId,
		alias: String,
	},
	SetToNodeOrLayer {
		node_id: NodeId,
		is_layer: bool,
	},
	ShiftNodes {
		node_ids: Vec<NodeId>,
		displacement_x: i32,
		displacement_y: i32,
		move_upstream: bool,
	},
	TogglePreview {
		node_id: NodeId,
	},
	TogglePreviewImpl {
		node_id: NodeId,
	},
	ToggleSelectedAsLayersOrNodes,
	ToggleSelectedLocked,
	ToggleLocked {
		node_id: NodeId,
	},
	SetLocked {
		node_id: NodeId,
		locked: bool,
	},
	ToggleSelectedVisibility,
	ToggleVisibility {
		node_id: NodeId,
	},
	SetVisibility {
		node_id: NodeId,
		visible: bool,
	},
	SetLockedOrVisibilitySideEffects {
		node_ids: Vec<NodeId>,
	},
	UpdateNewNodeGraph,
	UpdateTypes {
		#[serde(skip)]
		resolved_types: ResolvedDocumentNodeTypes,
		#[serde(skip)]
		node_graph_errors: GraphErrors,
	},
	UpdateActionButtons,
	SendSelectedNodes,
}
