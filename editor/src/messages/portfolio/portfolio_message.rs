use super::utility_types::ImaginateServerStatus;
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::prelude::*;

use document_legacy::layers::text_layer::Font;
use document_legacy::LayerId;
use graph_craft::document::NodeId;
use graph_craft::imaginate_input::ImaginateStatus;

use serde::{Deserialize, Serialize};

use super::DocumentId;

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
	AutoSaveDocument {
		document_id: DocumentId,
	},
	CloseActiveDocumentWithConfirmation,
	CloseAllDocuments,
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
	ImaginateSetGeneratingStatus {
		document_id: DocumentId,
		layer_path: Vec<LayerId>,
		node_path: Vec<NodeId>,
		percent: Option<f64>,
		status: ImaginateStatus,
	},
	ImaginateSetImageData {
		document_id: DocumentId,
		layer_path: Vec<LayerId>,
		node_path: Vec<NodeId>,
		image_data: Vec<u8>,
		width: u32,
		height: u32,
	},
	ImaginateSetServerStatus {
		status: ImaginateServerStatus,
	},
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
		document_id: DocumentId,
		layer_path: Vec<LayerId>,
		image_data: Vec<u8>,
		size: (u32, u32),
		imaginate_node: Option<Vec<NodeId>>,
	},
	SelectDocument {
		document_id: DocumentId,
	},
	SetActiveDocument {
		document_id: DocumentId,
	},
	SetImageBlobUrl {
		document_id: DocumentId,
		layer_path: Vec<LayerId>,
		blob_url: String,
		resolution: (f64, f64),
	},
	UpdateDocumentWidgets,
	UpdateOpenDocumentsList,
}
