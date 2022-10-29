use super::utility_types::ImaginateServerStatus;
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::prelude::*;

use graphene::layers::{imaginate_layer::ImaginateStatus, text_layer::Font};
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
	ImaginateCheckServerStatus,
	ImaginateSetBlobUrl {
		document_id: u64,
		layer_path: Vec<LayerId>,
		blob_url: String,
		resolution: (f64, f64),
	},
	ImaginateSetGeneratingStatus {
		document_id: u64,
		path: Vec<LayerId>,
		percent: Option<f64>,
		status: ImaginateStatus,
	},
	ImaginateSetImageData {
		document_id: u64,
		layer_path: Vec<LayerId>,
		image_data: Vec<u8>,
	},
	ImaginateSetServerStatus {
		status: ImaginateServerStatus,
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
	ProcessNodeGraphFrame {
		document_id: u64,
		layer_path: Vec<LayerId>,
		image_data: Vec<u8>,
		size: (u32, u32),
	},
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
