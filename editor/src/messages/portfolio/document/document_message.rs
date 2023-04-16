use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::utility_types::layer_panel::LayerMetadata;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis};
use crate::messages::prelude::*;

use document_legacy::boolean_ops::BooleanOperation as BooleanOperationType;
use document_legacy::document::Document as DocumentLegacy;
use document_legacy::layers::blend_mode::BlendMode;
use document_legacy::layers::style::ViewMode;
use document_legacy::LayerId;
use document_legacy::Operation as DocumentOperation;
use graph_craft::document::NodeId;
use graphene_core::raster::Image;
use graphene_core::Color;
use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, PortfolioMessage, Document)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum DocumentMessage {
	// Sub-messages
	#[remain::unsorted]
	DispatchOperation(Box<DocumentOperation>),
	#[remain::unsorted]
	#[child]
	Artboard(ArtboardMessage),
	#[remain::unsorted]
	#[child]
	Navigation(NavigationMessage),
	#[remain::unsorted]
	#[child]
	Overlays(OverlaysMessage),
	#[remain::unsorted]
	#[child]
	PropertiesPanel(PropertiesPanelMessage),
	#[remain::unsorted]
	#[child]
	NodeGraph(NodeGraphMessage),
	#[remain::unsorted]
	#[child]
	GraphOperation(GraphOperationMessage),

	// Messages
	AbortTransaction,
	AddSelectedLayers {
		additional_layers: Vec<Vec<LayerId>>,
	},
	AlignSelectedLayers {
		axis: AlignAxis,
		aggregate: AlignAggregate,
	},
	BackupDocument {
		document: DocumentLegacy,
		artboard: Box<ArtboardMessageHandler>,
		layer_metadata: HashMap<Vec<LayerId>, LayerMetadata>,
	},
	BooleanOperation(BooleanOperationType),
	ClearLayerTree,
	CommitTransaction,
	CreateEmptyFolder {
		container_path: Vec<LayerId>,
	},
	DebugPrintDocument,
	DeleteLayer {
		layer_path: Vec<LayerId>,
	},
	DeleteSelectedLayers,
	DeselectAllLayers,
	DirtyRenderDocument,
	DirtyRenderDocumentInOutlineView,
	DocumentHistoryBackward,
	DocumentHistoryForward,
	DocumentStructureChanged,
	DuplicateSelectedLayers,
	ExportDocument {
		file_name: String,
		file_type: FileType,
		scale_factor: f64,
		bounds: ExportBounds,
	},
	FlipSelectedLayers {
		flip_axis: FlipAxis,
	},
	FolderChanged {
		affected_folder_path: Vec<LayerId>,
	},
	FrameClear,
	GroupSelectedLayers,
	LayerChanged {
		affected_layer_path: Vec<LayerId>,
	},
	MoveSelectedLayersTo {
		folder_path: Vec<LayerId>,
		insert_index: isize,
		reverse_index: bool,
	},
	NodeGraphFrameClear {
		layer_path: Vec<LayerId>,
		node_id: NodeId,
		cached_index: usize,
	},
	NodeGraphFrameGenerate {
		layer_path: Vec<LayerId>,
	},
	NodeGraphFrameImaginate {
		layer_path: Vec<LayerId>,
		imaginate_node: Vec<NodeId>,
	},
	NodeGraphFrameImaginateRandom {
		layer_path: Vec<LayerId>,
		imaginate_node: Vec<NodeId>,
		then_generate: bool,
	},
	NodeGraphFrameImaginateTerminate {
		layer_path: Vec<LayerId>,
		node_path: Vec<NodeId>,
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
	Redo,
	RenameLayer {
		layer_path: Vec<LayerId>,
		new_name: String,
	},
	RenderDocument,
	RollbackTransaction,
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
		layer_path: Vec<LayerId>,
		ctrl: bool,
		shift: bool,
	},
	SetBlendModeForSelectedLayers {
		blend_mode: BlendMode,
	},
	SetImageBlobUrl {
		layer_path: Vec<LayerId>,
		blob_url: String,
		resolution: (f64, f64),
		document_id: u64,
	},
	SetLayerExpansion {
		layer_path: Vec<LayerId>,
		set_expanded: bool,
	},
	SetLayerName {
		layer_path: Vec<LayerId>,
		name: String,
	},
	SetOpacityForSelectedLayers {
		opacity: f64,
	},
	SetOverlaysVisibility {
		visible: bool,
	},
	SetSelectedLayers {
		replacement_selected_layers: Vec<Vec<LayerId>>,
	},
	SetSnapping {
		snap: bool,
	},
	SetTextboxEditability {
		path: Vec<LayerId>,
		editable: bool,
	},
	SetViewMode {
		view_mode: ViewMode,
	},
	StartTransaction,
	ToggleLayerExpansion {
		layer_path: Vec<LayerId>,
	},
	ToggleLayerVisibility {
		layer_path: Vec<LayerId>,
	},
	Undo,
	UndoFinished,
	UngroupLayers {
		folder_path: Vec<LayerId>,
	},
	UngroupSelectedLayers,
	UpdateLayerMetadata {
		layer_path: Vec<LayerId>,
		layer_metadata: LayerMetadata,
	},
	ZoomCanvasTo100Percent,
	ZoomCanvasTo200Percent,
	ZoomCanvasToFitAll,
}

impl From<DocumentOperation> for DocumentMessage {
	fn from(operation: DocumentOperation) -> DocumentMessage {
		DocumentMessage::DispatchOperation(Box::new(operation))
	}
}

impl From<DocumentOperation> for Message {
	fn from(operation: DocumentOperation) -> Message {
		DocumentMessage::DispatchOperation(Box::new(operation)).into()
	}
}
