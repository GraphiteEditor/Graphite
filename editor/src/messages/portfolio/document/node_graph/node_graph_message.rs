use super::utility_types::Direction;
use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate, OutputConnector};
use crate::messages::prelude::*;

use glam::IVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graph_craft::proto::GraphErrors;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypesDelta;

#[impl_message(Message, DocumentMessage, NodeGraph)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NodeGraphMessage {
	AddNodes {
		nodes: Vec<(NodeId, NodeTemplate)>,
		new_ids: HashMap<NodeId, NodeId>,
	},
	AddImport,
	AddExport,
	Init,
	SelectedNodesUpdated,
	Copy,
	CreateNodeInLayerNoTransaction {
		node_type: String,
		layer: LayerNodeIdentifier,
	},
	CreateNodeInLayerWithTransaction {
		node_type: String,
		layer: LayerNodeIdentifier,
	},
	CreateNodeFromContextMenu {
		node_id: Option<NodeId>,
		node_type: String,
		xy: Option<(i32, i32)>,
	},
	CreateWire {
		output_connector: OutputConnector,
		input_connector: InputConnector,
	},
	ConnectUpstreamOutputToInput {
		downstream_input: InputConnector,
		input_connector: InputConnector,
	},
	Cut,
	DeleteNodes {
		node_ids: Vec<NodeId>,
		delete_children: bool,
	},
	DeleteSelectedNodes {
		delete_children: bool,
	},
	DisconnectInput {
		input_connector: InputConnector,
	},
	DisconnectRootNode,
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
		node_id: NodeId,
		input_connector: InputConnector,
		insert_node_input_index: usize,
	},
	MergeSelectedNodes,
	MoveLayerToStack {
		layer: LayerNodeIdentifier,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	MoveNodeToChainStart {
		node_id: NodeId,
		parent: LayerNodeIdentifier,
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
	ForceRunDocumentGraph,
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
	SetGridAlignedEdges,
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
	ShiftNodePosition {
		node_id: NodeId,
		x: i32,
		y: i32,
	},
	ShiftSelectedNodes {
		direction: Direction,
		rubber_band: bool,
	},
	ShiftSelectedNodesByAmount {
		graph_delta: IVec2,
		rubber_band: bool,
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
	ToggleSelectedIsPinned,
	ToggleSelectedVisibility,
	ToggleVisibility {
		node_id: NodeId,
	},
	SetPinned {
		node_id: NodeId,
		pinned: bool,
	},
	SetVisibility {
		node_id: NodeId,
		visible: bool,
	},
	SetLockedOrVisibilitySideEffects {
		node_ids: Vec<NodeId>,
	},
	UpdateEdges,
	UpdateBoxSelection,
	UpdateLayerPanel,
	UpdateNewNodeGraph,
	UpdateTypes {
		#[serde(skip)]
		resolved_types: ResolvedDocumentNodeTypesDelta,
		#[serde(skip)]
		node_graph_errors: GraphErrors,
	},
	UpdateActionButtons,
	UpdateGraphBarRight,
	UpdateInSelectedNetwork,
	SendSelectedNodes,
}
