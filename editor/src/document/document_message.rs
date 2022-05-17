use super::layer_panel::LayerMetadata;
use super::utility_types::{AlignAggregate, AlignAxis, FlipAxis};
use crate::message_prelude::*;

use graphene::boolean_ops::BooleanOperation as BooleanOperationType;
use graphene::layers::blend_mode::BlendMode;
use graphene::layers::style::ViewMode;
use graphene::LayerId;
use graphene::Operation as DocumentOperation;

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
	Movement(MovementMessage),
	#[remain::unsorted]
	#[child]
	Overlays(OverlaysMessage),
	#[remain::unsorted]
	#[child]
	TransformLayers(TransformLayerMessage),
	#[remain::unsorted]
	#[child]
	PropertiesPanel(PropertiesPanelMessage),

	// Messages
	AbortTransaction,
	AddSelectedLayers {
		additional_layers: Vec<Vec<LayerId>>,
	},
	AlignSelectedLayers {
		axis: AlignAxis,
		aggregate: AlignAggregate,
	},
	BooleanOperation(BooleanOperationType),
	CommitTransaction,
	CreateEmptyFolder {
		container_path: Vec<LayerId>,
	},
	DebugPrintDocument,
	DeleteLayer {
		layer_path: Vec<LayerId>,
	},
	DeleteSelectedLayers,
	DeleteSelectedVectorPoints,
	DeselectAllLayers,
	DeselectAllVectorPoints,
	DeselectVectorPoints {
		layer_path: Vec<LayerId>,
		point_ids: Vec<u64>,
	},
	DirtyRenderDocument,
	DirtyRenderDocumentInOutlineView,
	DocumentHistoryBackward,
	DocumentHistoryForward,
	DocumentStructureChanged,
	DuplicateSelectedLayers,
	ExportDocument,
	FlipSelectedLayers {
		flip_axis: FlipAxis,
	},
	FolderChanged {
		affected_folder_path: Vec<LayerId>,
	},
	GroupSelectedLayers,
	LayerChanged {
		affected_layer_path: Vec<LayerId>,
	},
	MoveSelectedLayersTo {
		folder_path: Vec<LayerId>,
		insert_index: isize,
		reverse_index: bool,
	},
	MoveSelectedVectorPoints {
		delta_x: f64,
		delta_y: f64,
	},
	NudgeSelectedLayers {
		delta_x: f64,
		delta_y: f64,
	},
	Redo,
	RenameLayer {
		layer_path: Vec<LayerId>,
		new_name: String,
	},
	RenderDocument,
	ReorderSelectedLayers {
		relative_index_offset: isize,
	},
	RollbackTransaction,
	SaveDocument,
	SelectAllLayers,
	SelectionChanged,
	SelectLayer {
		layer_path: Vec<LayerId>,
		ctrl: bool,
		shift: bool,
	},
	SelectVectorPoints {
		layer_path: Vec<LayerId>,
		point_ids: Vec<u64>,
		add: bool,
	},
	SetBlendModeForSelectedLayers {
		blend_mode: BlendMode,
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
	SetTexboxEditability {
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
	UngroupLayers {
		folder_path: Vec<LayerId>,
	},
	UngroupSelectedLayers,
	UpdateLayerMetadata {
		layer_path: Vec<LayerId>,
		layer_metadata: LayerMetadata,
	},
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
