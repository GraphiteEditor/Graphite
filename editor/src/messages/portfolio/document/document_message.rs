use super::utility_types::misc::SnappingState;
use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis, GridSnapping};
use crate::messages::portfolio::utility_types::PanelType;
use crate::messages::prelude::*;

use graph_craft::document::NodeId;
use graphene_core::raster::BlendMode;
use graphene_core::raster::Image;
use graphene_core::vector::style::ViewMode;
use graphene_core::Color;
use graphene_std::renderer::ClickTarget;
use graphene_std::transform::Footprint;
use graphene_std::vector::VectorData;

use glam::DAffine2;

#[impl_message(Message, PortfolioMessage, Document)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DocumentMessage {
	Noop,
	// Sub-messages
	#[child]
	GraphOperation(GraphOperationMessage),
	#[child]
	Navigation(NavigationMessage),
	#[child]
	NodeGraph(NodeGraphMessage),
	#[child]
	Overlays(OverlaysMessage),
	#[child]
	PropertiesPanel(PropertiesPanelMessage),

	// Messages
	AlignSelectedLayers {
		axis: AlignAxis,
		aggregate: AlignAggregate,
	},
	ClearArtboards,
	ClearLayersPanel,
	InsertBooleanOperation {
		operation: graphene_core::vector::misc::BooleanOperation,
	},
	CreateEmptyFolder,
	DebugPrintDocument,
	DeleteSelectedLayers,
	DeselectAllLayers,
	DocumentHistoryBackward,
	DocumentHistoryForward,
	DocumentStructureChanged,
	DuplicateSelectedLayers,
	EnterNestedNetwork {
		node_id: NodeId,
	},
	Escape,
	ExitNestedNetwork {
		steps_back: usize,
	},
	FlipSelectedLayers {
		flip_axis: FlipAxis,
	},
	GraphViewOverlay {
		open: bool,
	},
	GraphViewOverlayToggle,
	GridOptions(GridSnapping),
	GridOverlays(OverlayContext),
	GridVisibility(bool),
	GroupSelectedLayers,
	ImaginateGenerate {
		imaginate_node: Vec<NodeId>,
	},
	ImaginateRandom {
		imaginate_node: Vec<NodeId>,
		then_generate: bool,
	},
	ImportSvg {
		id: NodeId,
		svg: String,
		transform: DAffine2,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	MoveSelectedLayersTo {
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	MoveSelectedLayersToGroup {
		parent: LayerNodeIdentifier,
	},
	NudgeSelectedLayers {
		delta_x: f64,
		delta_y: f64,
		resize: Key,
		resize_opposite_corner: Key,
	},
	DrawArtboardOverlays(OverlayContext),
	PasteImage {
		image: Image<Color>,
		mouse: Option<(f64, f64)>,
	},
	PasteSvg {
		svg: String,
		mouse: Option<(f64, f64)>,
	},
	Redo,
	RenameDocument {
		new_name: String,
	},
	RenderRulers,
	RenderScrollbars,
	SaveDocument,
	SelectAllLayers,
	SelectedLayersLower,
	SelectedLayersLowerToBack,
	SelectedLayersRaise,
	SelectedLayersRaiseToFront,
	SelectedLayersReorder {
		relative_index_offset: isize,
	},
	SelectLayer {
		id: NodeId,
		ctrl: bool,
		shift: bool,
	},
	SetActivePanel {
		active_panel: PanelType,
	},
	SetBlendModeForSelectedLayers {
		blend_mode: BlendMode,
	},
	SetOpacityForSelectedLayers {
		opacity: f64,
	},
	SetOverlaysVisibility {
		visible: bool,
	},
	SetRangeSelectionLayer {
		new_layer: Option<LayerNodeIdentifier>,
	},
	SetSnapping {
		#[serde(skip)]
		closure: Option<for<'a> fn(&'a mut SnappingState) -> &'a mut bool>,
		snapping_state: bool,
	},
	SetViewMode {
		view_mode: ViewMode,
	},
	StartTransaction,
	EndTransaction,
	CommitTransaction,
	AbortTransaction,
	AddTransaction,
	ToggleLayerExpansion {
		id: NodeId,
	},
	ToggleGridVisibility,
	ToggleOverlaysVisibility,
	ToggleSnapping,
	UpdateUpstreamTransforms {
		upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>,
	},
	UpdateClickTargets {
		click_targets: HashMap<NodeId, Vec<ClickTarget>>,
	},
	UpdateVectorModify {
		vector_modify: HashMap<NodeId, VectorData>,
	},
	Undo,
	UngroupSelectedLayers,
	UngroupLayer {
		layer: LayerNodeIdentifier,
	},
	PTZUpdate,
	SelectionStepBack,
	SelectionStepForward,
	ZoomCanvasTo100Percent,
	ZoomCanvasTo200Percent,
	ZoomCanvasToFitAll,
}
