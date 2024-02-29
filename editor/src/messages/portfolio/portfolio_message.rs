use super::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::prelude::*;

use graphene_core::text::Font;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Portfolio)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PortfolioMessage {
	// Sub-messages
	#[remain::unsorted]
	#[child]
	MenuBar(MenuBarMessage),
	#[remain::unsorted]
	#[child]
	Document(DocumentMessage),

	// Messages
	#[remain::unsorted]
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
	FontLoaded {
		font_family: String,
		font_style: String,
		preview_url: String,
		data: Vec<u8>,
		is_default: bool,
	},
	ImaginateCheckServerStatus,
	ImaginatePollServerStatus,
	ImaginatePreferences,
	ImaginateServerHostname,
	Import,
	LoadDocumentResources {
		document_id: DocumentId,
	},
	LoadFont {
		font: Font,
		is_default: bool,
	},
	NewDocumentWithName {
		name: String,
	},
	NextDocument,
	OpenDocument,
	OpenDocumentFile {
		document_name: String,
		document_serialized_content: String,
	},
	OpenDocumentFileWithId {
		document_id: DocumentId,
		document_name: String,
		document_is_auto_saved: bool,
		document_is_saved: bool,
		document_serialized_content: String,
	},
	PasteIntoFolder {
		clipboard: Clipboard,
		parent: LayerNodeIdentifier,
		insert_index: isize,
	},
	PasteSerializedData {
		data: String,
	},
	PrevDocument,
	SelectDocument {
		document_id: DocumentId,
	},
	SubmitDocumentExport {
		file_name: String,
		file_type: FileType,
		scale_factor: f64,
		bounds: ExportBounds,
		transparent_background: bool,
	},
	SubmitGraphRender {
		document_id: DocumentId,
	},
	ToggleRulers,
	UpdateDocumentWidgets,
	UpdateOpenDocumentsList,
}
