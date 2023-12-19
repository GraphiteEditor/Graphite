use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::utility_types::layer_panel::LayerMetadata;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis};
use crate::messages::prelude::*;

use document_legacy::document::Document as DocumentLegacy;
use document_legacy::document_metadata::LayerNodeIdentifier;
use document_legacy::layers::style::ViewMode;
use document_legacy::LayerId;
use document_legacy::Operation as DocumentOperation;
use graph_craft::document::NodeId;
use graphene_core::raster::BlendMode;
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
		layer_metadata: HashMap<Vec<LayerId>, LayerMetadata>,
	},
	ClearLayerTree,
	CommitTransaction,
	CreateEmptyFolder {
		parent: LayerNodeIdentifier,
	},
	DebugPrintDocument,
	DeleteLayer {
		layer_path: Vec<LayerId>,
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
	FolderChanged {
		affected_folder_path: Vec<LayerId>,
	},
	FrameClear,
	GroupSelectedLayers,
	ImaginateClear {
		layer_path: Vec<LayerId>,
	},
	ImaginateGenerate {
		layer_path: Vec<LayerId>,
	},
	ImaginateRandom {
		layer_path: Vec<LayerId>,
		imaginate_node: Vec<NodeId>,
		then_generate: bool,
	},
	InputFrameRasterizeRegionBelowLayer {
		layer_path: Vec<LayerId>,
	},
	LayerChanged {
		affected_layer_path: Vec<LayerId>,
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
	Redo,
	RenameDocument {
		new_name: String,
	},
	RenderDocument,
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
		bounding_box_snapping: Option<bool>,
		node_snapping: Option<bool>,
	},
	SetViewMode {
		view_mode: ViewMode,
	},
	StartTransaction,
	ToggleLayerExpansion {
		layer: NodeId,
	},
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
