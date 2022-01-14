use super::layer_panel::LayerMetadata;
use super::utility_types::{AlignAggregate, AlignAxis, FlipAxis};
use super::{ArtboardMessage, MovementMessage, TransformLayerMessage};
use crate::message_prelude::*;

use graphene::layers::blend_mode::BlendMode;
use graphene::layers::style::ViewMode;
use graphene::LayerId;
use graphene::Operation as DocumentOperation;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, PortfolioMessage, Document)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum DocumentMessage {
	AbortTransaction,
	AddSelectedLayers(Vec<Vec<LayerId>>),
	AlignSelectedLayers(AlignAxis, AlignAggregate),
	#[child]
	Artboard(ArtboardMessage),
	CommitTransaction,
	CreateEmptyFolder(Vec<LayerId>),
	DebugPrintDocument,
	DeleteLayer(Vec<LayerId>),
	DeleteSelectedLayers,
	DeselectAllLayers,
	DirtyRenderDocument,
	DirtyRenderDocumentInOutlineView,
	DispatchOperation(Box<DocumentOperation>),
	DocumentHistoryBackward,
	DocumentHistoryForward,
	DocumentStructureChanged,
	DuplicateSelectedLayers,
	ExportDocument,
	FlipSelectedLayers(FlipAxis),
	FolderChanged(Vec<LayerId>),
	GroupSelectedLayers,
	LayerChanged(Vec<LayerId>),
	#[child]
	Movement(MovementMessage),
	MoveSelectedLayersTo {
		path: Vec<LayerId>,
		insert_index: isize,
	},
	NudgeSelectedLayers(f64, f64),
	#[child]
	Overlays(OverlaysMessage),
	Redo,
	RenameLayer(Vec<LayerId>, String),
	RenderDocument,
	ReorderSelectedLayers(i32), // relative_position,
	RollbackTransaction,
	SaveDocument,
	SelectAllLayers,
	SelectionChanged,
	SelectLayer(Vec<LayerId>, bool, bool),
	SetBlendModeForSelectedLayers(BlendMode),
	SetLayerExpansion(Vec<LayerId>, bool),
	SetOpacityForSelectedLayers(f64),
	SetSelectedLayers(Vec<Vec<LayerId>>),
	SetSnapping(bool),
	SetViewMode(ViewMode),
	StartTransaction,
	ToggleLayerExpansion(Vec<LayerId>),
	ToggleLayerVisibility(Vec<LayerId>),
	#[child]
	TransformLayers(TransformLayerMessage),
	Undo,
	UngroupLayers(Vec<LayerId>),
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
