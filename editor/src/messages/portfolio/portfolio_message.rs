use super::document::utility_types::document_metadata::LayerNodeIdentifier;
use super::persistent_state::PersistentStateMessage;
use crate::messages::frontend::utility_types::{ExportBounds, FileType, PersistedState};
use crate::messages::prelude::*;
use document_container::AnyContainer;
use std::path::PathBuf;

#[impl_message(Message, Portfolio)]
#[derive(derivative::Derivative, serde::Serialize, serde::Deserialize)]
#[derivative(Clone, Debug, PartialEq)]
pub enum PortfolioMessage {
	// Sub-messages
	#[child]
	Document(DocumentMessage),
	#[child]
	FailedDocuments(FailedDocumentsMessage),
	#[child]
	Fonts(FontsMessage),
	#[child]
	Ingest(IngestMessage),
	#[child]
	PersistentState(PersistentStateMessage),
	#[child]
	Workspace(WorkspaceMessage),

	// Messages
	Init,
	DocumentPassMessage {
		document_id: DocumentId,
		message: DocumentMessage,
	},
	AutoSaveActiveDocument,
	AutoSaveAllDocuments,
	AutoSaveDocument {
		document_id: DocumentId,
	},
	CloseActiveDocumentWithConfirmation,
	CloseAllDocuments,
	CloseAllDocumentsWithConfirmation,
	CloseDocument {
		document_id: DocumentId,
	},
	CloseDocumentWithConfirmation {
		document_id: DocumentId,
	},
	DeleteDocument {
		document_id: DocumentId,
	},
	DestroyAllDocuments,
	EditorPreferences,
	GarbageCollectResources,
	ResolveResources,
	ResolveDocumentResources {
		document_id: DocumentId,
	},
	LoadPersistedState {
		state: PersistedState,
	},
	StoredDocumentsListed {
		document_ids: Option<Vec<DocumentId>>,
	},
	StoredDocumentLoaded {
		document_id: DocumentId,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore", Clone(clone_with = "clone_to_none"))]
		document: Option<Box<DocumentMessageHandler>>,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore", Clone(clone_with = "clone_to_none"))]
		gdd: Option<Box<document_format::GddV1>>,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore")]
		declarations: document_graph_storage::Declarations,
	},
	NewDocumentWithName {
		name: String,
	},
	OpenLegacyDocumentFile {
		document_name: Option<String>,
		document_path: Option<PathBuf>,
		document_serialized_content: String,
	},
	OpenDocumentFile {
		document_name: Option<String>,
		document_path: Option<PathBuf>,
		content: Vec<u8>,
	},
	DocumentFileLoaded {
		document_id: DocumentId,
		document_name: Option<String>,
		document_path: Option<PathBuf>,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore", Clone(clone_with = "clone_to_none"))]
		document: Option<Box<DocumentMessageHandler>>,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore", Clone(clone_with = "clone_to_none"))]
		gdd: Option<Box<document_format::GddV1>>,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore")]
		declarations: document_graph_storage::Declarations,
	},
	StorageUpdated,
	StorageAttached {
		document_id: DocumentId,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore", Clone(clone_with = "clone_to_none"))]
		container: Option<AnyContainer>,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore", Clone(clone_with = "clone_to_none"))]
		gdd: Option<Box<document_format::GddV1>>,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore")]
		declarations: document_graph_storage::Declarations,
	},
	NextDocument,
	PrevDocument,
	CenterLayers {
		layers: Vec<LayerNodeIdentifier>,
	},
	ReorderDocument {
		document_id: DocumentId,
		new_index: usize,
	},
	RequestWelcomeScreenButtonsLayout,
	RequestStatusBarInfoLayout,
	SelectDocument {
		document_id: DocumentId,
	},
	RenameDocument {
		new_name: String,
	},
	SubmitDocumentExport {
		name: String,
		file_type: FileType,
		scale_factor: f64,
		bounds: ExportBounds,
		artboard_name: Option<String>,
		artboard_count: usize,
	},
	SaveRasterizedExport {
		name: String,
		file_type: FileType,
		width: u32,
		height: u32,
		data: Vec<u8>,
	},
	SubmitActiveGraphRender,
	SubmitGraphRender {
		document_id: DocumentId,
		ignore_hash: bool,
	},
	SubmitEyedropperPreviewRender,
	ToggleResetNodesToDefinitionsOnOpen,
	ToggleRulers,
	UpdateDocumentWidgets,
	UpdateOpenDocumentsList,
	RequestSvgTextCopy {
		graphite_json: String,
	},
}

/// Clone helper for non-serializable payloads: a cloned completion message carries none.
fn clone_to_none<T>(_: &Option<T>) -> Option<T> {
	None
}
