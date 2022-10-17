use super::utility_types::AiArtistServerStatus;
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::prelude::*;

use graphene::layers::{ai_artist_layer::AiArtistStatus, text_layer::Font};
use graphene::LayerId;

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
		document_id: u64,
		message: DocumentMessage,
	},
	AiArtistCheckServerStatus,
	AiArtistSetBlobUrl {
		document_id: u64,
		layer_path: Vec<LayerId>,
		blob_url: String,
		resolution: (f64, f64),
	},
	AiArtistSetGeneratingStatus {
		document_id: u64,
		path: Vec<LayerId>,
		percent: Option<f64>,
		status: AiArtistStatus,
	},
	AiArtistSetImageData {
		document_id: u64,
		layer_path: Vec<LayerId>,
		image_data: Vec<u8>,
	},
	AiArtistSetServerStatus {
		status: AiArtistServerStatus,
	},
	AutoSaveActiveDocument,
	AutoSaveDocument {
		document_id: u64,
	},
	CloseActiveDocumentWithConfirmation,
	CloseAllDocuments,
	CloseDocument {
		document_id: u64,
	},
	CloseDocumentWithConfirmation {
		document_id: u64,
	},
	Copy {
		clipboard: Clipboard,
	},
	Cut {
		clipboard: Clipboard,
	},
	DestroyAllDocuments,
	FontLoaded {
		font_family: String,
		font_style: String,
		preview_url: String,
		data: Vec<u8>,
		is_default: bool,
	},
	Import,
	LoadDocumentResources {
		document_id: u64,
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
		document_id: u64,
		document_name: String,
		document_is_saved: bool,
		document_serialized_content: String,
	},
	// TODO: Paste message is unused, delete it?
	Paste {
		clipboard: Clipboard,
	},
	PasteIntoFolder {
		clipboard: Clipboard,
		folder_path: Vec<LayerId>,
		insert_index: isize,
	},
	PasteSerializedData {
		data: String,
	},
	PrevDocument,
	SelectDocument {
		document_id: u64,
	},
	SetActiveDocument {
		document_id: u64,
	},
	SetImageBlobUrl {
		document_id: u64,
		layer_path: Vec<LayerId>,
		blob_url: String,
		resolution: (f64, f64),
	},
	UpdateDocumentWidgets,
	UpdateOpenDocumentsList,
}
