use super::document::utility_types::document_metadata::LayerNodeIdentifier;
use super::persistent_state::PersistentStateMessage;
use crate::messages::frontend::utility_types::{ExportBounds, FileType, PersistedState};
use crate::messages::prelude::*;
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
	/// Delivers an asynchronously-built `Gdd` working copy, with the proto-node declarations its history
	/// references, into its document. Emitted by the mount future spawned in `load_document`. The `mounted`
	/// payload is non-serializable and a clone carries none (`clone_to_none`), so it travels exactly once.
	/// `reopened` is true when an existing working copy was opened: the persisted cursor is trusted as-is
	/// and the mount-time re-commit is skipped, since re-committing would stack a spurious interaction on
	/// the restored cursor and make the first undo a no-op.
	DocumentStorageMounted {
		document_id: DocumentId,
		reopened: bool,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore", Clone(clone_with = "clone_to_none"))]
		mounted: Option<Box<(document_format::GddV1, document_graph_storage::Declarations)>>,
	},
	DestroyAllDocuments,
	EditorPreferences,
	GarbageCollectResources,
	ResolveDocumentResources {
		document_id: DocumentId,
	},
	ResolveResources,
	LoadPersistedState {
		state: PersistedState,
	},
	LoadDocumentContent {
		document_id: DocumentId,
		document_serialized_content: String,
	},
	NewDocumentWithName {
		name: String,
	},
	NextDocument,
	OpenDocumentFile {
		document_name: Option<String>,
		document_path: Option<PathBuf>,
		document_serialized_content: String,
	},
	/// Open a `.gdd` document container (archive bytes), building the runtime from its stored registry.
	OpenGddDocument {
		document_name: Option<String>,
		document_path: Option<PathBuf>,
		content: Vec<u8>,
	},
	/// Delivers a document built asynchronously from a `.gdd` archive (registry → runtime, working copy
	/// mounted) into the portfolio. Travels once like [`DocumentStorageMounted`](Self::DocumentStorageMounted).
	GddDocumentLoaded {
		document_id: DocumentId,
		document_name: Option<String>,
		document_path: Option<PathBuf>,
		#[serde(skip, default)]
		#[derivative(Debug = "ignore", PartialEq = "ignore", Clone(clone_with = "clone_to_none"))]
		document: Option<Box<DocumentMessageHandler>>,
	},
	LoadDocument {
		document_id: DocumentId,
		document_name: Option<String>,
		document_path: Option<PathBuf>,
		document_is_auto_saved: bool,
		document_is_saved: bool,
		document_serialized_content: String,
	},
	CenterLayers {
		layers: Vec<LayerNodeIdentifier>,
	},
	PrevDocument,
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
}

/// Clone helper for non-serializable payloads: a cloned message carries none of the `Gdd` working copy,
/// its declarations, or the built document.
fn clone_to_none<T>(_: &Option<T>) -> Option<T> {
	None
}
