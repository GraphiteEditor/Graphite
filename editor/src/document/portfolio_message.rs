use super::clipboards::Clipboard;
use crate::message_prelude::*;

use graphene::LayerId;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Portfolio)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PortfolioMessage {
	// Sub-messages
	#[remain::unsorted]
	#[child]
	Document(DocumentMessage),

	#[remain::unsorted]
	#[child]
	NewDocumentDialog(NewDocumentDialogUpdate),

	// Messages
	AutoSaveActiveDocument,
	AutoSaveDocument {
		document_id: u64,
	},
	CloseActiveDocumentWithConfirmation,
	CloseAllDocuments,
	CloseAllDocumentsWithConfirmation,
	CloseDialogAndThen {
		followup: Box<Message>,
	},
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
	DisplayDialogError {
		title: String,
		description: String,
	},
	NewDocument,
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
	RequestAboutGraphiteDialog {
		release: String,
		timestamp: String,
		hash: String,
		branch: String,
	},
	RequestComingSoonDialog {
		issue: Option<i32>,
	},
	RequestNewDocumentDialog,
	SelectDocument {
		document_id: u64,
	},
	SetActiveDcoument {
		document_id: u64,
	},
	UpdateDocumentBar,
	UpdateOpenDocumentsList,
}
