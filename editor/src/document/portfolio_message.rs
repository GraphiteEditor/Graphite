use super::clipboards::Clipboard;
use crate::message_prelude::*;

use graphene::LayerId;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Portfolio)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PortfolioMessage {
	AutoSaveActiveDocument,
	AutoSaveDocument {
		document_id: u64,
	},
	CloseActiveDocumentWithConfirmation,
	CloseAllDocuments,
	CloseAllDocumentsWithConfirmation,
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
	#[child]
	Document(DocumentMessage),
	NewDocument,
	NextDocument,
	OpenDocument,
	OpenDocumentFile(String, String),
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
	PrevDocument,
	RequestAboutGraphiteDialog,
	SelectDocument {
		document_id: u64,
	},
	UpdateOpenDocumentsList,
}
