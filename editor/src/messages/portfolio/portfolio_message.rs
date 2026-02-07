use super::document::utility_types::document_metadata::LayerNodeIdentifier;
use super::utility_types::PanelType;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::portfolio::utility_types::FontCatalog;
use crate::messages::prelude::*;
use graphene_std::Color;
use graphene_std::raster::Image;
use graphene_std::text::Font;
use std::path::PathBuf;

#[impl_message(Message, Portfolio)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PortfolioMessage {
	// Sub-messages
	#[child]
	Document(DocumentMessage),

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
	Copy {
		clipboard: Clipboard,
	},
	Cut {
		clipboard: Clipboard,
	},
	DeleteDocument {
		document_id: DocumentId,
	},
	DestroyAllDocuments,
	EditorPreferences,
	FontCatalogLoaded {
		catalog: FontCatalog,
	},
	LoadFontData {
		font: Font,
	},
	FontLoaded {
		font_family: String,
		font_style: String,
		data: Vec<u8>,
	},
	LoadDocumentResources {
		document_id: DocumentId,
	},
	NewDocumentWithName {
		name: String,
	},
	NextDocument,
	Open,
	Import,
	OpenFile {
		path: PathBuf,
		content: Vec<u8>,
	},
	ImportFile {
		path: PathBuf,
		content: Vec<u8>,
	},
	OpenDocumentFile {
		document_name: Option<String>,
		document_path: Option<PathBuf>,
		document_serialized_content: String,
	},
	OpenDocumentFileWithId {
		document_id: DocumentId,
		document_name: Option<String>,
		document_path: Option<PathBuf>,
		document_is_auto_saved: bool,
		document_is_saved: bool,
		document_serialized_content: String,
		to_front: bool,
		select_after_open: bool,
	},
	OpenImage {
		name: Option<String>,
		image: Image<Color>,
	},
	OpenSvg {
		name: Option<String>,
		svg: String,
	},
	PasteSerializedData {
		data: String,
	},
	PasteSerializedVector {
		data: String,
	},
	PasteImage {
		name: Option<String>,
		image: Image<Color>,
		mouse: Option<(f64, f64)>,
		parent_and_insert_index: Option<(LayerNodeIdentifier, usize)>,
	},
	PasteSvg {
		name: Option<String>,
		svg: String,
		mouse: Option<(f64, f64)>,
		parent_and_insert_index: Option<(LayerNodeIdentifier, usize)>,
	},
	// TODO: Unused except by tests, remove?
	PasteIntoFolder {
		clipboard: Clipboard,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	CenterPastedLayers {
		layers: Vec<LayerNodeIdentifier>,
	},
	PrevDocument,
	RequestWelcomeScreenButtonsLayout,
	RequestStatusBarInfoLayout,
	SetActivePanel {
		panel: PanelType,
	},
	SelectDocument {
		document_id: DocumentId,
	},
	SubmitDocumentExport {
		name: String,
		file_type: FileType,
		scale_factor: f64,
		bounds: ExportBounds,
		transparent_background: bool,
		artboard_name: Option<String>,
		artboard_count: usize,
	},
	SubmitActiveGraphRender,
	SubmitGraphRender {
		document_id: DocumentId,
		ignore_hash: bool,
	},
	SubmitEyedropperPreviewRender,
	ToggleResetNodesToDefinitionsOnOpen,
	ToggleFocusDocument,
	ToggleDataPanelOpen,
	TogglePropertiesPanelOpen,
	ToggleLayersPanelOpen,
	ToggleRulers,
	UpdateDocumentWidgets,
	UpdateOpenDocumentsList,
	UpdateVelloPreference,
}
