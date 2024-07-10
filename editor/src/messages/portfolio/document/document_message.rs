use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis, GridSnapping};
use crate::messages::prelude::*;

use graph_craft::document::{NodeId, NodeNetwork};
use graphene_core::raster::BlendMode;
use graphene_core::raster::Image;
use graphene_core::vector::style::ViewMode;
use graphene_core::Color;

use glam::DAffine2;

use super::utility_types::misc::{OptionBoundsSnapping, OptionPointSnapping};

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
	AbortTransaction,
	AlignSelectedLayers {
		axis: AlignAxis,
		aggregate: AlignAggregate,
	},
	BackupDocument {
		network: NodeNetwork,
	},
	ClearArtboards,
	ClearLayersPanel,
	CommitTransaction,
	InsertBooleanOperation {
		operation: graphene_core::vector::misc::BooleanOperation,
	},
	CreateEmptyFolder,
	DebugPrintDocument,
	DeleteLayer {
		layer: LayerNodeIdentifier,
	},
	DeleteSelectedLayers,
	DeselectAllLayers,
	DocumentHistoryBackward,
	DocumentHistoryForward,
	DocumentStructureChanged,
	DuplicateSelectedLayers,
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
	ImaginateGenerate,
	ImaginateRandom {
		imaginate_node: Vec<NodeId>,
		then_generate: bool,
	},
	ImportSvg {
		id: NodeId,
		svg: String,
		transform: DAffine2,
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
	MoveSelectedLayersTo {
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
	NudgeSelectedLayers {
		delta_x: f64,
		delta_y: f64,
		resize: Key,
		resize_opposite_corner: Key,
	},
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
	ResetTransform,
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
		snapping_enabled: Option<bool>,
		bounding_box_snapping: Option<OptionBoundsSnapping>,
		geometry_snapping: Option<OptionPointSnapping>,
	},
	SetViewMode {
		view_mode: ViewMode,
	},
	StartTransaction,
	ToggleLayerExpansion {
		id: NodeId,
	},
	ToggleGridVisibility,
	ToggleOverlaysVisibility,
	ToggleSnapping,
	Undo,
	UndoFinished,
	UngroupSelectedLayers,
	UpdateDocumentTransform {
		transform: glam::DAffine2,
	},
	ZoomCanvasTo100Percent,
	ZoomCanvasTo200Percent,
	ZoomCanvasToFitAll,
}
